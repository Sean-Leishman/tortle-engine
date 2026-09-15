mod torte;

use crate::torte::torte::Torte;

fn main() {
    // `torte tune <quiet-labeled.epd> [epochs] [lambda]` — see torte/tune.rs.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("tune") {
        crate::torte::movegen::magic::init();
        let epochs = args.get(3).and_then(|e| e.parse().ok()).unwrap_or(2000);
        let lambda = args.get(4).and_then(|l| l.parse().ok()).unwrap_or(0.0);
        crate::torte::tune::run(&args[2], epochs, lambda);
        return;
    }
    let mut torte = Torte::new();
    torte.run();
}
