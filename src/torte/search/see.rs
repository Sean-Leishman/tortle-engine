//! Static exchange evaluation: what a capture actually wins once both sides
//! trade on the square, played out with the least valuable attacker each time.
//!
//! MVV-LVA ranks `QxP` highly because the victim is a pawn... no: it ranks by
//! victim first, so it likes `PxQ` and cannot tell a free rook from a rook
//! that walks into a pawn. SEE can: it resolves the whole exchange, including
//! the x-rays that open up behind each capture (the occupancy is updated as
//! pieces leave, so a rook behind a rook joins the sequence).

use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::piece_move::Move;
use crate::torte::core::sq::SQ;
use crate::torte::movegen::attacks::{king_attacks, knight_attacks, pawn_attacks};
use crate::torte::movegen::magic::{bishop_attacks, rook_attacks};
use crate::torte::search::eval::PIECE_VALUES;

/// Every piece of either colour bearing on `sq`, given an occupancy.
fn attackers_to(board: &Board, sq: SQ, occ: u64) -> u64 {
    let b = |i: usize| board.bbs[i].board;
    let bishops = b(2) | b(8) | b(4) | b(10);
    let rooks = b(3) | b(9) | b(4) | b(10);
    (pawn_attacks(sq, Color::Black).board & b(0))
        | (pawn_attacks(sq, Color::White).board & b(6))
        | (knight_attacks(sq).board & (b(1) | b(7)))
        | (king_attacks(sq).board & (b(5) | b(11)))
        | (bishop_attacks(sq, Bitboard::from_u64(occ)).board & bishops & occ)
        | (rook_attacks(sq, Bitboard::from_u64(occ)).board & rooks & occ)
}

/// Piece kind (0..6) sitting on `sq` for `color`, if any.
fn piece_on(board: &Board, sq: usize, color: Color) -> Option<usize> {
    let off = if color == Color::White { 0 } else { 6 };
    (0..6).find(|i| board.bbs[off + i].get(sq))
}

/// Centipawn gain of the capture `mv` after the full exchange on its
/// destination square. Positive = winning, negative = losing material.
/// Promotions are valued as the promotion gain only; en passant is treated as
/// a pawn capture.
pub fn see(board: &Board, mv: Move) -> i32 {
    let dest = mv.get_dest();
    let src = mv.get_src();
    let us = board.side_to_move;

    let mut occ = board.player_bbs[0].board | board.player_bbs[1].board;
    let captured = piece_on(board, dest.to_usize(), us.opposite());
    let ep = captured.is_none() && board.en_passant == Some(dest) && board.bbs[if us == Color::White { 0 } else { 6 }].get(src.to_usize());
    let mut gain = [0_i32; 32];
    gain[0] = match captured {
        Some(kind) => PIECE_VALUES[kind],
        None if ep => PIECE_VALUES[0],
        None => 0,
    };
    if ep {
        // The captured pawn sits behind the destination square.
        let cap_sq = if us == Color::White { dest.to_usize() - 8 } else { dest.to_usize() + 8 };
        occ &= !(1_u64 << cap_sq);
    }

    // The piece now standing on the square is the one that just moved.
    let mut on_square = match piece_on(board, src.to_usize(), us) {
        Some(kind) => kind,
        None => return 0,
    };
    occ &= !(1_u64 << src.to_usize());

    let mut side = us.opposite();
    let mut depth = 0;
    let mut attackers = attackers_to(board, dest, occ) & occ;

    loop {
        // Least valuable attacker for `side`.
        let off = if side == Color::White { 0 } else { 6 };
        let next = (0..6).find_map(|kind| {
            let bb = board.bbs[off + kind].board & attackers & occ;
            (bb != 0).then(|| (kind, bb & bb.wrapping_neg()))
        });
        let (kind, from) = match next {
            Some(v) => v,
            None => break,
        };
        // A king may only take last: capturing into a square the other side
        // still attacks is illegal, so the exchange stops rather than letting
        // the king be "recaptured".
        if kind == 5 {
            let enemy = board.player_bbs[side.opposite().to_index()].board;
            if attackers & occ & enemy & !from != 0 {
                break;
            }
        }
        depth += 1;
        if depth >= gain.len() {
            break;
        }
        // Standing on the square is worth what the previous occupant was.
        gain[depth] = PIECE_VALUES[on_square] - gain[depth - 1];
        on_square = kind;
        occ &= !from;
        // Recompute to pick up x-rays opened by the piece that just left.
        attackers = attackers_to(board, dest, occ) & occ;
        side = side.opposite();
    }

    // Walk back: each side takes the exchange only if it beats standing pat.
    while depth > 0 {
        gain[depth - 1] = -(-gain[depth - 1]).max(gain[depth]);
        depth -= 1;
    }
    gain[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(fen: &str) -> Board {
        crate::torte::movegen::magic::init();
        Board::parse(fen)
    }

    #[test]
    fn free_pawn_is_worth_a_pawn() {
        // Rook takes an undefended pawn.
        let board = pos("4k3/8/8/3p4/8/8/8/3RK3 w - - 0 1");
        assert_eq!(see(&board, Move::from_uci("d1d5")), PIECE_VALUES[0]);
    }

    #[test]
    fn defended_pawn_loses_the_rook() {
        // Rxd5 is met by cxd5: we win a pawn and lose a rook. The defending
        // pawn has to be on c6 — a c7 pawn attacks b6/d6, not d5.
        let board = pos("4k3/8/2p5/3p4/8/8/8/3RK3 w - - 0 1");
        assert_eq!(see(&board, Move::from_uci("d1d5")), PIECE_VALUES[0] - PIECE_VALUES[3]);
    }

    #[test]
    fn equal_trade_is_zero() {
        // Rooks trade on d5, each side recapturing once.
        let board = pos("3rk3/8/8/3r4/8/8/8/3RK2R w - - 0 1");
        assert_eq!(see(&board, Move::from_uci("d1d5")), 0);
    }

    #[test]
    fn defending_king_cannot_recapture_into_a_defended_square() {
        // Rxd5 and the black king is the only defender — but the c4 pawn
        // covers d5, so Kxd5 is illegal and white simply wins the pawn.
        // Without that rule the exchange reads as rook-for-pawn.
        let board = pos("8/8/4k3/3p4/2P5/8/8/3RK3 w - - 0 1");
        assert_eq!(see(&board, Move::from_uci("d1d5")), PIECE_VALUES[0]);
    }

    #[test]
    fn xray_behind_the_first_rook_joins_the_exchange() {
        // White rooks doubled on the d-file: the second one is why RxR works.
        let board = pos("3rk3/8/8/3r4/8/8/3R4/3RK3 w - - 0 1");
        assert_eq!(see(&board, Move::from_uci("d2d5")), PIECE_VALUES[3]);
    }
}
