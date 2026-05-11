use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::piece_move::Move;
use crate::torte::movegen::generator::{generate_legal_moves, is_attacked, king_square};
use crate::torte::search::eval::{eval, PIECE_VALUES};
use crate::torte::search::transposition::{
    zobrist_hash, Bound, TTEntry, TranspositionTable,
};
use std::time::{Duration, Instant};

pub const INFINITY: i32 = 30_000;
pub const MATE_SCORE: i32 = 29_000;

/// Maximum search depth supported for ply-indexed state (killers). The search
/// itself can go deeper; ply-state updates are skipped past this bound.
pub const MAX_PLY: usize = 64;

/// Per-ply killer-move slots. Two slots per ply, most recent first.
pub type KillerTable = [[Option<Move>; 2]; MAX_PLY];

pub fn new_killers() -> KillerTable {
    [[None; 2]; MAX_PLY]
}

/// Toggleable search features. New features (quiescence, iterative deepening,
/// transposition table, ...) get added as fields here so each can be turned on
/// or off independently — useful for measuring impact and for debugging
/// regressions.
#[derive(Clone, Copy, Debug)]
pub struct SearchConfig {
    pub move_ordering: bool,
    pub quiescence: bool,
    pub iterative_deepening: bool,
    pub transposition_table: bool,
    pub piece_square_tables: bool,
    pub killer_moves: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            move_ordering: true,
            quiescence: true,
            iterative_deepening: true,
            transposition_table: true,
            piece_square_tables: true,
            killer_moves: true,
        }
    }
}

pub fn find_best_move(board: &Board, depth: u32) -> Option<(Move, i32)> {
    find_best_move_with(board, depth, SearchConfig::default())
}

pub fn find_best_move_with(
    board: &Board,
    depth: u32,
    config: SearchConfig,
) -> Option<(Move, i32)> {
    let mut tt = TranspositionTable::default_size();
    let mut killers = new_killers();
    find_best_move_with_tt(board, depth, config, &mut tt, &mut killers)
}

/// Search progressively at depths 1..=max_depth. Allocates a fresh TT;
/// for repeated calls (e.g. UCI), prefer `iterative_deepening_with_tt`
/// to reuse the table across iterations.
pub fn iterative_deepening<F>(
    board: &Board,
    max_depth: u32,
    config: SearchConfig,
    deadline: Option<Instant>,
    on_iteration: F,
) -> Option<(Move, i32)>
where
    F: FnMut(u32, Move, i32, Duration),
{
    let mut tt = TranspositionTable::default_size();
    iterative_deepening_with_tt(board, max_depth, config, deadline, &mut tt, on_iteration)
}

pub fn iterative_deepening_with_tt<F>(
    board: &Board,
    max_depth: u32,
    config: SearchConfig,
    deadline: Option<Instant>,
    tt: &mut TranspositionTable,
    mut on_iteration: F,
) -> Option<(Move, i32)>
where
    F: FnMut(u32, Move, i32, Duration),
{
    let start = Instant::now();
    let mut last_best: Option<(Move, i32)> = None;
    // Killers persist across ID iterations: a quiet move that caused a cutoff
    // at depth N-1 is still a promising candidate at depth N.
    let mut killers = new_killers();

    for d in 1..=max_depth {
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                break;
            }
        }

        match find_best_move_with_tt(board, d, config, tt, &mut killers) {
            Some((mv, score)) => {
                on_iteration(d, mv, score, start.elapsed());
                last_best = Some((mv, score));
                if score.abs() >= MATE_SCORE - 1000 {
                    break;
                }
            }
            None => return last_best,
        }
    }

    last_best
}

pub fn find_best_move_with_tt(
    board: &Board,
    depth: u32,
    config: SearchConfig,
    tt: &mut TranspositionTable,
    killers: &mut KillerTable,
) -> Option<(Move, i32)> {
    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return None;
    }

    // At the root we don't return early on a TT hit (we need the best move),
    // but we do use the stored move as the ordering hint.
    let key = zobrist_hash(board);
    let tt_move = if config.transposition_table {
        tt.probe(key).and_then(|e| e.best_move)
    } else {
        None
    };

    if config.move_ordering {
        order_moves(board, &mut moves, tt_move, killer_slice(killers, 0, config));
    }

    let depth = depth.max(1);
    let mut best_move = moves[0];
    let mut best_score = -INFINITY;

    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let score = -negamax(&next, depth - 1, -INFINITY, -best_score, 1, config, tt, killers);
        if score > best_score {
            best_score = score;
            best_move = m;
        }
    }

    if config.transposition_table {
        tt.store(TTEntry {
            key,
            score: store_mate_score(best_score, 0),
            best_move: Some(best_move),
            depth: depth.min(u8::MAX as u32) as u8,
            bound: Bound::Exact,
        });
    }

    Some((best_move, best_score))
}

