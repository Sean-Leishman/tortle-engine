use crate::torte::board::board::{Board, CastlingRights};
use crate::torte::board::pieces::{Color, Piece};
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::piece_move::{Move, PromotionPiece};
use crate::torte::core::sq::SQ;
use crate::torte::movegen::attacks::{king_attacks, knight_attacks, pawn_attacks};
use crate::torte::movegen::magic::{bishop_attacks, queen_attacks, rook_attacks};

const FILE_A: u64 = 0x0101_0101_0101_0101;
const FILE_H: u64 = 0x8080_8080_8080_8080;
const RANK_1: u64 = 0x0000_0000_0000_00FF;
const RANK_2: u64 = 0x0000_0000_0000_FF00;
const RANK_7: u64 = 0x00FF_0000_0000_0000;
const RANK_8: u64 = 0xFF00_0000_0000_0000;

pub fn is_attacked(board: &Board, sq: SQ, by: Color) -> bool {
    let occ = board.player_bbs[0] | board.player_bbs[1];
    let by_idx_off = if by == Color::White { 0 } else { 6 };

    let by_pawns = board.bbs[by_idx_off];
    let pawn_atk = pawn_attacks(sq, by.opposite());
    if !(pawn_atk & by_pawns).is_empty() {
        return true;
    }

    let by_knights = board.bbs[by_idx_off + 1];
    if !(knight_attacks(sq) & by_knights).is_empty() {
        return true;
    }

    let by_kings = board.bbs[by_idx_off + 5];
    if !(king_attacks(sq) & by_kings).is_empty() {
        return true;
    }

    let by_bishops_queens = board.bbs[by_idx_off + 2] | board.bbs[by_idx_off + 4];
    if !(bishop_attacks(sq, occ) & by_bishops_queens).is_empty() {
        return true;
    }

    let by_rooks_queens = board.bbs[by_idx_off + 3] | board.bbs[by_idx_off + 4];
    if !(rook_attacks(sq, occ) & by_rooks_queens).is_empty() {
        return true;
    }

    false
}

pub fn king_square(board: &Board, c: Color) -> Option<SQ> {
    let idx = if c == Color::White { 5 } else { 11 };
    let bb = board.bbs[idx];
    if bb.is_empty() {
        None
    } else {
        Some(SQ(bb.get_lsb() as u8))
    }
}

pub fn generate_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(64);
    generate_pseudo_legal(board, &mut moves);
    let us = board.side_to_move;
    let opp = us.opposite();
    moves.retain(|&m| {
        let mut next = *board;
        if next.apply_move(m).is_err() {
            return false;
        }
        match king_square(&next, us) {
            Some(k) => !is_attacked(&next, k, opp),
            None => true,
        }
    });
    moves
}

fn generate_pseudo_legal(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    match us {
        Color::White => generate_white_pawn_moves(board, moves),
        Color::Black => generate_black_pawn_moves(board, moves),
    }
    generate_piece_moves(board, moves);
    generate_castling(board, moves);
}

fn push_promotions(moves: &mut Vec<Move>, from: SQ, to: SQ) {
    for prom in [
        PromotionPiece::Queen,
        PromotionPiece::Rook,
        PromotionPiece::Bishop,
        PromotionPiece::Knight,
    ] {
        moves.push(Move::with_promotion(from, to, prom));
    }
}

