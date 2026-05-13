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

/// Static evaluation from the side-to-move's perspective. When `use_pst` is
/// false this is material-only; when true, piece-square table contributions
/// are added. Decoupled from `SearchConfig` so eval can be reused without
/// pulling in the search module's types.
pub fn eval(board: &Board, use_pst: bool) -> i32 {
    let mut score = 0;
    let phase = if use_pst { game_phase(board) } else { 0 };
    for kind in 0..6 {
        let mut bb = board.bbs[kind].board;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            score += PIECE_VALUES[kind];
            if use_pst {
                score += pst_value(kind, sq, phase);
            }
            bb &= bb - 1;
        }
        let mut bb = board.bbs[kind + 6].board;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            score -= PIECE_VALUES[kind];
            if use_pst {
                score -= pst_value(kind, sq ^ 56, phase);
            }
            bb &= bb - 1;
        }
    }
    if use_pst {
        score += mobility(board);
    }
    if board.side_to_move == Color::White {
        score
    } else {
        -score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_is_balanced_material_only() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board, false), 0);
    }

    #[test]
    fn startpos_is_balanced_with_pst() {
        // White and black are mirror-symmetric in the start position, so PST
        // contributions must cancel exactly.
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board, true), 0);
    }

    #[test]
    fn extra_white_queen() {
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        assert_eq!(eval(&board, false), QUEEN);
    }

    #[test]
    fn extra_white_queen_black_to_move() {
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 b - - 0 1");
        assert_eq!(eval(&board, false), -QUEEN);
    }

    #[test]
    fn pst_prefers_central_knight_over_corner() {
        let center = Board::parse("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1");
        let corner = Board::parse("4k3/8/8/8/8/8/8/N3K3 w - - 0 1");
        // Same material; with PST on, central knight should score strictly higher.
        assert!(eval(&center, true) > eval(&corner, true));
        // With PST off the positions are identical material-wise.
        assert_eq!(eval(&center, false), eval(&corner, false));
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
        assert!(eval(&center, true) > eval(&corner, true));
        // Material-only: identical.
        assert_eq!(eval(&center, false), eval(&corner, false));
    }

    #[test]
    fn tapered_pawn_prefers_advanced_pawn_in_endgame() {
        // Pure-pawn endgame (phase=0). A white pawn near promotion (a7) must
        // score strictly higher than a pawn still on its starting square (a2).
        let advanced = Board::parse("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let starting = Board::parse("4k3/8/8/8/8/8/P7/4K3 w - - 0 1");
        assert!(eval(&advanced, true) > eval(&starting, true));
    }

    #[test]
    fn tapered_rook_prefers_seventh_rank_in_endgame() {
        // Endgame R+K vs k. A rook on the 7th rank should score strictly
        // higher than the same rook back-ranked, with the new EG table.
        let seventh = Board::parse("4k3/R7/8/8/8/8/8/4K3 w - - 0 1");
        let first = Board::parse("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
        assert!(eval(&seventh, true) > eval(&first, true));
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
        let v1 = eval(&board, true);
        // Black knight on d8 only.
        let v2 = eval(&mirrored, true);
        // The boards have one knight each on mirror-image squares (different
        // colours, same relative square). Their PST contributions should be
        // exact negatives of each other (white knight side > 0, black knight
        // side < 0, mirrored across the rank axis).
        assert_eq!(v1, -v2);
    }
}
