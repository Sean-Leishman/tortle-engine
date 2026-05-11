use crate::torte::core::bitboard::Bitboard;
use crate::torte::core::sq::SQ;
use std::sync::OnceLock;

const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

struct Magic {
    mask: u64,
    magic: u64,
    shift: u32,
    attacks: Vec<u64>,
}

struct SlidingTables {
    rook: [Magic; 64],
    bishop: [Magic; 64],
}

static TABLES: OnceLock<SlidingTables> = OnceLock::new();

fn tables() -> &'static SlidingTables {
    TABLES.get_or_init(|| {
        let mut rng = Xorshift64::new(0xDEAD_BEEF_CAFE_BABE);
        let rook = std::array::from_fn(|sq| build_magic(sq, &ROOK_DIRS, &mut rng));
        let bishop = std::array::from_fn(|sq| build_magic(sq, &BISHOP_DIRS, &mut rng));
        SlidingTables { rook, bishop }
    })
}

fn relevant_mask(sq: usize, dirs: &[(i32, i32)]) -> u64 {
    let rank = (sq / 8) as i32;
    let file = (sq % 8) as i32;
    let mut mask: u64 = 0;
    for &(dr, df) in dirs {
        let mut r = rank + dr;
        let mut f = file + df;
        loop {
            let nr = r + dr;
            let nf = f + df;
            if !(0..8).contains(&nr) || !(0..8).contains(&nf) {
                break;
            }
            mask |= 1u64 << (r * 8 + f);
            r = nr;
            f = nf;
        }
    }
    mask
}

