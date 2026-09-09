use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::sq::SQ;
use crate::torte::movegen::attacks::knight_attacks;
use crate::torte::movegen::magic::{bishop_attacks, queen_attacks, rook_attacks};

pub const PAWN: i32 = 100;
pub const KNIGHT: i32 = 320;
pub const BISHOP: i32 = 330;
pub const ROOK: i32 = 500;
pub const QUEEN: i32 = 900;

pub const PIECE_VALUES: [i32; 6] = [PAWN, KNIGHT, BISHOP, ROOK, QUEEN, 0];

// Piece-square tables. Each entry is from white's perspective indexed so that
// index 0 = a1, index 63 = h8. Black pieces look up `PST[sq ^ 56]` to mirror
// the rank. Values are centipawns added to material; positive = better square.
//
// MG = middlegame, EG = endgame. The active value is a phase-weighted lerp;
// see `pst_value` and `game_phase`. Phase = PHASE_MAX at startpos and decays
// toward 0 as non-pawn pieces leave the board.

#[rustfmt::skip]
const PAWN_MG_PST: [i32; 64] = [
    // rank 1 (a1..h1) — pawns never here, all zero
      0,   0,   0,   0,   0,   0,   0,   0,
    // rank 2 — starting squares; central pawns slightly bad (blocking bishops)
      5,  10,  10, -20, -20,  10,  10,   5,
    // rank 3
      5,  -5, -10,   0,   0, -10,  -5,   5,
    // rank 4 — central pawns valuable
      0,   0,   0,  20,  20,   0,   0,   0,
    // rank 5
      5,   5,  10,  25,  25,  10,   5,   5,
    // rank 6
     10,  10,  20,  30,  30,  20,  10,  10,
    // rank 7 — near promotion
     50,  50,  50,  50,  50,  50,  50,  50,
    // rank 8 — would be promoted; never actually a pawn here
      0,   0,   0,   0,   0,   0,   0,   0,
];

// Endgame pawn table: advancement matters far more than central control.
// Pawns left on rank 2 are a liability; pawns on rank 7 are nearly queens.
#[rustfmt::skip]
const PAWN_EG_PST: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    -10, -10, -10, -10, -10, -10, -10, -10,
     -5,  -5,  -5,  -5,  -5,  -5,  -5,  -5,
      5,   5,   5,   5,   5,   5,   5,   5,
     20,  20,  20,  20,  20,  20,  20,  20,
     40,  40,  40,  40,  40,  40,  40,  40,
     80,  80,  80,  80,  80,  80,  80,  80,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const KNIGHT_MG_PST: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -40, -20,   0,   0,   0,   0, -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50,
];

// Endgame knights: still want the centre, but corner/edge is less catastrophic
// because there's less to defend and fewer pieces to coordinate with.
#[rustfmt::skip]
const KNIGHT_EG_PST: [i32; 64] = [
    -40, -30, -20, -20, -20, -20, -30, -40,
    -30, -10,   0,   0,   0,   0, -10, -30,
    -20,   0,  10,  15,  15,  10,   0, -20,
    -20,   5,  15,  20,  20,  15,   5, -20,
    -20,   0,  15,  20,  20,  15,   0, -20,
    -20,   5,  10,  15,  15,  10,   5, -20,
    -30, -10,   0,   5,   5,   0, -10, -30,
    -40, -30, -20, -20, -20, -20, -30, -40,
];

#[rustfmt::skip]
const BISHOP_MG_PST: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -20, -10, -10, -10, -10, -10, -10, -20,
];

// Endgame bishops: similar shape to MG, slightly less corner penalty —
// bishops on long diagonals matter more than corner avoidance once the
// board is open.
#[rustfmt::skip]
const BISHOP_EG_PST: [i32; 64] = [
    -10,  -5, -10, -10, -10, -10,  -5, -10,
     -5,   0,   0,   0,   0,   0,   0,  -5,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,   5,  10,  15,  15,  10,   5, -10,
    -10,   0,  10,  15,  15,  10,   0, -10,
    -10,   5,  10,  10,  10,  10,   5, -10,
     -5,   0,   0,   0,   0,   0,   0,  -5,
    -10,  -5, -10, -10, -10, -10,  -5, -10,
];

