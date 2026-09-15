//! `torte bench [depth]` — fixed positions, fixed depth, total nodes.
//!
//! Node counts are deterministic, so this measures search work on a machine
//! too busy to play honest games. A change that cuts nodes at equal depth is
//! doing less work for the same answer; whether that is *Elo* still needs
//! `bench/sprt.sh`. Positions are the usual suspects: openings, kiwipete,
//! tactical middlegames, and endgames where depth matters most.

use std::time::Instant;

use crate::torte::board::board::Board;
use crate::torte::search::search::{iterative_deepening, SearchConfig};

const POSITIONS: [&str; 8] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", // kiwipete
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "8/8/1p1k4/1P6/8/3p3P/1r4P1/5K2 w - - 0 1",
    "2rq1rk1/pp1bppbp/2np1np1/8/2BNP3/2N1BP2/PPPQ2PP/2KR3R w - - 0 11",
    "4rrk1/pp1n1ppp/2pb4/3p4/3P4/2NBPN2/PP3PPP/2R2RK1 w - - 0 15",
    "8/5k2/3p4/1p1Pp2p/pP2Pp1P/P4P1K/8/8 b - - 0 1",
];

pub fn run(depth: u32) {
    let config = SearchConfig::default();
    let start = Instant::now();
    let mut total = 0_u64;
    for (i, fen) in POSITIONS.iter().enumerate() {
        let board = Board::parse(fen);
        let mut nodes = 0_u64;
        let best = iterative_deepening(&board, depth, config, None, |_, _, _, _, n| nodes += n);
        total += nodes;
        let mv = best.map_or("none".to_string(), |(m, _)| m.to_uci());
        println!("{:>2}. {:>12} nodes  {}", i + 1, nodes, mv);
    }
    let ms = start.elapsed().as_millis().max(1);
    println!(
        "bench depth {depth}: {total} nodes in {ms} ms ({} knps)",
        total / ms as u64
    );
}
