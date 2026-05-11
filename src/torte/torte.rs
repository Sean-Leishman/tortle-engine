use crate::torte::board::board::Board;
use crate::torte::movegen::magic;
use crate::torte::uci;

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
        magic::init();
        self.board = Board::parse(uci::STARTPOS);
        uci::run(&mut self.board);
    }
}
