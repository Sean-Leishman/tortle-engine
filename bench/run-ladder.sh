#!/usr/bin/env bash
# Calibration ladder: run `torte` as a gauntlet against a fixed set of
# open-source opponents and report Elo difference per opponent.
#
# torte plays every opponent; opponents do not play each other (gauntlet mode).
# Take each opponent's published CCRL Elo, add the reported diff -> torte's Elo.
# See README.md for the anchor numbers and caveats.
#
# Usage:
#   ./run-ladder.sh                  # full ladder, defaults below
#   ROUNDS=20 ./run-ladder.sh        # quick smoke run (40 games/opponent)
#   TC=20+0.2 ./run-ladder.sh        # gentler time control
#   OPP=sungorus ./run-ladder.sh     # single opponent (sungorus|vice|sf1500|sf1800|sf2100)
#
# Env knobs:
#   ROUNDS       rounds per opponent; each round = 2 games, colours reversed (default 100)
#   TC           time control, fastchess syntax sec+inc (default 10+0.1)
#   CONCURRENCY  parallel games (default: nproc)
#   OPP          restrict to one opponent by key (default: all)

set -euo pipefail
cd "$(dirname "$0")"

ROUNDS="${ROUNDS:-100}"
TC="${TC:-10+0.1}"
CONCURRENCY="${CONCURRENCY:-$(nproc)}"
OPP="${OPP:-all}"

TORTE="../target/release/torte"
FASTCHESS="./fastchess"
BOOK="books/noob_3moves.epd"

for f in "$TORTE" "$FASTCHESS" "$BOOK"; do
  [[ -e "$f" ]] || { echo "missing: $f  (see README.md)" >&2; exit 1; }
done

# Opponent gauntlet. Each entry: key|name|fastchess -engine args
declare -A OPPONENTS=(
  [sungorus]="Sungorus-1.4|cmd=engines/sungorus"
  [vice]="Vice-1.1|cmd=engines/vice"
  [sf1500]="SF18-1500|cmd=engines/stockfish-bin option.UCI_LimitStrength=true option.UCI_Elo=1500"
  [sf1800]="SF18-1800|cmd=engines/stockfish-bin option.UCI_LimitStrength=true option.UCI_Elo=1800"
  [sf2100]="SF18-2100|cmd=engines/stockfish-bin option.UCI_LimitStrength=true option.UCI_Elo=2100"
)

ENGINE_ARGS=()
for key in sungorus vice sf1500 sf1800 sf2100; do
  [[ "$OPP" == "all" || "$OPP" == "$key" ]] || continue
  IFS='|' read -r name args <<< "${OPPONENTS[$key]}"
  ENGINE_ARGS+=( -engine name="$name" $args )
done
[[ ${#ENGINE_ARGS[@]} -gt 0 ]] || { echo "no opponent matched OPP=$OPP" >&2; exit 1; }

# Only the selected opponents need to exist -- OPP=sungorus shouldn't demand a
# Stockfish download.
for arg in "${ENGINE_ARGS[@]}"; do
  [[ "$arg" == cmd=* ]] || continue
  [[ -e "${arg#cmd=}" ]] || { echo "missing: ${arg#cmd=}  (see README.md)" >&2; exit 1; }
done

mkdir -p results
STAMP="$(date +%Y%m%d-%H%M%S)"
PGN="results/ladder-$STAMP.pgn"
LOG="results/ladder-$STAMP.log"

echo "ladder: torte vs ${OPP}  |  TC=$TC  rounds=$ROUNDS  concurrency=$CONCURRENCY"
echo "pgn -> $PGN"
echo "log -> $LOG"
echo

# torte is the gauntlet engine: listed first, plays everyone.
"$FASTCHESS" \
  -engine name=torte cmd="$TORTE" \
  "${ENGINE_ARGS[@]}" \
  -each tc="$TC" \
  -openings file="$BOOK" format=epd order=random \
  -rounds "$ROUNDS" -games 2 -repeat \
  -tournament gauntlet \
  -concurrency "$CONCURRENCY" \
  -recover \
  -draw movenumber=40 movecount=8 score=10 \
  -resign movecount=4 score=600 \
  -ratinginterval 20 \
  -pgnout file="$PGN" \
  2>&1 | tee "$LOG"

echo
echo "done. Elo diffs are in the table above (torte vs each opponent)."
echo "torte Elo estimate = opponent's CCRL Elo + (torte's reported diff). See README.md."
