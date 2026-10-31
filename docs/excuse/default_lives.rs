//! Byte-identity check for deleting the regrowth excuse (docs/excuse). Copy
//! into examples/ and run at 2bf8f1c and after; the outputs must match.
use apprentice::life::lives_under;
use apprentice::memory::{Gate, Recall};

fn main() {
    let seeds: Vec<u64> = (1..=8).collect();
    let settings = [
        Recall::default(),
        Recall {
            gate: Some(Gate::default()),
            ..Recall::default()
        },
    ];
    for (seed, lives) in lives_under(&seeds, &settings).expect("lives") {
        for (i, l) in lives.iter().enumerate() {
            println!("{seed} {i} {l:?}");
        }
    }
}
