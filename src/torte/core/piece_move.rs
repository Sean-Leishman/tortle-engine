use super::sq::SQ;

pub struct Move {
    src: SQ,
    dest: SQ,
}

impl Move {
    pub fn new(src: SQ, dest: SQ) -> Move {
        Move { src, dest }
    }

    pub fn get_src(&self) -> SQ {
        self.src
    }

    pub fn get_dest(&self) -> SQ {
        self.dest
    }

    pub fn from_uci(uci: &str) -> Move {
        if uci.len() != 4 {
            panic!("Invalid UCI move: {}", uci);
        }

        let src = SQ::make(
            (uci.chars().nth(1).unwrap() as u8) - 49,
            (uci.chars().nth(0).unwrap() as u8) - 97,
        );
        let dest = SQ::make(
            (uci.chars().nth(3).unwrap() as u8) - 49,
            (uci.chars().nth(2).unwrap() as u8) - 97,
        );

        Move::new(src, dest)
    }
}
