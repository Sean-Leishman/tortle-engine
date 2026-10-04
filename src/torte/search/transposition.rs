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
    /// Which search wrote this. Set by `store`; callers leave it 0.
    pub generation: u8,
}

pub struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize,
    generation: u8,
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
            generation: 0,
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

    /// Bump the age counter. Called once per search so that entries from
    /// earlier searches lose their depth privilege — without this a deep
    /// entry from ten moves ago outranks everything the current search finds
    /// and the table stops accepting new work.
    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// `depth_preferred`: keep the deeper entry when two positions collide,
    /// instead of always overwriting. Same position, or an entry from an
    /// older search, is always replaced.
    pub fn store(&mut self, mut entry: TTEntry, depth_preferred: bool) {
        entry.generation = self.generation;
        let idx = (entry.key as usize) & self.mask;
        let replace = match &self.table[idx] {
            None => true,
            Some(old) => {
                !depth_preferred
                    || old.key == entry.key
                    || old.generation != self.generation
                    || entry.depth >= old.depth
            }
        };
        if replace {
            self.table[idx] = Some(entry);
        }
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

/// Key for a given piece kind (0..12) on a given square (0..64). Used by
/// `Board::apply_move` to XOR-update the hash incrementally.
#[inline]
pub fn piece_key(kind: usize, sq: usize) -> u64 {
    zobrist().pieces[kind][sq]
}

/// Key XORed into the hash when it's Black to move.
#[inline]
pub fn side_key() -> u64 {
    zobrist().side
}

/// Key for the full 4-bit castling-rights mask.
#[inline]
pub fn castling_key(rights: u8) -> u64 {
    zobrist().castling[(rights & 0xF) as usize]
}

/// Key for an en-passant target file (0..8).
#[inline]
pub fn ep_file_key(file: u8) -> u64 {
    zobrist().en_passant_file[(file & 7) as usize]
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
    fn fresh_board_hash_matches_recompute() {
        // `Board::new()` must satisfy the same invariant as `parse`:
        // `zobrist` is the from-scratch hash of the position it describes.
        let board = Board::new();
        assert_eq!(board.zobrist, zobrist_hash(&board));
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

    /// Perft-style walk: assert the incrementally maintained key equals the
    /// from-scratch recomputation at *every* node of the tree.
    fn walk_checking_hash(board: &Board, depth: u32) -> u64 {
        use crate::torte::movegen::generator::generate_legal_moves;
        assert_eq!(
            board.zobrist,
            zobrist_hash(board),
            "incremental hash drifted from the recomputation at\n{:?}",
            board
        );
        if depth == 0 {
            return 1;
        }
        let mut nodes = 1;
        for m in generate_legal_moves(board) {
            let mut next = *board;
            next.apply_move(m)
                .unwrap_or_else(|e| panic!("legal move {} failed to apply: {}", m, e));
            nodes += walk_checking_hash(&next, depth - 1);
        }
        nodes
    }

    #[test]
    fn incremental_hash_matches_recompute_at_every_perft_node() {
        // The oracle test for incremental Zobrist: ~470k positions across six
        // trees chosen so that every state component the from-scratch hash
        // covers actually changes somewhere in the walk.
        use crate::torte::movegen::magic;
        magic::init();
        let cases: &[(&str, u32)] = &[
            // Startpos: double pushes setting and clearing the ep square.
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 4),
            // Kiwipete: castling available both sides and for both colours,
            // rooks on their home squares to be captured or moved.
            (
                "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
                3,
            ),
            // Perft position 3: en-passant captures and pawn races.
            ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),
            // Perft position 4: capture-promotions (b2xa1, a7xb8) plus castling.
            (
                "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
                3,
            ),
            // Perft position 5: promotions with rights still live on one side.
            ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3),
            // Bare rooks and kings: castling both ways for both colours, and
            // rook-takes-rook on a home square, which revokes a right by
            // *capture* rather than by the rook moving.
            ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", 3),
        ];
        let mut total = 0;
        for (fen, depth) in cases {
            total += walk_checking_hash(&b(fen), *depth);
        }
        assert!(
            total > 300_000,
            "the walk should be large enough to be convincing, visited {}",
            total
        );
    }

    #[test]
    fn incremental_hash_stays_in_sync_over_long_sequence() {
        // Play a long forced sequence (one fully-legal game prefix) and
        // verify the incrementally-maintained `board.zobrist` matches
        // `zobrist_hash(&board)` after every single move.
        use crate::torte::movegen::magic;
        magic::init();
        let mut board = b("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let moves = [
            "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6",
            "b1c3", "a7a6", "f1e2", "e7e6", "e1g1", "f8e7", "f2f4", "e8g8",
            "c1e3", "b8c6", "d1d2", "e6e5",
        ];
        for mv in moves {
            board.apply_uci_move(mv).unwrap();
            assert_eq!(board.zobrist, zobrist_hash(&board), "drift after {}", mv);
        }
    }

    #[test]
    fn incremental_hash_matches_recompute_after_each_move() {
        // For each move kind we care about (quiet, double-push, capture,
        // en-passant, promotion, castle) verify board.zobrist after
        // apply_move equals zobrist_hash(&board) recomputed from scratch.
        let cases: &[(&str, &str)] = &[
            // quiet
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", "b1c3"),
            // double push (sets ep)
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", "e2e4"),
            // capture
            ("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2", "e4d5"),
            // en passant
            ("rnbqkbnr/pppp1ppp/8/3Pp3/8/8/PPP1PPPP/RNBQKBNR w KQkq e6 0 2", "d5e6"),
            // promotion
            ("8/P7/8/8/8/8/8/4k2K w - - 0 1", "a7a8q"),
            // kingside castle (zeroes castling rights for the side)
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1"),
            // rook move (loses one castling right)
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "a1a2"),
            // queenside castle (other direction, rook jumps the king)
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1c1"),
            // black castles (rights for the *other* colour are cleared)
            ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8c8"),
            // rook captures a rook on its home square: revokes a right
            // without the owning rook or king ever moving
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "a1a8"),
            // capture-promotion to a knight (both the captured piece and the
            // promoted piece differ from the mover)
            ("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7b8n"),
            // double push that *replaces* an existing ep square with a new one
            ("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1", "d7d5"),
            // quiet move that clears a standing ep square
            ("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1", "b8c6"),
        ];
        for (fen, mv) in cases {
            let mut board = b(fen);
            board.apply_uci_move(mv).unwrap();
            assert_eq!(board.zobrist, zobrist_hash(&board), "mismatch after {} from {}", mv, fen);
        }
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
            generation: 0,
        }, true);
        let entry = tt.probe(0x1234_5678).unwrap();
        assert_eq!(entry.score, 42);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.bound, Bound::Exact);
    }

    #[test]
    fn depth_preferred_keeps_the_deeper_entry_within_a_search() {
        let mut tt = TranspositionTable::new(1);
        let deep = TTEntry { key: 1, score: 10, best_move: None, depth: 8, bound: Bound::Exact, generation: 0 };
        // Same slot, different position: 1 + table size collides with key 1.
        let shallow = TTEntry { key: 1 + (tt.mask as u64 + 1), score: 20, depth: 2, ..deep };
        tt.store(deep, true);
        tt.store(shallow, true);
        assert_eq!(tt.probe(1).map(|e| e.score), Some(10), "deeper entry should survive");
        // An entry from an older search has no such privilege.
        tt.new_search();
        tt.store(shallow, true);
        assert!(tt.probe(1).is_none(), "stale deep entry should be replaceable");
        // With the toggle off, the last writer always wins.
        let mut tt = TranspositionTable::new(1);
        tt.store(deep, false);
        tt.store(shallow, false);
        assert!(tt.probe(1).is_none());
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
            generation: 0,
        }, true);
        assert!(tt.probe(0x1234).is_some());
        tt.clear();
        assert!(tt.probe(0x1234).is_none());
    }
}
