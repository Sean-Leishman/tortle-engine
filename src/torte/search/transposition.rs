use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::core::piece_move::Move;
use std::sync::OnceLock;

const DEFAULT_SIZE_MB: usize = 16;

/// Bound type of a TT entry's stored score. `Exact` means the score was the
/// true minimax value at the searched depth. `LowerBound` means a beta cutoff
/// happened, so the true score is ≥ stored. `UpperBound` means no move beat
/// alpha, so the true score is ≤ stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub key: u64,
    pub score: i32,
    pub best_move: Option<Move>,
    pub depth: u8,
    pub bound: Bound,
}

pub struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<Option<TTEntry>>().max(32);
        let bytes = size_mb.max(1) * 1024 * 1024;
        let mut count = (bytes / entry_size).max(1).next_power_of_two();
        // next_power_of_two rounds *up*; we want the largest power of two that fits.
        if count * entry_size > bytes && count > 1 {
            count /= 2;
        }
        Self {
            table: vec![None; count],
            mask: count - 1,
        }
    }

    pub fn default_size() -> Self {
        Self::new(DEFAULT_SIZE_MB)
    }

    pub fn probe(&self, key: u64) -> Option<&TTEntry> {
        let slot = &self.table[(key as usize) & self.mask];
        match slot {
            Some(entry) if entry.key == key => Some(entry),
            _ => None,
        }
    }

    pub fn store(&mut self, entry: TTEntry) {
        let idx = (entry.key as usize) & self.mask;
        self.table[idx] = Some(entry);
    }

    pub fn clear(&mut self) {
        for slot in self.table.iter_mut() {
            *slot = None;
        }
    }

    pub fn capacity(&self) -> usize {
        self.table.len()
    }
}

struct ZobristKeys {
    pieces: [[u64; 64]; 12],
    side: u64,
    castling: [u64; 16],
    en_passant_file: [u64; 8],
}

static ZOBRIST: OnceLock<ZobristKeys> = OnceLock::new();

fn zobrist() -> &'static ZobristKeys {
    ZOBRIST.get_or_init(|| {
        let mut rng = Xorshift64::new(0xC0FFEE_BABE_DEAD);
        let mut pieces = [[0u64; 64]; 12];
        for i in 0..12 {
            for j in 0..64 {
                pieces[i][j] = rng.next();
            }
        }
        let side = rng.next();
        let mut castling = [0u64; 16];
        for slot in castling.iter_mut() {
            *slot = rng.next();
        }
        let mut en_passant_file = [0u64; 8];
        for slot in en_passant_file.iter_mut() {
            *slot = rng.next();
        }
        ZobristKeys {
            pieces,
            side,
            castling,
            en_passant_file,
        }
    })
}

/// Compute the Zobrist hash of `board` from scratch.
pub fn zobrist_hash(board: &Board) -> u64 {
    let keys = zobrist();
    let mut h = 0u64;
    for kind in 0..12 {
        let mut bb = board.bbs[kind].board;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            h ^= keys.pieces[kind][sq];
            bb &= bb - 1;
        }
    }
    if board.side_to_move == Color::Black {
        h ^= keys.side;
    }
    h ^= keys.castling[(board.castling.0 & 0xF) as usize];
    if let Some(ep) = board.en_passant {
        h ^= keys.en_passant_file[(ep.0 % 8) as usize];
    }
    h
}

struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(fen: &str) -> Board {
        Board::parse(fen)
    }

    #[test]
    fn hash_is_deterministic() {
        let board = b("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(zobrist_hash(&board), zobrist_hash(&board));
    }

    #[test]
    fn hash_changes_with_side_to_move() {
        let w = b("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let bl = b("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1");
        assert_ne!(zobrist_hash(&w), zobrist_hash(&bl));
    }

    #[test]
    fn hash_changes_after_move() {
        let mut board = b("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let h1 = zobrist_hash(&board);
        board.apply_uci_move("e2e4").unwrap();
        let h2 = zobrist_hash(&board);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_castling_rights() {
        let full = b("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        let none = b("r3k2r/8/8/8/8/8/8/R3K2R w - - 0 1");
        assert_ne!(zobrist_hash(&full), zobrist_hash(&none));
    }

    #[test]
    fn tt_capacity_is_power_of_two() {
        let tt = TranspositionTable::new(1);
        assert!(tt.capacity().is_power_of_two());
    }

    #[test]
    fn tt_probe_miss_returns_none() {
        let tt = TranspositionTable::new(1);
        assert!(tt.probe(0xDEAD_BEEF).is_none());
    }

    #[test]
    fn tt_store_then_probe_hit() {
        let mut tt = TranspositionTable::new(1);
        tt.store(TTEntry {
            key: 0x1234_5678,
            score: 42,
            best_move: None,
            depth: 5,
            bound: Bound::Exact,
        });
        let entry = tt.probe(0x1234_5678).unwrap();
        assert_eq!(entry.score, 42);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.bound, Bound::Exact);
    }

    #[test]
    fn tt_clear_evicts_entries() {
        let mut tt = TranspositionTable::new(1);
        tt.store(TTEntry {
            key: 0x1234,
            score: 7,
            best_move: None,
            depth: 1,
            bound: Bound::Exact,
        });
        assert!(tt.probe(0x1234).is_some());
        tt.clear();
        assert!(tt.probe(0x1234).is_none());
    }
}
