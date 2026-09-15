use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::sq::SQ;
use crate::torte::movegen::attacks::knight_attacks;
use crate::torte::movegen::magic::{bishop_attacks, queen_attacks, rook_attacks};
use crate::torte::search::params::W;

// Nominal piece values for the *search* (MVV-LVA ordering, pruning margins).
// The eval's own material weights live in `params::W` and are tuned.
pub const PAWN: i32 = 100;
pub const KNIGHT: i32 = 320;
pub const BISHOP: i32 = 330;
pub const ROOK: i32 = 500;
pub const QUEEN: i32 = 900;

pub const PIECE_VALUES: [i32; 6] = [PAWN, KNIGHT, BISHOP, ROOK, QUEEN, 0];

// Every eval weight is a [middlegame, endgame] pair in `params::W`, found at
// one of these offsets. Each term reports "weight i, counted n times,
// white-minus-black" to a `Trace`; the real eval sums weight × count, and the
// tuner (`tune.rs`) records the counts instead. The final score is a single
// phase-weighted lerp of the MG and EG sums.
pub const MATERIAL: usize = 0; // P N B R Q K
pub const PST: usize = MATERIAL + 6; // 6 × 64, white's view, a1 = 0; black mirrors via sq ^ 56
pub const MOBILITY: usize = PST + 6 * 64; // N B R Q, per attacked non-own square
pub const DOUBLED_PAWN: usize = MOBILITY + 4; // per extra pawn on a file
pub const ISOLATED_PAWN: usize = DOUBLED_PAWN + 1;
pub const PASSED_PAWN: usize = ISOLATED_PAWN + 1; // by rank from own side, 0..8
pub const BISHOP_PAIR: usize = PASSED_PAWN + 8;
pub const SHIELD_PAWN_HOME: usize = BISHOP_PAIR + 1;
pub const SHIELD_PAWN_ADVANCED: usize = SHIELD_PAWN_HOME + 1;
pub const UNDEVELOPED_MINOR: usize = SHIELD_PAWN_ADVANCED + 1;
pub const EARLY_QUEEN: usize = UNDEVELOPED_MINOR + 1; // per undeveloped minor
pub const TEMPO: usize = EARLY_QUEEN + 1;
pub const NUM_PARAMS: usize = TEMPO + 1;

/// Receives eval terms as (weight index, white-minus-black count).
pub trait Trace {
    fn add(&mut self, param: usize, n: i32);
}

/// The real accumulator: MG and EG sums of weight × count.
#[derive(Default)]
pub struct Score {
    pub mg: i32,
    pub eg: i32,
}

impl Score {
    pub fn taper(&self, phase: i32) -> i32 {
        (self.mg * phase + self.eg * (PHASE_MAX - phase)) / PHASE_MAX
    }
}

impl Trace for Score {
    #[inline]
    fn add(&mut self, param: usize, n: i32) {
        self.mg += W[param][0] * n;
        self.eg += W[param][1] * n;
    }
}

// Game-phase weights (Fruit-style). Sum at startpos = 4*(N+B) + 2*R + 4*Q
// across both colours = 4 + 4 + 8 + 8 = 24 = `PHASE_MAX`. A pure pawn endgame
// has phase 0. Pawns and kings don't contribute to phase.
const PHASE_KNIGHT: i32 = 1;
const PHASE_BISHOP: i32 = 1;
const PHASE_ROOK: i32 = 2;
const PHASE_QUEEN: i32 = 4;
pub const PHASE_MAX: i32 = 24;

pub fn game_phase(board: &Board) -> i32 {
    let mut phase = 0_i32;
    // bbs layout: 0..5 white P/N/B/R/Q/K, 6..11 black P/N/B/R/Q/K
    phase += (board.bbs[1].board.count_ones() + board.bbs[7].board.count_ones()) as i32 * PHASE_KNIGHT;
    phase += (board.bbs[2].board.count_ones() + board.bbs[8].board.count_ones()) as i32 * PHASE_BISHOP;
    phase += (board.bbs[3].board.count_ones() + board.bbs[9].board.count_ones()) as i32 * PHASE_ROOK;
    phase += (board.bbs[4].board.count_ones() + board.bbs[10].board.count_ones()) as i32 * PHASE_QUEEN;
    phase.min(PHASE_MAX)
}

