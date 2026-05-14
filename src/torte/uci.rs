use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::search::search::{
    find_best_move_with_tt, iterative_deepening_with_tt, SearchConfig, MATE_SCORE,
};
use crate::torte::search::transposition::TranspositionTable;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

pub const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const NAME: &str = "torte";
const AUTHOR: &str = "Sean Leishman";
const DEFAULT_DEPTH: u32 = 6;
const DEFAULT_HASH_MB: usize = 16;
const MIN_HASH_MB: usize = 1;
const MAX_HASH_MB: usize = 1024;

pub fn run(board: &mut Board) {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    let mut config = SearchConfig::default();
    let mut tt = TranspositionTable::new(DEFAULT_HASH_MB);

    loop {
        line.clear();
        match handle.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cmd = trimmed.split_whitespace().next().unwrap_or("");
        let rest = trimmed[cmd.len()..].trim_start();

        match cmd {
            "uci" => {
                emit(&format!("id name {}", NAME));
                emit(&format!("id author {}", AUTHOR));
                emit_options(&config);
                emit("uciok");
            }
            "isready" => emit("readyok"),
            "ucinewgame" => {
                *board = Board::parse(STARTPOS);
                tt.clear();
            }
            "position" => {
                if let Some(b) = parse_position(rest) {
                    *board = b;
                } else {
                    emit("info string failed to parse position");
                }
            }
            "setoption" => apply_setoption(rest, &mut config, &mut tt),
            "go" => handle_go(board, parse_go(rest, board.side_to_move), config, &mut tt),
            "d" | "board" => {
                println!("{:?}", board);
                emit(&format!("info string config {:?}", config));
            }
            "stop" | "ponderhit" | "debug" | "register" => {}
            "quit" | "exit" => break,
            _ => {
                if board.apply_uci_move(trimmed).is_err() {
                    emit(&format!("info string unknown command or illegal move: {}", trimmed));
                }
            }
        }
    }
}

fn emit_options(config: &SearchConfig) {
    emit(&format!(
        "option name MoveOrdering type check default {}",
        config.move_ordering
    ));
    emit(&format!(
        "option name Quiescence type check default {}",
        config.quiescence
    ));
    emit(&format!(
        "option name IterativeDeepening type check default {}",
        config.iterative_deepening
    ));
    emit(&format!(
        "option name TranspositionTable type check default {}",
        config.transposition_table
    ));
    emit(&format!(
        "option name PieceSquareTables type check default {}",
        config.piece_square_tables
    ));
    emit(&format!(
        "option name PawnStructure type check default {}",
        config.pawn_structure
    ));
    emit(&format!(
        "option name BishopPair type check default {}",
        config.bishop_pair
    ));
    emit(&format!(
        "option name KingSafety type check default {}",
        config.king_safety
    ));
    emit(&format!(
        "option name KillerMoves type check default {}",
        config.killer_moves
    ));
    emit(&format!(
        "option name HistoryHeuristic type check default {}",
        config.history_heuristic
    ));
    emit(&format!(
        "option name MidSearchAbort type check default {}",
        config.mid_search_abort
    ));
    emit(&format!(
        "option name NullMovePruning type check default {}",
        config.null_move_pruning
    ));
    emit(&format!(
        "option name AspirationWindows type check default {}",
        config.aspiration_windows
    ));
    emit(&format!(
        "option name LateMoveReductions type check default {}",
        config.late_move_reductions
    ));
    emit(&format!(
        "option name FutilityPruning type check default {}",
        config.futility_pruning
    ));
    emit(&format!(
        "option name Razoring type check default {}",
        config.razoring
    ));
    emit(&format!(
        "option name Hash type spin default {} min {} max {}",
        DEFAULT_HASH_MB, MIN_HASH_MB, MAX_HASH_MB
    ));
}

