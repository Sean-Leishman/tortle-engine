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
5. **Incremental Zobrist hashing in `apply_move`** — avoid re-hashing the
   whole board each node. Probably ~20% speedup overall.
6. **Mid-search abort** — let `stop` and the time deadline interrupt within an
   iteration (atomic flag checked at every node). Currently the deadline is
   only checked between ID iterations.
7. **Aspiration windows** — narrow alpha/beta windows around the previous ID
   iteration's score; fall back on fail-high/low. Combines well with the TT.

Then: Lazy SMP multithreading (needs a concurrent TT — the real work; expect
~1.5–1.8× on 4 cores), late move pruning (LMP), MultiPV.

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
The Zobrist hash is recomputed from scratch each node (no incremental update
on `apply_move`). Always-replace eviction. Size hardcoded at 16 MB — no UCI
`Hash` spin option.

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
