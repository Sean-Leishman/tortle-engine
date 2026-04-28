# tortle-engine (chess)

## Purpose

An early-stage chess engine in Rust, internally codenamed "torte" (binary name) and "tortle-engine" on GitHub. The project is in its first scaffolding phase: a bitboard-based board representation, FEN parsing, and a UCI-style move loop on stdin. There is no move generation, search, or evaluation yet — the binary just lets you type four-character moves (e.g. `e2e4`) and prints the resulting board. Treat this as a learning / exploration project rather than a playable engine.

## Tech stack

- **Language:** Rust (edition 2021)
- **Build:** Cargo, no dependencies (`[dependencies]` is empty in `Cargo.toml`)
- **Binary:** `torte` (configured via `[[bin]]` in `Cargo.toml`, source at `src/main.rs`)
- **Crate name:** `chess` (the package), but the binary and module hierarchy are named `torte`

## Key files / entry points

- `src/main.rs` — boots a `Torte` and calls `run()`
- `src/torte/torte.rs` — the top-level engine struct; owns a `Board`, hardcodes the start-position FEN, and runs the stdin REPL
- `src/torte/board/board.rs` — `Board` struct (12 piece bitboards + 2 colour bitboards), FEN parser, `apply_uci_move`, `Debug` pretty-printer
- `src/torte/board/pieces.rs` — `Color` and `Piece` enums, FEN char <-> piece, Unicode glyphs
- `src/torte/core/bitboard.rs` — `Bitboard` newtype around `u64` with the usual bit ops, iterator, lsb/msb helpers
- `src/torte/core/sq.rs` — `SQ(u8)` square index (0..64, 64 = NONE)
- `src/torte/core/piece_move.rs` — `Move { src, dest }` and `Move::from_uci`

## How to run / dev

```
cargo run            # starts the REPL, prints the board, reads moves from stdin
cargo build          # builds debug binary at target/debug/torte
cargo build --release
```

At the prompt, type a 4-char UCI move like `e2e4`, or `exit` to quit. There are no tests, no benches, no CI config in the repo.

## Conventions noticed

- Modules are nested two deep with a re-exporting `mod.rs` at each level (`torte/mod.rs`, `torte/board/mod.rs`, `torte/core/mod.rs`). Files often share the name of their parent module (`board/board.rs`, `torte/torte.rs`), so imports look like `use crate::torte::board::board::Board`.
- Pieces and colours are indexed by hand-rolled `to_index` / `from_index` rather than `#[repr(u8)]` + `as`.
- `Board` carries both per-piece bitboards (`bbs[12]`) and per-colour aggregate bitboards (`player_bbs[2]`), but `move_piece` only updates `bbs` — `player_bbs` falls out of sync after any move. Likely an unfinished spot.
- Errors use `std::io::Error` with `InvalidInput` even for non-IO failures (e.g. "no piece at square"). Lightweight, not idiomatic.
- `Bitboard` overloads arithmetic ops with set-theoretic meaning: `Add` = union, `Sub` = difference, `Mul` = intersection. Be careful reading expressions.

## Gaps / unknowns

- No move generation (sliding-piece attacks, knight/king tables, pawn pushes/captures, castling, en passant, promotions) — none of it is implemented yet.
- No search, no evaluation, no transposition table, no UCI protocol handler (the loop reads moves directly, it doesn't speak UCI to a GUI).
- No turn tracking, castling rights, en-passant square, or halfmove/fullmove counters are stored on `Board`, even though `parse` splits the full FEN into `parts`.
- `while true` in `torte.rs` triggers a Rust warning; should be `loop`.
- No README in the repo, so the name "tortle" / "torte" is only inferred from the git remote and the binary name.