pub fn apply_setoption(args: &str, config: &mut SearchConfig, tt: &mut TranspositionTable) {
    let (name, value) = match parse_setoption(args) {
        Some(v) => v,
        None => return,
    };
    match name.as_str() {
        "MoveOrdering" => {
            if let Some(b) = parse_bool(&value) {
                config.move_ordering = b;
            }
        }
        "Quiescence" => {
            if let Some(b) = parse_bool(&value) {
                config.quiescence = b;
            }
        }
        "IterativeDeepening" => {
            if let Some(b) = parse_bool(&value) {
                config.iterative_deepening = b;
            }
        }
        "TranspositionTable" => {
            if let Some(b) = parse_bool(&value) {
                config.transposition_table = b;
            }
        }
        "PieceSquareTables" => {
            if let Some(b) = parse_bool(&value) {
                config.piece_square_tables = b;
            }
        }
        "PawnStructure" => {
            if let Some(b) = parse_bool(&value) {
                config.pawn_structure = b;
            }
        }
        "BishopPair" => {
            if let Some(b) = parse_bool(&value) {
                config.bishop_pair = b;
            }
        }
        "KingSafety" => {
            if let Some(b) = parse_bool(&value) {
                config.king_safety = b;
            }
        }
        "KillerMoves" => {
            if let Some(b) = parse_bool(&value) {
                config.killer_moves = b;
            }
        }
        "HistoryHeuristic" => {
            if let Some(b) = parse_bool(&value) {
                config.history_heuristic = b;
            }
        }
        "MidSearchAbort" => {
            if let Some(b) = parse_bool(&value) {
                config.mid_search_abort = b;
            }
        }
        "NullMovePruning" => {
            if let Some(b) = parse_bool(&value) {
                config.null_move_pruning = b;
            }
        }
        "AspirationWindows" => {
            if let Some(b) = parse_bool(&value) {
                config.aspiration_windows = b;
            }
        }
        "LateMoveReductions" => {
            if let Some(b) = parse_bool(&value) {
                config.late_move_reductions = b;
            }
        }
        "FutilityPruning" => {
            if let Some(b) = parse_bool(&value) {
                config.futility_pruning = b;
            }
        }
        "Razoring" => {
            if let Some(b) = parse_bool(&value) {
                config.razoring = b;
            }
        }
        "Hash" => {
            if let Ok(mb) = value.trim().parse::<usize>() {
                let clamped = mb.clamp(MIN_HASH_MB, MAX_HASH_MB);
                *tt = TranspositionTable::new(clamped);
            }
        }
        _ => {
            emit(&format!("info string unknown option: {}", name));
        }
    }
}