#[rustfmt::skip]
const ROOK_MG_PST: [i32; 64] = [
      0,   0,   0,   5,   5,   0,   0,   0,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
      5,  10,  10,  10,  10,  10,  10,   5,
      0,   0,   0,   0,   0,   0,   0,   0,
];

// Endgame rooks: 7th-rank bonus and central files matter more; back-rank
// penalty fades (the king is no longer there in EG).
#[rustfmt::skip]
const ROOK_EG_PST: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,
      0,   5,   5,   5,   5,   5,   5,   0,
      0,   5,   5,  10,  10,   5,   5,   0,
     10,  10,  10,  10,  10,  10,  10,  10,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const QUEEN_MG_PST: [i32; 64] = [
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -10,   5,   5,   5,   5,   5,   0, -10,
      0,   0,   5,   5,   5,   5,   0,  -5,
     -5,   0,   5,   5,   5,   5,   0,  -5,
    -10,   0,   5,   5,   5,   5,   0, -10,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -20, -10, -10,  -5,  -5, -10, -10, -20,
];

// Endgame queens: slightly more central preference and milder corner
// penalty than MG — there's less to attack near the home rank in EG.
#[rustfmt::skip]
const QUEEN_EG_PST: [i32; 64] = [
    -10,  -5,  -5,   0,   0,  -5,  -5, -10,
     -5,   5,   5,   5,   5,   5,   5,  -5,
     -5,   5,  10,  10,  10,  10,   5,  -5,
      0,   5,  10,  15,  15,  10,   5,   0,
      0,   5,  10,  15,  15,  10,   5,   0,
     -5,   5,  10,  10,  10,  10,   5,  -5,
     -5,   0,   5,   5,   5,   5,   0,  -5,
    -10,  -5,  -5,   0,   0,  -5,  -5, -10,
];

#[rustfmt::skip]
const KING_MG_PST: [i32; 64] = [
    // Castled positions (g1/c1, b1) on rank 1 are big positives;
    // center and rank 2 onwards are exposed.
     20,  30,  10,   0,   0,  10,  30,  20,
     20,  20,   0,   0,   0,   0,  20,  20,
    -10, -20, -20, -20, -20, -20, -20, -10,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
];

// Endgame king table: with few pieces left there's nothing to hide from, and
// the king is a fighting piece. Center is rewarded, edges/corners punished.
// Lerped against KING_MG_PST by game phase.
#[rustfmt::skip]
const KING_EG_PST: [i32; 64] = [
    -50, -30, -30, -30, -30, -30, -30, -50,
    -30, -30,   0,   0,   0,   0, -30, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -20, -10,   0,   0, -10, -20, -30,
    -50, -40, -30, -20, -20, -30, -40, -50,
];

// Game-phase weights (Fruit-style). Sum at startpos = 4*(N+B) + 2*R + 4*Q
// across both colours = 4 + 4 + 8 + 8 = 24 = `PHASE_MAX`. A pure pawn endgame
// has phase 0. Pawns and kings don't contribute to phase.
const PHASE_KNIGHT: i32 = 1;
const PHASE_BISHOP: i32 = 1;
const PHASE_ROOK: i32 = 2;
const PHASE_QUEEN: i32 = 4;
const PHASE_MAX: i32 = 24;

fn game_phase(board: &Board) -> i32 {
    let mut phase = 0_i32;
    // bbs layout: 0..5 white P/N/B/R/Q/K, 6..11 black P/N/B/R/Q/K
    phase += (board.bbs[1].board.count_ones() + board.bbs[7].board.count_ones()) as i32 * PHASE_KNIGHT;
    phase += (board.bbs[2].board.count_ones() + board.bbs[8].board.count_ones()) as i32 * PHASE_BISHOP;
    phase += (board.bbs[3].board.count_ones() + board.bbs[9].board.count_ones()) as i32 * PHASE_ROOK;
    phase += (board.bbs[4].board.count_ones() + board.bbs[10].board.count_ones()) as i32 * PHASE_QUEEN;
    phase.min(PHASE_MAX)
}

// Mobility weights (centipawns per square the piece can move to, excluding
// own-piece squares). Knights get the biggest coefficient because the
// difference between a 0-move and an 8-move knight is huge; queens get the
// smallest because they almost always have many moves so the coefficient
// would otherwise dominate.
const MOBILITY_KNIGHT: i32 = 4;
const MOBILITY_BISHOP: i32 = 3;
const MOBILITY_ROOK: i32 = 2;
const MOBILITY_QUEEN: i32 = 1;