fn negamax(
    board: &Board,
    depth: u32,
    alpha: i32,
    beta: i32,
    ply: u32,
    config: SearchConfig,
    tt: &mut TranspositionTable,
    killers: &mut KillerTable,
) -> i32 {
    let mut alpha = alpha;
    let original_alpha = alpha;
    let key = zobrist_hash(board);

    let mut tt_move: Option<Move> = None;
    if config.transposition_table {
        if let Some(entry) = tt.probe(key) {
            tt_move = entry.best_move;
            if (entry.depth as u32) >= depth {
                let score = retrieve_mate_score(entry.score, ply);
                match entry.bound {
                    Bound::Exact => return score,
                    Bound::LowerBound if score >= beta => return score,
                    Bound::UpperBound if score <= alpha => return score,
                    _ => {}
                }
            }
        }
    }

    if depth == 0 {
        return if config.quiescence {
            qsearch(board, alpha, beta, ply, config)
        } else {
            eval(board, config.piece_square_tables)
        };
    }

    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return terminal_score(board, ply);
    }
    if config.move_ordering {
        order_moves(board, &mut moves, tt_move, killer_slice(killers, ply, config));
    }

    let mut best_move = moves[0];

    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let score = -negamax(&next, depth - 1, -beta, -alpha, ply + 1, config, tt, killers);

        if score >= beta {
            if config.killer_moves && !is_capture(board, m) && (ply as usize) < MAX_PLY {
                let slot = &mut killers[ply as usize];
                if slot[0] != Some(m) {
                    slot[1] = slot[0];
                    slot[0] = Some(m);
                }
            }
            if config.transposition_table {
                tt.store(TTEntry {
                    key,
                    score: store_mate_score(beta, ply),
                    best_move: Some(m),
                    depth: depth.min(u8::MAX as u32) as u8,
                    bound: Bound::LowerBound,
                });
            }
            return beta;
        }
        if score > alpha {
            alpha = score;
            best_move = m;
        }
    }

    if config.transposition_table {
        let bound = if alpha > original_alpha {
            Bound::Exact
        } else {
            Bound::UpperBound
        };
        tt.store(TTEntry {
            key,
            score: store_mate_score(alpha, ply),
            best_move: Some(best_move),
            depth: depth.min(u8::MAX as u32) as u8,
            bound,
        });
    }

    alpha
}

fn store_mate_score(score: i32, ply: u32) -> i32 {
    if score >= MATE_SCORE - 1000 {
        score + ply as i32
    } else if score <= -MATE_SCORE + 1000 {
        score - ply as i32
    } else {
        score
    }
}

fn retrieve_mate_score(score: i32, ply: u32) -> i32 {
    if score >= MATE_SCORE - 1000 {
        score - ply as i32
    } else if score <= -MATE_SCORE + 1000 {
        score + ply as i32
    } else {
        score
    }
}

/// Quiescence search: at leaves of the main search, only follow captures
/// (with the "stand pat" lower bound from the static eval). Fixes the
/// horizon effect — without this the engine over-values trades whose
/// recapture sits one ply past the search depth.
fn qsearch(
    board: &Board,
    alpha: i32,
    beta: i32,
    ply: u32,
    config: SearchConfig,
) -> i32 {
    let stand_pat = eval(board, config.piece_square_tables);
    if stand_pat >= beta {
        return beta;
    }
    let mut alpha = alpha;
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let mut moves = generate_legal_moves(board);
    moves.retain(|m| mvv_lva_score(board, *m) > 0);
    if config.move_ordering {
        order_moves(board, &mut moves, None, [None, None]);
    }

    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let score = -qsearch(&next, -beta, -alpha, ply + 1, config);
        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    alpha
}

fn terminal_score(board: &Board, ply: u32) -> i32 {
    let king = match king_square(board, board.side_to_move) {
        Some(k) => k,
        None => return 0,
    };
    if is_attacked(board, king, board.side_to_move.opposite()) {
        -MATE_SCORE + ply as i32
    } else {
        0
    }
}

fn order_moves(
    board: &Board,
    moves: &mut Vec<Move>,
    hint: Option<Move>,
    killers: [Option<Move>; 2],
) {
    moves.sort_by_key(|m| {
        if Some(*m) == hint {
            return -1_000_000;
        }
        let mvv = mvv_lva_score(board, *m);
        if mvv > 0 {
            return -mvv;
        }
        if Some(*m) == killers[0] || Some(*m) == killers[1] {
            return -50;
        }
        0
    });
}

