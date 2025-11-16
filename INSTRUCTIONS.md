# Running the Chomping Glass Solver + On-Chain Client

This short guide walks through reproducing the exact setup we used to play the
mainnet Chomping Glass AI at [chompingglass.com](https://chompingglass.com),
including how to locate (or re-derive) the per-player Program Derived Account
(PDA) that the on-chain program uses for game state.

## 1. Prerequisites

- Rust 1.82+ (the repo uses the 2021 edition workspace)
- Solana CLI ≥ 1.18 (for RPC config + PDA inspection)
- `pkg-config` and OpenSSL headers (already noted in `README.md`)
- A funded Solana keypair JSON file (e.g. `~/chomp-keypair.json`)

```bash
solana config set --url https://api.mainnet-beta.solana.com
solana keygen recover -o ~/chomp-keypair.json   # or create/import your keypair
```

## 2. Build the workspace

```bash
cd solver
cargo build --release
```

This compiles both crates (`solver-core` and `cli`).

## 3. Running the solver offline

You can ask the solver about any manual state without touching the network:

```bash
cargo run -p cli --release -- suggest --state "0,0,-1,-1,-1,-1,-1,-1"
```

JSON output is available via `--json`, and policy export via
`cargo run -p cli -- export-policy --output chomping_glass_policy.json`.

## 4. Locating your PDA on-chain

The on-chain program with ID `ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D`
creates one PDA per player using the single seed `[player_pubkey]`. If you have
already played on chompingglass.com, the PDA will show up in Solscan as the
third account of the move instruction (e.g.
`HRQmDuGxDCJY9UqDoycmPKUZ8XYTKCDk7T57v48JukYN`).

You can re-derive it locally with the Solana CLI:

```bash
PLAYER=7sg1WCRhHALDvDFkKHpwjLHmA9GLrS41bkFE4PEz1Mrk
PROGRAM=ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
solana address -k /path/to/dummy.json --seed $PLAYER --program-id $PROGRAM
```

> Any JSON keypair works for `-k`; the seeds determine the PDA. The command will
> print the same base58 PDA you see on Solscan.

## 5. Playing directly against the AI

The CLI mirrors the website’s instruction format. Example:

```bash
cargo run -p cli --release -- play \
  --wallet ~/chomp-keypair.json \
  --rpc-url https://api.mainnet-beta.solana.com \
  --program ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
```

- The command loads your keypair, fetches the PDA state (deriving it from your
  wallet pubkey automatically), evaluates it, and submits the first winning move.
- Use `--dry-run` to print the transaction without sending it.
- To override the solver’s choice, add `--row <r> --col <c>` (1-indexed).

If the program has never seen your wallet before, the first transaction will
create the PDA (0.001 SOL deposit, as seen on Solscan). Subsequent moves reuse
that account.

## 6. Troubleshooting tips

- **Missing PDA / fresh player:** run one move on the website or submit a manual
  `--state` game via CLI; the program will auto-create your PDA.
- **Manual analysis:** pass `--state` to skip on-chain fetches and explore
  arbitrary board positions.
- **Logging:** set `RUST_LOG=debug` when running the CLI to inspect RPC calls.

With the PDA info (e.g. `HRQmDuGxDCJY9UqDoycmPKUZ8XYTKCDk7T57v48JukYN`) and the
commands above, anyone can reproduce the live AI match and verify the solver’s
suggested moves in real time.

## 7. Playing back-to-back games

Once you finish a round, the same PDA/account will hold the fresh board for the
next match. The quickest loop is:

1. **Preview the move**

   ```bash
   cargo run -p cli --release -- play \
     --wallet ~/chomp-keypair.json \
     --rpc-url https://api.mainnet-beta.solana.com \
     --program ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D \
     --dry-run
   ```

   This fetches your PDA, evaluates the board, and prints the recommended row &
   column without sending a transaction.

2. **Send the move** (remove `--dry-run`).

   ```bash
   cargo run -p cli --release -- play \
     --wallet ~/chomp-keypair.json \
     --rpc-url https://api.mainnet-beta.solana.com \
     --program ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
   ```

   Note the signature it prints and confirm it on Solscan if desired. After the
   AI responds, go back to step 1. You only need to re-run the website once (or
   manually supply `--state`) if the PDA ever gets reset.