/// Mobility eval: for each non-pawn, non-king piece, count the squares it
/// attacks excluding own-piece squares (we can't capture our own pieces).
/// Returns the white-minus-black mobility score in centipawns.
fn mobility(board: &Board) -> i32 {
    let occ = Bitboard::from_u64(board.player_bbs[0].board | board.player_bbs[1].board);
    let white_pieces = board.player_bbs[0].board;
    let black_pieces = board.player_bbs[1].board;

    let side_score = |knight_kind: usize, own: u64| -> i32 {
        let mut total = 0_i32;
        // knight_kind is the white knight slot (1 or 7); the next three are
        // bishop, rook, queen in our bbs layout.
        let mut bb = board.bbs[knight_kind].board;
        while bb != 0 {
            let sq = SQ(bb.trailing_zeros() as u8);
            total += MOBILITY_KNIGHT
                * (knight_attacks(sq).board & !own).count_ones() as i32;
            bb &= bb - 1;
        }
        let mut bb = board.bbs[knight_kind + 1].board;
        while bb != 0 {
            let sq = SQ(bb.trailing_zeros() as u8);
            total += MOBILITY_BISHOP
                * (bishop_attacks(sq, occ).board & !own).count_ones() as i32;
            bb &= bb - 1;
        }
        let mut bb = board.bbs[knight_kind + 2].board;
        while bb != 0 {
            let sq = SQ(bb.trailing_zeros() as u8);
            total += MOBILITY_ROOK
                * (rook_attacks(sq, occ).board & !own).count_ones() as i32;
            bb &= bb - 1;
        }
        let mut bb = board.bbs[knight_kind + 3].board;
        while bb != 0 {
            let sq = SQ(bb.trailing_zeros() as u8);
            total += MOBILITY_QUEEN
                * (queen_attacks(sq, occ).board & !own).count_ones() as i32;
            bb &= bb - 1;
        }
        total
    };

    side_score(1, white_pieces) - side_score(7, black_pieces)
}

// Pawn-structure weights (centipawns). Doubled and isolated pawns are
// liabilities; passed pawns are assets. The passed-pawn bonus is rank-scaled
// (an advanced passer is far more dangerous) and phase-scaled (a passer is
// worth roughly double in a pawn endgame, where there's nothing to stop it).
const DOUBLED_PAWN_PENALTY: i32 = 15;
const ISOLATED_PAWN_PENALTY: i32 = 15;
// Indexed by the pawn's rank *from its own side's perspective*: index 1 = own
// rank 2 (starting square), index 6 = one step from promotion. Indices 0 and 7
// can't occur (a pawn is never on its own back rank or the promotion rank).
const PASSED_PAWN_BONUS: [i32; 8] = [0, 5, 10, 20, 40, 70, 120, 0];

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

/// Pawn-structure eval: doubled (penalty per extra pawn on a file), isolated
/// (no friendly pawn on either adjacent file), and passed (no enemy pawn on
/// the same or adjacent file ahead). Returns the white-minus-black score in
/// centipawns. `phase` is the game phase (PHASE_MAX..0) used to amplify
/// passers in the endgame.
fn pawn_structure(board: &Board, phase: i32) -> i32 {
    let white_pawns = board.bbs[0].board;
    let black_pawns = board.bbs[6].board;

    let side = |pawns: u64, enemy: u64, white: bool| -> i32 {
        let mut score = 0_i32;
        // Doubled: penalty for each pawn beyond the first on a file.
        for f in 0..8 {
            let count = (pawns & file_bb(f)).count_ones() as i32;
            if count > 1 {
                score -= DOUBLED_PAWN_PENALTY * (count - 1);
            }
        }
        let mut bb = pawns;
        while bb != 0 {
            let sq = bb.trailing_zeros();
            bb &= bb - 1;
            let file = sq % 8;
            let rank = sq / 8;
            // Isolated: no friendly pawn on an adjacent file.
            if pawns & adjacent_files_bb(file) == 0 {
                score -= ISOLATED_PAWN_PENALTY;
            }
            // Passed: no enemy pawn on the same or adjacent file ahead of us.
            let front = if white {
                if rank == 7 { 0 } else { !0_u64 << ((rank + 1) * 8) }
            } else if rank == 0 {
                0
            } else {
                (1_u64 << (rank * 8)) - 1
            };
            if (file_bb(file) | adjacent_files_bb(file)) & front & enemy == 0 {
                let own_rank = if white { rank } else { 7 - rank } as usize;
                // 1x at full phase, scaling toward 2x as the board empties.
                score += PASSED_PAWN_BONUS[own_rank] * (2 * PHASE_MAX - phase)
                    / PHASE_MAX;
            }
        }
        score
    };

    side(white_pawns, black_pawns, true) - side(black_pawns, white_pawns, false)
}

