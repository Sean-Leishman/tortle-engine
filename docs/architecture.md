# Architecture

The engine is in a very early state — only board representation and a stdin move loop exist. This document describes what is there, and flags the standard chess-engine pieces that are not yet implemented.

## High-level shape

```
main.rs
  └── Torte (torte/torte.rs)
        └── Board (torte/board/board.rs)
              ├── bbs: [Bitboard; 12]      // one per (colour, piece-type)
              ├── player_bbs: [Bitboard; 2] // aggregate per colour
              ├── side_to_move: Color
              ├── castling: CastlingRights  // bitflags KQkq
              ├── en_passant: Option<SQ>
              ├── halfmove_clock: u16
              └── fullmove_number: u16
```

`main` constructs a `Torte`, which owns a `Board`, then calls `Torte::run`. `run` parses the start-position FEN, prints the board, then loops reading UCI-style move strings from stdin and applying them.

## Board representation

Bitboard-based. Each of the 12 (colour, piece) combinations gets its own `u64`, plus two aggregate-by-colour boards.

- `Bitboard` (`core/bitboard.rs`) is a `Copy` newtype wrapping `u64` with `set` / `clear` / `get` by bit index, `pop` (clear lsb, return its index) which also makes it an `Iterator<Item = usize>`, and `count` / `get_lsb` / `get_msb`.
- Operator overloads carry set-theoretic meaning: `&` and `*` both mean intersection, `|` and `+` both mean union, `^` is xor, `-` is set difference (`a & !b`), `!` is complement, `<<` / `>>` shift by `usize`. Reading `a + b - c` as "union, then minus" is intentional.
- Square indices (`SQ`) are `0..64` with `64` reserved as `NONE`. `SQ::make(rank, file) = rank * 8 + file`, so a1 = 0, h1 = 7, a8 = 56, h8 = 63 (rank-major, files 0..7 = a..h).

Indexing convention for `bbs`:

| index | piece          |
| ----- | -------------- |
| 0..5  | white P/N/B/R/Q/K |
| 6..11 | black P/N/B/R/Q/K |

This is set in `Piece::to_index` / `from_index` in `board/pieces.rs` and used everywhere.

## FEN parsing

`Board::parse` (in `board/board.rs`) handles the full FEN: the piece-placement field is walked rank-by-rank into `bbs` and `player_bbs`, then `side_to_move`, `castling`, `en_passant`, `halfmove_clock`, and `fullmove_number` are filled from `parts[1..6]`. Missing or unparseable trailing fields fall back to defaults (white to move, no castling rights, no ep square, halfmove 0, fullmove 1).

`parse` consumes the FEN ranks top-to-bottom (rank 8 first) but maps each FEN rank `fen_rank` to board rank `7 - fen_rank` before setting bits. So a1 sits at bit 0, h1 at bit 7, a8 at bit 56, h8 at bit 63 — the conventional "a1 = bit 0" layout, consistent with `SQ::make(rank, file) = rank * 8 + file` where `rank = uci_char - '1'`.

## Move application

`Move` (`core/piece_move.rs`) is `{ src: SQ, dest: SQ, promotion: Option<PromotionPiece> }`. `Move::from_uci` accepts the 4-char form (`e2e4`) and the 5-char promotion form (`e7e8q`); promotion suffix is one of `n`/`b`/`r`/`q`. It still panics on any other length or unknown promotion char.

`Board::apply_uci_move` -> `apply_move`. `apply_move` is the single entry point that does all state mutation. It identifies the moving piece via `piece_at_sq`, then infers the move's nature from `(piece, src, dest, en_passant)`:

- Castling: `is_king && |file_delta| == 2`. The matching rook is moved h↔f (kingside) or a↔d (queenside).
- En-passant capture: `is_pawn && file_delta != 0 && self.en_passant == Some(dest)`. The captured pawn sits one rank behind `dest` (relative to the mover) and is cleared.
- Double pawn push: `is_pawn && |rank_delta| == 2`. Sets `en_passant` to the square between src and dest. Otherwise `en_passant` is cleared.
- Promotion: pawn move to last rank requires a `Move::promotion` value; missing or stray suffixes return `InvalidInput`. The pawn bit is cleared and the promoted piece's bit is set on `dest`.
- Capture (any kind): clears the captured piece's bit and the opponent's `player_bbs` entry; halfmove clock resets.

Castling rights are then masked: a king move strips both rights for that color; a rook move *from* or capture *on* a1/h1/a8/h8 strips the corresponding right. The halfmove clock resets on captures or pawn moves and increments otherwise; `fullmove_number` increments after black's move; `side_to_move` flips at the end.

`apply_move` does **not** check legality or whose turn it is — it executes whatever move it's handed.

## Move generation

Partial. Two layers of attack tables exist in `torte/movegen/`:

**Non-sliding** (`attacks.rs`) — `static` arrays built at compile time via `const fn`:

- `KNIGHT_ATTACKS: [Bitboard; 64]`, `KING_ATTACKS: [Bitboard; 64]`
- `WHITE_PAWN_ATTACKS: [Bitboard; 64]` / `BLACK_PAWN_ATTACKS: [Bitboard; 64]` (capture squares only — pawn pushes are computed via shifts elsewhere)