fn generate_white_pawn_moves(board: &Board, moves: &mut Vec<Move>) {
    let pawns = board.bbs[Piece::WhitePawn.to_index()].board;
    let our = board.player_bbs[0].board;
    let enemy = board.player_bbs[1].board;
    let occ = our | enemy;
    let empty = !occ;

    let single = (pawns << 8) & empty;
    let single_quiet = single & !RANK_8;
    let single_promo = single & RANK_8;

    for to in Bitboard::from_u64(single_quiet) {
        moves.push(Move::new(SQ((to - 8) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(single_promo) {
        push_promotions(moves, SQ((to - 8) as u8), SQ(to as u8));
    }

    let dp = ((((pawns & RANK_2) << 8) & empty) << 8) & empty;
    for to in Bitboard::from_u64(dp) {
        moves.push(Move::new(SQ((to - 16) as u8), SQ(to as u8)));
    }

    let cap_nw = ((pawns & !FILE_A) << 7) & enemy;
    let cap_ne = ((pawns & !FILE_H) << 9) & enemy;

    for to in Bitboard::from_u64(cap_nw & !RANK_8) {
        moves.push(Move::new(SQ((to - 7) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(cap_nw & RANK_8) {
        push_promotions(moves, SQ((to - 7) as u8), SQ(to as u8));
    }
    for to in Bitboard::from_u64(cap_ne & !RANK_8) {
        moves.push(Move::new(SQ((to - 9) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(cap_ne & RANK_8) {
        push_promotions(moves, SQ((to - 9) as u8), SQ(to as u8));
    }

    if let Some(ep) = board.en_passant {
        let attackers = pawn_attacks(ep, Color::Black).board & pawns;
        for from in Bitboard::from_u64(attackers) {
            moves.push(Move::new(SQ(from as u8), ep));
        }
    }
}

fn generate_black_pawn_moves(board: &Board, moves: &mut Vec<Move>) {
    let pawns = board.bbs[Piece::BlackPawn.to_index()].board;
    let our = board.player_bbs[1].board;
    let enemy = board.player_bbs[0].board;
    let occ = our | enemy;
    let empty = !occ;

    let single = (pawns >> 8) & empty;
    let single_quiet = single & !RANK_1;
    let single_promo = single & RANK_1;

    for to in Bitboard::from_u64(single_quiet) {
        moves.push(Move::new(SQ((to + 8) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(single_promo) {
        push_promotions(moves, SQ((to + 8) as u8), SQ(to as u8));
    }

    let dp = ((((pawns & RANK_7) >> 8) & empty) >> 8) & empty;
    for to in Bitboard::from_u64(dp) {
        moves.push(Move::new(SQ((to + 16) as u8), SQ(to as u8)));
    }

    let cap_se = ((pawns & !FILE_H) >> 7) & enemy;
    let cap_sw = ((pawns & !FILE_A) >> 9) & enemy;

    for to in Bitboard::from_u64(cap_se & !RANK_1) {
        moves.push(Move::new(SQ((to + 7) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(cap_se & RANK_1) {
        push_promotions(moves, SQ((to + 7) as u8), SQ(to as u8));
    }
    for to in Bitboard::from_u64(cap_sw & !RANK_1) {
        moves.push(Move::new(SQ((to + 9) as u8), SQ(to as u8)));
    }
    for to in Bitboard::from_u64(cap_sw & RANK_1) {
        push_promotions(moves, SQ((to + 9) as u8), SQ(to as u8));
    }

    if let Some(ep) = board.en_passant {
        let attackers = pawn_attacks(ep, Color::White).board & pawns;
        for from in Bitboard::from_u64(attackers) {
            moves.push(Move::new(SQ(from as u8), ep));
        }
    }
}

fn generate_piece_moves(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let our = board.player_bbs[us.to_index()];
    let occ = board.player_bbs[0] | board.player_bbs[1];
    let not_us = Bitboard::from_u64(!our.board);
    let off = if us == Color::White { 0 } else { 6 };

    for from in board.bbs[off + 1] {
        let attacks = knight_attacks(SQ(from as u8)) & not_us;
        for to in attacks {
            moves.push(Move::new(SQ(from as u8), SQ(to as u8)));
        }
    }

    for from in board.bbs[off + 2] {
        let attacks = bishop_attacks(SQ(from as u8), occ) & not_us;
        for to in attacks {
            moves.push(Move::new(SQ(from as u8), SQ(to as u8)));
        }
    }

    for from in board.bbs[off + 3] {
        let attacks = rook_attacks(SQ(from as u8), occ) & not_us;
        for to in attacks {
            moves.push(Move::new(SQ(from as u8), SQ(to as u8)));
        }
    }

    for from in board.bbs[off + 4] {
        let attacks = queen_attacks(SQ(from as u8), occ) & not_us;
        for to in attacks {
            moves.push(Move::new(SQ(from as u8), SQ(to as u8)));
        }
    }

    for from in board.bbs[off + 5] {
        let attacks = king_attacks(SQ(from as u8)) & not_us;
        for to in attacks {
            moves.push(Move::new(SQ(from as u8), SQ(to as u8)));
        }
    }
}

fn generate_castling(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let opp = us.opposite();
    let occ = board.player_bbs[0] | board.player_bbs[1];

    let (king_sq, ks_right, qs_right, ks_path, qs_empty, qs_king_path) = match us {
        Color::White => (
            SQ::make(0, 4),
            CastlingRights::WHITE_KING,
            CastlingRights::WHITE_QUEEN,
            [SQ::make(0, 5), SQ::make(0, 6)],
            [1usize, 2, 3],
            [SQ::make(0, 3), SQ::make(0, 2)],
        ),
        Color::Black => (
            SQ::make(7, 4),
            CastlingRights::BLACK_KING,
            CastlingRights::BLACK_QUEEN,
            [SQ::make(7, 5), SQ::make(7, 6)],
            [57usize, 58, 59],
            [SQ::make(7, 3), SQ::make(7, 2)],
        ),
    };

    if is_attacked(board, king_sq, opp) {
        return;
    }

    if board.castling.has(ks_right)
        && !occ.get(ks_path[0].to_usize())
        && !occ.get(ks_path[1].to_usize())
        && !is_attacked(board, ks_path[0], opp)
        && !is_attacked(board, ks_path[1], opp)
    {
        moves.push(Move::new(king_sq, ks_path[1]));
    }

    if board.castling.has(qs_right)
        && !occ.get(qs_empty[0])
        && !occ.get(qs_empty[1])
        && !occ.get(qs_empty[2])
        && !is_attacked(board, qs_king_path[0], opp)
        && !is_attacked(board, qs_king_path[1], opp)
    {
        moves.push(Move::new(king_sq, qs_king_path[1]));
    }
}
