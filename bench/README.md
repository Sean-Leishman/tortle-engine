# bench — calibration ladder

Measures **how strong torte is, in Elo**, by playing it as a gauntlet against a
fixed set of open-source engines with known [CCRL](https://computerchess.org.uk/ccrl/)
ratings. (For *"is feature X worth keeping?"* — an engine-vs-itself A/B question —
use SPRT instead; not set up here yet.)

## Layout

| Path | Tracked? | What |
|------|----------|------|
| `run-ladder.sh` | yes | the runner |
| `README.md`, `.gitignore` | yes | this + ignore rules |
| `fastchess` | no | tournament manager binary |
| `engines/` | no | opponent binaries |
| `books/noob_3moves.epd` | no | opening book (150k 3-move positions) |
| `*-src/` | no | upstream source trees |
| `results/` | no | per-run PGN + log, timestamped |

Everything untracked is reproducible from the bootstrap steps below — that's why
it's gitignored rather than committed (binaries are platform-specific and large;
Stockfish alone is ~110 MB).

## Bootstrap (one-time)

Run from `bench/`. Needs `g++`, `make`, `git`, `curl`, `unzip`.

```sh
# fastchess (tournament manager)
git clone --depth 1 https://github.com/Disservin/fastchess.git fastchess-src
make -C fastchess-src -j"$(nproc)" && cp fastchess-src/fastchess ./fastchess

# opponents
mkdir -p engines
git clone --depth 1 https://github.com/rofl0r/sungorus.git sungorus-src
make -C sungorus-src && cp sungorus-src/sungorus engines/sungorus

git clone --depth 1 https://github.com/bluefeversoft/vice.git vice-src
make -C vice-src/Vice11/src && cp vice-src/Vice11/src/vice engines/vice

# Stockfish 18 — pick the build matching your CPU (bmi2 here; see release page)
curl -sSL -o sf.tar https://github.com/official-stockfish/Stockfish/releases/download/sf_18/stockfish-ubuntu-x86-64-bmi2.tar
tar xf sf.tar && cp stockfish/stockfish-ubuntu-x86-64-bmi2 engines/stockfish-bin && rm -rf sf.tar stockfish

# opening book
mkdir -p books
curl -sSL -o b.zip https://github.com/official-stockfish/books/raw/master/noob_3moves.epd.zip
unzip -o b.zip -d books && rm b.zip
```

torte itself: `cargo build --release` from the repo root.

## Running

```sh
./run-ladder.sh                 # full ladder: 100 rounds (200 games) per opponent
ROUNDS=20 ./run-ladder.sh       # quick smoke run
TC=20+0.2 ./run-ladder.sh       # gentler time control
OPP=sungorus ./run-ladder.sh    # single opponent
```

fastchess prints a results table with the **Elo difference** of torte vs each
opponent (with error bars). PGN and full log land in `results/`.

## Reading the result

**Last measured (2026-09-07):** vs Sungorus 1.4, 50 games at 10+0.1 —
**Elo −478 ± 219** (1W/45L/4D), no forfeits. Puts torte around **1500**. See
`docs/log.md` for the follow-up A/B that ruled out unsound pruning.

torte's estimated Elo ≈ **opponent's CCRL Elo + torte's reported diff**, averaged
across opponents whose diff is small (a ±400 blowout barely constrains the
estimate). Approximate CCRL Blitz anchors — **treat as soft**, verify against the
current CCRL list:

| Opponent | ~CCRL Blitz |
|----------|-------------|
| Sungorus 1.4 | ~2000 |
| Vice 1.1 | ~1800 |
| SF18 @ UCI_Elo 1500 / 1800 / 2100 | self-reported; poorly calibrated at the low end — use as a *relative* bracket, not an anchor |

## Caveats

- **Time losses.** No longer a concern: mid-search abort landed in 823b447, and
  a 50-game run at 10+0.1 on 2026-09-07 recorded *zero* time forfeits. Still
  worth grepping the log for `time forfeit` / `loses on time` after a run at a
  faster TC than 10+0.1.
- SF's `UCI_Elo` is convenient but its low-end calibration is widely considered
  unreliable — lean on Sungorus/Vice for the actual anchor, SF for bracketing.
- One opening book, `order=random` — fine for absolute strength. Same opening is
  played once from each side (`-games 2 -repeat`) to cancel book bias.
- Adjudication is on (`-draw`, `-resign`) to skip dead games; loosen it in the
  script if you suspect it's mis-calling positions torte would actually win.
