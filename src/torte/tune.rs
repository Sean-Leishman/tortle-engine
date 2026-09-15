//! Texel tuning: fit every weight in `search::params::W` so that
//! `sigmoid(K · static_eval)` predicts game results on a labelled set of
//! quiet positions (`fen ... "1-0";` per line, e.g. Zurichess quiet-labeled).
//!
//! The eval is linear in its weights given a board, so each position is
//! traced once into sparse (weight, count) pairs plus its phase; after that an
//! epoch is a cheap sweep over those pairs. Full-batch gradient descent with
//! Adam, parallel across threads. Writes the result back to `params.rs`.

use std::fs;
use std::thread;

use crate::torte::board::board::Board;
use crate::torte::board::pieces::Color;
use crate::torte::search::eval::*;
use crate::torte::search::params::W;

const PARAMS_RS: &str = "src/torte/search/params.rs";

struct Coeffs([i32; NUM_PARAMS]);

impl Trace for Coeffs {
    fn add(&mut self, param: usize, n: i32) {
        self.0[param] += n;
    }
}

struct Position {
    terms: Vec<(u16, i16)>,
    mg_frac: f64, // phase / PHASE_MAX
    result: f64,  // 1 white win, 0.5 draw, 0 black win
}

/// White-POV eval of `p` under weights `w` (flat: mg at 2i, eg at 2i+1).
fn model(p: &Position, w: &[f64]) -> f64 {
    let (mut mg, mut eg) = (0.0, 0.0);
    for &(i, n) in &p.terms {
        mg += w[2 * i as usize] * n as f64;
        eg += w[2 * i as usize + 1] * n as f64;
    }
    mg * p.mg_frac + eg * (1.0 - p.mg_frac)
}

fn sigmoid(k: f64, e: f64) -> f64 {
    1.0 / (1.0 + (-k * e).exp())
}

fn load(path: &str) -> Vec<Position> {
    let text = fs::read_to_string(path).expect("read dataset");
    let w0: Vec<f64> = W.iter().flatten().map(|&x| x as f64).collect();
    text.lines()
        .filter_map(|line| {
            let result = match line.split('"').nth(1)? {
                "1-0" => 1.0,
                "0-1" => 0.0,
                "1/2-1/2" => 0.5,
                _ => return None,
            };
            let fen: Vec<&str> = line.split_whitespace().take(4).collect();
            let board = Board::parse(&format!("{} 0 1", fen.join(" ")));
            let mut c = Coeffs([0; NUM_PARAMS]);
            trace(&board, EvalConfig::all(), &mut c);
            let phase = game_phase(&board);
            let p = Position {
                terms: (0..NUM_PARAMS)
                    .filter(|&i| c.0[i] != 0)
                    .map(|i| (i as u16, c.0[i] as i16))
                    .collect(),
                mg_frac: phase as f64 / PHASE_MAX as f64,
                result,
            };
            // The float model must reproduce the engine's integer eval.
            let engine = eval(&board, EvalConfig::all())
                * if board.side_to_move == Color::White { 1 } else { -1 };
            assert!((model(&p, &w0) - engine as f64).abs() <= 1.0, "trace mismatch: {line}");
            Some(p)
        })
        .collect()
}

