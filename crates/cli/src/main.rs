use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use solana_client::{
    client_error::{ClientError, ClientErrorKind},
    rpc_client::RpcClient,
    rpc_request::RpcError,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use solver_core::{export_policy_json, BoardState, Move, Solver};
use std::path::PathBuf;
use std::str::FromStr;

const DEFAULT_RPC: &str = "https://api.mainnet-beta.solana.com";
const DEFAULT_PROGRAM: &str = "ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D";
const FEE_COLLECTOR: &str = "EGJnqcxVbhJFJ6Xnchtaw8jmPSvoLXfN2gWsY9Etz5SZ";

#[derive(Parser)]
#[command(author, version, about = "Chomping Glass solver and on-chain client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Suggest winning moves for a board state.
    Suggest(SuggestArgs),
    /// Export the policy table to JSON.
    ExportPolicy { output: PathBuf },
    /// Play a move on-chain with your wallet.
    Play(PlayArgs),
}

#[derive(Parser, Debug)]
struct SuggestArgs {
    /// Manual column heights, e.g. "0,0,-1,-1,-1,-1,-1,-1".
    #[arg(long)]
    state: Option<String>,
    /// RPC endpoint when fetching live state.
    #[arg(long, default_value = DEFAULT_RPC)]
    rpc_url: String,
    /// Player public key for PDA derivation.
    #[arg(long)]
    player: Option<String>,
    /// Program ID to query.
    #[arg(long, default_value = DEFAULT_PROGRAM)]
    program: String,
    /// Emit JSON instead of text.
    #[arg(long)]
    json: bool,
}

#[derive(Parser, Debug)]
struct PlayArgs {
    /// Signing keypair JSON path.
    #[arg(long)]
    wallet: PathBuf,
    /// RPC endpoint.
    #[arg(long, default_value = DEFAULT_RPC)]
    rpc_url: String,
    /// Program ID to target.
    #[arg(long, default_value = DEFAULT_PROGRAM)]
    program: String,
    /// Manual board state override.
    #[arg(long)]
    state: Option<String>,
    /// Explicit row (1-indexed).
    #[arg(long)]
    row: Option<u8>,
    /// Explicit column (1-indexed).
    #[arg(long)]
    col: Option<u8>,
    /// Print the transaction without sending.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Serialize)]
struct SuggestReport {
    winning: bool,
    winning_moves: Vec<(u8, u8)>,
    recommended: Option<(u8, u8)>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Suggest(args) => handle_suggest(args),
        Commands::ExportPolicy { output } => {
            export_policy_json(&output)
                .with_context(|| format!("failed to export policy to {:?}", output))?;
            println!("Policy written to {:?}", output);
            Ok(())
        }
        Commands::Play(args) => handle_play(args),
    }
}

fn handle_suggest(args: SuggestArgs) -> Result<()> {
    let mut solver = Solver::new();
    let state = resolve_state(
        args.state.as_deref(),
        args.player.as_deref(),
        &args.program,
        &args.rpc_url,
    )?;
    let eval = solver.evaluate(state);
    if args.json {
        let report = SuggestReport {
            winning: eval.winning,
            winning_moves: eval
                .winning_moves
                .iter()
                .map(|mv| mv.to_one_indexed())
                .collect(),
            recommended: eval.winning_moves.first().map(|mv| mv.to_one_indexed()),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Current board:\n{}", state);
        println!("Winning position: {}", eval.winning);
        if eval.winning {
            let moves: Vec<String> = eval
                .winning_moves
                .iter()
                .map(|mv| {
                    let (r, c) = mv.to_one_indexed();
                    format!("({},{})", r, c)
                })
                .collect();
            println!("Winning moves: {}", moves.join(", "));
            if let Some(best) = eval.winning_moves.first() {
                let (r, c) = best.to_one_indexed();
                println!("Recommended move: ({},{})", r, c);
            }
        } else {
            println!("No forced win from this position—play for asymmetry and hope the AI errs.");
        }
    }
    Ok(())
}

fn handle_play(args: PlayArgs) -> Result<()> {
    let program_id = Pubkey::from_str(&args.program)?;
    let fee_collector = Pubkey::from_str(FEE_COLLECTOR)?;
    let payer = read_keypair_file(&args.wallet)
        .map_err(|err| anyhow!("failed to read keypair {}: {}", args.wallet.display(), err))?;
    let player_key = payer.pubkey();
    let mut solver = Solver::new();
    let state = resolve_state(
        args.state.as_deref(),
        Some(&player_key.to_string()),
        &args.program,
        &args.rpc_url,
    )?;
    let eval = solver.evaluate(state);

    let chosen_move = if let (Some(r), Some(c)) = (args.row, args.col) {
        to_zero_indexed_move(r, c)?
    } else {
        *eval
            .winning_moves
            .first()
            .ok_or_else(|| anyhow!("position is losing; specify --row/--col to move anyway"))?
    };

    let (row1, col1) = chosen_move.to_one_indexed();
    let opcode = ((row1 & 0xF) << 4) | (col1 & 0xF);
    let (game_pda, _) = Pubkey::find_program_address(&[player_key.as_ref()], &program_id);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &[opcode],
        vec![
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new(player_key, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(fee_collector, false),
        ],
    );

    if args.dry_run {
        println!(
            "Dry run: would send move ({},{}) with opcode 0x{:02X}",
            row1, col1, opcode
        );
        println!("Accounts: player={}, game={} (PDA)", player_key, game_pda);
        return Ok(());
    }

    let rpc = RpcClient::new_with_commitment(args.rpc_url.clone(), CommitmentConfig::confirmed());
    let blockhash = rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&[instruction], Some(&player_key), &[&payer], blockhash);
    let sig = rpc.send_and_confirm_transaction(&tx)?;
    println!("Submitted move ({},{}). Signature: {}", row1, col1, sig);
    Ok(())
}

