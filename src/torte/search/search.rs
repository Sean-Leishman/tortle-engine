use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::piece_move::Move;
use crate::torte::movegen::generator::{generate_legal_moves, is_attacked, king_square};
use crate::torte::search::eval::{eval, EvalConfig, PIECE_VALUES};
use crate::torte::search::transposition::{Bound, TTEntry, TranspositionTable};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

/// History heuristic table, indexed `[side][from][to]`. Each quiet move that
/// causes a beta cutoff has its slot bumped by `depth²`, so moves that have
/// historically been good get ordered earlier even when they aren't killers
/// at the current ply. Unlike killers this is *not* ply-indexed — the signal
/// is "this from→to move tends to be strong" regardless of where in the tree.
pub type HistoryTable = [[[i32; 64]; 64]; 2];

pub fn new_history() -> Box<HistoryTable> {
    Box::new([[[0; 64]; 64]; 2])
}

/// Cap on a single history slot. Bounds the ordering key's range (so it stays
/// in its band between killers and zero-history quiets) and prevents unbounded
/// growth over a long search.
const HISTORY_MAX: i32 = 1 << 20;

/// Combined "stop searching" signal: a wall-clock deadline and/or an atomic
/// flag (typically set by UCI `stop`). Checked inside negamax/qsearch when the
/// `mid_search_abort` toggle is on, so the search can interrupt a running
/// iteration rather than only checking between iterations.
#[derive(Clone, Default)]
pub struct AbortSignal {
    pub deadline: Option<Instant>,
    pub stop: Option<Arc<AtomicBool>>,
}

impl AbortSignal {
    pub fn never() -> Self {
        Self::default()
    }

    pub fn with_deadline(deadline: Option<Instant>) -> Self {
        Self { deadline, stop: None }
    }

    pub fn fire(&self) -> bool {
        if let Some(s) = self.stop.as_ref() {
            if s.load(Ordering::Relaxed) {
                return true;
            }
        }
        if let Some(dl) = self.deadline {
            if Instant::now() >= dl {
                return true;
            }
        }
        false
    }
}

/// Every NODE_CHECK_INTERVAL nodes, consult the abort signal. AtomicBool
/// loads are cheap, but `Instant::now()` adds up — bucketing the check keeps
/// the deadline-check overhead well under 1% at typical nps.
const NODE_CHECK_INTERVAL: u64 = 2048;

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
    pub pawn_structure: bool,
    pub bishop_pair: bool,
    pub king_safety: bool,
    pub killer_moves: bool,
    pub history_heuristic: bool,
    pub mid_search_abort: bool,
    pub null_move_pruning: bool,
    pub aspiration_windows: bool,
    pub late_move_reductions: bool,
    pub futility_pruning: bool,
    pub razoring: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            move_ordering: true,
            quiescence: true,
            iterative_deepening: true,
            transposition_table: true,
            piece_square_tables: true,
            pawn_structure: true,
            bishop_pair: true,
            king_safety: true,
            killer_moves: true,
            history_heuristic: true,
            mid_search_abort: true,
            null_move_pruning: true,
            aspiration_windows: true,
            late_move_reductions: true,
            futility_pruning: true,
            // Razoring is off by default — see RAZOR_MAX_DEPTH note. Toggle
            // on via `setoption name Razoring value true` for experiments.
            razoring: false,
        }
    }
}

impl SearchConfig {
    /// Project the eval-related toggles onto an `EvalConfig` for `eval`.
    pub fn eval_config(&self) -> EvalConfig {
        EvalConfig {
            piece_square_tables: self.piece_square_tables,
            pawn_structure: self.pawn_structure,
            bishop_pair: self.bishop_pair,
            king_safety: self.king_safety,
        }
    }
}

/// Starting half-width for the aspiration window around the previous ID
/// iteration's score. On fail high/low we re-search at full width.
/// Tactical mid-game positions can swing >50cp between iterations, so we
/// use 100 to make fail-high/low rare; the trade-off is a slightly wider
/// (less productive) window when aspiration succeeds.
const ASPIRATION_DELTA: i32 = 100;

