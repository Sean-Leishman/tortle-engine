use crate::torte::board::pieces::Piece;
use crate::torte::{Bitboard, BitboardArray};
use std::fmt;

#[derive(Clone, Copy)]
pub struct Board {
    pub bitboards: BitboardArray,
}

impl Board {
    pub fn new() -> Board {
        Board {
            bitboards: [Bitboard::empty(); 12],
        }
    }

    pub fn parse(fen: &str) -> Board {
        let mut bitboards = [Bitboard::empty(); 12];

        let parts = fen.split_whitespace().collect::<Vec<&str>>();
        let mut board = parts[0].split('/');
        let mut number_of_pieces = 0;

        for rank in 0..8 {
            let mut file = 0;
            for c in board.next().unwrap().chars() {
                if c.is_digit(10) {
                    file += c.to_digit(10).unwrap();
                } else {
                    let piece = Piece::from_fen(c).unwrap();
                    bitboards[piece.to_index()].set((rank * 8 + file) as usize);
                    number_of_pieces += 1;
                    file += 1;
                }
            }
        }

        println!("{:?}", bitboards);

        let mut board = Board { bitboards };

        board
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = String::new();

        for i in 0..12 {
            result.push_str(&format!("{:064b}\n", self.bitboards[i].board));
        }

        write!(f, "{}", result)
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = String::new();

        result.push_str("  +-------------------------------+\n");
        for rank in (0..8).rev() {
            result.push_str(&format!("{} ", rank + 1));
            for file in 0..8 {
                let mut piece: Option<Piece> = None;
                for k in 0..12 {
                    if self.bitboards[k].get(rank * 8 + file) {
                        piece = Some(Piece::from_index(k));
                        break;
                    }
                }
                result.push_str(&format!(
                    "| {:01} ",
                    match piece {
                        Some(p) => p.to_string(),
                        None => String::from(" "),
                    }
                ));
            }
            result.push_str("|\n");
        }
        result.push_str("    A   B   C   D   E   F   G   H\n");
        result.push_str("  +-------------------------------+\n");

        write!(f, "{}", result)
    }
}