/// Mobility: for each knight, bishop, rook and queen, the squares it attacks
/// excluding own-piece squares.
fn mobility<T: Trace>(board: &Board, t: &mut T) {
    let occ = Bitboard::from_u64(board.player_bbs[0].board | board.player_bbs[1].board);
    for (knight_kind, own, sign) in [
        (1, board.player_bbs[0].board, 1),
        (7, board.player_bbs[1].board, -1),
    ] {
        // knight_kind is the knight slot (1 or 7); the next three are bishop,
        // rook, queen in our bbs layout.
        for piece in 0..4 {
            let mut bb = board.bbs[knight_kind + piece].board;
            while bb != 0 {
                let sq = SQ(bb.trailing_zeros() as u8);
                let attacks = match piece {
                    0 => knight_attacks(sq),
                    1 => bishop_attacks(sq, occ),
                    2 => rook_attacks(sq, occ),
                    _ => queen_attacks(sq, occ),
                };
                t.add(MOBILITY + piece, sign * (attacks.board & !own).count_ones() as i32);
                bb &= bb - 1;
            }
        }
    }
}

const FILE_A_BB: u64 = 0x0101010101010101;

fn file_bb(file: u32) -> u64 {
    FILE_A_BB << file
}

fn adjacent_files_bb(file: u32) -> u64 {
    let mut mask = 0_u64;
    if file > 0 {
        mask |= FILE_A_BB << (file - 1);
    }
    if file < 7 {
        mask |= FILE_A_BB << (file + 1);
    }
    mask
}

/// Pawn structure: doubled (per extra pawn on a file), isolated (no friendly
/// pawn on either adjacent file), and passed (no enemy pawn on the same or
/// adjacent file ahead), the last indexed by rank from the pawn's own side.
fn pawn_structure<T: Trace>(board: &Board, t: &mut T) {
    let white_pawns = board.bbs[0].board;
    let black_pawns = board.bbs[6].board;

    for (pawns, enemy, white, sign) in
        [(white_pawns, black_pawns, true, 1), (black_pawns, white_pawns, false, -1)]
    {
        for f in 0..8 {
            let count = (pawns & file_bb(f)).count_ones() as i32;
            if count > 1 {
                t.add(DOUBLED_PAWN, sign * (count - 1));
            }
        }
        let mut bb = pawns;
        while bb != 0 {
            let sq = bb.trailing_zeros();
            bb &= bb - 1;
            let file = sq % 8;
            let rank = sq / 8;
            if pawns & adjacent_files_bb(file) == 0 {
                t.add(ISOLATED_PAWN, sign);
            }
            let front = if white {
                if rank == 7 { 0 } else { !0_u64 << ((rank + 1) * 8) }
            } else if rank == 0 {
                0
            } else {
                (1_u64 << (rank * 8)) - 1
            };
            if (file_bb(file) | adjacent_files_bb(file)) & front & enemy == 0 {
                let own_rank = if white { rank } else { 7 - rank } as usize;
                t.add(PASSED_PAWN + own_rank, sign);
            }
        }
    }
}

/// Home squares of the pieces the development term cares about, as White
/// bitboards. Black's are the same masks byte-swapped (rank mirror).
const KNIGHT_HOME_BB: u64 = 0x0000_0000_0000_0042; // b1, g1
const BISHOP_HOME_BB: u64 = 0x0000_0000_0000_0024; // c1, f1
const QUEEN_HOME_BB: u64 = 0x0000_0000_0000_0008; // d1

