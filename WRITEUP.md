# WRITEUP

## Architecture overview

- **solver-core (library)** implements the full impartial-game solver for the fixed 5×8 board. Board states are encoded as Ferrers shapes (column heights in `[-1, 4]`), while moves are zero-index `(row, col)` tuples. A memoized DFS (`Solver`) labels each state as winning/losing and captures every winning reply. The module also exposes `enumerate_states` and `export_policy_json` to regenerate the complete policy table used for explainability and regression tests.
- **cli (binary)** is a clap-based tool that layers production ergonomics over the solver. It can (a) inspect a manual or on-chain board state, (b) emit human or JSON suggestions, (c) export the policy JSON, and (d) submit fully signed Solana transactions. All RPC flows go through `solana-client`, derive the PDA `[player_pubkey]`, decode the on-chain `G { s: [u8;5] }` bitmap into the column-height state, and then delegate strategy selection to `solver-core`.

### Board coordinate system

- Rows are numbered **top → bottom** and columns **left → right** to match the game’s
	physical layout.
- The poison glass lives at `(5, 8)` (bottom-right). Attempting to chomp it loses.
- Chomping `(r, c)` removes that candy and **only** the candies that are both
	above it and to its left (rows `1..=r` ∩ columns `1..=c`). This is why the
	solver state can be represented as column heights.

## Hardest technical aspects

1. **State decoding from PDA data** – The on-chain program stores five row bitmasks, while the solver wants column heights. The CLI converts each column by checking the `M` mask bits (taken from the on-chain source) and recording the deepest set bit, guaranteeing parity with the Ferrers-shape invariant.
2. **Perfect-play validation** – The solver’s credibility hinges on reproducing the empirically known opening book. Unit tests assert the unique `(1,2)` opener plus the mandatory replies for each AI response, preventing regressions as optimizations land.
3. **Transaction construction** – The `play` command mirrors the on-chain account order (system program, player signer, PDA, fee collector) and encodes moves exactly as the program expects (4-bit row, 4-bit column). This keeps the CLI aligned with the deployed `ChompZ...` program and fee collector.

## Problem-solving approach

1. **Document digestion** – Exhaustively analyzed both attached PDFs to extract invariants (symmetry traps, P/N positions, second-move book) and align terminology (`(row, col)` 1-index UI vs. zero-index solver tuples).
2. **Core solver first** – Implemented the tuple-based state machine with memoized DFS, plus tests covering the published move lines. This produces deterministic evaluations for every reachable state.
3. **Data export + CLI** – Added BFS enumeration + JSON export for audits, then layered the CLI for strategy exploration, RPC fetching, and on-chain play. The CLI doubles as the proof generator (logging move, signature, PDA) for the submission package.

## Strategy overview

- Always open `(1,2)`; any other opener is a P-position.
- After the opener, the solver’s winning replies enforce asymmetry: `(1,5)` vs `(2,1)`, `(4,2)` vs `(1,3)`, `(2,3)`/`(1,4)` vs `(3,1)`, `(3,8)` vs `(4,1)`, `(4,6)` vs `(5,1)`.
- The engine automatically detects equal-arm L shapes around the poison (column 8 height = bottom row width) as losing states and steers clear by preferring moves that keep the arms unbalanced.
- When forced into a losing position, the CLI surfaces the lack of winning replies, signaling you should maximize asymmetry and hope for an AI error rather than relying on non-existent perfect play.
