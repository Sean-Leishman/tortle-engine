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

### 2026-10-17 — threats shipped, make/unmake rejected, an LMR indexing bug found

- **Threats: +24 ± 21 Elo over 801 games.** First eval term in five attempts to
  measure positive. Shipped on by default; `params.rs` carries its fit and
  `bench 8` matches the measured binary exactly (1,524,096 nodes).
- **Make/unmake: rejected without building it.** The board copy is 1.57 per node
  and ~8 inlined vector moves (`objdump` on `qsearch`), under 1% of runtime. Two
  wall-clock micro-benchmarks gave *opposite* answers — one confounded by a
  `black_box` barrier, one by LLVM swapping the inlined copy for `memcpy` at 248
  bytes. Reading the generated code was cheaper than both and the only thing
  that answered the question. Revisit only if `Board` grows a lot, i.e. as a
  prerequisite for an NNUE accumulator.
- **`LateMoveReductions` is indexed by all legal moves, not quiets** — the same
  defect fixed for LMP on 2026-09-28. Quiet-indexing it reduces far less (+41%
  bench nodes), so the bug has been buying aggression; whether that aggression
  is worth Elo is now under SPRT. This was found incidentally while chasing SEE
  ordering, and is the better lead of the two.
- **SEE ordering: built, default off, not queued.** `see < 0` is the wrong cut
  (8.4% of all beta cutoffs are caused by a capture SEE calls losing); ≥100 cp
  is −10% nodes with LMR off. Its measurement is entangled with the LMR index,
  so it waits.

Open question for next session: if `LmrQuietIndex` measures null, the search
category is close to exhausted at this strength and the direction decision
(NNUE, which needs make/unmake first, versus stopping at ~1840) is the real one.

### 2026-10-18 — the LMR index null, and why the LMP analogy broke

`LmrQuietIndex` measured **−1.3 ± 20.4 Elo over 800 games**, zero forfeits. The
open question from the last session is answered: it is a null, and the search
category is behaving as the eval-is-the-gap diagnosis predicted.

The useful part is why my prediction failed. I expected LMP-off's shape (+67%
nodes, +132 Elo) because the bug was literally the same bug in a sibling
feature. The difference is that **LMR verifies and LMP did not**: a reduced
search that beats alpha gets re-searched at full depth, so a wrong trigger index
costs a wasted shallow pass, whereas a pruned move is simply gone. Verified
speculation is insensitive to its trigger parameter; unverified speculation is
not — which also explains, retroactively, why three LMP threshold settings could
not be tuned into a gain and why NMP's R and the aspiration width both measure
level.

`SeeOrdering` is now unblocked and under SPRT. The LMR index measuring level
means the interaction cannot be hiding a win in it.

Still open, and now the real decision: the search category has four 800-game
nulls and one large negative against one speedup. Eval has one convert
(`Threats`, +24) in five attempts. NNUE is the only lever of a different
magnitude, and it needs make/unmake as a prerequisite — that is a different size
of project, not another toggle.

### 2026-10-20 — SEE ordering is promising and unproven; runs now get certified

`SeeOrdering` measured **+10.9 ± 20.6 over 800 games** (51.56%, LOS 85%). Best
search point estimate since lazy legality, and still consistent with zero.
Separating +11 from 0 needs ~2,800 games, so a 2,600-game confirmation is queued
rather than shipping on a 50/50 bet.

The run survived a machine that hit load 51 with swap completely full, engines at
28% CPU — and recorded **zero forfeits**, the check this project has been using to
certify runs clean. So that check passed on a starving match. `bench/certify-run.py`
replaces it with a direct measurement: the within-game depth differential, banded
by round order. Both engines play the same game, so a depth gap between them is the
engines' doing, not the position's. This run reads −0.032 ± 0.028 ply and flat while
mean depth climbs 7.88 → 9.86 as the machine recovers: shared starvation, unbiased
comparison, keep the games.

Building it produced two false alarms, both the same mistake — assuming a
differential means bias. It doesn't: `LmrQuietIndex` runs 0.18 ply shallower
because it searches 41% more nodes, which is the trade being measured. Only
*drift* indicates bias. And drift must be measured against round order, not
absolute depth: depth bands select long drawn endgames (138 plies, 45% draws vs
97 and 14%), which made a clean run look suspect. I wrote that warning in the
docstring and then banded by depth anyway.

Withdrawn from 10-18: the claim that starvation would understate `SeeOrdering`.
Unsupported, and the test proposed for it was confounded. The run is unbiased and
underpowered — different complaints, different fixes.