/// Bonus in centipawns for holding both bishops. The pair covers both square
/// colours and is worth more than two knights in most positions, so it earns
/// a flat bump beyond the per-piece material values.
/// Home squares of the pieces the development term cares about, as White
/// bitboards. Black's are the same masks byte-swapped (rank mirror).
const KNIGHT_HOME_BB: u64 = 0x0000_0000_0000_0042; // b1, g1
const BISHOP_HOME_BB: u64 = 0x0000_0000_0000_0024; // c1, f1
const QUEEN_HOME_BB: u64 = 0x0000_0000_0000_0008; // d1

/// Penalty per minor piece still sitting on its home square.
const UNDEVELOPED_MINOR_PENALTY: i32 = 12;

/// Extra penalty per undeveloped minor when the queen has already left home.
/// This is what punishes the early-queen sortie: the queen picks up ~15-20 cp
/// of mobility and central PST by coming out on move 4, and nothing else in
/// the eval charges her for doing it before the pieces behind her.
const EARLY_QUEEN_PENALTY: i32 = 10;

/// Flat bonus for having the move. Tiny, but it stops the search from being
/// indifferent between two positions that differ only by a lost tempo.
const TEMPO_BONUS: i32 = 10;

/// Development penalty for one side, as a positive number to subtract.
/// Phase-scaled by the caller so it vanishes in the endgame, where a knight
/// on b1 is a normal square rather than a sign of a wasted opening.
fn development_penalty(board: &Board, color: Color) -> i32 {
    let (knights, bishops, queens, mirror) = match color {
        Color::White => (board.bbs[1], board.bbs[2], board.bbs[4], false),
        Color::Black => (board.bbs[7], board.bbs[8], board.bbs[10], true),
    };
    let home = |mask: u64| if mirror { mask.swap_bytes() } else { mask };

    let undeveloped = (knights.board & home(KNIGHT_HOME_BB)).count_ones() as i32
        + (bishops.board & home(BISHOP_HOME_BB)).count_ones() as i32;
    let mut penalty = undeveloped * UNDEVELOPED_MINOR_PENALTY;

    // "Early" queen = off her home square while at least two minors are still
    // asleep. One developed minor is a normal move order (1. e4 Nf6 2. Qe2);
    // three is a Scholar's-mate impression.
    let queen_out = queens.board != 0 && (queens.board & home(QUEEN_HOME_BB)) == 0;
    if queen_out && undeveloped >= 2 {
        penalty += undeveloped * EARLY_QUEEN_PENALTY;
    }
    penalty
}

/// White-positive development term: how much more developed White is than
/// Black, faded out towards the endgame.
fn development(board: &Board, phase: i32) -> i32 {
    let diff = development_penalty(board, Color::Black)
        - development_penalty(board, Color::White);
    diff * phase / PHASE_MAX
}

const BISHOP_PAIR_BONUS: i32 = 30;

/// Bishop-pair eval: `+BISHOP_PAIR_BONUS` if white has two or more bishops,
/// `-BISHOP_PAIR_BONUS` if black does. White-minus-black, in centipawns.
fn bishop_pair(board: &Board) -> i32 {
    let mut score = 0;
    if board.bbs[2].board.count_ones() >= 2 {
        score += BISHOP_PAIR_BONUS;
    }
    if board.bbs[8].board.count_ones() >= 2 {
        score -= BISHOP_PAIR_BONUS;
    }
    score
}

// King-safety pawn shield. A castled king wants friendly pawns on the three
// files around it: a pawn still on its 2nd rank (directly sheltering the king)
// is worth more than one that has advanced a square and left a gap behind it.
const SHIELD_PAWN_HOME: i32 = 12;
const SHIELD_PAWN_ADVANCED: i32 = 6;

