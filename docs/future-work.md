# Future work

This is a learning engine — the point is to add strengthening features one at
a time, behind a toggle, and measure each in isolation. The roadmap below is
drawn from the strong-engine playbook, in rough priority order.

## Next toggles (priority order)

1. **Killer moves / history heuristic** — at each ply, remember moves that
   caused beta cutoffs and try them early. Compounds with MVV-LVA. ~30 LOC.
2. **Endgame king PST + tapered eval** — phase-detect on material and lerp
   between middlegame and endgame king tables. Without this the king never
   centralizes in endgames.
3. **Null-move pruning** — skip a turn, search shallower; if the score still
   ≥ beta, prune. Substantial speedup, but needs zugzwang/endgame guards.
4. **UCI `Hash` spin option** — let GUIs configure TT size. The 16 MB default
   is fine, but tournaments often request 128/256 MB.
5. ~~**Incremental Zobrist hashing in `apply_move`**~~ — landed 2026-05-11,
   and measured a wash on 2026-10-04: a from-scratch recompute runs once per
   node, the incremental update ~35 times per node inside the legality
   filter, so neither is faster than the other. What would actually pay is
   pin-aware legal move generation, so movegen stops copying a board per
   move.
6. **Mid-search abort** — let `stop` and the time deadline interrupt within an
   iteration (atomic flag checked at every node). Currently the deadline is
   only checked between ID iterations.
7. **Aspiration windows** — narrow alpha/beta windows around the previous ID
   iteration's score; fall back on fail-high/low. Combines well with the TT.

Then: Lazy SMP multithreading (needs a concurrent TT — the real work; expect
~1.5–1.8× on 4 cores), late move pruning (LMP), MultiPV.

## Plan: option B — pin-aware legal move generation (deferred, gated)

Option A landed 2026-10-04: the search generates pseudo-legal moves and tests
legality lazily on the board it makes anyway, which deleted ~35 board copies
per node. Option B is the full version — decide legality *without* making the
move at all.

> **GATE RESOLVED 2026-10-05 — do not build B without new evidence.** A measured
> **24.5% faster** (16/16 paired rounds on a quiet machine, identical node
> totals). The remainder B chases is one `is_attacked` call per *searched* move
> against the bug surface listed below. Spend the effort on make/unmake instead.

**Read this first: A has probably already banked most of the win.** Before A,
the legality filter copied and applied ~35 moves per node. After A, the only
legality cost left is one `is_attacked` call per move the search *actually
tries* — 1-3 per node, on a board it was going to build regardless. So B's
marginal gain is much smaller than it looked when the copy-per-move problem was
first spotted. **Do not start B until A is measured on a quiet machine** (A's
own numbers are direction-only: 5 of 6 paired rounds favour it, median ratio
0.71, spread 0.21-1.31 at load ~10). If A's real win is large, the remainder B
chases may not be worth the bug surface.

**The bigger remaining lever is probably not B at all.** The search still does
`let mut next = *board; next.apply_move(m)` — a 120-byte copy per *searched*
move. Replacing that with make/unmake (an undo stack carrying captured piece,
castling rights, ep square, clocks and the Zobrist key) attacks a cost B does
not touch. It is also the more invasive change, so measure first.

### Design, if it goes ahead

Per node, compute once:

- `checkers` — enemy pieces attacking our king.
- `pinned` — our pieces on a ray between our king and an enemy slider with no
  other piece between, plus each one's allowed ray mask.

Then:

- **Double check** (`checkers.count() >= 2`): king moves only.
- **Single check**: king moves to safe squares; captures of the checker; and,
  if the checker is a slider, blocks on the king-checker ray. Plus the ep
  capture when the checker is the pawn that just double-pushed.
- **No check**: every move is legal except a pinned piece leaving its ray, a
  king move into an attacked square, or an ep capture that exposes the king
  along a rank.

### The three places bugs will be

1. **King-move safety must remove our own king from the occupancy** before the
   attack test, or a slider checking along the king's current square is missed
   when the king steps backwards along the ray.
2. **En passant exposing the king along a rank** — king and enemy rook on the
   same rank with both pawns between them; removing two pawns at once is the
   case no pin mask catches.
3. **Double check** — easy to generate a "capture the checker" reply that is
   legal against one checker and not the other.

### Verification (reuse what A built)

- The differential test `lazy_filter_equals_strict_generator_over_a_walk` in
  `generator.rs` extends directly: assert pin-aware output equals
  `generate_legal_moves` **in the same order**, over a wider perft walk. Keep
  the strict generator forever as the oracle.
- **Generate in the current order**, or the index-based heuristics (futility,
  LMP) change meaning and `bench` node totals will move.
- Invariants: perft unchanged; `torte bench 8` node total identical
  (1,628,363 at the time of writing). A node-count change is a bug, not a win.
- Then knps on a quiet machine, then SPRT. Twice this fortnight a large node
  or speed win came with no Elo, so the match is the only verdict.

## Known limitations to clean up

### Eval
King safety is pawn-shield only — no attack-square counting around the king,
which is the single biggest remaining gap against ~2000 opposition. No
king-tropism, no rook-on-open-file, no space term. The development term is
crude (home-square counting); a real one would score piece activity instead.

### Search
No killers/history, null-move pruning, LMR, or aspiration windows. No
mid-search abort — the deadline is only checked between ID iterations.

### Transposition table
Single-entry buckets (no two-tier/bucketed replacement). *(Historical: the
always-replace eviction, the hardcoded 16 MB size and the from-scratch hash
listed here have all since landed — depth-preferred replacement 2026-09-16,
the `Hash` spin option, and incremental Zobrist hashing 2026-05-11.)*

### Quiescence
Stand-pats even when in check (no check-evasion handling); skips
promotion-only moves (no capture component).

### Move encoding
`Move::from_uci` panics on malformed input — the REPL crashes on bad input.
Inferring castle/ep/double-push from src+dest+piece is fine for legal play but
not robust to arbitrary input.

### UCI
`stop` during search is a no-op (the search is synchronous). No `Hash` size
option, no `Ponder`, no `MultiPV`.

### Hygiene
`while true` in `torte.rs` should be `loop`. A few `dead_code` warnings on
bitboard helpers (`new`, `count`, `get_msb`) and `SQ` constants that upcoming
features will use.

## Naming

Three names for one thing: GitHub repo `tortle-engine`, Cargo package
`chess`, binary + module `torte`. Worth picking one and renaming the rest.
