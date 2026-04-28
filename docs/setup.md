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

Type moves in plain UCI, e.g.:

```
e2e4
g1f3
```

Type `exit` to quit.

Caveats while playing with it:

- Only 4-char moves work. Promotions like `e7e8q` will panic in `Move::from_uci`.
- No legality checking — you can move any piece anywhere, including on top of your own pieces. Captures don't actually remove the captured piece from its bitboard.
- `player_bbs` desyncs after the first move. If you start writing logic that relies on it, be aware.

## Tests

There are no tests in the repo. `cargo test` will compile and report `0 passed`.

## Formatting / linting

No `rustfmt.toml` or `clippy.toml` in the tree, so defaults apply. `cargo fmt` and `cargo clippy` are both reasonable to run before committing; the codebase is small enough that they should stay clean.

## Editor

Anything with rust-analyzer is fine. There is no `.vscode/`, `.idea/`, or other editor config checked in.
