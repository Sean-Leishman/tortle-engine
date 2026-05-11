use super::sq::SQ;
use crate::torte::board::pieces::{Color, Piece};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromotionPiece {
    Knight,
    Bishop,
    Rook,
    Queen,
}

impl PromotionPiece {
    pub fn to_piece(&self, color: Color) -> Piece {
        match (color, self) {
            (Color::White, PromotionPiece::Knight) => Piece::WhiteKnight,
            (Color::White, PromotionPiece::Bishop) => Piece::WhiteBishop,
            (Color::White, PromotionPiece::Rook) => Piece::WhiteRook,
            (Color::White, PromotionPiece::Queen) => Piece::WhiteQueen,
            (Color::Black, PromotionPiece::Knight) => Piece::BlackKnight,
            (Color::Black, PromotionPiece::Bishop) => Piece::BlackBishop,
            (Color::Black, PromotionPiece::Rook) => Piece::BlackRook,
            (Color::Black, PromotionPiece::Queen) => Piece::BlackQueen,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    src: SQ,
    dest: SQ,
    promotion: Option<PromotionPiece>,
}

impl Move {
    pub fn new(src: SQ, dest: SQ) -> Move {
        Move {
            src,
            dest,
            promotion: None,
        }
    }

    pub fn with_promotion(src: SQ, dest: SQ, promotion: PromotionPiece) -> Move {
        Move {
            src,
            dest,
            promotion: Some(promotion),
        }
    }

    pub fn get_src(&self) -> SQ {
        self.src
    }

    pub fn get_dest(&self) -> SQ {
        self.dest
    }

    pub fn get_promotion(&self) -> Option<PromotionPiece> {
        self.promotion
    }

    pub fn to_uci(&self) -> String {
        let src_file = (b'a' + self.src.0 % 8) as char;
        let src_rank = (b'1' + self.src.0 / 8) as char;
        let dst_file = (b'a' + self.dest.0 % 8) as char;
        let dst_rank = (b'1' + self.dest.0 / 8) as char;
        let mut s = format!("{}{}{}{}", src_file, src_rank, dst_file, dst_rank);
        if let Some(prom) = self.promotion {
            s.push(match prom {
                PromotionPiece::Queen => 'q',
                PromotionPiece::Rook => 'r',
                PromotionPiece::Bishop => 'b',
                PromotionPiece::Knight => 'n',
            });
        }
        s
    }

    pub fn from_uci(uci: &str) -> Move {
        let bytes = uci.as_bytes();
        if bytes.len() != 4 && bytes.len() != 5 {
            panic!("Invalid UCI move: {}", uci);
        }

        let src = SQ::make(bytes[1] - b'1', bytes[0] - b'a');
        let dest = SQ::make(bytes[3] - b'1', bytes[2] - b'a');

        let promotion = if bytes.len() == 5 {
            Some(match bytes[4] {
                b'n' => PromotionPiece::Knight,
                b'b' => PromotionPiece::Bishop,
                b'r' => PromotionPiece::Rook,
                b'q' => PromotionPiece::Queen,
                _ => panic!("Invalid promotion piece in UCI move: {}", uci),
            })
        } else {
            None
        };

        Move {
            src,
            dest,
            promotion,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}