/// Development: minors still on their home squares, plus an extra charge per
/// undeveloped minor once the queen has left home — the early-queen sortie
/// the ladder games showed.
fn development<T: Trace>(board: &Board, t: &mut T) {
    for (color, sign) in [(Color::White, 1), (Color::Black, -1)] {
        let (knights, bishops, queens, mirror) = match color {
            Color::White => (board.bbs[1], board.bbs[2], board.bbs[4], false),
            Color::Black => (board.bbs[7], board.bbs[8], board.bbs[10], true),
        };
        let home = |mask: u64| if mirror { mask.swap_bytes() } else { mask };

        let undeveloped = (knights.board & home(KNIGHT_HOME_BB)).count_ones() as i32
            + (bishops.board & home(BISHOP_HOME_BB)).count_ones() as i32;
        t.add(UNDEVELOPED_MINOR, sign * undeveloped);

        // "Early" queen = off her home square while at least two minors are
        // still asleep. One developed minor is a normal move order (1. e4 Nf6
        // 2. Qe2); three is a Scholar's-mate impression.
        let queen_out = queens.board != 0 && (queens.board & home(QUEEN_HOME_BB)) == 0;
        if queen_out && undeveloped >= 2 {
            t.add(EARLY_QUEEN, sign * undeveloped);
        }
    }
}

/// Bishop pair: one count per side holding two or more bishops.
fn bishop_pair<T: Trace>(board: &Board, t: &mut T) {
    let n = (board.bbs[2].board.count_ones() >= 2) as i32
        - (board.bbs[8].board.count_ones() >= 2) as i32;
    t.add(BISHOP_PAIR, n);
}

/// King-safety pawn shield for a king still on its home rank (plausibly
/// castled): friendly pawns on the three files around it, split into "still
/// on the 2nd rank" and "advanced one square".
fn king_safety<T: Trace>(board: &Board, t: &mut T) {
    for (king_bb, pawns, white, sign) in [
        (board.bbs[5].board, board.bbs[0].board, true, 1),
        (board.bbs[11].board, board.bbs[6].board, false, -1),
    ] {
        if king_bb == 0 {
            continue;
        }
        let ksq = king_bb.trailing_zeros();
        let home_rank = if white { 0 } else { 7 };
        if ksq / 8 != home_rank {
            continue;
        }
        let kfile = ksq % 8;
        let (home_pawn_rank, advanced_pawn_rank) = if white { (1, 2) } else { (6, 5) };
        for f in kfile.saturating_sub(1)..=(kfile + 1).min(7) {
            if pawns & (1_u64 << (home_pawn_rank * 8 + f)) != 0 {
                t.add(SHIELD_PAWN_HOME, sign);
            } else if pawns & (1_u64 << (advanced_pawn_rank * 8 + f)) != 0 {
                t.add(SHIELD_PAWN_ADVANCED, sign);
            }
        }
    }
}

/// Which evaluation terms are active. Defined here (not in `search`) so `eval`
/// stays decoupled from `SearchConfig` — `search` builds one of these from its
/// own config. All-false is a pure material eval.
#[derive(Clone, Copy, Debug)]
pub struct EvalConfig {
    /// Tapered piece-square tables + mobility.
    pub piece_square_tables: bool,
    /// Doubled / isolated / passed-pawn terms.
    pub pawn_structure: bool,
    /// Bonus for holding both bishops.
    pub bishop_pair: bool,
    /// Pawn-shield bonus for a castled king.
    pub king_safety: bool,
    /// Undeveloped-minor / early-queen penalties plus the tempo bonus.
    pub development: bool,
}

impl EvalConfig {
    /// Material only — every positional term off.
    pub fn material_only() -> Self {
        Self {
            piece_square_tables: false,
            pawn_structure: false,
            bishop_pair: false,
            king_safety: false,
            development: false,
        }
    }

    /// Every positional term on.
    pub fn all() -> Self {
        Self {
            development: true,
            piece_square_tables: true,
            pawn_structure: true,
            bishop_pair: true,
            king_safety: true,
        }
    }
}

