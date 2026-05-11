use crate::torte::core::bitboard::Bitboard;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SQ(pub u8);

impl SQ {
    pub const NONE: SQ = SQ(64);

    pub const fn make(rank: u8, file: u8) -> SQ {
        SQ(rank * 8 + file)
    }

    pub const fn is_ok(&self) -> bool {
        self.0 < 64
    }

    pub const fn is_none(&self) -> bool {
        self.0 == 64
    }

    pub const fn to_usize(&self) -> usize {
        self.0 as usize
    }

    pub fn to_bb(&self) -> Bitboard {
        Bitboard::from_u64(1) << self.0 as usize
    }

    pub fn from_uci(s: &str) -> Option<SQ> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = bytes[0].checked_sub(b'a')?;
        let rank = bytes[1].checked_sub(b'1')?;
        if file > 7 || rank > 7 {
            return None;
        }
        Some(SQ::make(rank, file))
    }
}
