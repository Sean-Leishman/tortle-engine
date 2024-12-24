use crate::torte::board::pieces::Piece;
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::{piece_move::Move, sq::SQ};
use std::fmt;

#[derive(Clone, Copy)]
pub struct Board {
    pub bbs: [Bitboard; 12],
    pub player_bbs: [Bitboard; 2],
}

impl Board {
    pub fn new() -> Board {
        Board {
            bbs: [Bitboard::empty(); 12],
            player_bbs: [Bitboard::empty(); 2],
        }
    }

    pub fn apply_uci_move(&mut self, move_str: &str) -> Result<(), std::io::Error> {
        /* move_str is a string of 4 characters, where the first two characters
         * are the file and rank of the square the piece is moving from, and the
         * last two characters are the file and rank of the square the piece is
         * moving to.
         * move_str can be split into two strings, from and to, where from is the
         * first two characters and to is the last two characters.
         */

        let piece_move = Move::from_uci(move_str);
        self.apply_move(piece_move)
    }

    pub fn apply_move(&mut self, piece_move: Move) -> Result<(), std::io::Error> {
        let from = piece_move.get_src();
        let to = piece_move.get_dest();
        let piece = self.piece_at_sq(from)?;

        self.move_piece(piece, from, to)
    }

    fn move_piece(&mut self, piece: Piece, from: SQ, to: SQ) -> Result<(), std::io::Error> {
        for i in 0..12 {
            if self.bbs[i].get(from.to_usize()) {
                self.bbs[i].clear(from.to_usize());
                self.bbs[i].set(to.to_usize());
                return Ok(());
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No piece found at square",
        ))
    }

    fn piece_at_sq(&self, sq: SQ) -> Result<Piece, std::io::Error> {
        for i in 0..12 {
            if self.bbs[i].get(sq.to_usize()) {
                return Ok(Piece::from_index(i));
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No piece found at square",
        ))
    }

    pub fn parse(fen: &str) -> Board {
        let mut bbs = [Bitboard::empty(); 12];
        let mut player_bbs = [Bitboard::empty(); 2];

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
                    bbs[piece.to_index()].set((rank * 8 + file) as usize);
                    player_bbs[piece.color().to_index()].set((rank * 8 + file) as usize);

                    number_of_pieces += 1;
                    file += 1;
                }
            }
        }

        let mut board = Board { bbs, player_bbs };

        board
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = String::new();

        for i in 0..12 {
            result.push_str(&format!("{:064b}\n", self.bbs[i].board));
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
                    if self.bbs[k].get(rank * 8 + file) {
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