/// Feed every active term to `t`, white-minus-black.
pub fn trace<T: Trace>(board: &Board, cfg: EvalConfig, t: &mut T) {
    for kind in 0..6 {
        for (bb, sign, mirror) in [(board.bbs[kind].board, 1, 0), (board.bbs[kind + 6].board, -1, 56)] {
            let mut bb = bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                t.add(MATERIAL + kind, sign);
                if cfg.piece_square_tables {
                    t.add(PST + kind * 64 + (sq ^ mirror), sign);
                }
                bb &= bb - 1;
            }
        }
    }
    if cfg.piece_square_tables {
        mobility(board, t);
    }
    if cfg.pawn_structure {
        pawn_structure(board, t);
    }
    if cfg.bishop_pair {
        bishop_pair(board, t);
    }
    if cfg.king_safety {
        king_safety(board, t);
    }
    if cfg.development {
        development(board, t);
        // Tempo belongs to whoever is on the move.
        t.add(TEMPO, if board.side_to_move == Color::White { 1 } else { -1 });
    }
}

/// Static evaluation from the side-to-move's perspective. With `EvalConfig::
/// material_only()` this is a pure material count; each `EvalConfig` flag
/// layers on its positional term.
pub fn eval(board: &Board, cfg: EvalConfig) -> i32 {
    let mut score = Score::default();
    trace(board, cfg, &mut score);
    let white = score.taper(game_phase(board));
    if board.side_to_move == Color::White {
        white
    } else {
        -white
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything except development — for tests asserting an exact symmetry
    /// that the side-to-move tempo bonus deliberately breaks.
    fn no_development() -> EvalConfig {
        EvalConfig { development: false, ..EvalConfig::all() }
    }

    /// Material eval plus only the pawn-structure term.
    fn pawn_only() -> EvalConfig {
        EvalConfig { pawn_structure: true, ..EvalConfig::material_only() }
    }

    /// Material eval plus only the king-safety term.
    fn king_safety_only() -> EvalConfig {
        EvalConfig { king_safety: true, ..EvalConfig::material_only() }
    }

    /// Material eval plus only the development term.
    fn development_only() -> EvalConfig {
        EvalConfig { development: true, ..EvalConfig::material_only() }
    }

    /// How many times `param` is counted (white-minus-black) — tests the
    /// terms' logic without pinning the tuned weights' signs.
    fn count(param: usize, board: &Board, cfg: EvalConfig) -> i32 {
        struct Count(usize, i32);
        impl Trace for Count {
            fn add(&mut self, p: usize, n: i32) {
                if p == self.0 {
                    self.1 += n;
                }
            }
        }
        let mut c = Count(param, 0);
        trace(board, cfg, &mut c);
        c.1
    }

    /// One term's white-minus-black contribution, tapered at `phase`.
    fn term(f: impl FnOnce(&mut Score), phase: i32) -> i32 {
        let mut s = Score::default();
        f(&mut s);
        s.taper(phase)
    }

    #[test]
    fn startpos_is_balanced_material_only() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board, EvalConfig::material_only()), 0);
    }

    #[test]
    fn startpos_is_balanced_with_pst() {
        // White and black are mirror-symmetric in the start position, so PST
        // contributions must cancel exactly. Development off: its tempo bonus
        // deliberately isn't symmetric.
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board, no_development()), 0);
    }

    #[test]
    fn extra_white_queen() {
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let queen = Score { mg: W[MATERIAL + 4][0], eg: W[MATERIAL + 4][1] }.taper(PHASE_QUEEN);
        assert_eq!(eval(&board, EvalConfig::material_only()), queen);
    }

    #[test]
    fn extra_white_queen_black_to_move() {
        let white = Board::parse("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let black = Board::parse("4k3/8/8/8/8/8/8/3QK3 b - - 0 1");
        assert_eq!(
            eval(&black, EvalConfig::material_only()),
            -eval(&white, EvalConfig::material_only())
        );
    }

    #[test]
    fn pst_prefers_central_knight_over_corner() {
        let center = Board::parse("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1");
        let corner = Board::parse("4k3/8/8/8/8/8/8/N3K3 w - - 0 1");
        assert!(eval(&center, EvalConfig::all()) > eval(&corner, EvalConfig::all()));
        assert_eq!(
            eval(&center, EvalConfig::material_only()),
            eval(&corner, EvalConfig::material_only())
        );
    }

    #[test]
    fn game_phase_at_startpos_is_max() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(game_phase(&board), PHASE_MAX);
    }

    #[test]
    fn game_phase_at_pure_king_pawn_is_zero() {
        let board = Board::parse("4k3/pppppppp/8/8/8/8/PPPPPPPP/4K3 w - - 0 1");
        assert_eq!(game_phase(&board), 0);
    }

    #[test]
    fn endgame_king_prefers_center_over_corner() {
        let center = Board::parse("8/8/4k3/8/4K3/8/4P3/8 w - - 0 1");
        let corner = Board::parse("8/8/4k3/8/8/8/4P3/K7 w - - 0 1");
        assert!(eval(&center, EvalConfig::all()) > eval(&corner, EvalConfig::all()));
        assert_eq!(
            eval(&center, EvalConfig::material_only()),
            eval(&corner, EvalConfig::material_only())
        );
    }

    #[test]
    fn tapered_pawn_prefers_advanced_pawn_in_endgame() {
        let advanced = Board::parse("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let starting = Board::parse("4k3/8/8/8/8/8/P7/4K3 w - - 0 1");
        assert!(eval(&advanced, EvalConfig::all()) > eval(&starting, EvalConfig::all()));
    }

    #[test]
    fn tapered_rook_prefers_seventh_rank_in_endgame() {
        let seventh = Board::parse("4k3/R7/8/8/8/8/8/4K3 w - - 0 1");
        let first = Board::parse("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
        assert!(eval(&seventh, EvalConfig::all()) > eval(&first, EvalConfig::all()));
    }

    #[test]
    fn mobility_prefers_open_bishop_over_blocked_bishop() {
        let open = Board::parse("4k3/8/8/8/2B5/8/8/4K3 w - - 0 1");
        let blocked = Board::parse("4k3/8/8/8/8/8/1P1P4/2B1K3 w - - 0 1");
        let m_open = term(|t| mobility(&open, t), PHASE_MAX);
        let m_blocked = term(|t| mobility(&blocked, t), PHASE_MAX);
        assert!(m_open > m_blocked, "open bishop mobility {} should exceed blocked {}", m_open, m_blocked);
    }

    #[test]
    fn mobility_is_zero_at_startpos() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(term(|t| mobility(&board, t), PHASE_MAX), 0);
    }

    #[test]
    fn pst_mirrors_for_black_pieces() {
        // White knight on d1 vs black knight on d8: exact negatives.
        let board = Board::parse("4k3/8/8/8/8/8/8/3NK3 w - - 0 1");
        let mirrored = Board::parse("3nk3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(eval(&board, no_development()), -eval(&mirrored, no_development()));
    }

    #[test]
    fn development_counts_minors_at_home() {
        let developed = Board::parse(
            "rnbqkbnr/pppppppp/8/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1",
        );
        let undeveloped =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        // White-minus-black: white has two fewer minors at home.
        assert_eq!(count(UNDEVELOPED_MINOR, &developed, development_only()), -2);
        assert_eq!(count(UNDEVELOPED_MINOR, &undeveloped, development_only()), 0);
    }

    #[test]
    fn development_flags_the_early_queen() {
        // White's queen is on b3 with every minor still at home.
        let queen_out =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/1Q6/PPPPPPPP/RNB1KBNR w KQkq - 0 1");
        let queen_home =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(count(EARLY_QUEEN, &queen_out, development_only()), 4);
        assert_eq!(count(EARLY_QUEEN, &queen_home, development_only()), 0);
    }

    #[test]
    fn tempo_belongs_to_the_side_to_move() {
        // Identical position, opposite side to move: each side sees +tempo.
        let white = Board::parse("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let black = Board::parse("4k3/8/8/8/8/8/8/4K3 b - - 0 1");
        assert_eq!(eval(&white, development_only()), W[TEMPO][1]);
        assert_eq!(eval(&black, development_only()), W[TEMPO][1]);
    }

    #[test]
    fn pawn_structure_penalizes_doubled_pawns() {
        let doubled = Board::parse("4k3/4p3/8/8/8/3P4/3P4/4K3 w - - 0 1");
        let spread = Board::parse("4k3/4p3/8/8/8/3P4/4P3/4K3 w - - 0 1");
        assert!(eval(&spread, pawn_only()) > eval(&doubled, pawn_only()));
    }

    #[test]
    fn pawn_structure_penalizes_isolated_pawns() {
        let isolated = Board::parse("4k3/8/8/8/8/8/3P1P2/4K3 w - - 0 1");
        let connected = Board::parse("4k3/8/8/8/8/8/3PP3/4K3 w - - 0 1");
        assert!(eval(&connected, pawn_only()) > eval(&isolated, pawn_only()));
    }

    #[test]
    fn pawn_structure_rewards_passed_pawn() {
        let blocked = Board::parse("4k3/3p4/8/3P4/8/8/8/4K3 w - - 0 1");
        let passer = Board::parse("4k3/p7/8/3P4/8/8/8/4K3 w - - 0 1");
        assert!(eval(&passer, pawn_only()) > eval(&blocked, pawn_only()));
    }

    #[test]
    fn passed_pawn_bonus_amplified_in_endgame() {
        let board = Board::parse("4k3/8/8/3P4/8/8/8/4K3 w - - 0 1");
        let eg = term(|t| pawn_structure(&board, t), 0);
        let mg = term(|t| pawn_structure(&board, t), PHASE_MAX);
        assert!(eg > mg, "passer in EG ({}) should beat MG ({})", eg, mg);
    }

    #[test]
    fn pawn_structure_toggle_changes_score() {
        let doubled = Board::parse("4k3/4p3/8/8/8/3P4/3P4/4K3 w - - 0 1");
        assert_ne!(
            eval(&doubled, pawn_only()),
            eval(&doubled, EvalConfig::material_only())
        );
    }

    #[test]
    fn bishop_pair_detects_two_bishops() {
        let pair = W[BISHOP_PAIR][0];
        let white_pair = Board::parse("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        assert_eq!(term(|t| bishop_pair(&white_pair, t), PHASE_MAX), pair);
        let black_pair = Board::parse("2b1kb2/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(term(|t| bishop_pair(&black_pair, t), PHASE_MAX), -pair);
        let one_each = Board::parse("4kb2/8/8/8/8/8/8/2B1K3 w - - 0 1");
        assert_eq!(term(|t| bishop_pair(&one_each, t), PHASE_MAX), 0);
    }

    #[test]
    fn king_safety_rewards_intact_pawn_shield() {
        let sheltered = Board::parse("4k3/8/8/8/8/8/5PPP/6K1 w - - 0 1");
        let exposed = Board::parse("4k3/8/8/8/8/8/8/6K1 w - - 0 1");
        assert!(
            term(|t| king_safety(&sheltered, t), PHASE_MAX)
                > term(|t| king_safety(&exposed, t), PHASE_MAX)
        );
    }

    #[test]
    fn king_safety_ignores_king_off_home_rank() {
        let marched = Board::parse("4k3/8/8/8/8/5PPP/6K1/8 w - - 0 1");
        assert_eq!(term(|t| king_safety(&marched, t), PHASE_MAX), 0);
    }

    #[test]
    fn king_safety_counts_the_shield() {
        let board = Board::parse("r3k3/8/8/8/8/8/5PPP/R5K1 w - - 0 1");
        assert_eq!(count(SHIELD_PAWN_HOME, &board, king_safety_only()), 3);
        assert_eq!(count(SHIELD_PAWN_HOME, &board, EvalConfig::material_only()), 0);
    }
}
