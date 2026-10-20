#!/usr/bin/env python3
"""Certify a match PGN was not biased by machine starvation.

Forfeit count is the usual check and it is necessary but not sufficient: a
starved machine can distort a match without ever blowing a clock (2026-10-18,
where swap filled mid-run, the engines dropped to 28% CPU and 800 games still
recorded zero forfeits).

Starvation does not change move *times* — the budget is wall-clock — it
collapses search *depth*. So the fingerprint is in the PGN. The number to look
at is the **within-game depth differential**: both engines play the same game
from the same position, so a depth gap between them is attributable to the
engines, not to the position.

Its *magnitude* is not the test, and reading it that way raises false alarms.
A feature that searches a bigger tree per depth legitimately reaches lower
depth at a fixed clock — `LmrQuietIndex` costs 41% more nodes and runs 0.18
ply shallower, which is the trade under measurement, not a defect in the match.
What separates feature cost from machine bias is whether the differential
*moves* with machine condition: a constant offset is the feature, an offset
that grows as the machine degrades is starvation. The condition axis is **round
order**, which is a direct proxy for time and so for whatever the machine was
doing. Do not band by absolute depth for this: depth tracks game type, so
depth bands select long drawn endgames and a feature whose cost varies by phase
reads as drift that is not there (that mistake flagged a clean run as suspect).

Do NOT read absolute depth as a load proxy. Mean depth per game is dominated by
game *type* — endgames search deeper for free — so splitting on it selects long
drawn games, not quiet machines (the deep tercile of that same run was 138 plies
and 45% draws against 97 and 14%).

Usage: certify-run.py <pgn> [engine-name]
"""
import re, sys, statistics

def games(path):
    for c in re.split(r'(?=\[Event )', open(path).read()):
        if '[White ' not in c:
            continue
        wh = re.search(r'\[White "(.*?)"\]', c).group(1)
        bl = re.search(r'\[Black "(.*?)"\]', c).group(1)
        # fastchess move comments look like {+0.07/8 0.359s} -> depth 8
        rnd = re.search(r'\[Round "(\d+)"\]', c)
        ds = [int(d) for d in re.findall(r'\{[^}]*?/(\d+) \d+\.\d+s\}', c)]
        if len(ds) >= 10:
            yield (wh, bl, statistics.mean(ds[0::2]), statistics.mean(ds[1::2]),
                   int(rnd.group(1)) if rnd else 0)

def main(path, name=None):
    rows = list(games(path))
    if not rows:
        sys.exit(f"{path}: no games with depth comments")
    name = name or rows[0][0]
    diffs = [(w_d if wh == name else b_d) - (b_d if wh == name else w_d)
             for wh, bl, w_d, b_d, _ in rows]
    depths = [(w_d + b_d) / 2 for _, _, w_d, b_d, _ in rows]
    rounds = [r for *_, r in rows]
    def stat(xs):
        m = statistics.mean(xs)
        se = (statistics.stdev(xs) / len(xs) ** 0.5) if len(xs) > 1 else 0.0
        return m, 1.96 * se

    d, dci = stat(diffs)
    print(f"{path}: {len(rows)} games, mean depth {statistics.mean(depths):.2f} "
          f"(min game {min(depths):.1f}, max {max(depths):.1f})")
    print(f"depth differential for {name}: {d:+.3f} +/- {dci:.3f} (95%)")

    # Band by round order: a direct proxy for wall-clock, hence for machine
    # condition. Mean depth per band is printed as the corroborating signal —
    # if the machine degraded, depth sags where the load was.
    order = sorted(range(len(diffs)), key=lambda i: rounds[i])
    t = len(order) // 3
    bands = [("early", order[:t]), ("mid", order[t:2 * t]), ("late", order[2 * t:])]
    vals = []
    for label, idx in bands:
        m, ci = stat([diffs[i] for i in idx])
        vals.append((m, ci))
        print(f"  {label:<6} rounds {rounds[idx[0]]:>4}-{rounds[idx[-1]]:<4} n={len(idx):>4}"
              f"  depth {statistics.mean(depths[i] for i in idx):5.2f}"
              f"  differential {m:+.3f} +/- {ci:.3f}")

    (lo, lo_ci), (_, _), (hi, hi_ci) = vals
    shift = abs(hi - lo)
    tol = (lo_ci ** 2 + hi_ci ** 2) ** 0.5
    if shift <= tol:
        print(f"CLEAN — differential is constant across conditions "
              f"(shift {shift:.3f} within +/-{tol:.3f}); any offset is the feature's own cost, "
              f"not starvation. Comparison stands.")
    else:
        print(f"SUSPECT — differential shifts {shift:.3f} (tolerance +/-{tol:.3f}) from the "
              f"first third of the match to the last: one side lost more depth than the other "
              f"as conditions changed. Do not trust the Elo.")

if __name__ == "__main__":
    main(*sys.argv[1:3])
