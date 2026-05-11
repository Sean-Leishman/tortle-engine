use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;

pub const PAWN: i32 = 100;
pub const KNIGHT: i32 = 320;
pub const BISHOP: i32 = 330;
pub const ROOK: i32 = 500;
pub const QUEEN: i32 = 900;

pub const PIECE_VALUES: [i32; 6] = [PAWN, KNIGHT, BISHOP, ROOK, QUEEN, 0];

/// Material-only static evaluation, returned from the side-to-move's perspective.
pub fn eval(board: &Board) -> i32 {
    let mut score = 0;
    for kind in 0..6 {
        score += board.bbs[kind].count() as i32 * PIECE_VALUES[kind];
        score -= board.bbs[kind + 6].count() as i32 * PIECE_VALUES[kind];
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
    fn startpos_is_balanced() {
        let board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(eval(&board), 0);
    }

    #[test]
    fn extra_white_queen() {
        // White has an extra queen. White to move -> +900 from white's perspective.
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        assert_eq!(eval(&board), QUEEN);
    }

    #[test]
    fn extra_white_queen_black_to_move() {
        // Same position, black to move -> -900 from black's perspective.
        let board = Board::parse("4k3/8/8/8/8/8/8/3QK3 b - - 0 1");
        assert_eq!(eval(&board), -QUEEN);
    }
}
