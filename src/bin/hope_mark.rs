//! The second false-hope mark of docs/sweep (Nano, 2026-10-30), judged from
//! the CSV that `sweep` writes: steps wasted on extra false hopes must cost
//! at most half the steps the fix saves on sleeps, per 1,000 steps of the
//! barren test world, paired by seed, at the 90% interval's worst end.

use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;

use apprentice::judge::Interval;

const USAGE: &str = "usage: hope_mark CSV

Reads seed, setting (A and S), barren_sleeps and barren_hopes from CSV
(as written by `sweep`) and prints the mark's verdict.";

/// Steps wasted per false hope and lost per sleep, measured on the default
/// apprentice, seeds 101-110 (`cargo run --release --example hope_costs`).
const W: f64 = 0.50;
const S: f64 = 3.69;
/// Steps of the barren test world, in thousands; false hopes are counted
/// over its first 1,000 steps only.
const BARREN_THOUSANDS: f64 = 2.0;

type Error = Box<dyn std::error::Error>;

/// Per seed: (false hopes, sleeps) for A and for S.
type Pairs = BTreeMap<u64, [Option<(f64, f64)>; 2]>;

fn read(text: &str) -> Result<Pairs, Error> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().ok_or("empty CSV")?.split(',').collect();
    let col = |name: &str| {
        head.iter()
            .position(|h| *h == name)
            .ok_or(format!("no column {name}"))
    };
    let (seed, setting, sleeps, hopes) = (
        col("seed")?,
        col("setting")?,
        col("barren_sleeps")?,
        col("barren_hopes")?,
    );
    let mut pairs = Pairs::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let arm = match f[setting] {
            "A" => 0,
            "S" => 1,
            _ => continue,
        };
        let entry = pairs.entry(f[seed].parse()?).or_insert([None, None]);
        entry[arm] = Some((f[hopes].parse()?, f[sleeps].parse()?));
    }
    Ok(pairs)
}

/// Per seed: extra false hopes x W, less half the sleeps saved x S, per
/// 1,000 steps. At most 0 passes.
fn excess(pairs: &Pairs) -> Result<Vec<f64>, Error> {
    pairs
        .iter()
        .map(|(seed, arms)| {
            let [Some((ha, sa)), Some((hs, ss))] = *arms else {
                return Err(format!("seed {seed} lacks a setting").into());
            };
            let saved = (sa - ss) / BARREN_THOUSANDS;
            Ok((hs - ha) * W - 0.5 * saved * S)
        })
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let run = || -> Result<(), Error> {
        let pairs = read(&fs::read_to_string(path)?)?;
        let i = Interval::mean(&excess(&pairs)?)?;
        println!(
            "3b'. wasted on extra false hopes less half the sleep steps saved, per 1,000 steps: {:+.2} [{:+.2}, {:+.2}] steps over {} seeds: {}",
            i.mid,
            i.lo,
            i.hi,
            pairs.len(),
            i.at_most(0.0)
        );
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("hope_mark: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_hopes_are_weighed_against_half_the_sleeps_saved() {
        let csv =
            "seed,setting,barren_hopes,barren_sleeps\n1,A,1,40\n1,S,3,20\n2,A,0,10\n2,S,0,10\n";
        let pairs = read(csv).expect("read");
        let e = excess(&pairs).expect("paired");
        assert!((e[0] - (2.0 * W - 0.5 * 10.0 * S)).abs() < 1e-12);
        assert_eq!(e[1], 0.0);
        assert!(
            excess(&read("seed,setting,barren_hopes,barren_sleeps\n1,A,1,4\n").expect("read"))
                .is_err()
        );
    }
}
