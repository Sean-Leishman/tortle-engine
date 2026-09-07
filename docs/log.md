# Work log

A running journal of substantial changes, newest at the top. Each entry notes
what changed, *why*, and any visible side-effect (test count, perf number,
qualitative play difference). Mechanical refactors and typo fixes are not
logged. Derived from the `jj`/git history.

- **2026-09-07 — bench: first real calibration — torte is ~1500 Elo, and the
  gap is eval, not bugs** — bootstrapped `bench/` (fastchess 1.8.2, Sungorus
  1.4, noob_3moves book) and ran the ladder for the first time since the
  aborted 3-round run in May. Result vs Sungorus 1.4 (~2000 CCRL), 50 games at
  10+0.1: **Elo −478 ± 219** (1W/45L/4D). Diagnosis, in the order the
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
