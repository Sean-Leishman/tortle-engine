use crate::torte::board::board::Board;
use crate::torte::core::piece_move::Move;
use crate::torte::movegen::generator::generate_legal_moves;

pub fn perft(board: &Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = generate_legal_moves(board);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut count = 0;
    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        count += perft(&next, depth - 1);
    }
    count
}

pub fn perft_divide(board: &Board, depth: u32) -> Vec<(Move, u64)> {
    let moves = generate_legal_moves(board);
    let mut result = Vec::with_capacity(moves.len());
    for m in moves {
        let mut next = *board;
        next.apply_move(m).unwrap();
        let count = if depth <= 1 { 1 } else { perft(&next, depth - 1) };
        result.push((m, count));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torte::movegen::magic;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const KIWIPETE: &str =
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

    fn pos(fen: &str) -> Board {
        magic::init();
        Board::parse(fen)
    }

    #[test]
    fn perft_startpos_d1() {
        assert_eq!(perft(&pos(STARTPOS), 1), 20);
    }

    #[test]
    fn perft_startpos_d2() {
        assert_eq!(perft(&pos(STARTPOS), 2), 400);
    }

    #[test]
    fn perft_startpos_d3() {
        assert_eq!(perft(&pos(STARTPOS), 3), 8902);
    }

    #[test]
    fn perft_startpos_d4() {
        assert_eq!(perft(&pos(STARTPOS), 4), 197281);
    }

    #[test]
    #[ignore]
    fn perft_startpos_d5() {
        assert_eq!(perft(&pos(STARTPOS), 5), 4865609);
    }

    #[test]
    fn perft_kiwipete_d1() {
        assert_eq!(perft(&pos(KIWIPETE), 1), 48);
    }

    #[test]
    fn perft_kiwipete_d2() {
        assert_eq!(perft(&pos(KIWIPETE), 2), 2039);
    }

    #[test]
    #[ignore]
    fn perft_kiwipete_d3() {
        assert_eq!(perft(&pos(KIWIPETE), 3), 97862);
    }
}
