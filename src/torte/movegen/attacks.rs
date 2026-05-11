use crate::torte::board::pieces::Color;
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::sq::SQ;

const KNIGHT_OFFSETS: [(i32, i32); 8] = [
    (-2, -1),
    (-2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
    (2, -1),
    (2, 1),
];

const KING_OFFSETS: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

const fn build_offset_table(offsets: &[(i32, i32)]) -> [Bitboard; 64] {
    let mut table = [Bitboard::from_u64(0); 64];
    let mut sq = 0;
    while sq < 64 {
        let rank = (sq / 8) as i32;
        let file = (sq % 8) as i32;
        let mut bb: u64 = 0;
        let mut i = 0;
        while i < offsets.len() {
            let dr = offsets[i].0;
            let df = offsets[i].1;
            let r = rank + dr;
            let f = file + df;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                bb |= 1u64 << (r * 8 + f);
            }
            i += 1;
        }
        table[sq] = Bitboard::from_u64(bb);
        sq += 1;
    }
    table
}

const fn build_pawn_attacks(white: bool) -> [Bitboard; 64] {
    let mut table = [Bitboard::from_u64(0); 64];
    let mut sq = 0;
    while sq < 64 {
        let rank = (sq / 8) as i32;
        let file = (sq % 8) as i32;
        let dr = if white { 1 } else { -1 };
        let mut bb: u64 = 0;
        let r = rank + dr;
        if r >= 0 && r < 8 {
            if file - 1 >= 0 {
                bb |= 1u64 << (r * 8 + (file - 1));
            }
            if file + 1 < 8 {
                bb |= 1u64 << (r * 8 + (file + 1));
            }
        }
        table[sq] = Bitboard::from_u64(bb);
        sq += 1;
    }
    table
}

pub static KNIGHT_ATTACKS: [Bitboard; 64] = build_offset_table(&KNIGHT_OFFSETS);
pub static KING_ATTACKS: [Bitboard; 64] = build_offset_table(&KING_OFFSETS);
pub static WHITE_PAWN_ATTACKS: [Bitboard; 64] = build_pawn_attacks(true);
pub static BLACK_PAWN_ATTACKS: [Bitboard; 64] = build_pawn_attacks(false);

pub fn knight_attacks(sq: SQ) -> Bitboard {
    KNIGHT_ATTACKS[sq.to_usize()]
}

pub fn king_attacks(sq: SQ) -> Bitboard {
    KING_ATTACKS[sq.to_usize()]
}

pub fn pawn_attacks(sq: SQ, color: Color) -> Bitboard {
    match color {
        Color::White => WHITE_PAWN_ATTACKS[sq.to_usize()],
        Color::Black => BLACK_PAWN_ATTACKS[sq.to_usize()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bb_of(squares: &[(u8, u8)]) -> Bitboard {
        let mut bb: u64 = 0;
        for (rank, file) in squares {
            bb |= 1u64 << (rank * 8 + file);
        }
        Bitboard::from_u64(bb)
    }

    #[test]
    fn knight_corner_a1() {
        // Knight on a1 attacks b3 and c2.
        assert_eq!(
            knight_attacks(SQ::make(0, 0)),
            bb_of(&[(2, 1), (1, 2)])
        );
    }

    #[test]
    fn knight_center_d4() {
        // d4 = (rank 3, file 3). Knight has 8 attacks: b3, b5, c2, c6, e2, e6, f3, f5.
        let expected = bb_of(&[
            (2, 1),
            (4, 1),
            (1, 2),
            (5, 2),
            (1, 4),
            (5, 4),
            (2, 5),
            (4, 5),
        ]);
        assert_eq!(knight_attacks(SQ::make(3, 3)), expected);
    }

    #[test]
    fn king_corner_h8() {
        // King on h8 attacks g7, g8, h7.
        assert_eq!(
            king_attacks(SQ::make(7, 7)),
            bb_of(&[(6, 6), (7, 6), (6, 7)])
        );
    }

    #[test]
    fn king_center_e4() {
        // 8 surrounding squares.
        let e4 = SQ::make(3, 4);
        assert_eq!(king_attacks(e4).count(), 8);
    }

    #[test]
    fn white_pawn_attacks_e2() {
        // White pawn on e2 attacks d3 and f3.
        assert_eq!(
            pawn_attacks(SQ::make(1, 4), Color::White),
            bb_of(&[(2, 3), (2, 5)])
        );
    }

    #[test]
    fn black_pawn_attacks_e7() {
        // Black pawn on e7 attacks d6 and f6.
        assert_eq!(
            pawn_attacks(SQ::make(6, 4), Color::Black),
            bb_of(&[(5, 3), (5, 5)])
        );
    }

    #[test]
    fn pawn_attacks_a_file_no_wrap() {
        // White pawn on a4 attacks only b5, never h5.
        assert_eq!(
            pawn_attacks(SQ::make(3, 0), Color::White),
            bb_of(&[(4, 1)])
        );
    }
}
