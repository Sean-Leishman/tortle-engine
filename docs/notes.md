# Notes

Loose observations and questions for future-me. Not a roadmap, not a spec.

## Naming

- GitHub repo: `tortle-engine`
- Cargo package: `chess`
- Binary: `torte`
- Top-level struct + module: `Torte` / `torte`

So the "real" name is probably *Tortle* (with the engine binary clipped to *torte*), but the local checkout directory is just `chess/`. Worth picking one and renaming the others at some point.

## Things that are obviously broken / unfinished

- `Move::from_uci` still panics on truly malformed input (length != 4 && != 5, or unknown promotion char). The REPL crashes on bad input as a result.
- `apply_move` does no legality checking: doesn't enforce side-to-move, doesn't check castling-through-check or rook-still-there, accepts pawn captures onto empty squares (sometimes resolving as ep, sometimes as a no-op capture), etc. Movegen is the layer that should produce only legal moves.
- `while true` in `Torte::run` should be `loop`.
- `let mut board = parts[0].split('/');` is later shadowed by the real `Board { ... }` binding. Confusing but harmless.
- `number_of_pieces` is computed in `parse` and dropped on the floor.

## Bitboard layout choice

The board uses the conventional "a1 = bit 0" layout: a1..h1 = bits 0..7, a8..h8 = bits 56..63. White pawn pushes are `bb << 8`, black pawn pushes are `bb >> 8`. `Board::parse` flips FEN ranks (rank 8 first in the string -> bits 56..63) so the storage matches `SQ::make(rank, file) = rank * 8 + file`.

## Operator overload semantics

`Bitboard` overloads `+`, `-`, `*`, `&`, `|`, `^`, `!`, `<<`, `>>` with set-theoretic meanings (union, difference, intersection, ...). It's clever and concise but it means `a + b` is *not* numeric addition, which can surprise. Particularly unusual: `*` aliases `&`. If movegen ends up doing magic bitboard multiplications, that overload will need to come back as actual `u64` multiplication or be renamed.

## Things that would unlock real engine work

In rough order:

1. ~~Add side-to-move + castling/en-passant/clock fields to `Board`, finish FEN parsing.~~ (done)
2. ~~Extend `move_piece` (and `Move`) to handle castling, en-passant, and promotion, and to *mutate* `side_to_move` / castling rights / ep square / clocks on each move.~~ (done — `apply_move` now infers castle/ep/double-push from src+dest+piece, handles 5-char promotion UCI, and mutates all the new fields)
3. ~~Attack tables for all six pieces.~~ (done — non-sliding via compile-time `const fn` in `attacks.rs`; sliding via plain magic bitboards in `magic.rs`, magics searched at startup with xorshift sparse-magic search.)
4. ~~`generate_moves(&Board) -> Vec<Move>` and a `perft` driver.~~ (done — `generator.rs` does pseudo-legal + post-move king-safety filter; `perft.rs` matches published numbers for startpos d1..d5 and kiwipete d1..d3.)
5. Search and eval. Eval is material-only (`search/eval.rs`); search is negamax + alpha-beta + mate scoring (`search/search.rs`). Each strengthening feature is a `SearchConfig` field that can be toggled via UCI `setoption` — this lets you A/B-test improvements one at a time. Done so far:
   - MVV-LVA move ordering (toggle `MoveOrdering`, default on; ~3× speedup on tactical positions).
   - Quiescence search (toggle `Quiescence`, default on; fixes the horizon effect at low depths).
   - Iterative deepening (toggle `IterativeDeepening`, default on; emits info per iteration; short-circuits on mate; enables `go movetime` / `go wtime/btime` via between-iteration deadline check).
   - Transposition table (toggle `TranspositionTable`, default on; 16 MB default; Zobrist hashing computed from scratch per node; ~2.2× speedup on kiwipete d6; mate scores adjusted by ply on store/retrieve; TT move used as PV ordering hint).
   - Piece-square tables (toggle `PieceSquareTables`, default on; six 64-entry tables P/N/B/R/Q/K-mg; black pieces mirror via `sq ^ 56`; gives real opening play vs material-only's `a2a3`).
   - Pending: killers/history, null-move pruning, mid-search abort, qsearch refinements (check evasions, promotion handling), incremental Zobrist hashing, UCI `Hash` size option, endgame king PST + tapered eval.
6. ~~Real UCI loop.~~ (done — `uci.rs` handles `uci`/`isready`/`ucinewgame`/`position`/`go`/`stop`/`quit` and emits `info ... pv <mv>` + `bestmove`. No time management yet — only `go depth N` is honoured; other `go` variants fall back to default depth 6. Async/stop-during-search isn't supported because the search is synchronous.)

## Open questions

- Why "tortle"? (D&D? Just a pun on torte?)
- Is the goal a competitive engine, a teaching engine, or just an excuse to write Rust?