/// Skip aspiration windowing below this depth. Shallow iterations are fast
/// regardless, and their score is least stable — narrow-windowing them
/// causes more re-searches than savings.
const ASPIRATION_MIN_DEPTH: u32 = 5;

/// Reduction `R` for null-move search: search at `depth - 1 - R`. R=2 is the
/// classic conservative value; some engines use R=3 above depth 6.
const NULL_MOVE_REDUCTION: u32 = 2;

/// Skip null-move pruning shallower than this. Below the threshold the saved
/// work doesn't justify the risk of pruning a tactical line.
const NULL_MOVE_MIN_DEPTH: u32 = 3;

/// LMR ply reduction. Conservative R=1 keeps the risk of tactical blindness
/// low while still trimming the tail of the move list at every node.
const LMR_REDUCTION: u32 = 1;

/// Skip LMR shallower than this; below 3 the saved work isn't worth the
/// re-search risk.
const LMR_MIN_DEPTH: u32 = 3;

/// Index (0-based) at which to start reducing in the move list. The first
/// three moves (TT move, top MVV-LVA captures, killers) are searched at
/// full depth; the tail gets reduced.
const LMR_MIN_MOVE_IDX: usize = 3;

/// Apply futility pruning only at frontier depths. At depth 3+ the search
/// tree is large enough that a single quiet move can swing the score by more
/// than a fixed margin.
const FUTILITY_MAX_DEPTH: u32 = 2;

/// Per-depth futility margin in centipawns. The eval would have to gain more
/// than this from any single quiet move to lift static_eval above alpha; if
/// even with the margin we can't reach alpha, prune the quiet moves.
fn futility_margin(depth: u32) -> i32 {
    (depth as i32) * 150
}

/// Apply razoring only at shallow depths where dropping into qsearch
/// approximates the full search closely enough that the heuristic mistake
/// rate is acceptable.
///
/// NOTE: razoring is **off by default** in this engine — empirical testing
/// on kiwipete d7 showed it was a wash-to-loss against our already-strong
/// pruning stack (TT, NMP, futility, LMR, aspiration). The code is kept
/// behind the toggle for re-tuning experiments (try null-window qsearch,
/// different margins, depth-only ranges, different positions).
const RAZOR_MAX_DEPTH: u32 = 3;

