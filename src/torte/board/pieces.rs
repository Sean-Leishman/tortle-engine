pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(&self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }
}

pub enum Piece {
    WhitePawn,
    WhiteKnight,
    WhiteBishop,
    WhiteRook,
    WhiteQueen,
    WhiteKing,
    BlackPawn,
    BlackKnight,
    BlackBishop,
    BlackRook,
    BlackQueen,
    BlackKing,
}

impl Piece {
    pub fn color(&self) -> Color {
        match self {
            Piece::WhitePawn
            | Piece::WhiteKnight
            | Piece::WhiteBishop
            | Piece::WhiteRook
            | Piece::WhiteQueen
            | Piece::WhiteKing => Color::White,
            Piece::BlackPawn
            | Piece::BlackKnight
            | Piece::BlackBishop
            | Piece::BlackRook
            | Piece::BlackQueen
            | Piece::BlackKing => Color::Black,
        }
    }

    pub fn from_index(index: usize) -> Piece {
        match index {
            0 => Piece::WhitePawn,
            1 => Piece::WhiteKnight,
            2 => Piece::WhiteBishop,
            3 => Piece::WhiteRook,
            4 => Piece::WhiteQueen,
            5 => Piece::WhiteKing,
            6 => Piece::BlackPawn,
            7 => Piece::BlackKnight,
            8 => Piece::BlackBishop,
            9 => Piece::BlackRook,
            10 => Piece::BlackQueen,
            11 => Piece::BlackKing,
            _ => panic!("Invalid piece index"),
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            Piece::WhitePawn => 0,
            Piece::WhiteKnight => 1,
            Piece::WhiteBishop => 2,
            Piece::WhiteRook => 3,
            Piece::WhiteQueen => 4,
            Piece::WhiteKing => 5,
            Piece::BlackPawn => 6,
            Piece::BlackKnight => 7,
            Piece::BlackBishop => 8,
            Piece::BlackRook => 9,
            Piece::BlackQueen => 10,
            Piece::BlackKing => 11,
        }
    }

    pub fn from_fen(fen: char) -> Option<Piece> {
        match fen {
            'P' => Some(Piece::WhitePawn),
            'N' => Some(Piece::WhiteKnight),
            'B' => Some(Piece::WhiteBishop),
            'R' => Some(Piece::WhiteRook),
            'Q' => Some(Piece::WhiteQueen),
            'K' => Some(Piece::WhiteKing),
            'p' => Some(Piece::BlackPawn),
            'n' => Some(Piece::BlackKnight),
            'b' => Some(Piece::BlackBishop),
            'r' => Some(Piece::BlackRook),
            'q' => Some(Piece::BlackQueen),
            'k' => Some(Piece::BlackKing),
            _ => None,
        }
    }
}

impl std::fmt::Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Piece::WhitePawn => write!(f, "♙"),
            Piece::WhiteKnight => write!(f, "♘"),
            Piece::WhiteBishop => write!(f, "♗"),
            Piece::WhiteRook => write!(f, "♖"),
            Piece::WhiteQueen => write!(f, "♕"),
            Piece::WhiteKing => write!(f, "♔"),
            Piece::BlackPawn => write!(f, "♟"),
            Piece::BlackKnight => write!(f, "♞"),
            Piece::BlackBishop => write!(f, "♝"),
            Piece::BlackRook => write!(f, "♜"),
            Piece::BlackQueen => write!(f, "♛"),
            Piece::BlackKing => write!(f, "♚"),
        }
    }
}

impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
