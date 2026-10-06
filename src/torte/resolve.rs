//! `torte resolve <in.epd> <out.epd>` — turn labelled positions into positions
//! whose *static* eval is meaningful, by playing out the capture sequence.
//!
//! The Zurichess quiet-labeled set gets its quiet positions by **discarding**
//! any position where a capture or check is available. That is why a king
//! danger term fitted on it learned nothing (2026-10-06): attacking middlegames
//! are exactly the positions it throws away. Resolving instead of filtering
//! keeps the position — the attack, the king's exposure, the piece placement —
//! and merely walks it forward to the end of the forcing sequence, which is
//! what the static eval is supposed to score.
//!
//! Resolution is deliberately cheap: repeatedly play the highest-SEE capture
//! that wins material, at most `MAX_PLIES` times. It is not a quiescence
//! search — there is no stand-pat alternative — but a capture with SEE > 0 is
//! materially good by construction, so playing it is sound enough for data.

use std::fs;

use crate::torte::board::board::Board;
use crate::torte::board::pieces::{Color, Piece};
use crate::torte::core::piece_move::Move;
use crate::torte::movegen::generator::{
    generate_pseudo_legal_moves, is_attacked, king_square, own_king_safe,
};
use crate::torte::search::see::see;

const MAX_PLIES: usize = 12;

fn in_check(board: &Board) -> bool {
    match king_square(board, board.side_to_move) {
        Some(k) => is_attacked(board, k, board.side_to_move.opposite()),
        None => false,
    }
}

fn legal_moves(board: &Board) -> Vec<Move> {
    generate_pseudo_legal_moves(board)
        .into_iter()
        .filter(|&m| {
            let mut next = *board;
            next.apply_move(m).is_ok() && own_king_safe(&next, board.side_to_move)
        })
        .collect()
}

/// Walk forward through winning captures. Returns None if the position is in
/// check at the end (or start) — a side in check has no quiet evaluation, and
/// those are a small minority.
fn resolve(board: &Board) -> Option<Board> {
    let mut board = *board;
    for _ in 0..MAX_PLIES {
        if in_check(&board) {
            return None;
        }
        let best = legal_moves(&board)
            .into_iter()
            .filter(|&m| see(&board, m) > 0)
            .max_by_key(|&m| see(&board, m));
        match best {
            Some(m) => {
                board.apply_move(m).ok()?;
            }
            None => return Some(board),
        }
    }
    if in_check(&board) { None } else { Some(board) }
}

fn to_fen(board: &Board) -> String {
    let mut s = String::new();
    for rank in (0..8).rev() {
        let mut empty = 0;
        for file in 0..8 {
            let sq = rank * 8 + file;
            let piece = (0..12).find(|&i| board.bbs[i].get(sq));
            match piece {
                Some(i) => {
                    if empty > 0 {
                        s.push_str(&empty.to_string());
                        empty = 0;
                    }
                    s.push(glyph(i));
                }
                None => empty += 1,
            }
        }
        if empty > 0 {
            s.push_str(&empty.to_string());
        }
        if rank > 0 {
            s.push('/');
        }
    }
    s.push(' ');
    s.push(if board.side_to_move == Color::White { 'w' } else { 'b' });
    s.push(' ');
    let c = board.castling;
    let mut any = false;
    for (flag, ch) in [(1_u8, 'K'), (2, 'Q'), (4, 'k'), (8, 'q')] {
        if c.has(flag) {
            s.push(ch);
            any = true;
        }
    }
    if !any {
        s.push('-');
    }
    s.push(' ');
    match board.en_passant {
        Some(sq) => {
            let i = sq.to_usize();
            s.push((b'a' + (i % 8) as u8) as char);
            s.push((b'1' + (i / 8) as u8) as char);
        }
        None => s.push('-'),
    }
    s.push_str(&format!(" {} {}", board.halfmove_clock, board.fullmove_number));
    s
}

fn glyph(piece_index: usize) -> char {
    let c = match Piece::from_index(piece_index) {
        Piece::WhitePawn | Piece::BlackPawn => 'p',
        Piece::WhiteKnight | Piece::BlackKnight => 'n',
        Piece::WhiteBishop | Piece::BlackBishop => 'b',
        Piece::WhiteRook | Piece::BlackRook => 'r',
        Piece::WhiteQueen | Piece::BlackQueen => 'q',
        Piece::WhiteKing | Piece::BlackKing => 'k',
    };
    if piece_index < 6 { c.to_ascii_uppercase() } else { c }
}

pub fn run(input: &str, output: &str) {
    let text = fs::read_to_string(input).expect("read input epd");
    let mut out = String::new();
    let (mut kept, mut dropped, mut moved) = (0_u64, 0_u64, 0_u64);
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let Some(result) = line.split('"').nth(1) else { continue };
        let fen: Vec<&str> = line.split_whitespace().take(6).collect();
        let board = Board::parse(&fen.join(" "));
        match resolve(&board) {
            Some(quiet) => {
                let f = to_fen(&quiet);
                if f != to_fen(&board) {
                    moved += 1;
                }
                // Dedupe on placement + side + rights, as the extractor does.
                let key: String = f.split_whitespace().take(4).collect::<Vec<_>>().join(" ");
                if !seen.insert(key) {
                    dropped += 1;
                    continue;
                }
                out.push_str(&format!("{f} c9 \"{result}\";\n"));
                kept += 1;
            }
            None => dropped += 1,
        }
    }
    fs::write(output, out).expect("write output epd");
    println!("{kept} positions written ({moved} moved by resolution), {dropped} dropped (in check or duplicate)");
}
