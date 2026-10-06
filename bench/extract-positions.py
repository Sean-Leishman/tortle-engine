"""Extract labelled positions from our own games.

Why not reuse the Zurichess quiet-labeled set: it filters out positions where a
capture or check is available, which is precisely where king attacks and
threats decide games — so a king-danger term fitted on it has nothing to learn
from (2026-10-06). These games are our own distribution, attacks included.

Emitted in GAME ORDER so a contiguous tail split holds out whole games;
positions within one game are heavily correlated, and splitting by position
would leak a game's own positions across the train/held-out boundary.
"""
import sys, glob, chess, chess.pgn

SAMPLE_EVERY = 4      # plies; consecutive positions are near-duplicates
SKIP_OPENING = 8      # plies of book/near-book
SKIP_TAIL = 4         # plies; the final moves are decided, not evaluative

out = open(sys.argv[-1], "w")
games = kept = 0
seen = set()
for path in sys.argv[1:-1]:
    with open(path) as fh:
        while (g := chess.pgn.read_game(fh)):
            res = g.headers.get("Result", "*")
            if res not in ("1-0", "0-1", "1/2-1/2"):
                continue
            moves = list(g.mainline_moves())
            if len(moves) < SKIP_OPENING + SKIP_TAIL + 2:
                continue
            games += 1
            board = g.board()
            for i, mv in enumerate(moves):
                board.push(mv)
                if i < SKIP_OPENING or i >= len(moves) - SKIP_TAIL:
                    continue
                if i % SAMPLE_EVERY:
                    continue
                if board.is_check():          # resolved by the Rust pass, not here
                    pass
                fen = board.fen()
                key = " ".join(fen.split()[:4])
                if key in seen:
                    continue
                seen.add(key)
                out.write(f'{fen} c9 "{res}";\n')
                kept += 1
out.close()
print(f"{games} games -> {kept} positions (deduped), game-ordered")
