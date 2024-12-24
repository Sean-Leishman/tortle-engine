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

        while true {
            let mut input = String::new();
            println!("Enter move: ");
            std::io::stdin().read_line(&mut input).unwrap();

            if input.trim() == "exit" {
                break;
            }

            let res = self.board.apply_uci_move(input.trim());
            if res.is_err() {
                println!("Invalid move: {:?}", res.err().unwrap());
            }

            println!("{:?}", self.board);
        }
        println!("{:?}", self.board);
    }
}