fn ray_attacks(sq: usize, occ: u64, dirs: &[(i32, i32)]) -> u64 {
    let rank = (sq / 8) as i32;
    let file = (sq % 8) as i32;
    let mut attacks: u64 = 0;
    for &(dr, df) in dirs {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let bit = 1u64 << (r * 8 + f);
            attacks |= bit;
            if (occ & bit) != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

fn build_magic(sq: usize, dirs: &[(i32, i32)], rng: &mut Xorshift64) -> Magic {
    let mask = relevant_mask(sq, dirs);
    let n_bits = mask.count_ones();
    let shift = 64 - n_bits;

    let mut subsets: Vec<u64> = Vec::new();
    let mut subset_attacks: Vec<u64> = Vec::new();
    let mut sub: u64 = 0;
    loop {
        subsets.push(sub);
        subset_attacks.push(ray_attacks(sq, sub, dirs));
        sub = sub.wrapping_sub(mask) & mask;
        if sub == 0 {
            break;
        }
    }

    let size = 1usize << n_bits;
    loop {
        let magic = rng.next_sparse();
        if n_bits >= 6 && (mask.wrapping_mul(magic) >> 56).count_ones() < 6 {
            continue;
        }
        let mut table = vec![0u64; size];
        let mut collision = false;
        for i in 0..subsets.len() {
            let idx = (subsets[i].wrapping_mul(magic) >> shift) as usize;
            if table[idx] == 0 {
                table[idx] = subset_attacks[i];
            } else if table[idx] != subset_attacks[i] {
                collision = true;
                break;
            }
        }
        if !collision {
            return Magic {
                mask,
                magic,
                shift,
                attacks: table,
            };
        }
    }
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
    fn next_sparse(&mut self) -> u64 {
        self.next() & self.next() & self.next()
    }
}

/// Force one-time magic-table initialization. Cheap if already initialized.
pub fn init() {
    let _ = tables();
}

pub fn rook_attacks(sq: SQ, occ: Bitboard) -> Bitboard {
    let t = tables();
    let m = &t.rook[sq.to_usize()];
    let idx = ((occ.board & m.mask).wrapping_mul(m.magic) >> m.shift) as usize;
    Bitboard::from_u64(m.attacks[idx])
}

pub fn bishop_attacks(sq: SQ, occ: Bitboard) -> Bitboard {
    let t = tables();
    let m = &t.bishop[sq.to_usize()];
    let idx = ((occ.board & m.mask).wrapping_mul(m.magic) >> m.shift) as usize;
    Bitboard::from_u64(m.attacks[idx])
}

pub fn queen_attacks(sq: SQ, occ: Bitboard) -> Bitboard {
    Bitboard::from_u64(rook_attacks(sq, occ).board | bishop_attacks(sq, occ).board)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bit(rank: u8, file: u8) -> u64 {
        1u64 << (rank * 8 + file)
    }

    fn bb_of(squares: &[(u8, u8)]) -> Bitboard {
        let mut bb: u64 = 0;
        for &(r, f) in squares {
            bb |= bit(r, f);
        }
        Bitboard::from_u64(bb)
    }

    #[test]
    fn rook_a1_empty_board() {
        // Rook on a1, empty board: full a-file (above) + full first rank (right of a1).
        let empty = Bitboard::from_u64(0);
        let attacks = rook_attacks(SQ::make(0, 0), empty);
        let expected = bb_of(&[
            (0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6), (0, 7),
            (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0),
        ]);
        assert_eq!(attacks, expected);
    }

    #[test]
    fn rook_d4_with_blockers() {
        // Rook on d4 (rank 3, file 3). Blocker on d6 (rank 5) and on b4 (file 1).
        // Expected attacks: d3, d2, d1, d5, d6 (incl), c4, b4 (incl), e4, f4, g4, h4.
        let occ = Bitboard::from_u64(bit(5, 3) | bit(3, 1));
        let attacks = rook_attacks(SQ::make(3, 3), occ);
        let expected = bb_of(&[
            (2, 3), (1, 3), (0, 3),
            (4, 3), (5, 3),
            (3, 2), (3, 1),
            (3, 4), (3, 5), (3, 6), (3, 7),
        ]);
        assert_eq!(attacks, expected);
    }

    #[test]
    fn bishop_d4_empty() {
        // d4 = (3, 3). Diagonals: a1-h8 and a7-g1.
        let empty = Bitboard::from_u64(0);
        let attacks = bishop_attacks(SQ::make(3, 3), empty);
        let expected = bb_of(&[
            (0, 0), (1, 1), (2, 2), (4, 4), (5, 5), (6, 6), (7, 7),
            (4, 2), (5, 1), (6, 0),
            (2, 4), (1, 5), (0, 6),
            (0, 6), // dup ok
            (2, 4),
            (4, 2),
            (4, 4),
        ]);
        assert_eq!(attacks, expected);
    }

    #[test]
    fn bishop_with_blocker() {
        // Bishop on c1 (rank 0, file 2). Blocker on f4 (rank 3, file 5).
        // NE ray: d2, e3, f4 (incl, blocked). NW ray: b2, a3. SE/SW: empty.
        let occ = Bitboard::from_u64(bit(3, 5));
        let attacks = bishop_attacks(SQ::make(0, 2), occ);
        let expected = bb_of(&[(1, 3), (2, 4), (3, 5), (1, 1), (2, 0)]);
        assert_eq!(attacks, expected);
    }

    #[test]
    fn queen_combines_rook_and_bishop() {
        // Sanity: queen attacks = rook | bishop.
        let occ = Bitboard::from_u64(bit(2, 2) | bit(5, 5));
        let sq = SQ::make(3, 3);
        let q = queen_attacks(sq, occ);
        let r = rook_attacks(sq, occ);
        let b = bishop_attacks(sq, occ);
        assert_eq!(q, Bitboard::from_u64(r.board | b.board));
    }

    #[test]
    fn rook_corner_h8() {
        // Rook on h8, blocker on h5 and on c8. Attacks: h7, h6, h5 (incl), g8, f8, e8, d8, c8 (incl).
        let occ = Bitboard::from_u64(bit(4, 7) | bit(7, 2));
        let attacks = rook_attacks(SQ::make(7, 7), occ);
        let expected = bb_of(&[
            (6, 7), (5, 7), (4, 7),
            (7, 6), (7, 5), (7, 4), (7, 3), (7, 2),
        ]);
        assert_eq!(attacks, expected);
    }
}