fn parse_setoption(args: &str) -> Option<(String, String)> {
    let mut tokens = args.split_whitespace();
    if tokens.next()? != "name" {
        return None;
    }
    let mut name_parts: Vec<&str> = Vec::new();
    let mut value_parts: Vec<&str> = Vec::new();
    let mut in_value = false;
    for tok in tokens {
        if !in_value && tok == "value" {
            in_value = true;
        } else if in_value {
            value_parts.push(tok);
        } else {
            name_parts.push(tok);
        }
    }
    if name_parts.is_empty() {
        return None;
    }
    Some((name_parts.join(" "), value_parts.join(" ")))
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Some(true),
        "false" | "0" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn emit(s: &str) {
    let stdout = io::stdout();
    let mut h = stdout.lock();
    let _ = writeln!(h, "{}", s);
    let _ = h.flush();
}

fn handle_go(board: &Board, args: GoArgs, config: SearchConfig, tt: &mut TranspositionTable) {
    let start = Instant::now();
    let deadline = args
        .time_budget_ms
        .map(|ms| start + Duration::from_millis(ms));

    let result = if config.iterative_deepening {
        iterative_deepening_with_tt(
            board,
            args.max_depth,
            config,
            deadline,
            tt,
            |d, mv, score, elapsed| {
                emit(&format!(
                    "info depth {} score {} time {} pv {}",
                    d,
                    format_score(score),
                    elapsed.as_millis(),
                    mv
                ));
            },
        )
    } else {
        let mut killers = crate::torte::search::search::new_killers();
        let mut history = crate::torte::search::search::new_history();
        let abort = crate::torte::search::search::AbortSignal::with_deadline(deadline);
        let r = find_best_move_with_tt(
            board,
            args.max_depth,
            config,
            tt,
            &mut killers,
            &mut history,
            &abort,
        );
        if let Some((mv, score)) = r {
            emit(&format!(
                "info depth {} score {} time {} pv {}",
                args.max_depth,
                format_score(score),
                start.elapsed().as_millis(),
                mv
            ));
        }
        r
    };

    match result {
        Some((mv, _)) => emit(&format!("bestmove {}", mv)),
        None => emit("bestmove 0000"),
    }
}

pub fn parse_position(args: &str) -> Option<Board> {
    let mut tokens = args.split_whitespace().peekable();
    let first = tokens.next()?;
    let mut board = match first {
        "startpos" => Board::parse(STARTPOS),
        "fen" => {
            let mut parts: Vec<&str> = Vec::new();
            while let Some(&t) = tokens.peek() {
                if t == "moves" {
                    break;
                }
                parts.push(tokens.next().unwrap());
            }
            if parts.is_empty() {
                return None;
            }
            Board::parse(&parts.join(" "))
        }
        _ => return None,
    };

    if let Some(t) = tokens.next() {
        if t != "moves" {
            return None;
        }
        for mv in tokens {
            board.apply_uci_move(mv).ok()?;
        }
    }
    Some(board)
}

const MAX_DEPTH_TIMED: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoArgs {
    pub max_depth: u32,
    pub time_budget_ms: Option<u64>,
}

pub fn parse_go(args: &str, side: Color) -> GoArgs {
    let mut depth: Option<u32> = None;
    let mut movetime: Option<u64> = None;
    let mut wtime: Option<u64> = None;
    let mut btime: Option<u64> = None;
    let mut winc: u64 = 0;
    let mut binc: u64 = 0;

    let mut iter = args.split_whitespace();
    while let Some(tok) = iter.next() {
        match tok {
            "depth" => depth = iter.next().and_then(|s| s.parse().ok()),
            "movetime" => movetime = iter.next().and_then(|s| s.parse().ok()),
            "wtime" => wtime = iter.next().and_then(|s| s.parse().ok()),
            "btime" => btime = iter.next().and_then(|s| s.parse().ok()),
            "winc" => winc = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            "binc" => binc = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }

    // REPL fallback: a bare integer means "depth N".
    if depth.is_none() && movetime.is_none() && wtime.is_none() && btime.is_none() {
        if let Ok(d) = args.trim().parse::<u32>() {
            depth = Some(d);
        }
    }

    let time_budget_ms = if let Some(mt) = movetime {
        Some(mt)
    } else {
        match (wtime, btime) {
            (Some(wt), Some(bt)) => {
                let (t, inc) = if side == Color::White { (wt, winc) } else { (bt, binc) };
                // Crude budget: 1/30th of remaining time plus half the increment.
                Some(t / 30 + inc / 2)
            }
            _ => None,
        }
    };

    let max_depth = depth.unwrap_or(if time_budget_ms.is_some() {
        MAX_DEPTH_TIMED
    } else {
        DEFAULT_DEPTH
    });

    GoArgs {
        max_depth,
        time_budget_ms,
    }
}

fn format_score(score: i32) -> String {
    if score.abs() >= MATE_SCORE - 1000 {
        let ply_to_mate = MATE_SCORE - score.abs();
        let moves_to_mate = (ply_to_mate + 1) / 2;
        let signed = if score > 0 { moves_to_mate } else { -moves_to_mate };
        format!("mate {}", signed)
    } else {
        format!("cp {}", score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_startpos() {
        let b = parse_position("startpos").unwrap();
        let start = Board::parse(STARTPOS);
        for i in 0..12 {
            assert_eq!(b.bbs[i].board, start.bbs[i].board);
        }
    }

    #[test]
    fn position_startpos_with_moves() {
        let b = parse_position("startpos moves e2e4 e7e5").unwrap();
        // After 1.e4 e5 it's white to move again.
        use crate::torte::board::pieces::Color;
        assert_eq!(b.side_to_move, Color::White);
        assert_eq!(b.fullmove_number, 2);
    }

    #[test]
    fn position_fen_with_moves() {
        let b = parse_position(
            "fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves e2e4",
        )
        .unwrap();
        use crate::torte::board::pieces::Color;
        assert_eq!(b.side_to_move, Color::Black);
        assert_eq!(b.en_passant.unwrap().0, 20); // e3
    }

    #[test]
    fn go_depth_uci() {
        let g = parse_go("depth 5", Color::White);
        assert_eq!(g.max_depth, 5);
        assert_eq!(g.time_budget_ms, None);
    }

    #[test]
    fn go_depth_overrides_time() {
        // Explicit `depth N` wins over time arguments.
        let g = parse_go("wtime 30000 btime 30000 depth 7", Color::White);
        assert_eq!(g.max_depth, 7);
    }

    #[test]
    fn go_repl_fallback_integer_is_depth() {
        let g = parse_go("3", Color::White);
        assert_eq!(g.max_depth, 3);
        assert_eq!(g.time_budget_ms, None);
    }

    #[test]
    fn go_default_with_no_args() {
        let g = parse_go("", Color::White);
        assert_eq!(g.max_depth, DEFAULT_DEPTH);
        assert_eq!(g.time_budget_ms, None);
    }

    #[test]
    fn go_movetime_sets_budget_and_high_depth() {
        let g = parse_go("movetime 1500", Color::White);
        assert_eq!(g.max_depth, MAX_DEPTH_TIMED);
        assert_eq!(g.time_budget_ms, Some(1500));
    }

    #[test]
    fn go_wtime_btime_picks_side() {
        // White to move with 60s on each clock -> 60000/30 = 2000ms.
        let gw = parse_go("wtime 60000 btime 30000", Color::White);
        assert_eq!(gw.time_budget_ms, Some(2000));
        // Black to move with same args -> 30000/30 = 1000ms.
        let gb = parse_go("wtime 60000 btime 30000", Color::Black);
        assert_eq!(gb.time_budget_ms, Some(1000));
    }

    #[test]
    fn go_wtime_btime_includes_half_increment() {
        // 60000ms + 2000ms increment -> 60000/30 + 2000/2 = 2000 + 1000 = 3000.
        let g = parse_go("wtime 60000 btime 60000 winc 2000 binc 2000", Color::White);
        assert_eq!(g.time_budget_ms, Some(3000));
    }

    #[test]
    fn format_cp_score() {
        assert_eq!(format_score(123), "cp 123");
        assert_eq!(format_score(-456), "cp -456");
    }

    fn setopt(args: &str, config: &mut SearchConfig) {
        let mut tt = TranspositionTable::new(1);
        apply_setoption(args, config, &mut tt);
    }

    #[test]
    fn setoption_toggles_move_ordering() {
        let mut config = SearchConfig::default();
        assert!(config.move_ordering);
        setopt("name MoveOrdering value false", &mut config);
        assert!(!config.move_ordering);
        setopt("name MoveOrdering value true", &mut config);
        assert!(config.move_ordering);
    }

    #[test]
    fn setoption_accepts_alternate_bools() {
        let mut config = SearchConfig::default();
        setopt("name MoveOrdering value off", &mut config);
        assert!(!config.move_ordering);
        setopt("name MoveOrdering value on", &mut config);
        assert!(config.move_ordering);
        setopt("name MoveOrdering value 0", &mut config);
        assert!(!config.move_ordering);
    }

    #[test]
    fn setoption_toggles_quiescence() {
        let mut config = SearchConfig::default();
        assert!(config.quiescence);
        setopt("name Quiescence value false", &mut config);
        assert!(!config.quiescence);
        setopt("name Quiescence value true", &mut config);
        assert!(config.quiescence);
    }

    #[test]
    fn setoption_toggles_iterative_deepening() {
        let mut config = SearchConfig::default();
        assert!(config.iterative_deepening);
        setopt("name IterativeDeepening value false", &mut config);
        assert!(!config.iterative_deepening);
        setopt("name IterativeDeepening value true", &mut config);
        assert!(config.iterative_deepening);
    }

    #[test]
    fn setoption_toggles_transposition_table() {
        let mut config = SearchConfig::default();
        assert!(config.transposition_table);
        setopt("name TranspositionTable value false", &mut config);
        assert!(!config.transposition_table);
        setopt("name TranspositionTable value true", &mut config);
        assert!(config.transposition_table);
    }

    #[test]
    fn setoption_toggles_piece_square_tables() {
        let mut config = SearchConfig::default();
        assert!(config.piece_square_tables);
        setopt("name PieceSquareTables value false", &mut config);
        assert!(!config.piece_square_tables);
        setopt("name PieceSquareTables value true", &mut config);
        assert!(config.piece_square_tables);
    }

    #[test]
    fn setoption_toggles_pawn_structure() {
        let mut config = SearchConfig::default();
        assert!(config.pawn_structure);
        setopt("name PawnStructure value false", &mut config);
        assert!(!config.pawn_structure);
        setopt("name PawnStructure value true", &mut config);
        assert!(config.pawn_structure);
    }

    #[test]
    fn setoption_toggles_bishop_pair() {
        let mut config = SearchConfig::default();
        assert!(config.bishop_pair);
        setopt("name BishopPair value false", &mut config);
        assert!(!config.bishop_pair);
        setopt("name BishopPair value true", &mut config);
        assert!(config.bishop_pair);
    }

    #[test]
    fn setoption_toggles_king_safety() {
        let mut config = SearchConfig::default();
        assert!(config.king_safety);
        setopt("name KingSafety value false", &mut config);
        assert!(!config.king_safety);
        setopt("name KingSafety value true", &mut config);
        assert!(config.king_safety);
    }

    #[test]
    fn setoption_toggles_killer_moves() {
        let mut config = SearchConfig::default();
        assert!(config.killer_moves);
        setopt("name KillerMoves value false", &mut config);
        assert!(!config.killer_moves);
        setopt("name KillerMoves value true", &mut config);
        assert!(config.killer_moves);
    }

    #[test]
    fn setoption_toggles_history_heuristic() {
        let mut config = SearchConfig::default();
        assert!(config.history_heuristic);
        setopt("name HistoryHeuristic value false", &mut config);
        assert!(!config.history_heuristic);
        setopt("name HistoryHeuristic value true", &mut config);
        assert!(config.history_heuristic);
    }

    #[test]
    fn setoption_toggles_mid_search_abort() {
        let mut config = SearchConfig::default();
        assert!(config.mid_search_abort);
        setopt("name MidSearchAbort value false", &mut config);
        assert!(!config.mid_search_abort);
        setopt("name MidSearchAbort value true", &mut config);
        assert!(config.mid_search_abort);
    }

    #[test]
    fn setoption_toggles_null_move_pruning() {
        let mut config = SearchConfig::default();
        assert!(config.null_move_pruning);
        setopt("name NullMovePruning value false", &mut config);
        assert!(!config.null_move_pruning);
        setopt("name NullMovePruning value true", &mut config);
        assert!(config.null_move_pruning);
    }

    #[test]
    fn setoption_toggles_aspiration_windows() {
        let mut config = SearchConfig::default();
        assert!(config.aspiration_windows);
        setopt("name AspirationWindows value false", &mut config);
        assert!(!config.aspiration_windows);
        setopt("name AspirationWindows value true", &mut config);
        assert!(config.aspiration_windows);
    }

    #[test]
    fn setoption_toggles_razoring() {
        let mut config = SearchConfig::default();
        // Razoring is experimental and defaults off — see SearchConfig::default.
        assert!(!config.razoring);
        setopt("name Razoring value true", &mut config);
        assert!(config.razoring);
        setopt("name Razoring value false", &mut config);
        assert!(!config.razoring);
    }

    #[test]
    fn setoption_toggles_futility_pruning() {
        let mut config = SearchConfig::default();
        assert!(config.futility_pruning);
        setopt("name FutilityPruning value false", &mut config);
        assert!(!config.futility_pruning);
        setopt("name FutilityPruning value true", &mut config);
        assert!(config.futility_pruning);
    }

    #[test]
    fn setoption_toggles_late_move_reductions() {
        let mut config = SearchConfig::default();
        assert!(config.late_move_reductions);
        setopt("name LateMoveReductions value false", &mut config);
        assert!(!config.late_move_reductions);
        setopt("name LateMoveReductions value true", &mut config);
        assert!(config.late_move_reductions);
    }

    #[test]
    fn setoption_unknown_is_noop() {
        let mut config = SearchConfig::default();
        setopt("name SomethingElse value true", &mut config);
        // No panic, default unchanged.
        assert!(config.move_ordering);
    }

    #[test]
    fn setoption_hash_resizes_tt() {
        let mut config = SearchConfig::default();
        let mut tt = TranspositionTable::new(1);
        let small_cap = tt.capacity();
        apply_setoption("name Hash value 64", &mut config, &mut tt);
        assert!(tt.capacity() > small_cap);
    }

    #[test]
    fn setoption_hash_clamps_to_range() {
        let mut config = SearchConfig::default();
        let mut tt = TranspositionTable::new(16);
        let baseline = tt.capacity();
        // Value above max should still produce a valid resize at max.
        apply_setoption("name Hash value 99999", &mut config, &mut tt);
        assert!(tt.capacity() >= baseline);
    }

    #[test]
    fn format_mate_score() {
        // MATE_SCORE - 1 = mate in 1 ply -> mate 1
        assert_eq!(format_score(MATE_SCORE - 1), "mate 1");
        // MATE_SCORE - 3 = mate in 3 ply -> mate 2 (rounded up)
        assert_eq!(format_score(MATE_SCORE - 3), "mate 2");
        // Negative: being mated.
        assert_eq!(format_score(-(MATE_SCORE - 1)), "mate -1");
    }
}
