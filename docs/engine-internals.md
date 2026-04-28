# Engine internals

A closer look at the data structures and the edges between them. Pairs with `architecture.md` (which is the high-level view).

## Module layout

```
src/
  main.rs                       // entry, builds a Torte and calls run()
  torte/
    mod.rs                      // pub mod board; pub mod core; pub mod torte;
    torte.rs                    // struct Torte { board: Board }; run() = stdin REPL
    board/
      mod.rs                    // pub mod board; pub mod pieces;
      board.rs                  // struct Board, FEN parse, apply_*_move, Debug printer
      pieces.rs                 // enum Color, enum Piece, FEN/index/glyph mappings
    core/
      mod.rs                    // pub mod bitboard; pub mod piece_move; pub mod sq;
      bitboard.rs               // struct Bitboard(u64) and op overloads
      piece_move.rs             // struct Move { src, dest }, from_uci
      sq.rs                     // struct SQ(u8), make(rank, file), to_bb()
```

Note: every directory uses a `mod.rs` that just re-exports siblings, and each leaf file is named after its primary type's snake_case (e.g. `Board` lives in `board.rs` inside `board/`). Imports therefore look stuttery: `crate::torte::board::board::Board`.

## Bitboard

```rust
pub struct Bitboard { pub board: u64 }
```

| method               | meaning                                  |
| -------------------- | ---------------------------------------- |
| `new()` / `empty()`  | zero bitboard                            |
| `from_u64(u64)`      | const constructor                        |
| `set(i)` / `clear(i)`| set/clear bit `i`                        |
| `get(i)`             | test bit `i`                             |
| `is_empty()`         | board == 0                               |
| `pop()`              | clear lsb, return its index, or `None`   |
| `count()`            | popcount                                 |
| `get_lsb()` / `get_msb()` | trailing/leading zeros (raw)        |
| `Iterator`           | wraps `pop()` — `for sq in bb { ... }`   |

Operator semantics:

| op  | meaning            | also available as |
| --- | ------------------ | ----------------- |
| `&` | intersection       | `*`               |
| `\|`| union              | `+`               |
| `^` | symmetric diff     |                   |
| `-` | difference (a & !b)|                   |
| `!` | complement         |                   |
| `<<` / `>>` | shift `usize` | also `<<=` / `>>=` |

## Square (SQ)

`SQ(pub u8)` — index `0..=63`, with `64` reserved as `SQ::NONE`. Construction is `SQ::make(rank, file) = SQ(rank * 8 + file)`. The `is_ok` / `is_none` / `to_usize` / `to_bb` helpers are all `const`. `to_bb` returns `Bitboard::from_u64(1) << self.0 as usize`.

## Move

```rust
pub struct Move { src: SQ, dest: SQ }
```

Just two squares. No promotion piece, no flags for castle / en-passant / capture / double-pawn-push. `from_uci` is hard-coded to `len() == 4` and panics otherwise. UCI move strings are parsed as `(file, rank, file, rank)` characters, ASCII-arithmetic-style:

```
file = uci[i] - b'a'   // 'a'..'h' -> 0..7
rank = uci[i+1] - b'1' // '1'..'8' -> 0..7
```

## Piece / Color

`enum Color { White, Black }` with `opposite()` and `to_index()` (white=0, black=1). `enum Piece` has 12 variants in the order white-PNBRQK then black-PNBRQK, matching the bitboard array layout. Mappings:

- `Piece::from_fen(char) -> Option<Piece>` — uppercase = white, lowercase = black.
- `Piece::to_index()` / `from_index(usize)` — round-trip with the `bbs` array.
- `Piece::color()` — match on variant.
- `Display` / `Debug` — Unicode chess glyphs (♙♘♗♖♕♔♟♞♝♜♛♚).

## Board

```rust
pub struct Board {
    pub bbs: [Bitboard; 12],
    pub player_bbs: [Bitboard; 2],
}
```

State *not* on `Board` (yet):
- side to move
- castling rights
- en-passant target square
- halfmove clock
- fullmove number
- zobrist hash

`Board::parse(fen)` only consumes the piece-placement field. `Board::new()` returns a fully empty board.

`apply_uci_move` -> `apply_move(Move)` -> `move_piece(piece, from, to)`. The chain re-scans the bitboards in both `apply_move` (via `piece_at_sq`) and `move_piece`, so the `piece` argument to `move_piece` is currently dead. Only `bbs` is updated; `player_bbs` is not. Captures, castling, en-passant, and promotion are not handled.

## Torte (driver)

```rust
pub struct Torte { pub board: Board }
```

`run()`:
1. Print "Running Torte".
2. `self.board = Board::parse(STARTPOS_FEN)`.
3. Loop: print board, read line, break on `exit`, else `apply_uci_move`, print error or board.
4. After loop, print board once more.

This is the only "engine driver" code in the repo. There is no UCI handler, no time management, no thread for searching, no anything else.
