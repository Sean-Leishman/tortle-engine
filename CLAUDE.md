# tortle-engine (chess)

## Purpose

A working UCI chess engine in Rust, internally codenamed "torte" (binary name) and "tortle-engine" on GitHub. The engine has bitboard-based move generation (magic bitboards for sliders, const-fn tables for non-sliders), negamax + alpha-beta search with toggleable enhancements (MVV-LVA ordering, quiescence, iterative deepening, transposition table, piece-square tables), and a real UCI loop that speaks to GUIs. Verified by perft against published numbers (startpos through depth 5, kiwipete through depth 3). It's a learning project — every search/eval refinement lives behind a `setoption` toggle so its impact can be measured independently.

## Tech stack

- **Language:** Rust (edition 2021)
- **Build:** Cargo, no dependencies (`[dependencies]` is empty in `Cargo.toml`)
- **Binary:** `torte` (configured via `[[bin]]` in `Cargo.toml`, source at `src/main.rs`)
- **Crate name:** `chess` (the package), but the binary and module hierarchy are named `torte`
- **VCS:** Colocated `jj` + `git`. Use `jj describe` / `jj split` for new commits — see the `feedback_use_jj_for_commits` memory note.

## Key files / entry points

```
src/
  main.rs                       // entry; constructs a Torte and calls run()
  torte/
    mod.rs                      // declares: board, core, movegen, search, torte, uci
    torte.rs                    // Torte struct; run() initializes magic tables, parses startpos, delegates to uci::run
    uci.rs                      // UCI protocol loop: uci/isready/ucinewgame/position/go/setoption/quit
    board/
      board.rs                  // Board struct + CastlingRights, FEN parse, apply_move (castling/ep/promotion), Debug printer
      pieces.rs                 // Color, Piece enums; FEN/index/glyph mappings
    core/
      bitboard.rs               // Bitboard(u64) with set-theoretic operator overloads
      sq.rs                     // SQ(u8) square index (a1=0, h8=63); make(rank,file); from_uci
      piece_move.rs             // Move{src,dest,promotion}; PromotionPiece; to_uci/Display
    movegen/
      attacks.rs                // KNIGHT/KING/PAWN attack tables built via const fn
      magic.rs                  // plain magic bitboards for rook/bishop, search-at-startup
      generator.rs              // is_attacked, generate_legal_moves, king_square
      perft.rs                  // perft + perft_divide for movegen verification
    search/
      eval.rs                   // material + piece-square tables; eval(board, use_pst) -> i32
      search.rs                 // SearchConfig, negamax, qsearch, iterative_deepening, find_best_move_*
      transposition.rs          // Zobrist keys+hash, TTEntry, Bound, TranspositionTable
```

## How to run / dev

```
cargo run --release      # release binary at target/release/torte; recommended for play
cargo run                # debug; magic-bitboard search at startup takes ~15s
cargo test --release     # ~3s for normal tests
cargo test --release -- --ignored   # adds perft depth-5 startpos + depth-3 kiwipete (~2s more)
```

The engine starts speaking UCI on stdin:

```
uci                                         # responds with id name/author + option lines + uciok
isready                                     # responds with readyok
position startpos moves e2e4 e7e5
go depth 6                                  # streams info depth N ... pv <mv>, then bestmove <uci>
go movetime 1000                            # search ~1s (deadline checked between ID iterations)
go wtime 60000 btime 60000 winc 1000 binc 1000   # budget = time/30 + inc/2
setoption name MoveOrdering value false     # any toggle, see docs/architecture.md
quit
```

REPL conveniences alongside UCI: `d`/`board` prints the position + current `SearchConfig`; bare UCI moves (`e2e4`) apply directly; `exit` is an alias for `quit`; plain integer after `go` (`go 4`) is treated as `go depth 4`.

## Conventions noticed