/// Mean squared error and (if `want_grad`) its gradient, across threads.
fn loss_and_grad(data: &[Position], w: &[f64], k: f64, want_grad: bool) -> (f64, Vec<f64>) {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = data.len().div_ceil(threads);
    let parts: Vec<(f64, Vec<f64>)> = thread::scope(|s| {
        let handles: Vec<_> = data
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    let mut loss = 0.0;
                    let mut grad = vec![0.0; if want_grad { w.len() } else { 0 }];
                    for p in part {
                        let sg = sigmoid(k, model(p, w));
                        let err = sg - p.result;
                        loss += err * err;
                        if want_grad {
                            let d = 2.0 * err * sg * (1.0 - sg) * k;
                            for &(i, n) in &p.terms {
                                grad[2 * i as usize] += d * n as f64 * p.mg_frac;
                                grad[2 * i as usize + 1] += d * n as f64 * (1.0 - p.mg_frac);
                            }
                        }
                    }
                    (loss, grad)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = data.len() as f64;
    let mut grad = vec![0.0; if want_grad { w.len() } else { 0 }];
    let mut loss = 0.0;
    for (l, g) in parts {
        loss += l;
        grad.iter_mut().zip(g).for_each(|(a, b)| *a += b / n);
    }
    (loss / n, grad)
}

/// `lambda` is an L2 pull toward the weights the run *starts* from (whatever
/// `params.rs` held at build time), per centipawn² per weight. It keeps rarely
/// seen weights — a middlegame king on the 7th rank — from chasing noise. To
/// anchor on a specific table, check that `params.rs` out first and rebuild.
pub fn run(path: &str, epochs: usize, lambda: f64) {
    let (mut data, mut valid) = (Vec::new(), Vec::new());
    for (i, p) in load(path).into_iter().enumerate() {
        if i % 10 == 0 { valid.push(p) } else { data.push(p) }
    }
    let mut w: Vec<f64> = W.iter().flatten().map(|&x| x as f64).collect();
    let w0 = w.clone();
    println!(
        "{} train / {} held-out positions, {} weights, lambda {lambda:e}",
        data.len(),
        valid.len(),
        w.len()
    );

    // Fit the eval-to-probability scale K to the *current* weights, then hold
    // it fixed — otherwise K and the weights trade off against each other.
    let (mut lo, mut hi) = (0.0005_f64, 0.05_f64);
    for _ in 0..40 {
        let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if loss_and_grad(&data, &w, a, false).0 < loss_and_grad(&data, &w, b, false).0 {
            hi = b;
        } else {
            lo = a;
        }
    }
    let k = (lo + hi) / 2.0;
    println!("K = {k:.6} per cp, start loss {:.6}", loss_and_grad(&data, &w, k, false).0);

    // Adam. lr is in centipawns per step.
    let (lr, b1, b2) = (1.0, 0.9, 0.999);
    let mut m = vec![0.0; w.len()];
    let mut v = vec![0.0; w.len()];
    for epoch in 1..=epochs {
        let (loss, g) = loss_and_grad(&data, &w, k, true);
        for i in 0..w.len() {
            let g = g[i] + 2.0 * lambda * (w[i] - w0[i]);
            m[i] = b1 * m[i] + (1.0 - b1) * g;
            v[i] = b2 * v[i] + (1.0 - b2) * g * g;
            let mh = m[i] / (1.0 - b1.powi(epoch as i32));
            let vh = v[i] / (1.0 - b2.powi(epoch as i32));
            w[i] -= lr * mh / (vh.sqrt() + 1e-12);
        }
        if epoch % 100 == 0 || epoch == epochs {
            let held_out = loss_and_grad(&valid, &w, k, false).0;
            println!("epoch {epoch} train {loss:.6} held-out {held_out:.6}");
        }
    }
    fs::write(PARAMS_RS, render(&w)).expect("write params.rs");
    println!("wrote {PARAMS_RS}");
}

fn render(w: &[f64]) -> String {
    const SECTIONS: [(&str, usize, usize, usize); 18] = [
        ("material: P N B R Q K", MATERIAL, 6, 6),
        ("pawn PST, a1..h8", PST, 64, 8),
        ("knight PST, a1..h8", PST + 64, 64, 8),
        ("bishop PST, a1..h8", PST + 128, 64, 8),
        ("rook PST, a1..h8", PST + 192, 64, 8),
        ("queen PST, a1..h8", PST + 256, 64, 8),
        ("king PST, a1..h8", PST + 320, 64, 8),
        ("mobility per square: N B R Q", MOBILITY, 4, 4),
        ("doubled pawn (per extra pawn on a file)", DOUBLED_PAWN, 1, 1),
        ("isolated pawn", ISOLATED_PAWN, 1, 1),
        ("passed pawn by own rank 1..8", PASSED_PAWN, 8, 8),
        ("bishop pair", BISHOP_PAIR, 1, 1),
        ("king shield pawn: home rank, advanced one", SHIELD_PAWN_HOME, 2, 2),
        ("undeveloped minor", UNDEVELOPED_MINOR, 1, 1),
        ("early queen (per undeveloped minor)", EARLY_QUEEN, 1, 1),
        ("tempo", TEMPO, 1, 1),
        ("rook file: open, half-open", ROOK_OPEN_FILE, 2, 2),
        ("king attack per zone square: N B R Q", KING_ATTACK, 4, 4),
    ];
    let mut out = String::from(
        "// Eval weights as [middlegame, endgame] centipawn pairs, indexed by the\n\
         // offsets in `eval.rs`. Generated by `torte tune` — rerun it rather than\n\
         // hand-editing.\n\nuse super::eval::NUM_PARAMS;\n\n#[rustfmt::skip]\n\
         pub const W: [[i32; 2]; NUM_PARAMS] = [\n",
    );
    for (name, off, len, per_line) in SECTIONS {
        out += &format!("    // {name}\n");
        for row in (off..off + len).step_by(per_line) {
            out += "   ";
            for j in row..(row + per_line).min(off + len) {
                let (mg, eg) = (w[2 * j].round() as i32, w[2 * j + 1].round() as i32);
                out += &format!(" [{mg:4}, {eg:4}],");
            }
            out += "\n";
        }
    }
    out + "];\n"
}
