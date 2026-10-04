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
    moves.retain(|&m| {
        let mut next = *board;
        // Hash is never read on this copy — skip the Zobrist update.
        if next.apply_move_unhashed(m).is_err() {
            return false;
        }
        own_king_safe(&next, us)
    });
    moves
}

/// Pseudo-legal moves: every move the pieces can make, without checking
/// whether it leaves our own king attacked. The search filters these lazily —
/// it already copies and applies each move it tries, so it tests legality on
/// that board rather than paying a copy per move up front here (a node often
/// generates ~35 moves and searches 1-3 of them). Castling is generated
/// legality-checked, so castles never need the lazy test.
pub fn generate_pseudo_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(64);
    generate_pseudo_legal(board, &mut moves);
    moves
}

/// True if `after` — a board on which `us` has just moved — leaves our own
/// king unattacked, i.e. the move was legal. A missing king counts as safe so
/// that test positions without one behave as they did before.
pub fn own_king_safe(after: &Board, us: Color) -> bool {
    match king_square(after, us) {
        Some(k) => !is_attacked(after, k, us.opposite()),
        None => true,
    }
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

#[cfg(test)]
mod lazy_legality_tests {
    use super::*;
    use crate::torte::movegen::magic;

    fn pos(fen: &str) -> Board {
        magic::init();
        Board::parse(fen)
    }

    /// The search filters pseudo-legal moves lazily; that is only sound if
    /// "pseudo-legal, then drop the ones leaving our king attacked" is exactly
    /// `generate_legal_moves` — same moves, same order, since the index-based
    /// heuristics depend on order.
    fn assert_agrees(board: &Board) {
        let strict = generate_legal_moves(board);
        let lazy: Vec<Move> = generate_pseudo_legal_moves(board)
            .into_iter()
            .filter(|&m| {
                let mut next = *board;
                next.apply_move_unhashed(m).is_ok() && own_king_safe(&next, board.side_to_move)
            })
            .collect();
        assert_eq!(strict, lazy, "generators disagree on {:?}", board);
    }

    fn walk(board: &Board, depth: u32, visited: &mut u64) {
        assert_agrees(board);
        *visited += 1;
        if depth == 0 {
            return;
        }
        for m in generate_legal_moves(board) {
            let mut next = *board;
            if next.apply_move(m).is_ok() {
                walk(&next, depth - 1, visited);
            }
        }
    }

    #[test]
    fn lazy_filter_equals_strict_generator_over_a_walk() {
        // Trees chosen for the cases where legality actually bites: pins,
        // en passant (including an ep capture that would expose the king),
        // being in check, double check, and castling rights.
        let trees: [(&str, u32); 6] = [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 3),
            ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 2),
            ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 3),
            ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 2),
            // White king on e1, black rook on e8: the e-file pawn is pinned.
            ("4k2r/8/8/8/8/8/4P3/4K3 w k - 0 1", 3),
            // In check from two pieces at once — only king moves are legal.
            ("4k3/8/8/8/8/8/3qr3/4K3 w - - 0 1", 3),
        ];
        let mut visited = 0_u64;
        for (fen, depth) in trees {
            walk(&pos(fen), depth, &mut visited);
        }
        assert!(visited > 5_000, "walk shrank to {visited} nodes");
    }
}
