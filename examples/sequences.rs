//! Each world of both life sequences, one by one, for today's default.
use std::thread;

use apprentice::apprentice::{Apprentice, Drives};
use apprentice::life::{
    BARREN_STEPS, BUSH_STEPS, Learner, Stay, UPBRINGING, WORLD_STEPS, barren, bushes, live,
};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;

fn sequence(seed: u64, bush_first: bool) -> Vec<Stay> {
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind: Learner = Apprentice::with_model(Drives::default(), memory, seed);
    let (first, second) = if bush_first {
        (bushes(), barren())
    } else {
        (barren(), bushes())
    };
    let mut out = Vec::new();
    for i in 0..UPBRINGING {
        let c = if i % 2 == 0 {
            first.clone()
        } else {
            second.clone()
        };
        out.push(live(&mut mind, c, seed + 100 * i, WORLD_STEPS).expect("sim"));
    }
    let (steps, offset) = if bush_first {
        (BUSH_STEPS, 6_000)
    } else {
        (BARREN_STEPS, 5_000)
    };
    out.push(live(&mut mind, first, seed + offset, steps).expect("sim"));
    out
}

fn main() {
    let seeds: Vec<u64> = (1..=40).collect();
    for bush_first in [false, true] {
        let all: Vec<Vec<Stay>> = thread::scope(|s| {
            let hs: Vec<_> = seeds
                .chunks(10)
                .map(|part| {
                    s.spawn(move || {
                        part.iter()
                            .map(|&sd| sequence(sd, bush_first))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            hs.into_iter()
                .flat_map(|h| h.join().expect("join"))
                .collect()
        });
        println!(
            "{}",
            if bush_first {
                "BUSH sequence"
            } else {
                "BARREN sequence"
            }
        );
        for w in 0..=UPBRINGING as usize {
            let n = all.len() as f64;
            let m = |f: fn(&Stay) -> u32| all.iter().map(|v| f64::from(f(&v[w]))).sum::<f64>() / n;
            println!(
                "  world {} sleeps {:.1} meals {:.1} hopes {:.1} futile {:.1}",
                w + 1,
                m(|s| s.sleeps),
                m(|s| s.meals),
                m(|s| s.hopes),
                m(|s| s.futile)
            );
        }
    }
}
