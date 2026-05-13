# Engine internals

A closer look at the data structures and the edges between them. Pairs with `architecture.md` (which is the high-level view).

## Module layout

```
src/
  main.rs                       // entry, builds a Torte and calls run()
  torte/
    mod.rs                      // pub mod board; pub mod core; pub mod movegen; pub mod torte;
    torte.rs                    // struct Torte { board: Board }; run() = stdin REPL
    board/
      mod.rs                    // pub mod board; pub mod pieces;
      board.rs                  // struct Board, struct CastlingRights, FEN parse, apply_move, Debug printer
      pieces.rs                 // enum Color, enum Piece, FEN/index/glyph mappings
    core/
      mod.rs                    // pub mod bitboard; pub mod piece_move; pub mod sq;
      bitboard.rs               // struct Bitboard(u64) and op overloads
      piece_move.rs             // struct Move, enum PromotionPiece, from_uci
      sq.rs                     // struct SQ(u8), make(rank, file), from_uci, to_bb()
    movegen/
      mod.rs                    // pub mod attacks, generator, magic, perft;
      attacks.rs                // KNIGHT/KING/PAWN attack tables built via const fn
      magic.rs                  // plain magic bitboards for rook/bishop, search-at-startup
      generator.rs              // is_attacked, generate_legal_moves, king_square
      perft.rs                  // perft + perft_divide for movegen verification
    search/
      mod.rs                    // pub mod eval, search, transposition;
      eval.rs                   // material-only static eval
      search.rs                 // negamax + alpha-beta + ID + qsearch + TT integration
      transposition.rs          // Zobrist keys/hash, TTEntry, Bound, TranspositionTable
    uci.rs                      // UCI protocol loop (run, parse_position, parse_go_depth, format_score)
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
pub struct Move {
    src: SQ,
    dest: SQ,
    promotion: Option<PromotionPiece>,
}

pub enum PromotionPiece { Knight, Bishop, Rook, Queen }
```

Two squares plus an optional promotion piece. No explicit flags for castle / en-passant / double-pawn-push — those are inferred in `Board::apply_move` from `(piece, src, dest, en_passant)`. `from_uci` accepts 4-char (`e2e4`) and 5-char (`e7e8q`) forms; promotion char is `n`/`b`/`r`/`q`. Anything else panics.

UCI move strings are parsed as `(file, rank, file, rank)` characters, ASCII-arithmetic-style:

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
    pub side_to_move: Color,
    pub castling: CastlingRights, // u8 bitfield, KQkq
    pub en_passant: Option<SQ>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
}
```

State still *not* on `Board`: zobrist hash.

`Board::parse(fen)` consumes all six FEN fields; missing trailing fields fall back to defaults (white to move, no castling, no ep, halfmove 0, fullmove 1). `Board::new()` returns an empty board with the same defaults. Castling rights are stored as a `u8` with constants `WHITE_KING | WHITE_QUEEN | BLACK_KING | BLACK_QUEEN`.

`apply_move` mutates all of these fields each call: flips `side_to_move`, sets/clears `en_passant`, masks `castling` on king/rook/rook-capture moves, resets `halfmove_clock` on captures or pawn moves (else +1), and increments `fullmove_number` after black's move.

`apply_uci_move` -> `apply_move(Move)` -> `move_piece(piece, from, to)`. `apply_move` looks up the moving piece via `piece_at_sq`; `move_piece` then updates `bbs` and `player_bbs` together and clears any opponent piece on `to` (capture). Castling, en-passant, and promotion are not handled.

## Torte (driver)

```rust
pub struct Torte { pub board: Board }
```

`run()`: initializes magic tables, parses the start FEN into `self.board`, and hands off to `uci::run(&mut self.board)` which is the actual command loop. There is no separate REPL implementation any more — the UCI loop in `uci.rs` accepts both UCI commands and a few REPL conveniences (bare moves, `d`, plain-integer `go N`).

No time management, no async search thread, no ponder.
