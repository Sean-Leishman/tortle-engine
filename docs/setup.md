# Setup

## Prerequisites

- Rust toolchain (edition 2021). Any reasonably recent stable `rustc` / `cargo` should work — the project has zero dependencies, so there is no MSRV pressure beyond what edition 2021 needs.

Install via [rustup](https://rustup.rs/) if needed:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Clone

```
git clone git@github.com:Sean-Leishman/tortle-engine.git chess
cd chess
```

The local working copy on this machine lives at `/home/seanleishman/personal/chess`.

## Build

```
cargo build              # debug build -> target/debug/torte
cargo build --release    # optimized -> target/release/torte
```

The binary is named `torte`, not `chess`, because of the `[[bin]]` block in `Cargo.toml`.

## Run

```
cargo run
```

Expected output: the string `Running Torte`, then a Debug-printed 8x8 board with Unicode pieces in the start position, then a `Enter move: ` prompt.

The loop speaks UCI. The minimal handshake:

```
uci
isready
position startpos moves e2e4 e7e5
go depth 4
quit
```

Output for `go` looks like `info depth 4 score cp -100 time 18 pv a2a3` followed by `bestmove a2a3`. Mate scores are reported as `score mate <signed-moves>`.

Toggle search features via UCI `setoption`. Currently available:

```
setoption name MoveOrdering value false       # disable MVV-LVA captures-first ordering
setoption name Quiescence value false         # disable quiescence search at leaves
setoption name IterativeDeepening value false # search directly at max depth (no per-depth info)
setoption name TranspositionTable value false # disable TT probing/storing (16 MB table is still allocated)
setoption name PieceSquareTables value false  # material-only eval (no positional bonuses)
```

Each feature is on by default; turning them off is mainly useful for A/B comparison and debugging. The `d` command prints the current `SearchConfig` so you can see what's active.

REPL conveniences (not part of UCI but handy for human use):

- `d` or `board` — pretty-prints the position.
- A bare UCI move like `e2e4` applies it directly.
- `go 4` (plain integer) is treated as `go depth 4`.
- `exit` is accepted as an alias for `quit`.

`go` supports `depth N`, `movetime ms`, and `wtime W btime B [winc Wi] [binc Bi]`. The time arguments allocate a budget of `time/30 + inc/2` ms for the side to move. Deadlines are checked between ID iterations; once an iteration starts it always runs to completion.

Caveats while playing with it:

- 4-char moves and 5-char promotions (`e7e8q`, with `n`/`b`/`r`/`q`) both work. Anything else panics.
- No legality checking — you can move any piece anywhere, ignore whose turn it is, and move onto your own pieces (which silently corrupts the bitboards). Castling and en-passant fire on shape alone, not on whether the rights actually exist.
- Castling, en passant, and promotion are handled. `side_to_move`, castling rights, ep square, halfmove clock, and fullmove number all mutate after each move.

## Tests

Tests cover attack tables, magic bitboards, and movegen via perft. Run with:

```
cargo test --release            # ~2s, includes startpos d1..d4 and kiwipete d1..d2
cargo test --release -- --ignored  # adds startpos d5 (4.9M nodes) and kiwipete d3 (97K nodes)
```

69 tests total (attack tables, magic bitboards, perft, eval incl. PST mirroring + center vs corner, search incl. MVV-LVA + ordering + quiescence + iterative deepening + TT A/B, mate-score round-trip, Zobrist hash invariants, UCI parsing + setoption + time-management). Debug-mode `cargo test` works but is slow (~15s for the magic search alone, plus several seconds per perft test).

## Formatting / linting

No `rustfmt.toml` or `clippy.toml` in the tree, so defaults apply. `cargo fmt` and `cargo clippy` are both reasonable to run before committing; the codebase is small enough that they should stay clean.

## Editor

Anything with rust-analyzer is fine. There is no `.vscode/`, `.idea/`, or other editor config checked in.