- Modules nest two deep with a re-exporting `mod.rs` at each level. Files often share the parent module's name (`board/board.rs`, `torte/torte.rs`, `search/search.rs`), so imports look like `use crate::torte::board::board::Board`. Newer additions (`uci.rs`, `movegen/attacks.rs`, `search/eval.rs`) skip the stutter when there's only one obvious type.
- **Search-feature toggle pattern**: every search/eval refinement is a `bool` field on `SearchConfig` (in `search/search.rs`). To add one: (1) add the field, (2) add the dispatch inside the search code, (3) add an `option name ... type check default true` line in `uci::emit_options`, (4) add a `setoption` arm in `uci::apply_setoption`. Default everything to `true`; the toggle exists for A/B measurement and debugging regressions. Identity tests (same score with/without) catch unintended semantic changes; perf tests show the optimization actually saves work.
- Pieces and colours are indexed by hand-rolled `to_index` / `from_index` rather than `#[repr(u8)]` + `as`.
- Bitboards: `a1 = bit 0`, `h8 = bit 63`. White pawns push `bb << 8`, black pawns `bb >> 8`.
- `Board` carries per-piece bitboards (`bbs[12]`) plus per-colour aggregate bitboards (`player_bbs[2]`). `apply_move` keeps both in sync, including captures, castling rook moves, en-passant, and promotions.
- Operator overloads on `Bitboard` have set-theoretic meaning: `Add` = union, `Sub` = difference, `Mul` = intersection. Be careful reading expressions.
- Errors use `std::io::Error` with `InvalidInput` even for non-IO failures (lightweight; not idiomatic, but fine for now).
- Mate scores follow the `MATE_SCORE - ply` convention internally; UCI's `score mate N` is signed moves-to-mate (positive = we mate). Conversion happens in `format_score`.

## Current toggles

(See `docs/architecture.md` for full details.)

| Setoption name        | Default | What it controls |
|-----------------------|---------|------------------|
| `MoveOrdering`        | true    | MVV-LVA captures-first ordering (~3× speedup on kiwipete d5) |
| `Quiescence`          | true    | Recursive capture search at leaves; fixes horizon effect |
| `IterativeDeepening`  | true    | Searches depths 1..max with deadline aborts and per-depth info |
| `TranspositionTable`  | true    | 16 MB Zobrist-keyed cache; ~2.2× speedup at kiwipete d6 |
| `PieceSquareTables`   | true    | Positional bonuses (tapered PSTs + mobility); makes opening play actually look like chess |
| `PawnStructure`       | true    | Doubled/isolated penalties + rank-and-phase-scaled passed-pawn bonus |
| `BishopPair`          | true    | Flat +30 cp for holding both bishops |
| `KingSafety`          | true    | Pawn-shield bonus for a castled king, phase-scaled (fades in the endgame) |
| `Development`         | true    | Undeveloped-minor + early-queen penalties (phase-scaled) and a flat 10 cp tempo bonus. **+26 ± 47 Elo** self-play; **no measurable effect vs Sungorus** |
| `KillerMoves`         | true    | Two quiet cutoff moves remembered per ply, tried early; persists across ID iterations |
| `HistoryHeuristic`    | true    | `[side][from][to]` table bumped by depth² on quiet cutoffs; orders quiets globally |
| `MidSearchAbort`      | true    | Checks the `AbortSignal` (deadline + atomic stop) every 2048 nodes so `go movetime` can interrupt inside an iteration |
| `NullMovePruning`     | true    | R=2 at depth ≥ 3, with in-check and zugzwang (no non-pawn material) guards |
| `AspirationWindows`   | true    | ±100 cp window around the previous ID score from depth 5; full re-search on fail high/low |
| `LateMoveReductions`  | true    | R=1 on quiet non-promotion moves from index 3 at depth ≥ 3, full-depth re-search if it beats alpha |
| `FutilityPruning`     | true    | Frontier depths 1-2, 150 cp × depth margin, on quiet non-promotion moves |
| `Razoring`            | **false** | Drop to qsearch at depths 1-3 when static eval + margin < alpha. Measured a wash-to-loss against the rest of the stack; kept for re-tuning |
| `DrawDetection`       | true    | Repetition (2-fold within the search path, including the played game) + fifty-move rule, scored 0. **+62 ± 45 Elo** self-play; **no measurable effect vs Sungorus** — kept as a correctness fix |
| `Hash`                | 16 (spin) | TT size in MB, 1..1024 |

