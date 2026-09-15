#!/usr/bin/env bash
# SPRT: is <new> stronger than <base>? Plays until the log-likelihood ratio
# crosses a bound, so a clear change stops early and a marginal one keeps going.
# This is the "is feature X worth keeping?" tool; run-ladder.sh is the "how
# strong is torte?" tool.
#
# Usage:
#   cp ../target/release/torte engines/torte-base     # before the change
#   ... make the change, cargo build --release ...
#   ./sprt.sh ../target/release/torte engines/torte-base
#
# Env knobs:
#   ELO0, ELO1   nElo bounds: H0 = new is no better than ELO0, H1 = at least ELO1 (default 0, 10)
#   TC           time control (default 10+0.1)
#   CONCURRENCY  parallel games (default: nproc)
#   FORCE=1      run even when the machine is already busy
#
# Result: "H1 accepted" = keep it, "H0 accepted" = it doesn't gain ELO1.

set -euo pipefail
cd "$(dirname "$0")"

NEW="${1:?usage: sprt.sh <new-binary> <base-binary>}"
BASE="${2:?usage: sprt.sh <new-binary> <base-binary>}"
ELO0="${ELO0:-0}"
ELO1="${ELO1:-10}"
TC="${TC:-10+0.1}"
CONCURRENCY="${CONCURRENCY:-$(nproc)}"

for f in "$NEW" "$BASE" ./fastchess books/noob_3moves.epd; do
  [[ -e "$f" ]] || { echo "missing: $f  (see README.md)" >&2; exit 1; }
done

# At 10+0.1 a loaded machine turns into time forfeits, not a strength signal.
load=$(cut -d' ' -f1 /proc/loadavg)
if [[ "${FORCE:-0}" != 1 ]] && awk -v l="$load" -v n="$(nproc)" 'BEGIN { exit !(l > n / 4) }'; then
  echo "load average $load on $(nproc) cores — games would be time-starved. FORCE=1 to run anyway." >&2
  exit 1
fi

mkdir -p results
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="results/sprt-$STAMP.log"
echo "sprt: $NEW vs $BASE  |  nElo [$ELO0, $ELO1]  TC=$TC  concurrency=$CONCURRENCY  log -> $LOG"

./fastchess \
  -engine name=new cmd="$NEW" \
  -engine name=base cmd="$BASE" \
  -each tc="$TC" \
  -openings file=books/noob_3moves.epd format=epd order=random \
  -rounds 20000 -games 2 -repeat \
  -concurrency "$CONCURRENCY" \
  -recover \
  -sprt elo0="$ELO0" elo1="$ELO1" alpha=0.05 beta=0.05 model=normalized \
  -draw movenumber=40 movecount=8 score=10 \
  -resign movecount=4 score=600 \
  -ratinginterval 50 \
  -pgnout file="results/sprt-$STAMP.pgn" \
  2>&1 | tee "$LOG"

echo "time forfeits: $(grep -ciE 'loses on time' "$LOG" || true)"
