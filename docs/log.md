# Work log

A running journal of substantial changes, newest at the top. Each entry notes
what changed, *why*, and any visible side-effect (test count, perf number,
qualitative play difference). Mechanical refactors and typo fixes are not
logged. Derived from the `jj`/git history.

- **2026-09-16 — search: qsearch check evasions (toggleable)** — qsearch stand-patted while in check, so a mate delivered *at* the horizon scored as "material up" instead of mate. `qsearch` now detects check at the node, skips the stand-pat floor entirely, searches **all** legal evasions rather than captures-only, and returns `terminal_score` when there are none; delta pruning is disabled while evading (the move list is escapes, not optional captures, so skipping one can drop the only escape). New toggle `QsearchCheckEvasions` (default on). Worked example — `7k/8/6K1/8/8/8/5Q2/8 w`, mate in 1: **with** evasions depth 1 reports `mate 1` in 63 nodes; **without**, depth 1 reports `cp 1378` and the mate only surfaces at depth 2. The ID mate test moved from depth 2 to depth 1 for the same reason. Cost is real but not uniform: `torte bench 8` rises **1,094,575 → 1,244,878 nodes (+14%)**, while a position with a loose back rank got *cheaper* at depth 5 (2,977 → 717 nodes) because resolving the checks cuts the tree earlier. SPRT pending with the rest. (1 new test, 137 total.)
- **2026-09-16 — search: depth-preferred TT replacement with aging (toggleable)** — the TT was always-replace, so a shallow entry could evict a deep one in the same search. `store` now keeps the deeper entry on a collision, except when the slot holds the same position or an entry from an *older* search; `TranspositionTable::new_search()` bumps a generation counter once per `go`, which is what stops a deep entry from ten moves ago outranking everything the current search finds. New toggle `TTDepthPreferred` (default on). **`torte bench` cannot see this** — it gives each position a fresh table, and the depth-8 total moved 1,093,928 → 1,094,575 (+0.06%, noise). The realistic measurement is a table reused across the moves of a game: replaying 12 successive searches at depth 9 from one opening line, **1 MB hash: 2,229,142 → 1,726,019 nodes (−23%)**; at the default **16 MB: 1,639,268 → 1,608,576 (−2%)**. So the win scales with table pressure — worth most in long games or when a GUI hands us a small `Hash`. SPRT still pending along with the rest. (1 new test, 136 total.)
- **2026-09-16 — search: node counts out of the search, `torte bench`, late-move and delta pruning** — the 2026-09-15 loss analysis left search depth as the open suspect (median 6 vs Sungorus's 8), and games were unmeasurable on a machine another project had at load 12-17. Node counts don't care about load, so: the node counter is now plumbed out of `find_best_move_with_window` / `search_one_iteration` into the ID callback, `info` lines carry `nodes` and `nps` (a documented UCI gap), and `torte bench [depth]` runs 8 fixed positions at fixed depth for a deterministic total. Then two prunings, both toggleable: **`LateMovePruning`** (skip quiet moves past 6/10/16 by depth ≤3, not in check, safe window) and **`DeltaPruning`** (in qsearch, skip a capture whose victim + 200 cp can't reach alpha). Bench depth 8: **2,492,044 → 1,093,928 nodes (−56%)**; LMP is 2.4× of it, delta ~1.13×. At a fixed 1 s budget the engine reaches **+1 ply** on all three test positions. Cost: on one bench position the depth-9 score moves 434 → 422 cp and the move changes, which is the expected pruning trade. **SPRT pending for both** (machine still loaded), so neither has an Elo number. One test changed meaning rather than breaking: `ordering_does_not_change_best_score` now sets `late_move_pruning: false`, because **pruning by move index makes ordering semantic, not cosmetic** — a good move sorted late is pruned, not merely searched late. Also gave `run-ladder.sh` the load guard `sprt.sh` already had, since it produced the contaminated run. (135 tests.)
- **2026-09-15 — eval: Texel tuning, plus rook-file and king-attack terms** — ran a cheap check first: the 343 losses to Sungorus are slow positional slides (in 248 torte's own eval was already ≤ −0.8 before it went two pawns down; only 15 were tactical blindsides), and no single missing feature stood out (rook-on-open-file z=2.2, king-zone attackers z=2.1, rest ≤0.5) — pointing at mis-weighted terms rather than one missing term. So: every eval weight now lives in a generated `[mg, eg]` table (`search/params.rs`); each term reports (weight, count) through a `Trace`, so the same code drives both `eval` and the tuner. `torte tune <epd> [epochs] [lambda]` (`tune.rs`) fits all weights with full-batch Adam against 725k Zurichess quiet-labeled positions (`bench/data/`, gitignored; mirror at github.com/KierenP/ChessTrainingSets), with a 10% held-out split and an optional L2 pull toward the starting weights. Loss 0.0651 → 0.0583. **Tuned vs hand weights: +129 ± 34 Elo** self-play (400 games). Vs Sungorus **−167 ± 53** over 179 games (previously −397) — but that run was cut short by the OOM killer on a machine loaded by other jobs, with 6 time forfeits, so treat it as direction, not a number. L2 sweep: any lambda ≥1e-7 hurt held-out loss (0.0585 → 0.0607+), so lambda 0 ships. Then added `RookOpenFile` (open/half-open) and `KingAttack` (per-piece-type hits on the enemy king zone, gated on ≥2 attackers) and retuned: held-out 0.05853 → 0.05800; **SPRT vs the tuned build still pending** (machine too loaded to play 10+0.1 games honestly). New `bench/sprt.sh` (refuses to start on a loaded machine). Weight-sign tests rewritten to check what a term counts, not the tuned sign. (135 tests.)

- **2026-09-09 — search+eval: draw detection and a development term, both
  toggleable** — the two fixes the 2026-09-07 calibration pointed at.
  - **Draw detection** (`DrawDetection`, default on). The search had none:
    no repetition check, no fifty-move rule. `negamax` is now a thin gate in
    front of `negamax_inner` that, before pushing the node's own key, returns
    `DRAW_SCORE` (0) if the position repeats an ancestor or the halfmove clock
    has hit 100. The repetition path is a `Vec<u64>` threaded alongside
    `killers`/`history`, seeded from the *played game* — `position ... moves`
    now returns those keys via `parse_position_with_history`, so the engine can
    see a repetition back into the game rather than only inside its own tree.
    Two subtleties worth keeping: the scan steps by 2 (only same-side-to-move
    positions can repeat) and stops after `halfmove_clock` plies (a capture or
    pawn move makes everything earlier unreachable); and the null-move child
    sets `halfmove_clock = 0` so the scan can't match across a null-move
    boundary, which isn't reachable by real play. The fifty-move return is
    guarded so a checkmate delivered on the 100th halfmove still wins.
  - **Development term** (`Development`, default on). `development()` penalises
    each minor still on its home square, adds an extra penalty per undeveloped
    minor when the queen has already left home, phase-scales both so they
    vanish in the endgame, and adds a flat 10 cp tempo bonus after the
    perspective flip. Aimed squarely at the early-queen wandering the ladder
    games showed (Qb3/Qb5+/Qb4/Qc4/Qe2/Qd1 inside 17 moves with every minor at
    home): the queen was picking up ~15-20 cp of mobility and central PST for
    coming out early and nothing was charging her for it.

  **Measurement, and a caveat that matters more than the numbers.** Self-play
  gauntlet, 320 games at 10+0.1, each toggle disabled in turn against the new
  default:

  | Term | Self-play Elo | Note |
  |------|---------------|------|
  | `DrawDetection` | **+62 ± 45** | clear win against itself |
  | `Development` | **+26 ± 47** | positive, error bar crosses zero |

  But against a *stronger* opponent the gain does not show up. Matched
  200-game runs vs Sungorus 1.4, same TC, same book, toggles the only
  difference:

  | Build | Elo vs Sungorus | Score |
  |-------|-----------------|-------|
  | both toggles **off** (= the old engine) | −386.6 ± 64.7 | 9.75% |
  | both toggles **on** (new default) | −396.7 ± 80.9 | 9.25% |

  That is *no measurable change* — the two overlap comfortably, and the new
  build is nominally the worse of the pair. The reading: draw detection buys
  points in positions that are actually drawable, and against an opponent ~400
  Elo stronger those barely arise; torte is being outplayed, not shuffled into
  repetitions. Self-play A/B measures value against an equal, which is a
  different and easier question. Kept both on anyway — draw detection is a
  correctness fix before it is a strength feature (an engine that cannot see a
  repetition or the fifty-move rule will happily shuffle away a won game), and
  neither term costs anything measurable.

  **Also: the −478 from 2026-09-07 was noise.** That figure came from 50
  games (±219). The matched 200-game baseline above puts the pre-change engine
  at −386.6 ± 65, so torte was never ~1500 — it was ~1610 then and is ~1600
  now. Lesson recorded: 50 games is not a measurement, it is a hint. The
  intermediate 50-game run of the new build read −263 ± 114 and would have
  supported a triumphant "+215 Elo!" write-up that the 200-game run flatly
  contradicts.

  Qualitative: opening play is visibly saner — 1. e4 e5 2. Nf3 Nc6 3. d4
  rather than an early queen sortie.

  Six existing exact-score tests had to be scoped (`development: false`) rather
  than have the tempo constant baked into their expected values — they assert
  PST/material symmetry, and tempo is deliberately *not* symmetric because it
  belongs to whoever is on the move. 15 new tests, 135 total (+2 ignored perft).

- **2026-09-07 — bench: first real calibration — torte is ~1500 Elo, and the
  gap is eval, not bugs** — bootstrapped `bench/` (fastchess 1.8.2, Sungorus
  1.4, noob_3moves book) and ran the ladder for the first time since the
  aborted 3-round run in May. Result vs Sungorus 1.4 (~2000 CCRL), 50 games at
  10+0.1: **Elo −478 ± 219** (1W/45L/4D). *(Superseded 2026-09-09: a matched
  200-game run put this same build at −386.6 ± 64.7. The 50-game figure was
  noise — the diagnosis below still holds, the absolute number doesn't.)*
  Diagnosis, in the order the
  hypotheses were killed:
  - *Not the clock.* Zero time forfeits across 50 games. The `bench/README.md`
    caveat about deadlines only being checked between ID iterations was stale —
    mid-search abort (823b447) fixed it. Caveat rewritten.
  - *Not crashes or illegal moves.* Zero disconnects, zero illegal moves.
  - *Not unsound pruning.* Self-play A/B, default vs
    `NullMovePruning`/`LateMoveReductions`/`FutilityPruning`/`AspirationWindows`
    all off, 200 games at 10+0.1: **default wins by +166 ± 47 Elo, LOS 100%**.
    The speculative-pruning stack is a large net gain, not a soundness hole.
  - *Not raw speed.* Depth 10 on startpos in 1.5 s, depth 8 on kiwipete in 2 s.
  - *It's the eval.* Game records show no hung pieces and evals that track
    Sungorus move for move, then drift 0.00 → −2.5 over ~30 quiet moves at 2–3
    plies less depth (torte 5–8, Sungorus 7–10 in real middlegames). Visible
    symptom: early queen wandering (Qb3/Qb5+/Qb4/Qc4/Qe2/Qd1 inside 17 moves)
    with no development or tempo term to punish it.
  - *Also found:* the search has **no draw detection at all** — no repetition
    check, no fifty-move rule (`grep` for either in `search/` comes back
    empty). The engine cannot see a perpetual coming, cannot claim a saving
    repetition, and scores a repeated position by material instead of 0. Four
    of the 50 games ended in repetition draws. Logged in `future-work.md`.
  Also fixed `run-ladder.sh`, whose preflight demanded `engines/vice` and
  `engines/stockfish-bin` even for `OPP=sungorus`, so a single-opponent run
  couldn't start without a 110 MB Stockfish download. Untracked
  `bench/config.json` — it's a fastchess autosave that rewrites itself on every
  run and was dirtying the tree.
- **2026-05-14 — eval: bishop pair + king-safety pawn shield (toggleable)** —
  `bishop_pair()` adds a flat ±30 cp for holding both bishops; `king_safety()`
  rewards an intact f/g/h (or mirrored) pawn shield in front of a king still
  on its home rank, phase-scaled so it fades to 0 in the endgame. Introduced
  `EvalConfig` (in `eval.rs`) — `eval` now takes `eval(board, EvalConfig)`
  instead of a growing list of bools; `SearchConfig::eval_config()` projects
  the search toggles onto it. New toggles `BishopPair`, `KingSafety` (both
  default on). 8 new tests, 116 total.
- **2026-05-14 — eval: pawn structure (toggleable)** — `pawn_structure()` in
  `eval.rs`: doubled (−15 cp per extra pawn on a file), isolated (−15 cp, no
  friendly pawn on adjacent files), passed (rank-scaled `PASSED_PAWN_BONUS`,
  phase-scaled to ~2× in a pure pawn endgame). New toggle `PawnStructure`
  (default on). Pairs with the tapered pawn EG table — passers get amplified
  exactly where they matter. 6 new tests, 108 total.
- **2026-05-11 — eval+search: piece-square tables (toggleable)** — added 6
  PST arrays (P/N/B/R/Q/K-mg) in `eval.rs`; black pieces look up
  `PST[sq ^ 56]` to mirror rank. New toggle `PieceSquareTables` (default on).
  Qualitative effect: the engine plays `1. Nc3` from startpos instead of
  `1. a3`. 4 new tests, 69 total.
- **2026-05-11 — search: transposition table (toggleable)** —
  `transposition.rs` with Zobrist keys (seeded xorshift, lazy `OnceLock`),
  `TTEntry { key, score, best_move, depth, bound }`, power-of-two-sized table.
  Mate scores adjusted by ply on store/retrieve. TT move used as PV-first
  ordering hint. New toggle `TranspositionTable` (default on); cleared on
  `ucinewgame`. ~2.2× speedup on kiwipete d6 (7520ms → 3395ms), same best
  move. 13 new tests.
- **2026-05-11 — search: iterative deepening + UCI time controls** —
  `iterative_deepening` loops depths 1..=max with a deadline; short-circuits
  on mate. New `GoArgs { max_depth, time_budget_ms }` from `parse_go` handles
  `depth N`, `movetime ms`, `wtime/btime/winc/binc`. New toggle
  `IterativeDeepening` (default on). Mid-iteration abort isn't supported yet.
  9 new tests.
- **2026-05-11 — search: quiescence (toggleable)** — `qsearch` recursively
  follows captures at depth 0 with stand-pat. Fixes the horizon effect at low
  depths (depth-1 score for knight-takes-defended-queen drops from +320 to 0).
  New toggle `Quiescence` (default on). 3 new tests.
- **2026-05-11 — search: MVV-LVA move ordering (toggleable; `SearchConfig`
  pattern established)** — `mvv_lva_score` ranks captures by
  `victim * 10 - attacker`; `order_moves` sorts before searching. First toggle
  on `SearchConfig`; the pattern all subsequent search features follow. ~3.2×
  speedup on kiwipete d5. 5 new tests.
- **2026-05-11 — uci: real UCI protocol loop** — `uci.rs` handles
  `uci`/`isready`/`ucinewgame`/`position`/`go`/`setoption`/`quit`, emits
  `info ... pv <mv>` + `bestmove`. `Torte::run` delegates here. Replaces the
  original hand-rolled REPL. 8 new tests.
- **2026-05-11 — search + eval: negamax with alpha-beta + material eval** —
  `find_best_move` does negamax + alpha-beta + mate scoring; eval is
  material-only at this point. `MATE_SCORE = 29_000`, `INFINITY = 30_000`.
  Mate-in-1, stalemate, free-capture tests pass. 6 new tests.
- **2026-05-11 — movegen: legal moves + perft** — `is_attacked`,
  `generate_legal_moves` (pseudo-legal + post-move king-safety filter).
  Castling explicitly checks rights/path-empty/king-safety. `perft` verified
  against startpos d1..d5 (4865609) and kiwipete d1..d3 (97862). 8 new tests.
- **2026-05-11 — movegen: sliding-piece magic bitboards** — plain magic
  bitboards in `magic.rs` for rook/bishop, magics searched at startup via
  xorshift sparse-random. Tables behind `OnceLock`; `magic::init()` eagerly
  initializes at engine startup so first-move latency doesn't surprise. 6 new
  tests.
- **2026-05-11 — movegen: knight/king/pawn attack tables** — `attacks.rs`
  with `const fn` static arrays built at compile time. Accessors
  `knight_attacks`, `king_attacks`, `pawn_attacks(sq, color)`. 7 new tests —
  the first tests in the repo.
- **2026-05-11 — board: complete `apply_move`** — infers castle/ep/double-push
  from src+dest+piece; handles promotion (5-char UCI like `e7e8q`); mutates
  `side_to_move`, `castling`, `en_passant`, `halfmove_clock`,
  `fullmove_number` correctly. No legality checks — that's the movegen layer.
- **2026-05-11 — board: FEN fields + state on `Board`** — `side_to_move`,
  `castling: CastlingRights` (u8 bitflags), `en_passant: Option<SQ>`,
  `halfmove_clock`, `fullmove_number`. `Board::parse` consumes all six FEN
  fields with sensible defaults. `Debug` printer shows the state below the
  board.
- **2026-05-11 — board: bitboard layout flip to a1 = bit 0** — `parse` was
  storing FEN ranks upside-down. Now: a1=0, h8=63, white pawn push =
  `bb << 8`. The Display printer was already iterating in reverse so it stayed
  correct. Cleared the way for movegen.
- **2026-05-11 — board: `move_piece` bug fix** — used the passed `Piece`
  argument (previously ignored), synced `player_bbs` (previously drifted), and
  cleared the captured opponent piece on `to`.