/// King-safety eval: rewards an intact pawn shield in front of a king that is
/// still on its back rank (plausibly castled). White-minus-black, in
/// centipawns, and phase-scaled — the shield matters in the middlegame and
/// fades to nothing in the endgame, where the king should be active instead.
fn king_safety(board: &Board, phase: i32) -> i32 {
    let side = |king_bb: u64, pawns: u64, white: bool| -> i32 {
        if king_bb == 0 {
            return 0;
        }
        let ksq = king_bb.trailing_zeros();
        let krank = ksq / 8;
        let kfile = ksq % 8;
        // Only score a shield for a king still on its home rank — a king that
        // has marched up the board isn't sheltering behind pawns.
        let home_rank = if white { 0 } else { 7 };
        if krank != home_rank {
            return 0;
        }
        let (home_pawn_rank, advanced_pawn_rank) = if white { (1, 2) } else { (6, 5) };
        let lo = kfile.saturating_sub(1);
        let hi = (kfile + 1).min(7);
        let mut score = 0;
        for f in lo..=hi {
            if pawns & (1_u64 << (home_pawn_rank * 8 + f)) != 0 {
                score += SHIELD_PAWN_HOME;
            } else if pawns & (1_u64 << (advanced_pawn_rank * 8 + f)) != 0 {
                score += SHIELD_PAWN_ADVANCED;
            }
        }
        score
    };
    let raw = side(board.bbs[5].board, board.bbs[0].board, true)
        - side(board.bbs[11].board, board.bbs[6].board, false);
    raw * phase / PHASE_MAX
}

