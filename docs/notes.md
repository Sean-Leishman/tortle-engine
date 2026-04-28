# Notes

Loose observations and questions for future-me. Not a roadmap, not a spec.

## Naming

- GitHub repo: `tortle-engine`
- Cargo package: `chess`
- Binary: `torte`
- Top-level struct + module: `Torte` / `torte`

So the "real" name is probably *Tortle* (with the engine binary clipped to *torte*), but the local checkout directory is just `chess/`. Worth picking one and renaming the others at some point.

## Things that are obviously broken / unfinished

- `move_piece` doesn't touch `player_bbs`, doesn't remove captured pieces, and re-scans for the moving piece instead of trusting the `piece: Piece` argument it was passed.
- `Board::parse` sets `player_bbs` correctly *during parsing* but never thereafter, so it's only a snapshot of the start position.
- `Board::parse` ignores everything after the first FEN field. There is also nowhere on `Board` to put side-to-move, castling rights, en passant, halfmove, fullmove.
- `Move::from_uci` panics on anything other than a 4-char string. UCI legitimately uses 5 chars for promotions.
- `while true` in `Torte::run` should be `loop`.
- `let mut board = parts[0].split('/');` is later shadowed by the real `Board { ... }` binding. Confusing but harmless.
- `number_of_pieces` is computed in `parse` and dropped on the floor.

## Bitboard layout choice

The current FEN parser stores rank 8 in bits 0..7 and rank 1 in bits 56..63 — the opposite of the more common "a1 = bit 0" convention. The display layer compensates, so the board prints right-side-up, but any future move-generation code (especially anything that wants to use shifts to push pawns) needs to pick a convention and stick with it. Switching to a1 = bit 0 is the standard choice and makes pawn pushes a clean `bb << 8` (white) / `bb >> 8` (black). Worth doing before adding any movegen.

## Operator overload semantics

`Bitboard` overloads `+`, `-`, `*`, `&`, `|`, `^`, `!`, `<<`, `>>` with set-theoretic meanings (union, difference, intersection, ...). It's clever and concise but it means `a + b` is *not* numeric addition, which can surprise. Particularly unusual: `*` aliases `&`. If movegen ends up doing magic bitboard multiplications, that overload will need to come back as actual `u64` multiplication or be renamed.

## Things that would unlock real engine work

In rough order:

1. Add side-to-move + castling/en-passant/clock fields to `Board`, finish FEN parsing.
2. Fix `move_piece` to maintain `player_bbs`, remove captured pieces, and handle castling / en-passant / promotion.
3. Add an attack-table layer for non-sliding pieces (knight, king, pawn) plus sliding-piece movegen (start with Kindergarten or simple ray-loop, magic bitboards later).
4. `generate_moves(&Board) -> Vec<Move>` and a `perft` driver (compare against known node counts from the start position — `perft(5) = 4865609`).
5. Then think about search and eval. Negamax + alpha-beta + simple material eval is the usual smallest playable thing.
6. Real UCI loop: `uci`, `isready`, `position [startpos|fen ...] [moves ...]`, `go`, `bestmove`, `quit`.

## Open questions

- Why "tortle"? (D&D? Just a pun on torte?)
- Is the goal a competitive engine, a teaching engine, or just an excuse to write Rust?
