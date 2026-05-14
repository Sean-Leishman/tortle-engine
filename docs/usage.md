# Usage

The engine binary is `torte`. It speaks UCI on stdin/stdout, plus a few REPL
conveniences for driving it by hand.

## Running

```
cargo run --release      # release binary; recommended for actual play
cargo run                # debug; the startup magic-bitboard search takes ~15s
```

On start it prints `Running Torte`, a Debug-printed start position, and an
`Enter move:` prompt.

## UCI handshake

```
uci                                       # id name/author + option lines + uciok
isready                                   # readyok
ucinewgame                                # clears the transposition table
position startpos moves e2e4 e7e5
go depth 6                                # streams `info depth N ... pv <mv>`, then `bestmove <uci>`
quit
```

## The `go` command

| Form | Behaviour |
|---|---|
| `go depth N` | Search to fixed depth `N`. |
| `go movetime MS` | Search ~`MS` ms. Deadline checked between iterative-deepening iterations. |
| `go wtime W btime B winc Wi binc Bi` | Time-control budget: `time/30 + inc/2` ms for the side to move. |
| `go N` (bare integer) | REPL shortcut — treated as `go depth N`. |

Once an ID iteration starts it runs to completion; there is no mid-iteration
abort yet. `stop` is currently a no-op (the search is synchronous).

Scores: centipawns as `score cp N`; forced mates as `score mate N` where `N`
is signed moves-to-mate (positive = engine mates).

## Search-feature toggles

Every search/eval refinement is behind a `setoption` toggle, default `true`.
Turning one off is for A/B measurement and regression debugging.

```
setoption name MoveOrdering value false        # MVV-LVA captures-first ordering
setoption name Quiescence value false          # capture search at leaves
setoption name IterativeDeepening value false  # search straight at max depth
setoption name TranspositionTable value false  # TT probing/storing (table still allocated)
setoption name PieceSquareTables value false   # material-only eval
setoption name PawnStructure value false       # doubled/isolated/passed-pawn terms
setoption name BishopPair value false          # +30 cp for the pair
setoption name KingSafety value false          # castled-king pawn-shield bonus
```

See `architecture.md` for what each toggle measures and `engine-internals.md`
for the algorithms behind them.

## REPL conveniences

Not part of UCI, but handy for human use:

- `d` or `board` — pretty-prints the position and the current `SearchConfig`.
- A bare UCI move (`e2e4`) — applied directly to the board.
- `exit` — alias for `quit`.

## Caveats while playing by hand

- 4-char moves and 5-char promotions (`e7e8q`) both work. **Anything else
  panics** — `Move::from_uci` is not robust to malformed input.
- There is **no legality checking** on hand-entered moves: you can move any
  piece anywhere, ignore whose turn it is, or move onto your own pieces
  (which silently corrupts the bitboards). Castling and en-passant fire on
  move *shape* alone. Legality is the movegen layer's job — it only applies
  to moves the engine itself generates.

## Tests and perft

```
cargo test --release                  # ~3s; perft startpos d1..d4, kiwipete d1..d2
cargo test --release -- --ignored      # adds startpos d5 (4.9M nodes) + kiwipete d3
```

Perft is verified against published node counts — startpos through depth 5,
kiwipete through depth 3 — which is the correctness gate for move generation.
116 tests total. Debug-mode `cargo test` works but is slow (~15s just for the
startup magic search).