Accessors: `knight_attacks(SQ)`, `king_attacks(SQ)`, `pawn_attacks(SQ, Color)`.

**Sliding** (`magic.rs`) — plain magic bitboards for rook/bishop, with magics found at startup by random search (xorshift RNG, sparse-magic candidates):

- `rook_attacks(SQ, occupancy: Bitboard) -> Bitboard`
- `bishop_attacks(SQ, occupancy: Bitboard) -> Bitboard`
- `queen_attacks(SQ, occupancy)` = rook | bishop

The 64 rook + 64 bishop tables (each indexed by `(occ & mask) * magic >> shift`) live behind a `OnceLock<SlidingTables>` and initialize on first call. `magic::init()` forces this eagerly; `Torte::run` calls it before the REPL starts so the latency (~0.5–1s release, ~15s debug) doesn't surprise you mid-game.

**Move generation** (`generator.rs`):

- `is_attacked(&Board, SQ, Color) -> bool` — true if `by` attacks `sq` in the given position. Used for check detection and castling legality (combines pawn/knight/king tables with sliding attacks).
- `generate_legal_moves(&Board) -> Vec<Move>` — generates pseudo-legal moves (pawn pushes/captures/ep/promotions, knight/king/sliders, castling) then filters by post-move king safety. Castling has its own gen-time checks (rights present, path empty, king not in/through/into check).

**Perft** (`perft.rs`):

- `perft(&Board, depth) -> u64` and `perft_divide(&Board, depth) -> Vec<(Move, u64)>`.
- Verified against published values: startpos d1..d5 (20/400/8902/197281/4865609) and kiwipete d1..d3 (48/2039/97862). Kiwipete hits castling, ep, promotions, and discovered checks.

## Search

Negamax + alpha-beta in `torte/search/search.rs`:

- `find_best_move(&Board, depth) -> Option<(Move, score)>` — top-level entry with default config; returns `None` only if there are no legal moves.
- `find_best_move_with(&Board, depth, SearchConfig) -> ...` — explicit-config variant.
- Internal `negamax(board, depth, alpha, beta, ply, config)` — scores from side-to-move's perspective. At depth 0 returns `eval(board)`. With no legal moves, returns `-MATE_SCORE + ply` if in check (mate; faster mates score higher because larger `ply` is closer to 0), or `0` if stalemate.
- Constants: `INFINITY = 30_000`, `MATE_SCORE = 29_000`.

### `SearchConfig` — toggleable features

Each search-strengthening feature is a boolean field on `SearchConfig`. The pattern: add a field, add a UCI option name in `uci::emit_options` and a match arm in `apply_setoption`, then dispatch on the field inside the search. This makes A/B comparison trivial and keeps regressions easy to bisect.

Currently:

- `move_ordering: bool` (default `true`) — when on, moves are sorted by MVV-LVA before being searched. Captures of high-value victims by low-value attackers come first; quiet moves last. Score is `victim_value * 10 - attacker_value` (king attacker = 0). Toggle via `setoption name MoveOrdering value <true|false>`.
  - Empirical speedup on kiwipete depth-5: 1662ms → 516ms (~3.2×). Identical best score, identical PV.
- `quiescence: bool` (default `true`) — at the leaves of the main search (depth 0), instead of returning the static eval directly, run `qsearch`: a recursive search that follows only captures (filtered by `mvv_lva_score > 0`), with the static eval as a "stand pat" lower bound. Fixes the horizon effect — at depth 1, a position where white can capture a defended queen no longer scores `+320` (the immediate gain) but `0` (the gain after the forced recapture). Toggle via `setoption name Quiescence value <true|false>`.
  - Skips check-evasion handling (qsearch stand-pats even when in check) and skips promotion-only moves (no capture component). Both are standard refinements deferred for later.
- `iterative_deepening: bool` (default `true`) — instead of jumping straight to `max_depth`, search at depths 1, 2, ..., max_depth in sequence. Each iteration's result is reported via `info depth N ... pv <move>`. Short-circuits on mate detection (`|score| >= MATE_SCORE - 1000`). Aborts before any iteration that would start past the deadline (between-iteration check; mid-search abort isn't supported). Toggle via `setoption name IterativeDeepening value <true|false>`.
  - Combined with TT below, the previous iterations are no longer redundant — each ID iteration's PV is reused via TT-move-first ordering at every node.
- `transposition_table: bool` (default `true`) — caches `(zobrist_hash, depth, score, bound, best_move)` per position in a fixed-size hash table. At each node, probes for a usable hit (sufficient depth + bound consistent with the alpha/beta window) and short-circuits if so. The TT move is always used as the move-ordering hint, even on insufficient-depth hits. Mate scores are stored as distance-from-this-node and restored to distance-from-root on probe via `store_mate_score` / `retrieve_mate_score`. Toggle via `setoption name TranspositionTable value <true|false>`.
  - Default size 16 MB (rounded down to a power-of-two number of `Option<TTEntry>` slots). Cleared on `ucinewgame`. Hash is computed from scratch each node (no incremental update yet).
  - Empirical speedup on kiwipete depth-6: 7520ms → 3395ms (~2.2×). Identical best score and PV. Saving compounds with ID — earlier iterations populate the TT so deeper iterations can reuse cached subtrees and PV moves.
