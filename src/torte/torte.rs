use crate::torte::board::board::Board;

pub struct Torte {
    pub board: Board,
}

impl Torte {
    pub fn new() -> Torte {
        Torte {
            board: Board::new(),
        }
    }

    pub fn run(&mut self) {
        println!("Running Torte");
        self.board = Board::parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        println!("{:?}", self.board);
    }
}