fn pst_value(kind: usize, sq: usize, phase: i32) -> i32 {
    let (mg, eg) = match kind {
        0 => (PAWN_MG_PST[sq], PAWN_EG_PST[sq]),
        1 => (KNIGHT_MG_PST[sq], KNIGHT_EG_PST[sq]),
        2 => (BISHOP_MG_PST[sq], BISHOP_EG_PST[sq]),
        3 => (ROOK_MG_PST[sq], ROOK_EG_PST[sq]),
        4 => (QUEEN_MG_PST[sq], QUEEN_EG_PST[sq]),
        5 => (KING_MG_PST[sq], KING_EG_PST[sq]),
        _ => return 0,
    };
    (mg * phase + eg * (PHASE_MAX - phase)) / PHASE_MAX
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
    /// Flat bonus for holding both bishops.
    pub bishop_pair: bool,
    /// Pawn-shield bonus for a castled king (phase-scaled).
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

/// Static evaluation from the side-to-move's perspective. With `EvalConfig::
/// material_only()` this is a pure material count; each `EvalConfig` flag
/// layers on its positional term. Decoupled from `SearchConfig` so eval can be
/// reused without pulling in the search module's types.
pub fn eval(board: &Board, cfg: EvalConfig) -> i32 {
    let mut score = 0;
    let needs_phase = cfg.piece_square_tables
        || cfg.pawn_structure
        || cfg.king_safety
        || cfg.development;
    let phase = if needs_phase { game_phase(board) } else { 0 };
    for kind in 0..6 {
        let mut bb = board.bbs[kind].board;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            score += PIECE_VALUES[kind];
            if cfg.piece_square_tables {
                score += pst_value(kind, sq, phase);
            }
            bb &= bb - 1;
        }
        let mut bb = board.bbs[kind + 6].board;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            score -= PIECE_VALUES[kind];
            if cfg.piece_square_tables {
                score -= pst_value(kind, sq ^ 56, phase);
            }
            bb &= bb - 1;
        }
    }
    if cfg.piece_square_tables {
        score += mobility(board);
    }
    if cfg.pawn_structure {
        score += pawn_structure(board, phase);
    }
    if cfg.bishop_pair {
        score += bishop_pair(board);
    }
    if cfg.king_safety {
        score += king_safety(board, phase);
    }
    if cfg.development {
        score += development(board, phase);
    }
    // Flip to the side-to-move's perspective, then add tempo — tempo always
    // belongs to whoever is on the move, so it goes on after the flip.
    let score = if board.side_to_move == Color::White {
        score
    } else {
        -score
    };
    if cfg.development {
        score + TEMPO_BONUS
    } else {
        score
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

    /// Material eval plus only the pawn-structure term — for the
    /// structure-specific tests that want to isolate it.
    fn pawn_only() -> EvalConfig {
        EvalConfig {
            pawn_structure: true,
            ..EvalConfig::material_only()
        }
    }

    /// Material eval plus only the bishop-pair term.
    fn bishop_pair_only() -> EvalConfig {
        EvalConfig {
            bishop_pair: true,
            ..EvalConfig::material_only()
        }
    }

    /// Material eval plus only the king-safety term.
    fn king_safety_only() -> EvalConfig {
        EvalConfig {
            king_safety: true,
            ..EvalConfig::material_only()
        }
    }

    #[test]
    fn startpos_is_balanced_material_only() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board, EvalConfig::material_only()), 0);
    }

    #[test]
    fn startpos_is_balanced_with_pst() {
        // White and black are mirror-symmetric in the start position, so PST
        // contributions must cancel exactly.
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        // Development off: the tempo bonus inside it is deliberately *not*
        // symmetric (it belongs to whoever is on the move), and this test is
        // about the PST halves cancelling.
        assert_eq!(eval(&board, no_development()), 0);
    }

    #[test]
    fn extra_white_queen() {
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        assert_eq!(eval(&board, EvalConfig::material_only()), QUEEN);
    }

    #[test]
    fn extra_white_queen_black_to_move() {
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 b - - 0 1");
        assert_eq!(eval(&board, EvalConfig::material_only()), -QUEEN);
    }

    #[test]
    fn pst_prefers_central_knight_over_corner() {
        let center = Board::parse("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1");
        let corner = Board::parse("4k3/8/8/8/8/8/8/N3K3 w - - 0 1");
        // Same material; with PST on, central knight should score strictly higher.
        assert!(eval(&center, EvalConfig::all()) > eval(&corner, EvalConfig::all()));
        // With PST off the positions are identical material-wise.
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
        // Only kings and pawns: no phase contribution.
        let board = Board::parse("4k3/pppppppp/8/8/8/8/PPPPPPPP/4K3 w - - 0 1");
        assert_eq!(game_phase(&board), 0);
    }

    #[test]
    fn endgame_king_prefers_center_over_corner() {
        // K+P vs k endgame. With tapered king PST, the white king on the
        // center should evaluate strictly better than the white king in the
        // corner — same material, but only positional difference is the king.
        let center = Board::parse("8/8/4k3/8/4K3/8/4P3/8 w - - 0 1");
        let corner = Board::parse("8/8/4k3/8/8/8/4P3/K7 w - - 0 1");
        assert!(eval(&center, EvalConfig::all()) > eval(&corner, EvalConfig::all()));
        // Material-only: identical.
        assert_eq!(
            eval(&center, EvalConfig::material_only()),
            eval(&corner, EvalConfig::material_only())
        );
    }

    #[test]
    fn tapered_pawn_prefers_advanced_pawn_in_endgame() {
        // Pure-pawn endgame (phase=0). A white pawn near promotion (a7) must
        // score strictly higher than a pawn still on its starting square (a2).
        let advanced = Board::parse("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let starting = Board::parse("4k3/8/8/8/8/8/P7/4K3 w - - 0 1");
        assert!(eval(&advanced, EvalConfig::all()) > eval(&starting, EvalConfig::all()));
    }

    #[test]
    fn tapered_rook_prefers_seventh_rank_in_endgame() {
        // Endgame R+K vs k. A rook on the 7th rank should score strictly
        // higher than the same rook back-ranked, with the new EG table.
        let seventh = Board::parse("4k3/R7/8/8/8/8/8/4K3 w - - 0 1");
        let first = Board::parse("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
        assert!(eval(&seventh, EvalConfig::all()) > eval(&first, EvalConfig::all()));
    }

    #[test]
    fn mobility_prefers_open_bishop_over_blocked_bishop() {
        // Same material on both sides; only the white bishop differs.
        // Open: bishop on c4 with no blockers, 11 squares of mobility.
        // Blocked: bishop on c1 with pawns on b2 and d2 blocking both diagonals.
        // The open position should score strictly higher with PST on (mobility),
        // and identical with PST off (material-only).
        let open = Board::parse("4k3/8/8/8/2B5/8/8/4K3 w - - 0 1");
        let blocked = Board::parse("4k3/8/8/8/8/8/1P1P4/2B1K3 w - - 0 1");
        // Subtract pawn material from blocked side so material-only equals.
        // Easier: just check mobility() in isolation matches the expectation,
        // and that eval(true) - eval(false) reflects positional difference.
        let m_open = mobility(&open);
        let m_blocked = mobility(&blocked);
        assert!(m_open > m_blocked, "open bishop mobility {} should exceed blocked {}", m_open, m_blocked);
    }

    #[test]
    fn mobility_is_zero_at_startpos() {
        // Symmetric starting position: white and black mobility are mirror
        // images, so the white-minus-black total must be zero.
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(mobility(&board), 0);
    }

    #[test]
    fn pst_mirrors_for_black_pieces() {
        // A symmetric setup (white knight on e1, black knight on e8) should
        // evaluate to 0 with PST enabled. e1 = b'a..h'[4] + rank 1, e8 = mirror.
        let board = Board::parse("4k3/8/8/8/8/8/8/3NK3 w - - 0 1");
        let mirrored = Board::parse("3nk3/8/8/8/8/8/8/4K3 w - - 0 1");
        // White knight on d1 only.
        let v1 = eval(&board, no_development());
        // Black knight on d8 only.
        let v2 = eval(&mirrored, no_development());
        // The boards have one knight each on mirror-image squares (different
        // colours, same relative square). Their PST contributions should be
        // exact negatives of each other (white knight side > 0, black knight
        // side < 0, mirrored across the rank axis).
        assert_eq!(v1, -v2);
    }

    /// Material eval plus only the development term.
    fn development_only() -> EvalConfig {
        EvalConfig { development: true, ..EvalConfig::material_only() }
    }

    #[test]
    fn development_rewards_getting_minors_out() {
        // Same material; White has both knights out, Black has none.
        let developed = Board::parse(
            "rnbqkbnr/pppppppp/8/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1",
        );
        let undeveloped =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(
            eval(&developed, development_only()) > eval(&undeveloped, development_only()),
            "two knights out should beat two knights home"
        );
    }

    #[test]
    fn development_penalizes_the_early_queen() {
        // White's queen is on b3 with every minor still at home — the exact
        // pattern the ladder games showed torte drifting into.
        let queen_out =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/1Q6/PPPPPPPP/RNB1KBNR w KQkq - 0 1");
        let queen_home =
            Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(
            eval(&queen_out, development_only()) < eval(&queen_home, development_only()),
            "an early queen sortie should cost, not pay"
        );
    }

    #[test]
    fn development_fades_in_the_endgame() {
        // Bare kings and pawns: phase is 0, so a knight's home square carries
        // no penalty any more. Only the tempo bonus is left.
        let board = Board::parse("4k3/pppppppp/8/8/8/8/PPPPPPPP/4K3 w - - 0 1");
        assert_eq!(eval(&board, development_only()), eval(&board, EvalConfig::material_only()) + TEMPO_BONUS);
    }

    #[test]
    fn tempo_belongs_to_the_side_to_move() {
        // Identical position, opposite side to move: each side sees +tempo.
        let white = Board::parse("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let black = Board::parse("4k3/8/8/8/8/8/8/4K3 b - - 0 1");
        assert_eq!(eval(&white, development_only()), TEMPO_BONUS);
        assert_eq!(eval(&black, development_only()), TEMPO_BONUS);
    }

    #[test]
    fn pawn_structure_penalizes_doubled_pawns() {
        // White has doubled d-pawns; spreading them to adjacent files (still
        // two pawns, same material) must score strictly better for white.
        let doubled = Board::parse("4k3/4p3/8/8/8/3P4/3P4/4K3 w - - 0 1");
        let spread = Board::parse("4k3/4p3/8/8/8/3P4/4P3/4K3 w - - 0 1");
        assert!(eval(&spread, pawn_only()) > eval(&doubled, pawn_only()));
    }

    #[test]
    fn pawn_structure_penalizes_isolated_pawns() {
        // Two white pawns: isolated on d/f vs connected on d/e. Same material;
        // the connected pair must score strictly better.
        let isolated = Board::parse("4k3/8/8/8/8/8/3P1P2/4K3 w - - 0 1");
        let connected = Board::parse("4k3/8/8/8/8/8/3PP3/4K3 w - - 0 1");
        assert!(eval(&connected, pawn_only()) > eval(&isolated, pawn_only()));
    }

    #[test]
    fn pawn_structure_rewards_passed_pawn() {
        // White d5-pawn with the enemy pawn on d7 blocking the file: not a
        // passer. Move the enemy pawn to a7 (same material) and nothing stops
        // the d-pawn — it becomes a passer and white should score better.
        let blocked = Board::parse("4k3/3p4/8/3P4/8/8/8/4K3 w - - 0 1");
        let passer = Board::parse("4k3/p7/8/3P4/8/8/8/4K3 w - - 0 1");
        assert!(eval(&passer, pawn_only()) > eval(&blocked, pawn_only()));
    }

    #[test]
    fn passed_pawn_bonus_amplified_in_endgame() {
        // The same passed pawn is worth more at phase 0 (pure endgame) than at
        // PHASE_MAX (full middlegame).
        let board = Board::parse("4k3/8/8/3P4/8/8/8/4K3 w - - 0 1");
        let eg = pawn_structure(&board, 0);
        let mg = pawn_structure(&board, PHASE_MAX);
        assert!(eg > mg, "passer in EG ({}) should beat MG ({})", eg, mg);
    }

    #[test]
    fn pawn_structure_toggle_changes_score() {
        // With the toggle off, the doubled-pawn penalty disappears.
        let doubled = Board::parse("4k3/4p3/8/8/8/3P4/3P4/4K3 w - - 0 1");
        assert_ne!(
            eval(&doubled, pawn_only()),
            eval(&doubled, EvalConfig::material_only())
        );
    }

    #[test]
    fn bishop_pair_detects_two_bishops() {
        let white_pair = Board::parse("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        assert_eq!(bishop_pair(&white_pair), BISHOP_PAIR_BONUS);
        let black_pair = Board::parse("2b1kb2/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(bishop_pair(&black_pair), -BISHOP_PAIR_BONUS);
        // One bishop each: no pair on either side.
        let one_each = Board::parse("4kb2/8/8/8/8/8/8/2B1K3 w - - 0 1");
        assert_eq!(bishop_pair(&one_each), 0);
    }

    #[test]
    fn bishop_pair_reflected_in_eval() {
        // White holds both bishops; enabling the term lifts eval by exactly
        // the bonus over the material-only score.
        let board = Board::parse("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        let with = eval(&board, bishop_pair_only());
        let without = eval(&board, EvalConfig::material_only());
        assert_eq!(with - without, BISHOP_PAIR_BONUS);
    }

    #[test]
    fn king_safety_rewards_intact_pawn_shield() {
        // White king castled on g1 with the f/g/h pawns intact scores better
        // than the same king with no shield at all.
        let sheltered = Board::parse("4k3/8/8/8/8/8/5PPP/6K1 w - - 0 1");
        let exposed = Board::parse("4k3/8/8/8/8/8/8/6K1 w - - 0 1");
        assert!(
            king_safety(&sheltered, PHASE_MAX) > king_safety(&exposed, PHASE_MAX)
        );
    }

    #[test]
    fn king_safety_fades_in_endgame() {
        // The shield bonus is full at PHASE_MAX and gone at phase 0.
        let board = Board::parse("4k3/8/8/8/8/8/5PPP/6K1 w - - 0 1");
        assert!(king_safety(&board, PHASE_MAX) > king_safety(&board, 0));
        assert_eq!(king_safety(&board, 0), 0);
    }

    #[test]
    fn king_safety_ignores_king_off_home_rank() {
        // A king that has left its back rank gets no shield bonus even with
        // pawns alongside it.
        let marched = Board::parse("4k3/8/8/8/8/5PPP/6K1/8 w - - 0 1");
        assert_eq!(king_safety(&marched, PHASE_MAX), 0);
    }

    #[test]
    fn king_safety_reflected_in_eval() {
        // Rooks on a1/a8 keep the game phase above zero (king safety fades to
        // nothing at phase 0); material stays symmetric, so the only eval
        // difference is white's intact pawn shield.
        let board = Board::parse("r3k3/8/8/8/8/8/5PPP/R5K1 w - - 0 1");
        let with = eval(&board, king_safety_only());
        let without = eval(&board, EvalConfig::material_only());
        assert!(with > without);
    }
}
