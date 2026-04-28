# Architecture

The engine is in a very early state — only board representation and a stdin move loop exist. This document describes what is there, and flags the standard chess-engine pieces that are not yet implemented.

## High-level shape

```
main.rs
  └── Torte (torte/torte.rs)
        └── Board (torte/board/board.rs)
              ├── bbs: [Bitboard; 12]      // one per (colour, piece-type)
              └── player_bbs: [Bitboard; 2] // aggregate per colour
```

`main` constructs a `Torte`, which owns a `Board`, then calls `Torte::run`. `run` parses the start-position FEN, prints the board, then loops reading UCI-style move strings from stdin and applying them.

## Board representation

Bitboard-based. Each of the 12 (colour, piece) combinations gets its own `u64`, plus two aggregate-by-colour boards.

- `Bitboard` (`core/bitboard.rs`) is a `Copy` newtype wrapping `u64` with `set` / `clear` / `get` by bit index, `pop` (clear lsb, return its index) which also makes it an `Iterator<Item = usize>`, and `count` / `get_lsb` / `get_msb`.
- Operator overloads carry set-theoretic meaning: `&` and `*` both mean intersection, `|` and `+` both mean union, `^` is xor, `-` is set difference (`a & !b`), `!` is complement, `<<` / `>>` shift by `usize`. Reading `a + b - c` as "union, then minus" is intentional.
- Square indices (`SQ`) are `0..64` with `64` reserved as `NONE`. `SQ::make(rank, file) = rank * 8 + file`, so a1 = 0, h1 = 7, a8 = 56, h8 = 63 (rank-major, files 0..7 = a..h).

Indexing convention for `bbs`:

| index | piece          |
| ----- | -------------- |
| 0..5  | white P/N/B/R/Q/K |
| 6..11 | black P/N/B/R/Q/K |

This is set in `Piece::to_index` / `from_index` in `board/pieces.rs` and used everywhere.

## FEN parsing

`Board::parse` (in `board/board.rs`) handles the piece-placement field of FEN. It splits on whitespace, walks the eight ranks of `parts[0]`, and sets bits in both `bbs` and `player_bbs`.

It does **not** read the side-to-move, castling-rights, en-passant, halfmove, or fullmove fields, even though `parts` contains them. There is also no place on `Board` to store any of that state yet.

The rank loop in `parse` iterates `for rank in 0..8` while consuming the FEN ranks top-to-bottom (rank 8 first). The bit it sets is `rank * 8 + file`, which means the FEN's rank 8 (top of the board) ends up at bit indices 0..7, and rank 1 (bottom) ends up at 56..63. The `Debug` printer iterates `for rank in (0..8).rev()` and reads `rank * 8 + file`, which un-flips this for display, so the visual output is correct — but anyone writing move logic against `SQ::make(rank, file)` semantics should know the stored layout is upside-down relative to the comment "a1 = 0".

## Move application

`Move` (`core/piece_move.rs`) is just `{ src: SQ, dest: SQ }`. `Move::from_uci` parses a 4-char string like `e2e4`:

```
src  = SQ::make(rank=uci[1]-'1', file=uci[0]-'a')
dest = SQ::make(rank=uci[3]-'1', file=uci[2]-'a')
```

It panics on any other length. There is no support for promotion suffix (`e7e8q`).

`Board::apply_uci_move` -> `apply_move` -> `move_piece`. `move_piece` scans all 12 piece bitboards, finds the one that has `from` set, clears that bit, and sets `to`. It does not:

- update `player_bbs` (so colour aggregates drift after the first move),
- remove a captured piece on `to` from any bitboard,
- handle castling, en passant, or promotion,
- check legality or whose turn it is.

`piece_at_sq` does the same scan to identify the moving piece, but the result is unused by `move_piece` (the function takes `piece` as an argument and ignores it, doing its own scan).

## Move generation

Not implemented. There are no attack tables, no magic bitboards, no `generate_moves` function, no perft. Adding this is the obvious next step.

## Search

Not implemented. No alpha-beta, no iterative deepening, no quiescence, no transposition table.

## Evaluation

Not implemented. No material count, no piece-square tables, no eval function at all.

## I/O

The current "protocol" is a hand-rolled stdin loop in `Torte::run` that reads a line, treats `exit` as quit, and otherwise tries to apply it as a UCI move. It is not a real UCI engine — there is no `uci` / `isready` / `position` / `go` handling and no stdout protocol output, just `Debug` prints of the board.