The whole speculative-pruning stack together (`NullMovePruning` +
`LateMoveReductions` + `FutilityPruning` + `AspirationWindows`) is worth
**+166 ± 47 Elo** over having all four off — measured, 200 games at 10+0.1.

## Gaps / known limitations

- **Strength**: ~1600 (measured 2026-09-09: −397 ± 81 Elo vs Sungorus 1.4, 200 games at 10+0.1). The remaining gap to ~2000 is *eval*, not search — see `docs/log.md` for the runs that ruled out the clock, crashes, and unsound pruning.
- **Eval**: king safety is pawn-shield only — no attack-square counting around the king, which is the biggest single gap left. No king tropism, no rook-on-open-file, no space term. `Development` is crude home-square counting rather than real activity scoring.
- **Search**: no LMP, no MultiPV, no SMP. (Killers, history, NMP, LMR, aspiration, futility and mid-search abort all landed — see the toggle table.)
- **TT**: always-replace eviction, single-entry buckets (no depth-preferred or two-tier replacement).
- **Quiescence**: stand-pats even when in check (no check-evasion handling); skips promotion-only moves (no capture component).
- **Move encoding**: `Move::from_uci` panics on malformed input — the REPL crashes on bad input. Inferring castle/ep/double-push from src+dest+piece is fine for legal play but not robust to arbitrary inputs.
- **UCI**: `stop` is still a no-op — `AbortSignal` *has* a `stop: Option<Arc<AtomicBool>>` and `MidSearchAbort` checks it, but `handle_go` only ever builds `AbortSignal::with_deadline`, so nothing sets the flag and the search is driven synchronously from the reader loop anyway. Wiring it up is mostly moving the search off the reader thread. No `Ponder` / `MultiPV`. `info` lines carry no `nodes`/`nps`, and the `pv` is a single move (no PV line extraction).
- **Hygiene**: `while true` in `torte.rs` left over from original scaffolding (replaceable with `loop`); a few `dead_code` warnings on bitboard helpers (`new`, `count`, `get_msb`) and SQ constants (`NONE`, `is_ok`, etc.) that will be used by upcoming features.

## Work log

A running journal of substantial changes, newest at the top. Each entry should reference its `jj` commit and note: what changed, *why* (the intent), and any visible side-effect (test count, perf number, qualitative play difference). Don't log mechanical refactors or single-line typo fixes.