fn killer_slice(killers: &KillerTable, ply: u32, config: SearchConfig) -> [Option<Move>; 2] {
    if !config.killer_moves || (ply as usize) >= MAX_PLY {
        return [None, None];
    }
    killers[ply as usize]
}

pub fn is_capture(board: &Board, mv: Move) -> bool {
    mvv_lva_score(board, mv) > 0
}

/// MVV-LVA: Most Valuable Victim minus Least Valuable Attacker. Captures of
/// high-value pieces by low-value attackers rank highest. Quiet moves score 0.
pub fn mvv_lva_score(board: &Board, mv: Move) -> i32 {
    let dest_sq = mv.get_dest();
    let src_sq = mv.get_src();
    let dest = dest_sq.to_usize();
    let src = src_sq.to_usize();
    let us = board.side_to_move;
    let opp = us.opposite();
    let opp_off = if opp == Color::White { 0 } else { 6 };
    let our_off = if us == Color::White { 0 } else { 6 };

    let victim = if board.player_bbs[opp.to_index()].get(dest) {
        let mut v = 0;
        for i in 0..6 {
            if board.bbs[opp_off + i].get(dest) {
                v = PIECE_VALUES[i];
                break;
            }
        }
        v
    } else if board.en_passant == Some(dest_sq) && board.bbs[our_off].get(src) {
        PIECE_VALUES[0]
    } else {
        return 0;
    };

    let mut attacker = 0;
    for i in 0..6 {
        if board.bbs[our_off + i].get(src) {
            attacker = PIECE_VALUES[i];
            break;
        }
    }

    victim * 10 - attacker
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torte::movegen::magic;

    fn pos(fen: &str) -> Board {
        magic::init();
        Board::parse(fen)
    }

    #[test]
    fn finds_mate_in_one() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (mv, score) = find_best_move(&board, 2).unwrap();
        assert_eq!(score, MATE_SCORE - 1);

        let mut after = board;
        after.apply_move(mv).unwrap();
        assert!(generate_legal_moves(&after).is_empty());
    }

    #[test]
    fn no_legal_moves_returns_none() {
        let board = pos("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
        assert!(find_best_move(&board, 2).is_none());
    }

    #[test]
    fn captures_free_material() {
        let board = pos("4k3/8/8/1q6/8/2N5/8/4K3 w - - 0 1");
        // PST contributions change the absolute score; disable to keep this
        // test focused on material math.
        let (mv, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { piece_square_tables: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(mv.to_uci(), "c3b5");
        assert_eq!(score, 320);
    }

    #[test]
    fn ordering_does_not_change_best_score() {
        // Move ordering is a search-efficiency optimization; the best score
        // must be identical regardless of whether it's enabled.
        let board = pos("4k3/8/8/1q6/8/2N5/8/4K3 w - - 0 1");
        let with_ordering = find_best_move_with(
            &board,
            3,
            SearchConfig { move_ordering: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_ordering = find_best_move_with(
            &board,
            3,
            SearchConfig { move_ordering: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_ordering.1, without_ordering.1);
    }

    #[test]
    fn ordering_finds_mate_too() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { move_ordering: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn mvv_lva_pawn_takes_queen_ranks_above_queen_takes_pawn() {
        // White pawn on d4 can capture black queen on e5; white queen on a1
        // can capture black pawn on a8. Pawn-takes-queen should rank higher.
        let board = pos("q7/8/8/4q3/3P4/8/8/Q3K2k w - - 0 1");
        let pxq = Move::new(
            crate::torte::core::sq::SQ::make(3, 3),
            crate::torte::core::sq::SQ::make(4, 4),
        );
        let qxp = Move::new(
            crate::torte::core::sq::SQ::make(0, 0),
            crate::torte::core::sq::SQ::make(7, 0),
        );
        assert!(mvv_lva_score(&board, pxq) > mvv_lva_score(&board, qxp));
    }

    #[test]
    fn quiescence_sees_recapture_at_depth_one() {
        // White knight on c4 attacks black queen on b6, defended by black king on c7.
        // At depth=1: without quiescence, white sees Nxb6 as +320 (captures queen,
        // ignores recapture). With quiescence, qsearch follows Kxb6 and the trade nets 0.
        let board = pos("8/2k5/1q6/8/2N5/8/8/4K3 w - - 0 1");

        let with_q = find_best_move_with(
            &board,
            1,
            SearchConfig {
                move_ordering: true,
                quiescence: true,
                piece_square_tables: false,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        let without_q = find_best_move_with(
            &board,
            1,
            SearchConfig {
                move_ordering: true,
                quiescence: false,
                piece_square_tables: false,
                ..SearchConfig::default()
            },
        )
        .unwrap();

        assert_eq!(without_q.1, 320, "no-q should overestimate the trade");
        assert_eq!(with_q.1, 0, "qsearch should see the recapture");
        assert!(with_q.1 < without_q.1);
    }

    #[test]
    fn iterative_deepening_matches_direct_search() {
        let board = pos("4k3/8/8/1q6/8/2N5/8/4K3 w - - 0 1");
        let config = SearchConfig::default();
        let direct = find_best_move_with(&board, 4, config).unwrap();
        let id = iterative_deepening(&board, 4, config, None, |_, _, _, _| {}).unwrap();
        assert_eq!(direct.1, id.1);
    }

    #[test]
    fn iterative_deepening_calls_callback_per_depth() {
        let board = pos("4k3/8/8/1q6/8/2N5/8/4K3 w - - 0 1");
        let mut depths = Vec::new();
        iterative_deepening(&board, 3, SearchConfig::default(), None, |d, _, _, _| {
            depths.push(d);
        });
        assert_eq!(depths, vec![1, 2, 3]);
    }

    #[test]
    fn iterative_deepening_short_circuits_on_mate() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let mut last_depth = 0;
        let result = iterative_deepening(&board, 6, SearchConfig::default(), None, |d, _, _, _| {
            last_depth = d;
        })
        .unwrap();
        assert_eq!(result.1, MATE_SCORE - 1);
        // Mate-in-1 should be found at depth 2 and stop the loop.
        assert_eq!(last_depth, 2);
    }

    #[test]
    fn transposition_table_matches_no_tt_score() {
        // TT is an optimization; results should be identical to the no-TT search
        // on the same position at the same depth.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_tt = find_best_move_with(
            &board,
            3,
            SearchConfig {
                transposition_table: true,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        let without_tt = find_best_move_with(
            &board,
            3,
            SearchConfig {
                transposition_table: false,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(with_tt.1, without_tt.1);
    }

    #[test]
    fn transposition_table_finds_mate_in_one() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig {
                transposition_table: true,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn mate_score_round_trip() {
        // Storing then retrieving a mate score with the same ply must be identity.
        for ply in 0..10 {
            let stored = store_mate_score(MATE_SCORE - 5, ply);
            let retrieved = retrieve_mate_score(stored, ply);
            assert_eq!(retrieved, MATE_SCORE - 5);

            let stored_neg = store_mate_score(-(MATE_SCORE - 5), ply);
            let retrieved_neg = retrieve_mate_score(stored_neg, ply);
            assert_eq!(retrieved_neg, -(MATE_SCORE - 5));
        }
    }

    #[test]
    fn mate_score_not_adjusted_for_normal_scores() {
        // Non-mate scores must pass through unchanged.
        for ply in 0..10 {
            assert_eq!(store_mate_score(123, ply), 123);
            assert_eq!(retrieve_mate_score(123, ply), 123);
            assert_eq!(store_mate_score(-456, ply), -456);
        }
    }

    #[test]
    fn iterative_deepening_respects_past_deadline() {
        let board = pos("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        // Deadline already in the past: zero iterations should run.
        let past = Instant::now() - Duration::from_secs(1);
        let mut count = 0;
        let result = iterative_deepening(
            &board,
            5,
            SearchConfig::default(),
            Some(past),
            |_, _, _, _| count += 1,
        );
        assert_eq!(count, 0);
        assert!(result.is_none());
    }

    #[test]
    fn quiescence_does_not_break_simple_finds() {
        // Sanity: every test that passed without quiescence still passes with it.
        // Disable PSTs so the assertion is on raw material (320 = knight value).
        let board = pos("4k3/8/8/1q6/8/2N5/8/4K3 w - - 0 1");
        let (mv, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { piece_square_tables: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(mv.to_uci(), "c3b5");
        assert_eq!(score, 320);
    }

    #[test]
    fn mvv_lva_quiet_is_zero() {
        let board = pos("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let e2e4 = Move::new(
            crate::torte::core::sq::SQ::make(1, 4),
            crate::torte::core::sq::SQ::make(3, 4),
        );
        assert_eq!(mvv_lva_score(&board, e2e4), 0);
    }

    #[test]
    fn killer_moves_match_no_killer_score() {
        // Killers are an ordering heuristic; results should be identical to a
        // search with the heuristic off, on the same position at the same depth.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_killers = find_best_move_with(
            &board,
            3,
            SearchConfig {
                killer_moves: true,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        let without_killers = find_best_move_with(
            &board,
            3,
            SearchConfig {
                killer_moves: false,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(with_killers.1, without_killers.1);
    }

    #[test]
    fn killer_moves_finds_mate_in_one() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig {
                killer_moves: true,
                ..SearchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }
}
