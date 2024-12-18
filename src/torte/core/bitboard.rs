use std::{
    fmt,
    ops::{Add, BitAnd, BitOr, BitXor, Mul, Not, Shl, ShlAssign, Shr, ShrAssign, Sub},
};

#[derive(Clone, Copy)]
pub struct Bitboard {
    pub board: u64,
}

impl Bitboard {
    pub fn new() -> Bitboard {
        Bitboard { board: 0 }
    }

    pub fn set(&mut self, index: usize) {
        self.board |= 1 << index;
    }

    pub fn clear(&mut self, index: usize) {
        self.board &= !(1 << index);
    }

    pub fn get(&self, index: usize) -> bool {
        (self.board & (1 << index)) != 0
    }

    pub fn empty() -> Bitboard {
        Bitboard { board: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.board == 0
    }

    pub fn pop(&mut self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }

        let index = self.board.trailing_zeros() as usize;
        self.clear(index);
        Some(index)
    }

    pub fn count(&self) -> u32 {
        self.board.count_ones()
    }

    pub fn get_msb(&self) -> usize {
        self.board.leading_zeros() as usize
    }

    pub fn get_lsb(&self) -> usize {
        self.board.trailing_zeros() as usize
    }
}

impl Iterator for Bitboard {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }
}

impl Mul for Bitboard {
    type Output = Bitboard;

    fn mul(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board & rhs.board,
        }
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;

    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board & rhs.board,
        }
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;

    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board | rhs.board,
        }
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;

    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board ^ rhs.board,
        }
    }
}

impl Shl<usize> for Bitboard {
    type Output = Bitboard;

    fn shl(self, rhs: usize) -> Bitboard {
        Bitboard {
            board: self.board << rhs,
        }
    }
}

impl Shr<usize> for Bitboard {
    type Output = Bitboard;

    fn shr(self, rhs: usize) -> Bitboard {
        Bitboard {
            board: self.board >> rhs,
        }
    }
}

impl ShrAssign<usize> for Bitboard {
    fn shr_assign(&mut self, rhs: usize) {
        self.board >>= rhs;
    }
}

impl ShlAssign<usize> for Bitboard {
    fn shl_assign(&mut self, rhs: usize) {
        self.board <<= rhs;
    }
}

impl Not for Bitboard {
    type Output = Bitboard;

    fn not(self) -> Bitboard {
        Bitboard { board: !self.board }
    }
}

impl Add for Bitboard {
    type Output = Bitboard;

    fn add(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board | rhs.board,
        }
    }
}

impl Sub for Bitboard {
    type Output = Bitboard;

    fn sub(self, rhs: Bitboard) -> Bitboard {
        Bitboard {
            board: self.board & !rhs.board,
        }
    }
}

impl PartialEq for Bitboard {
    fn eq(&self, other: &Bitboard) -> bool {
        self.board == other.board
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut board = self.board;
        for _ in 0..8 {
            for _ in 0..8 {
                write!(f, "{}", board & 1).unwrap();
                board >>= 1;
            }
            write!(f, "\n").unwrap();
        }
        Ok(())
    }
}