- **2026-09-09 — search+eval: draw detection + development term (toggleable)** — the search had *no* draw detection at all: no repetition check, no fifty-move rule. `negamax` is now a gate in front of `negamax_inner` that returns 0 for a repeated position or a clock at 100, using a `Vec<u64>` path threaded alongside `killers`/`history` and seeded from the played game via the new `parse_position_with_history`. Scan steps by 2 (same side to move) and stops after `halfmove_clock` plies; null-move children zero the clock so the scan can't cross a boundary real play can't reach. Separately, `development()` penalises minors on home squares and an early queen, phase-scaled, plus a 10 cp tempo bonus — aimed at the early-queen wandering the ladder games exposed. New toggles `DrawDetection`, `Development`. Self-play (320 games): DrawDetection **+62 ± 45**, Development **+26 ± 47**. But matched 200-game runs vs Sungorus read −386.6 ± 64.7 (both off) vs −396.7 ± 80.9 (both on) — **no measurable gain against a stronger opponent**; self-play A/B answers a different, easier question. Kept on: draw detection is a correctness fix first. Also established that 2026-09-07's −478 was noise — the real baseline is −387, so torte was ~1610 all along. (15 new tests, 135 total.)
- **2026-09-07 — bench: first real calibration, and what it ruled out** — torte measured at **−478 ± 219 Elo vs Sungorus 1.4** (~2000 CCRL), 50 games at 10+0.1. *(That absolute number was noise; a matched 200-game run on 2026-09-09 reads −387 ± 65. The diagnosis below stands.)* Zero time forfeits, zero crashes, zero illegal moves. A self-play A/B with the whole speculative-pruning stack disabled showed the stack is worth **+166 ± 47 Elo**, killing the "unsound pruning" hypothesis. Speed is fine (depth 10 startpos in 1.5 s). The gap is eval: games show no hung pieces and evals that track Sungorus, then drift 0.00 → −2.5 over ~30 quiet moves. Fixed `run-ladder.sh`'s preflight (demanded every opponent binary even for `OPP=sungorus`) and untracked `bench/config.json` (a fastchess autosave).
- **2026-05-14 — eval: bishop pair + king-safety pawn shield (toggleable)** — `bishop_pair()` adds a flat ±30 cp for holding both bishops; `king_safety()` rewards an intact f/g/h (or mirrored) pawn shield in front of a king still on its home rank, phase-scaled so it fades to 0 in the endgame. Introduced `EvalConfig` (in `eval.rs`) — `eval` now takes `eval(board, EvalConfig)` instead of a growing list of bools; `SearchConfig::eval_config()` projects the search toggles onto it. New toggles `BishopPair`, `KingSafety` (both default on). (8 new tests, 116 total.)
- **2026-05-14 — eval: pawn structure (toggleable)** — `pawn_structure()` in `eval.rs`: doubled (−15 cp per extra pawn on a file), isolated (−15 cp, no friendly pawn on adjacent files), passed (rank-scaled `PASSED_PAWN_BONUS`, phase-scaled to ~2× in a pure pawn endgame). `eval` signature changed to `eval(board, use_pst, use_pawn_structure)`. New toggle `PawnStructure` (default on). Pairs with the tapered pawn EG table — passers get amplified exactly where they matter. (6 new tests, 108 total.)
- **2026-05-11 — eval+search: piece-square tables (toggleable)** — added 6 PST arrays (P/N/B/R/Q/K-mg) in `eval.rs`; black pieces look up `PST[sq ^ 56]` to mirror rank. New toggle `PieceSquareTables` (default on). Qualitative effect: engine plays `1. Nc3` from startpos instead of `1. a3`. (4 new tests, 69 total.)
- **2026-05-11 — search: transposition table (toggleable)** — `transposition.rs` with Zobrist keys (seeded xorshift, lazy `OnceLock`), `TTEntry { key, score, best_move, depth, bound }`, power-of-two-sized table. Mate scores adjusted by ply on store/retrieve. TT move used as PV-first ordering hint. New toggle `TranspositionTable` (default on); cleared on `ucinewgame`. ~2.2× speedup on kiwipete d6 (7520ms → 3395ms), same best move. (13 new tests.)
- **2026-05-11 — search: iterative deepening + UCI time controls** — `iterative_deepening` loops depths 1..=max with a deadline; short-circuits on mate. New `GoArgs { max_depth, time_budget_ms }` from `parse_go` handles `depth N`, `movetime ms`, `wtime/btime/winc/binc`. New toggle `IterativeDeepening` (default on). Mid-iteration abort isn't supported yet. (9 new tests.)
- **2026-05-11 — search: quiescence (toggleable)** — `qsearch` recursively follows captures at depth 0 with stand-pat. Fixes horizon effect at low depths (depth-1 score for knight-takes-defended-queen drops from +320 to 0). New toggle `Quiescence` (default on). (3 new tests.)
- **2026-05-11 — search: MVV-LVA move ordering (toggleable + `SearchConfig` pattern established)** — `mvv_lva_score` ranks captures by `victim * 10 - attacker`; `order_moves` sorts before searching. First toggle on `SearchConfig`; pattern that all subsequent search features follow. ~3.2× speedup on kiwipete d5. (5 new tests.)
- **2026-05-11 — uci: real UCI protocol loop** — `uci.rs` handles `uci`/`isready`/`ucinewgame`/`position`/`go`/`setoption`/`quit`, emits `info ... pv <mv>` + `bestmove`. `Torte::run` delegates here. Replaces the original hand-rolled REPL. (8 new tests.)
- **2026-05-11 — search + eval: negamax with alpha-beta + material eval** — `find_best_move` does negamax + alpha-beta + mate scoring; eval is material-only at this point. `MATE_SCORE = 29_000`, `INFINITY = 30_000`. Mate-in-1, stalemate, free-capture tests pass. (6 new tests.)
- **2026-05-11 — movegen: legal moves + perft** — `is_attacked`, `generate_legal_moves` (pseudo-legal + post-move king-safety filter). Castling explicit checks for rights/path-empty/king-safety. `perft` verified against startpos d1..d5 (4865609) and kiwipete d1..d3 (97862). (8 new tests.)
- **2026-05-11 — movegen: sliding-piece magic bitboards** — plain magic bitboards in `magic.rs` for rook/bishop, magics searched at startup via xorshift sparse-random. Tables behind `OnceLock`; `magic::init()` eagerly initializes at engine startup so first-move latency doesn't surprise. (6 new tests.)
- **2026-05-11 — movegen: knight/king/pawn attack tables** — `attacks.rs` with `const fn` static arrays built at compile time. Accessors `knight_attacks`, `king_attacks`, `pawn_attacks(sq, color)`. (7 new tests, first tests in the repo.)
- **2026-05-11 — board: complete `apply_move`** — infers castle/ep/double-push from src+dest+piece; handles promotion (5-char UCI like `e7e8q`); mutates `side_to_move`, `castling`, `en_passant`, `halfmove_clock`, `fullmove_number` correctly. No legality checks — that's the movegen layer's job.
- **2026-05-11 — board: FEN fields + state on `Board`** — `side_to_move`, `castling: CastlingRights` (u8 bitflags), `en_passant: Option<SQ>`, `halfmove_clock`, `fullmove_number`. `Board::parse` consumes all six FEN fields with sensible defaults for missing ones. `Debug` printer shows the state below the board.
- **2026-05-11 — board: bitboard layout flip to a1 = bit 0** — `parse` was storing FEN ranks upside-down. Now: a1=0, h8=63, white pawn push = `bb << 8`. Display printer was already iterating in reverse so it stayed correct. Cleared the way for movegen.
- **2026-05-11 — board: `move_piece` bug fix** — used the passed `Piece` argument (previously ignored), synced `player_bbs` (previously drifted), and cleared captured opponent piece on `to`.

## Roadmap (planned, in priority order)

These are the next likely toggles, drawn from the strong-engine playbook:

1. **Killer moves / history heuristic** — at each ply, remember moves that caused beta cutoffs and try them early. Compounds with MVV-LVA. ~30 LOC.
2. **Endgame king PST + tapered eval** — phase-detect on material and lerp between MG and EG king tables. Without this the king never centralizes in endgames.
3. **Null-move pruning** — skip a turn, search shallower; if score still ≥ beta, prune. Substantial speedup, but zugzwang/endgame guards needed.
4. **UCI `Hash` spin option** — let GUIs configure TT size (default 16 MB is fine, but tournaments often request 128/256 MB).
5. **Incremental Zobrist hashing in `apply_move`** — avoid re-hashing the whole board each node. Probably ~20% speedup overall.
6. **Mid-search abort** — let `stop` and time deadline interrupt within an iteration (atomic flag checked at every node).
7. **Aspiration windows** — narrow alpha/beta windows around the previous ID iteration's score; fall back on fail-high/low. Combines well with TT.

Then: Lazy SMP multithreading (needs a concurrent TT — the real work; ~1.5–1.8× on 4 cores), late move pruning (LMP), MultiPV.
