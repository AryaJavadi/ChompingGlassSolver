# Chomping Glass Solver

A production-ready solver and command-line agent for the 5×8 **Chomping Glass** puzzle on Solana. The project is split into a `solver-core` library (game logic + policy export) and a `cli` crate (strategy exploration plus on-chain move submission).

## Prerequisites

- Rust 1.82+ (for the 2021 edition workspace)
- Solana CLI (to manage keypairs and inspect accounts)
- `pkg-config` and OpenSSL headers (required by `solana-client`)
  - macOS (Homebrew):
    ```bash
    brew install pkg-config openssl@3
    ```
- A funded Solana keypair (e.g. `~/chomp-keypair.json`) with ~0.1 SOL for fees

## Workspace layout

```
Cargo.toml (workspace)
crates/
  solver-core/   # state machine, memoized solver, policy exporter
  cli/           # clap-based CLI with solver + RPC integration
README.md
WRITEUP.md
```

## Building and testing

```bash
cargo fmt
cargo test
```

> **Note:** if the build fails with `Could not find directory of OpenSSL installation`, ensure `pkg-config` and `openssl@3` are installed and exposed via `OPENSSL_DIR=/opt/homebrew/opt/openssl@3` (or your platform equivalent).

## CLI usage

All commands live in the `cli` crate:

```bash
cargo run -p cli -- <COMMAND> [flags]
```

### Inspect / suggest moves

Evaluate the live on-chain state for a wallet (defaults to mainnet RPC):

```bash
cargo run -p cli -- suggest --player 7sg1WCRhHALDvDFkKHpwjLHmA9GLrS41bkFE4PEz1Mrk
```

Provide a manual tuple of column heights (comma-separated, `-1` = untouched):

```bash
cargo run -p cli -- suggest --state "0,0,-1,-1,-1,-1,-1,-1"
```

Add `--json` for machine-friendly output.

#### Board orientation and chomping rule

- Rows are numbered **top to bottom** and columns **left to right**.
- The poison glass sits at **row 5, column 8** (bottom-right corner).
- When you chomp a candy at `(row, col)` you remove **that candy and every candy
  that is both above it and to its left** (i.e., rows `1..=row` intersected with
  columns `1..=col`). This produces the characteristic top-left “L” shapes used
  by the solver.

All CLI state inputs follow this convention: `-1` means untouched, `0` means the
top row of that column has been eaten, etc.

### Export the full policy table

```bash
cargo run -p cli -- export-policy --output chomping_glass_policy.json
```

The exported JSON maps every reachable Ferrers-shape tuple to `(winning, winning_moves)`.

### Submit a move on-chain

```bash
cargo run -p cli -- play \
  --wallet ~/chomp-keypair.json \
  --rpc-url https://api.mainnet-beta.solana.com
```

The solver fetches your PDA game account, evaluates it, and plays the first winning move. Use `--row`/`--col` (1-indexed) to override or `--dry-run` to print the transaction without broadcasting.

See `INSTRUCTIONS.md` for a step-by-step walkthrough (wallet prep, PDA lookup,
dry runs, manual overrides) when playing directly against the website’s AI.

The instruction uses:

- Program ID
- Fee collector
- PDA seed

## Strategy quick reference

- Unique winning opener: `(1,2)`
- Mandatory replies:
  - AI `(2,1)` → `(1,5)`
  - AI `(1,3)` → `(4,2)`
  - AI `(3,1)` → `(2,3)` or `(1,4)`
  - AI `(4,1)` → `(3,8)`
  - AI `(5,1)` → `(4,6)`
- Avoid gifting the AI an equal-arm L around the poison square; the solver exposes these P-positions automatically.

## Signing the submission message

Use the Solana CLI (or `solana-keygen sign`) to sign the required message:

```bash
solana-keygen sign \
  --keypair ~/chomp-keypair.json \
  --message "Chomping Glass submission by <Your Name> on <Date>"
```

## Winning proof

- **Solscan TX:** https://solscan.io/tx/3Sti5P7b3BUgyWDLHEvok8GXySkZpTJy4izoUu31KaKvCcWmHmSWdReYidvW1QRsKJgH63m8tcLi45ue36GbdhcL

<video controls muted playsinline width="640">
  <source src="Game_sample.mp4?raw=1" type="video/mp4" />
  Your browser does not support inline video. Download <a href="Game_sample.mp4?raw=1">Game_sample.mp4</a> instead.
</video>