fn resolve_state(
    manual: Option<&str>,
    player: Option<&str>,
    program: &str,
    rpc_url: &str,
) -> Result<BoardState> {
    if let Some(state_str) = manual {
        return parse_state(state_str);
    }
    let player =
        player.ok_or_else(|| anyhow!("player pubkey is required when --state is not provided"))?;
    let player_key = Pubkey::from_str(player)?;
    let program_id = Pubkey::from_str(program)?;
    fetch_state_from_chain(&player_key, &program_id, rpc_url)
}

fn parse_state(raw: &str) -> Result<BoardState> {
    let values: Vec<i8> = raw
        .split(',')
        .map(|s| s.trim().parse::<i8>())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.len() != solver_core::COLS {
        return Err(anyhow!(
            "expected {} columns, got {}",
            solver_core::COLS,
            values.len()
        ));
    }
    let mut heights = [-1i8; solver_core::COLS];
    heights.copy_from_slice(&values);
    Ok(BoardState::from_heights(heights))
}

fn fetch_state_from_chain(
    player: &Pubkey,
    program_id: &Pubkey,
    rpc_url: &str,
) -> Result<BoardState> {
    let rpc = RpcClient::new(rpc_url.to_string());
    let (game_pda, _) = Pubkey::find_program_address(&[player.as_ref()], program_id);
    let data = match rpc.get_account_data(&game_pda) {
        Ok(data) => data,
        Err(err) => {
            if account_missing(&err) {
                return Ok(BoardState::new());
            }
            return Err(err.into());
        }
    };

    let mut heights = [-1i8; solver_core::COLS];
    for col in 0..solver_core::COLS {
        let mask = 1u8 << (7 - col);
        for row in 0..solver_core::ROWS {
            if data.get(row).copied().unwrap_or_default() & mask != 0 {
                heights[col] = row as i8;
            }
        }
    }
    Ok(BoardState::from_heights(heights))
}

fn to_zero_indexed_move(row: u8, col: u8) -> Result<Move> {
    if !(1..=solver_core::ROWS as u8).contains(&row) {
        return Err(anyhow!("row must be between 1 and {}", solver_core::ROWS));
    }
    if !(1..=solver_core::COLS as u8).contains(&col) {
        return Err(anyhow!(
            "column must be between 1 and {}",
            solver_core::COLS
        ));
    }
    Ok(Move::new(row - 1, col - 1))
}

fn account_missing(error: &ClientError) -> bool {
    match error.kind() {
        ClientErrorKind::RpcError(RpcError::ForUser(msg)) => msg.contains("AccountNotFound"),
        ClientErrorKind::RpcError(RpcError::RpcResponseError { message, .. }) => {
            message.contains("does not exist")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_state_valid() {
        let result = parse_state("0,0,-1,-1,-1,-1,-1,-1");
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.heights()[0], 0);
        assert_eq!(state.heights()[1], 0);
        assert_eq!(state.heights()[2], -1);
    }

    #[test]
    fn test_parse_state_invalid_count() {
        let result = parse_state("0,0,-1");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected 8 columns"));
    }

    #[test]
    fn test_parse_state_invalid_value() {
        let result = parse_state("0,0,-1,-1,-1,-1,-1,x");
        assert!(result.is_err());
    }

    #[test]
    fn test_to_zero_indexed_move_valid() {
        let result = to_zero_indexed_move(1, 1);
        assert!(result.is_ok());
        let mv = result.unwrap();
        assert_eq!(mv.to_one_indexed(), (1, 1));
    }

    #[test]
    fn test_to_zero_indexed_move_max_bounds() {
        let result = to_zero_indexed_move(5, 8);
        assert!(result.is_ok());
        let mv = result.unwrap();
        assert_eq!(mv.to_one_indexed(), (5, 8));
    }

    #[test]
    fn test_to_zero_indexed_move_row_out_of_bounds() {
        let result = to_zero_indexed_move(0, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("row must be"));

        let result = to_zero_indexed_move(9, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("row must be"));
    }

    #[test]
    fn test_to_zero_indexed_move_col_out_of_bounds() {
        let result = to_zero_indexed_move(1, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("column must be"));

        let result = to_zero_indexed_move(1, 9);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("column must be"));
    }
}