/// Per-depth razoring margin. Wider than futility's because razoring
/// replaces the whole subtree with a capture-only search — we only want to
/// risk that when the static eval is *very* far below alpha. A misfire
/// (qsearch ≥ alpha) costs the qsearch plus the full search we still have
/// to do.
fn razor_margin(depth: u32) -> i32 {
    match depth {
        1 => 500,
        2 => 800,
        3 => 1300,
        _ => 0,
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
    let mut history = new_history();
    find_best_move_with_tt(
        board,
        depth,
        config,
        &mut tt,
        &mut killers,
        &mut history,
        &AbortSignal::never(),
    )
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
    iterative_deepening_with_abort(
        board,
        max_depth,
        config,
        AbortSignal::with_deadline(deadline),
        tt,
        &mut on_iteration,
    )
}

/// Like `iterative_deepening_with_tt`, but accepts a full `AbortSignal`
/// (deadline + optional stop flag) so a UCI `stop` can interrupt the search
/// mid-iteration when `config.mid_search_abort` is on.
pub fn iterative_deepening_with_abort<F>(
    board: &Board,
    max_depth: u32,
    config: SearchConfig,
    abort: AbortSignal,
    tt: &mut TranspositionTable,
    on_iteration: &mut F,
) -> Option<(Move, i32)>
where
    F: FnMut(u32, Move, i32, Duration),
{
    let start = Instant::now();
    let mut last_best: Option<(Move, i32)> = None;
    // Killers and history persist across ID iterations: a quiet move that
    // caused a cutoff at depth N-1 is still a promising candidate at depth N.
    let mut killers = new_killers();
    let mut history = new_history();

    for d in 1..=max_depth {
        if abort.fire() {
            break;
        }

        let result = search_one_iteration(
            board, d, &last_best, config, tt, &mut killers, &mut history, &abort,
        );
        // If the iteration was interrupted mid-flight, its result is unreliable —
        // discard it and fall back to the deepest fully-completed iteration.
        if abort.fire() {
            break;
        }
        match result {
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

/// One ID iteration. If aspiration windows are enabled and we have a previous
/// iteration's score, search with a narrow `[score-delta, score+delta]` window;
/// on fail-high or fail-low, re-search with a full window. Otherwise full
/// window directly.
fn search_one_iteration(
    board: &Board,
    depth: u32,
    last_best: &Option<(Move, i32)>,
    config: SearchConfig,
    tt: &mut TranspositionTable,
    killers: &mut KillerTable,
    history: &mut HistoryTable,
    abort: &AbortSignal,
) -> Option<(Move, i32)> {
    if !config.aspiration_windows || depth < ASPIRATION_MIN_DEPTH {
        return find_best_move_with_tt(board, depth, config, tt, killers, history, abort);
    }
    let prev_score = match *last_best {
        Some((_, s)) if s.abs() < MATE_SCORE - 1000 => s,
        // First iteration or near-mate score: no useful window to narrow to.
        _ => return find_best_move_with_tt(board, depth, config, tt, killers, history, abort),
    };

    let alpha = prev_score - ASPIRATION_DELTA;
    let beta = prev_score + ASPIRATION_DELTA;
    let result =
        find_best_move_with_window(board, depth, alpha, beta, config, tt, killers, history, abort)?;
    let (_, score) = result;
    if score <= alpha || score >= beta {
        // Fail high/low: the true score lies outside our guess. Re-search at
        // full width to get the correct best move and score.
        find_best_move_with_tt(board, depth, config, tt, killers, history, abort)
    } else {
        Some(result)
    }
}

pub fn find_best_move_with_tt(
    board: &Board,
    depth: u32,
    config: SearchConfig,
    tt: &mut TranspositionTable,
    killers: &mut KillerTable,
    history: &mut HistoryTable,
    abort: &AbortSignal,
) -> Option<(Move, i32)> {
    find_best_move_with_window(
        board, depth, -INFINITY, INFINITY, config, tt, killers, history, abort,
    )
}

/// Root search with a caller-supplied `[alpha, beta]` window. Used by
/// aspiration windows: passing a narrow window enables more cutoffs deeper
/// in the tree. Returns the best move and its score; the score may be
/// outside the window (fail-high / fail-low), and the caller is expected
/// to detect that and re-search if needed.
pub fn find_best_move_with_window(
    board: &Board,
    depth: u32,
    alpha: i32,
    beta: i32,
    config: SearchConfig,
    tt: &mut TranspositionTable,
    killers: &mut KillerTable,
    history: &mut HistoryTable,
    abort: &AbortSignal,
) -> Option<(Move, i32)> {
    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return None;
    }

    // At the root we don't return early on a TT hit (we need the best move),
    // but we do use the stored move as the ordering hint.
    let key = board.zobrist;
    let tt_move = if config.transposition_table {
        tt.probe(key).and_then(|e| e.best_move)
    } else {
        None
    };

    if config.move_ordering {
        order_moves(board, &mut moves, tt_move, killer_slice(killers, 0, config), history);
    }

    let depth = depth.max(1);
    let mut best_move = moves[0];
    // Seed best_score below the window so any in-window child score becomes
    // the new best; if all children fall at-or-below alpha we still return
    // alpha as the (fail-low) score.
    let mut best_score = alpha;
    let mut nodes: u64 = 0;

    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let score = -negamax(
            &next,
            depth - 1,
            -beta,
            -best_score,
            1,
            config,
            tt,
            killers,
            history,
            abort,
            &mut nodes,
        );
        if config.mid_search_abort && abort.fire() {
            // Result of this branch is unreliable; bail out and let the caller
            // discard the iteration.
            return Some((best_move, best_score));
        }
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
    history: &mut HistoryTable,
    abort: &AbortSignal,
    nodes: &mut u64,
) -> i32 {
    *nodes += 1;
    if config.mid_search_abort
        && (*nodes & (NODE_CHECK_INTERVAL - 1)) == 0
        && abort.fire()
    {
        return 0;
    }
    let mut alpha = alpha;
    let original_alpha = alpha;
    let key = board.zobrist;

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
            qsearch(board, alpha, beta, ply, config, history, abort, nodes)
        } else {
            eval(board, config.eval_config())
        };
    }

    let node_in_check = in_check(board);

    // Static eval is needed by razoring and by futility eligibility below;
    // compute it once, lazily, the first time something asks for it.
    let mut static_eval: Option<i32> = None;
    let in_safe_window =
        alpha.abs() < MATE_SCORE - 1000 && beta.abs() < MATE_SCORE - 1000;

    // Razoring: at shallow depths, if even a generous margin can't lift the
    // static eval to alpha, replace the search with qsearch. If qsearch
    // confirms below alpha, return that score (a fail-low at this node).
    // Caller's alpha-beta will not improve alpha from a sub-alpha return,
    // so this is heuristic but doesn't change the result when it's right.
    if config.razoring
        && depth <= RAZOR_MAX_DEPTH
        && !node_in_check
        && in_safe_window
    {
        let se = *static_eval
            .get_or_insert_with(|| eval(board, config.eval_config()));
        if se + razor_margin(depth) < alpha {
            let score = qsearch(board, alpha, beta, ply, config, history, abort, nodes);
            if config.mid_search_abort && abort.fire() {
                return alpha;
            }
            if score < alpha {
                return score;
            }
        }
    }

    // Null-move pruning. Skip in check (illegal — leaves king en prise) and
    // in pawn/king endgames (zugzwang risk: the side to move benefits from
    // giving up the turn, which never happens with real pieces). Skip near
    // mate windows so we don't prune through a mate. Reduce by R.
    if config.null_move_pruning
        && depth >= NULL_MOVE_MIN_DEPTH
        && beta.abs() < MATE_SCORE - 1000
        && !node_in_check
        && has_non_pawn_material(board, board.side_to_move)
    {
        let mut null = *board;
        // Keep zobrist in sync: clear ep file (if any) and flip side.
        if let Some(ep) = null.en_passant {
            null.zobrist ^=
                crate::torte::search::transposition::ep_file_key(ep.0 % 8);
        }
        null.en_passant = None;
        null.side_to_move = board.side_to_move.opposite();
        null.zobrist ^= crate::torte::search::transposition::side_key();
        let reduced = depth - 1 - NULL_MOVE_REDUCTION;
        let score = -negamax(
            &null,
            reduced,
            -beta,
            -beta + 1,
            ply + 1,
            config,
            tt,
            killers,
            history,
            abort,
            nodes,
        );
        if config.mid_search_abort && abort.fire() {
            return alpha;
        }
        if score >= beta {
            return beta;
        }
    }

    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return terminal_score(board, ply);
    }
    if config.move_ordering {
        order_moves(board, &mut moves, tt_move, killer_slice(killers, ply, config), history);
    }

    let mut best_move = moves[0];

    // Futility pruning eligibility (constant for this node): at frontier
    // depths, if even adding a generous margin to the static eval can't
    // reach alpha, then any *quiet* non-promotion move is very unlikely to
    // lift the score above alpha — so we skip them entirely. We always
    // evaluate the first move (best-ordered) so the node has a real score.
    let futility_prune = config.futility_pruning
        && depth <= FUTILITY_MAX_DEPTH
        && !node_in_check
        && in_safe_window
        && {
            let se = *static_eval
                .get_or_insert_with(|| eval(board, config.eval_config()));
            se + futility_margin(depth) <= alpha
        };

    for (move_index, m) in moves.into_iter().enumerate() {
        if futility_prune
            && move_index > 0
            && !is_capture(board, m)
            && m.get_promotion().is_none()
        {
            continue;
        }
        let mut next = *board;
        next.apply_move(m).unwrap();
        let do_lmr = config.late_move_reductions
            && depth >= LMR_MIN_DEPTH
            && move_index >= LMR_MIN_MOVE_IDX
            && !node_in_check
            && !is_capture(board, m)
            && m.get_promotion().is_none();
        let mut score = if do_lmr {
            -negamax(
                &next,
                depth - 1 - LMR_REDUCTION,
                -beta,
                -alpha,
                ply + 1,
                config,
                tt,
                killers,
                history,
                abort,
                nodes,
            )
        } else {
            -negamax(
                &next,
                depth - 1,
                -beta,
                -alpha,
                ply + 1,
                config,
                tt,
                killers,
                history,
                abort,
                nodes,
            )
        };
        // Re-search at full depth if the reduced search beat alpha — the
        // reduction may have hidden a real improvement.
        if do_lmr && score > alpha && !(config.mid_search_abort && abort.fire()) {
            score = -negamax(
                &next,
                depth - 1,
                -beta,
                -alpha,
                ply + 1,
                config,
                tt,
                killers,
                history,
                abort,
                nodes,
            );
        }
        if config.mid_search_abort && abort.fire() {
            return alpha;
        }

        if score >= beta {
            if !is_capture(board, m) {
                if config.killer_moves && (ply as usize) < MAX_PLY {
                    let slot = &mut killers[ply as usize];
                    if slot[0] != Some(m) {
                        slot[1] = slot[0];
                        slot[0] = Some(m);
                    }
                }
                // History: reward this quiet from→to move by depth², so it
                // sorts ahead of untried quiets at every ply, not just here.
                if config.history_heuristic {
                    let side = board.side_to_move.to_index();
                    let slot =
                        &mut history[side][m.get_src().to_usize()][m.get_dest().to_usize()];
                    *slot = (*slot + (depth * depth) as i32).min(HISTORY_MAX);
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
    history: &HistoryTable,
    abort: &AbortSignal,
    nodes: &mut u64,
) -> i32 {
    *nodes += 1;
    if config.mid_search_abort
        && (*nodes & (NODE_CHECK_INTERVAL - 1)) == 0
        && abort.fire()
    {
        return 0;
    }
    let stand_pat = eval(board, config.eval_config());
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
        order_moves(board, &mut moves, None, [None, None], history);
    }

    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let score = -qsearch(&next, -beta, -alpha, ply + 1, config, history, abort, nodes);
        if config.mid_search_abort && abort.fire() {
            return alpha;
        }
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

fn in_check(board: &Board) -> bool {
    match king_square(board, board.side_to_move) {
        Some(k) => is_attacked(board, k, board.side_to_move.opposite()),
        None => false,
    }
}

/// True if `side` has at least one knight/bishop/rook/queen on the board.
/// Used as a zugzwang guard for null-move pruning — in pawn endgames giving
/// up the turn often helps the side to move, which can't happen with real
/// moves, so null-move-prune would lie.
fn has_non_pawn_material(board: &Board, side: Color) -> bool {
    let off = if side == Color::White { 0 } else { 6 };
    // kinds 1..=4 are N/B/R/Q (kind 0 is pawn, kind 5 is king).
    (board.bbs[off + 1].board
        | board.bbs[off + 2].board
        | board.bbs[off + 3].board
        | board.bbs[off + 4].board)
        != 0
}

fn order_moves(
    board: &Board,
    moves: &mut Vec<Move>,
    hint: Option<Move>,
    killers: [Option<Move>; 2],
    history: &HistoryTable,
) {
    let side = board.side_to_move.to_index();
    moves.sort_by_key(|m| {
        // Lower key = searched earlier. Each category sits in its own
        // disjoint band: the TT move, then captures (MVV-LVA), then killers,
        // then quiet moves ranked by history score, with untried quiets
        // (history 0) last. The band offsets are wide enough that no two
        // categories can ever interleave.
        if Some(*m) == hint {
            return i32::MIN;
        }
        let mvv = mvv_lva_score(board, *m);
        if mvv > 0 {
            return -(1 << 28) - mvv;
        }
        if Some(*m) == killers[0] || Some(*m) == killers[1] {
            return -(1 << 24);
        }
        -history[side][m.get_src().to_usize()][m.get_dest().to_usize()].min(HISTORY_MAX)
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
    fn mid_search_abort_off_matches_default_score() {
        // Toggling mid-search abort off should not change the score: it's a
        // pure interruption mechanism, not a search-shape change.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_abort = find_best_move_with(
            &board,
            3,
            SearchConfig { mid_search_abort: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_abort = find_best_move_with(
            &board,
            3,
            SearchConfig { mid_search_abort: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_abort.1, without_abort.1);
    }

    #[test]
    fn mid_search_abort_stops_via_atomic_flag() {
        // A pre-set stop flag should make iterative deepening return no result
        // (or at most the last completed iteration, which is none here).
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let stop = Arc::new(AtomicBool::new(true));
        let abort = AbortSignal {
            deadline: None,
            stop: Some(stop),
        };
        let mut tt = TranspositionTable::default_size();
        let mut on_iter = |_: u32, _: Move, _: i32, _: Duration| {};
        let result =
            iterative_deepening_with_abort(&board, 6, SearchConfig::default(), abort, &mut tt, &mut on_iter);
        assert!(result.is_none());
    }

    #[test]
    fn aspiration_windows_match_no_aspiration_score() {
        // Aspiration is a window-narrowing optimization; final scores must be
        // identical to the full-window search after fail-high/low re-search.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let mut tt_a = TranspositionTable::default_size();
        let mut tt_b = TranspositionTable::default_size();
        let with_asp = iterative_deepening_with_tt(
            &board,
            4,
            SearchConfig { aspiration_windows: true, ..SearchConfig::default() },
            None,
            &mut tt_a,
            |_, _, _, _| {},
        )
        .unwrap();
        let without_asp = iterative_deepening_with_tt(
            &board,
            4,
            SearchConfig { aspiration_windows: false, ..SearchConfig::default() },
            None,
            &mut tt_b,
            |_, _, _, _| {},
        )
        .unwrap();
        assert_eq!(with_asp.1, without_asp.1);
    }

    #[test]
    fn aspiration_recovers_on_fail_high() {
        // Mate-in-1 produces a score far outside any 50cp window around 0
        // (the depth-1 score before the mate is found). Aspiration must
        // re-search and still find the mate.
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let mut tt = TranspositionTable::default_size();
        let result = iterative_deepening_with_tt(
            &board,
            4,
            SearchConfig::default(),
            None,
            &mut tt,
            |_, _, _, _| {},
        )
        .unwrap();
        assert_eq!(result.1, MATE_SCORE - 1);
    }

    #[test]
    fn null_move_pruning_still_finds_mate_in_one() {
        // NMP is a pruning heuristic; obvious tactics like mate-in-1 should
        // still be found at any depth >= 2.
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { null_move_pruning: true, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn null_move_pruning_disabled_in_pawn_endgame() {
        // KP vs K — only pawns and kings, so the zugzwang guard should hold
        // and NMP shouldn't kick in. We verify by checking the result is the
        // same as without NMP entirely.
        let board = pos("8/8/4k3/8/8/8/4P3/4K3 w - - 0 1");
        let with_nmp = find_best_move_with(
            &board,
            4,
            SearchConfig { null_move_pruning: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_nmp = find_best_move_with(
            &board,
            4,
            SearchConfig { null_move_pruning: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_nmp.1, without_nmp.1);
    }

    #[test]
    fn razoring_finds_mate_in_one() {
        // Razoring is gated by `in_safe_window` (alpha not near mate), so the
        // mate-in-1 search should never razor through the mate.
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { razoring: true, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn razoring_finds_best_move_on_kiwipete() {
        // Razoring is a heuristic so we can't require exact score parity, but
        // on a well-known tactical position the best move should be unchanged
        // from the no-razoring search at the same depth.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_r = find_best_move_with(
            &board,
            6,
            SearchConfig { razoring: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_r = find_best_move_with(
            &board,
            6,
            SearchConfig { razoring: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_r.0, without_r.0);
    }

    #[test]
    fn futility_pruning_matches_no_futility_score() {
        // Futility is a pruning heuristic, but skipped quiet moves are by
        // construction unable to lift static_eval above alpha. The score must
        // therefore match the no-futility search at the same depth.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_f = find_best_move_with(
            &board,
            4,
            SearchConfig { futility_pruning: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_f = find_best_move_with(
            &board,
            4,
            SearchConfig { futility_pruning: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_f.1, without_f.1);
    }

    #[test]
    fn futility_pruning_finds_mate_in_one() {
        // Mate-in-1 must still be found: the mate-window check disables
        // futility when alpha is near MATE_SCORE.
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { futility_pruning: true, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn late_move_reductions_match_no_lmr_score() {
        // LMR is a search-shape optimization; with re-search on alpha-improvement
        // the final score must match the full-depth search.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_lmr = find_best_move_with(
            &board,
            4,
            SearchConfig { late_move_reductions: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_lmr = find_best_move_with(
            &board,
            4,
            SearchConfig { late_move_reductions: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_lmr.1, without_lmr.1);
    }

    #[test]
    fn late_move_reductions_finds_mate_in_one() {
        // Mate-in-1 must still be found with LMR on — the mate move is the
        // first move tried (TT/MVV hints), well before the LMR cutoff.
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { late_move_reductions: true, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
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

    #[test]
    fn history_heuristic_matches_no_history_score() {
        // History is a pure move-ordering heuristic; the best score must be
        // identical to a search with it off, on the same position and depth.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let with_history = find_best_move_with(
            &board,
            4,
            SearchConfig { history_heuristic: true, ..SearchConfig::default() },
        )
        .unwrap();
        let without_history = find_best_move_with(
            &board,
            4,
            SearchConfig { history_heuristic: false, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(with_history.1, without_history.1);
    }

    #[test]
    fn history_heuristic_finds_mate_in_one() {
        let board = pos("k7/8/1K6/3Q4/8/8/8/8 w - - 0 1");
        let (_, score) = find_best_move_with(
            &board,
            2,
            SearchConfig { history_heuristic: true, ..SearchConfig::default() },
        )
        .unwrap();
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn order_moves_bands_are_disjoint() {
        // The capture / killer / history-quiet ordering bands must never
        // interleave: a capture sorts before any killer, and a killer before
        // any history-ranked quiet — even when a history score has grown all
        // the way to HISTORY_MAX.
        let board = pos("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let mut moves = generate_legal_moves(&board);
        let side = board.side_to_move.to_index();
        let mut quiets = moves.iter().copied().filter(|m| !is_capture(&board, *m));
        let killer = quiets.next().expect("kiwipete has quiet moves");
        let hist_quiet = quiets.next().expect("kiwipete has >1 quiet move");
        let capture = moves
            .iter()
            .copied()
            .find(|m| is_capture(&board, *m))
            .expect("kiwipete has captures");

        let mut history = new_history();
        history[side][hist_quiet.get_src().to_usize()][hist_quiet.get_dest().to_usize()] =
            HISTORY_MAX;
        order_moves(&board, &mut moves, None, [Some(killer), None], &history);

        let idx = |target: Move| moves.iter().position(|m| *m == target).unwrap();
        assert!(idx(capture) < idx(killer), "capture must sort before killer");
        assert!(
            idx(killer) < idx(hist_quiet),
            "killer must sort before a history-maxed quiet"
        );
    }
}