- `piece_square_tables: bool` (default `true`) — adds per-square positional bonuses (tapered MG/EG PSTs) plus mobility on top of material in `eval`. Toggle via `setoption name PieceSquareTables value <true|false>`.
  - Qualitative impact: with PST on, the engine plays `b1c3` from startpos (knight development); with PST off, it plays `a2a3` (the first move enumerated, no positional reason to prefer anything).
- `pawn_structure: bool` (default `true`) — adds doubled (−15 cp per extra pawn on a file), isolated (−15 cp), and passed-pawn terms to `eval`. The passed-pawn bonus is rank-scaled and phase-scaled (~2× in a pure pawn endgame). Toggle via `setoption name PawnStructure value <true|false>`.

Not implemented yet (each will get its own toggle): killers/history, null-move pruning, late-move reductions, incremental Zobrist hashing, TT size as a UCI `Hash` spin option, endgame king PST + tapered eval.

## Evaluation

`torte/search/eval.rs` — `eval(&Board, use_pst: bool, use_pawn_structure: bool) -> i32` returns a centipawn score from the side-to-move's perspective.

- **Material**: P=100, N=320, B=330, R=500, Q=900, K=0. Exposed as `PIECE_VALUES` for the MVV-LVA code.
- **Piece-square tables** (when `use_pst` is true): per-piece 64-entry `i32` arrays from white's perspective, indexed so `PST[0] = a1`. Black pieces look up `PST[sq ^ 56]` to mirror the rank. Tables are tapered: separate MG/EG arrays for each piece, lerped by game phase. Mobility (per-piece move counts, weighted) is added in the same `use_pst` branch. No king-safety pawn-shield yet.
- **Pawn structure** (when `use_pawn_structure` is true): doubled (penalty per extra pawn on a file), isolated (no friendly pawn on an adjacent file), and passed (no enemy pawn on the same/adjacent file ahead). The passed-pawn bonus is rank-scaled and phase-scaled to ~2× in a pure pawn endgame.

The toggles live on `SearchConfig` as `piece_square_tables` and `pawn_structure` (both default `true`). `eval` itself takes plain `bool`s to avoid pulling `SearchConfig` into the eval module — callers (`negamax`, `qsearch`) forward `config.piece_square_tables` and `config.pawn_structure`.

## REPL search command

`Torte::run` accepts `go <depth>` in addition to UCI moves and `exit`. It calls `find_best_move`, prints `bestmove <uci> score <i32>`, and applies the chosen move so engine self-play is possible.

## I/O

The engine speaks UCI over stdin/stdout via `torte/uci.rs`. `Torte::run` initializes the magic tables, sets the start position, then delegates to `uci::run(&mut Board)`. Supported UCI commands:

- `uci` — emits `id name torte`, `id author <name>`, `uciok`.
- `isready` — emits `readyok`.
- `ucinewgame` — resets to startpos.
- `position [startpos | fen <fen>] [moves m1 m2 ...]` — sets the position and applies any trailing moves.
- `go [depth N | movetime ms | wtime W btime B [winc Wi] [binc Bi] | infinite]` — runs the search and emits one or more `info depth N score cp/mate X time T pv <mv>` lines (one per ID iteration when `IterativeDeepening` is on) followed by `bestmove <uci>`.
  - `depth N` searches at most depth N.
  - `movetime ms` searches up to `MAX_DEPTH_TIMED = 64` with a deadline `ms` from now.
  - `wtime/btime/winc/binc` allocate a budget per the side to move: `time/30 + inc/2` ms — a crude time-management heuristic.
  - With no arguments (or `infinite`), falls back to `DEFAULT_DEPTH = 6`. Deadlines are checked between iterations; mid-iteration abort isn't supported.
- `setoption name <name> value <value>` — sets a search-feature toggle (currently `MoveOrdering`). Unknown names emit `info string unknown option: <name>`. Accepted boolean values: `true`/`false`/`on`/`off`/`yes`/`no`/`1`/`0`.
- `stop`, `ponderhit`, `debug`, `register` — accepted and silently ignored.
- `quit` / `exit` — clean exit. EOF on stdin also exits.

REPL conveniences (not part of UCI but handy for human use through the same loop):

- `d` or `board` — pretty-prints the current board via the `Debug` impl.
- A bare UCI move like `e2e4` applies it directly to the current position. Unknown commands or illegal moves emit `info string unknown command or illegal move: ...`.
- `go N` (plain integer) is treated as `go depth N`.

Score formatting follows UCI: scores within `MATE_SCORE - 1000` of `MATE_SCORE` are reported as `mate <signed-moves>` (rounded up, sign indicates whether we mate or are mated); everything else is `cp <centipawns>`.
