use crate::torte::board::pieces::{Color, Piece};
use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::{piece_move::Move, sq::SQ};
use std::fmt;

#[derive(Clone, Copy)]
pub struct CastlingRights(pub u8);

impl CastlingRights {
    pub const WHITE_KING: u8 = 1 << 0;
    pub const WHITE_QUEEN: u8 = 1 << 1;
    pub const BLACK_KING: u8 = 1 << 2;
    pub const BLACK_QUEEN: u8 = 1 << 3;

    pub fn empty() -> Self {
        CastlingRights(0)
    }

    pub fn has(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    pub fn from_fen(s: &str) -> Option<Self> {
        if s == "-" {
            return Some(CastlingRights::empty());
        }
        let mut bits = 0u8;
        for c in s.chars() {
            bits |= match c {
                'K' => Self::WHITE_KING,
                'Q' => Self::WHITE_QUEEN,
                'k' => Self::BLACK_KING,
                'q' => Self::BLACK_QUEEN,
                _ => return None,
            };
        }
        Some(CastlingRights(bits))
    }
}

#[derive(Clone, Copy)]
pub struct Board {
    pub bbs: [Bitboard; 12],
    pub player_bbs: [Bitboard; 2],
    pub side_to_move: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<SQ>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
}

impl Board {
    pub fn new() -> Board {
        Board {
            bbs: [Bitboard::empty(); 12],
            player_bbs: [Bitboard::empty(); 2],
            side_to_move: Color::White,
            castling: CastlingRights::empty(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
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

        let from_idx = from.to_usize();
        let to_idx = to.to_usize();
        let mover = piece.color();
        let mover_idx = mover.to_index();
        let opp_idx = mover.opposite().to_index();
        let from_rank = from.0 / 8;
        let to_rank = to.0 / 8;
        let from_file = from.0 % 8;
        let to_file = to.0 % 8;

        let is_pawn = matches!(piece, Piece::WhitePawn | Piece::BlackPawn);
        let is_king = matches!(piece, Piece::WhiteKing | Piece::BlackKing);

        let is_castle = is_king && (to_file as i8 - from_file as i8).abs() == 2;
        let is_ep_capture =
            is_pawn && from_file != to_file && self.en_passant == Some(to);
        let is_double_push =
            is_pawn && (to_rank as i8 - from_rank as i8).abs() == 2;
        let is_normal_capture = self.player_bbs[opp_idx].get(to_idx);
        let is_capture = is_normal_capture || is_ep_capture;

        let last_rank = if mover == Color::White { 7 } else { 0 };
        let promotion_piece = match (piece_move.get_promotion(), is_pawn && to_rank == last_rank) {
            (Some(prom), true) => Some(prom.to_piece(mover)),
            (Some(_), false) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Promotion suffix only valid on pawn move to last rank",
                ));
            }
            (None, true) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Pawn move to last rank requires promotion suffix",
                ));
            }
            (None, false) => None,
        };

        let dest_piece_idx = match promotion_piece {
            Some(p) => p.to_index(),
            None => piece.to_index(),
        };

        self.bbs[piece.to_index()].clear(from_idx);
        self.bbs[dest_piece_idx].set(to_idx);
        self.player_bbs[mover_idx].clear(from_idx);
        self.player_bbs[mover_idx].set(to_idx);

        if is_ep_capture {
            let cap_idx = match mover {
                Color::White => to_idx - 8,
                Color::Black => to_idx + 8,
            };
            let opp_pawn_idx = match mover {
                Color::White => Piece::BlackPawn,
                Color::Black => Piece::WhitePawn,
            }
            .to_index();
            self.bbs[opp_pawn_idx].clear(cap_idx);
            self.player_bbs[opp_idx].clear(cap_idx);
        } else if is_normal_capture {
            let opp_range = match mover.opposite() {
                Color::White => 0..6,
                Color::Black => 6..12,
            };
            for i in opp_range {
                if self.bbs[i].get(to_idx) {
                    self.bbs[i].clear(to_idx);
                    break;
                }
            }
            self.player_bbs[opp_idx].clear(to_idx);
        }

        if is_castle {
            let (rook_from, rook_to) = match (mover, to_file) {
                (Color::White, 6) => (7usize, 5usize),
                (Color::White, 2) => (0, 3),
                (Color::Black, 6) => (63, 61),
                (Color::Black, 2) => (56, 59),
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid castling destination",
                    ));
                }
            };
            let rook_idx = match mover {
                Color::White => Piece::WhiteRook,
                Color::Black => Piece::BlackRook,
            }
            .to_index();
            self.bbs[rook_idx].clear(rook_from);
            self.bbs[rook_idx].set(rook_to);
            self.player_bbs[mover_idx].clear(rook_from);
            self.player_bbs[mover_idx].set(rook_to);
        }

        if is_king {
            let mask = match mover {
                Color::White => CastlingRights::WHITE_KING | CastlingRights::WHITE_QUEEN,
                Color::Black => CastlingRights::BLACK_KING | CastlingRights::BLACK_QUEEN,
            };
            self.castling.0 &= !mask;
        }
        let rook_mask = |sq_idx: usize| -> u8 {
            match sq_idx {
                0 => CastlingRights::WHITE_QUEEN,
                7 => CastlingRights::WHITE_KING,
                56 => CastlingRights::BLACK_QUEEN,
                63 => CastlingRights::BLACK_KING,
                _ => 0,
            }
        };
        self.castling.0 &= !rook_mask(from_idx);
        self.castling.0 &= !rook_mask(to_idx);

        self.en_passant = if is_double_push {
            let ep_idx = match mover {
                Color::White => from_idx + 8,
                Color::Black => from_idx - 8,
            };
            Some(SQ(ep_idx as u8))
        } else {
            None
        };

        if is_pawn || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }
        if mover == Color::Black {
            self.fullmove_number += 1;
        }
        self.side_to_move = self.side_to_move.opposite();

        Ok(())
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

        for fen_rank in 0..8 {
            let rank = 7 - fen_rank;
            let mut file = 0;
            for c in board.next().unwrap().chars() {
                if c.is_digit(10) {
                    file += c.to_digit(10).unwrap();
                } else {
                    let piece = Piece::from_fen(c).unwrap();
                    let idx = (rank * 8 + file) as usize;
                    bbs[piece.to_index()].set(idx);
                    player_bbs[piece.color().to_index()].set(idx);

                    number_of_pieces += 1;
                    file += 1;
                }
            }
        }

        let side_to_move = parts
            .get(1)
            .and_then(|s| s.chars().next())
            .and_then(Color::from_fen)
            .unwrap_or(Color::White);

        let castling = parts
            .get(2)
            .and_then(|s| CastlingRights::from_fen(s))
            .unwrap_or_else(CastlingRights::empty);

        let en_passant = match parts.get(3) {
            Some(&"-") | None => None,
            Some(s) => SQ::from_uci(s),
        };

        let halfmove_clock = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
        let fullmove_number = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(1);

        Board {
            bbs,
            player_bbs,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
        }
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

        let stm = match self.side_to_move {
            Color::White => "w",
            Color::Black => "b",
        };
        let mut castling = String::new();
        if self.castling.has(CastlingRights::WHITE_KING) {
            castling.push('K');
        }
        if self.castling.has(CastlingRights::WHITE_QUEEN) {
            castling.push('Q');
        }
        if self.castling.has(CastlingRights::BLACK_KING) {
            castling.push('k');
        }
        if self.castling.has(CastlingRights::BLACK_QUEEN) {
            castling.push('q');
        }
        if castling.is_empty() {
            castling.push('-');
        }
        let ep = match self.en_passant {
            None => String::from("-"),
            Some(sq) => {
                let file = (sq.0 % 8) as u8;
                let rank = (sq.0 / 8) as u8;
                format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
            }
        };
        result.push_str(&format!(
            "  stm={} castling={} ep={} halfmove={} fullmove={}\n",
            stm, castling, ep, self.halfmove_clock, self.fullmove_number,
        ));

        write!(f, "{}", result)
    }
}
