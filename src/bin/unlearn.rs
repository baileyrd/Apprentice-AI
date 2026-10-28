//! Unlearning experiment: does the apprentice let go of a pattern once the
//! world stops following it, and does a longer-held belief take longer?
//!
//! Each apprentice grows up in the bush world (an eaten bush grows back in
//! place) for 1,000, 3,000 or 9,000 steps, then lives 4,000 steps in a new
//! world where eaten bushes never grow back and new food springs up anywhere.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::memory::{Memory, Recall};
use apprentice::model::{Rates, kind};
use apprentice::patterns::Last;
use apprentice::sim::{Config, Event, Regrowth, Sim, SimError};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: unlearn [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10). For upbringings of 1,000, 3,000 and
9,000 steps in the bush world, prints how many seeds believed eaten bushes
grow back, how long that belief took to fall below 0.5 in a world where they
never do, and futile returns early and late; writes DIR/unlearn.csv (default
DIR: runs/unlearn).";

type Error = Box<dyn std::error::Error>;
type Learner = Apprentice<Memory>;

const UPBRINGINGS: [usize; 3] = [1_000, 3_000, 9_000];
const AFTER: usize = 4_000;
const WINDOW: usize = 1_000;
/// Age of the belief read: a square eaten from 64–127 steps ago.
const AGE: u64 = 64;
const BELIEVES: f64 = 0.5;
/// Steps after a meal in which a bush would have grown back in the bush
/// world, and up to which the belief is read.
const REGROWN: usize = 31;
const REGROWN_UNTIL: usize = 128;

/// Chance of food it gives a square it ate from `AGE` steps ago; 0 until it
/// has learned that pattern.
fn belief(mind: &Learner) -> f64 {
    mind.model()
        .expects(Last::Ate, AGE)
        .map_or(0.0, |b| b[kind(Cell::Food)])
}

/// One life: what it believed when it moved, when that fell below
/// `BELIEVES`, and futile returns in the first and last `WINDOW` steps.
struct Life {
    before: f64,
    unlearned: Option<usize>,
    early: u32,
    late: u32,
    /// Futile returns to a square eaten `REGROWN`–`REGROWN_UNTIL` steps
    /// before, when a bush would have grown back: first and last window.
    expected: (u32, u32),
}

fn barren(bushes: &Config) -> Config {
    Config {
        regrowth: Regrowth::Anywhere,
        ..bushes.clone()
    }
}

fn live(upbringing: usize, seed: u64) -> Result<Life, SimError> {
    let bushes = Setup::for_change(Change::Food).config;
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
    let mut home = Sim::new(bushes.clone(), seed)?;
    for _ in 0..upbringing {
        let outcome = home.step(mind.act(&home.observe()));
        mind.feel(&outcome);
    }
    let before = belief(&mind);
    let mut sim = Sim::new(barren(&bushes), seed + 5_000)?;
    let mut eaten: HashMap<Pos, usize> = HashMap::new();
    let mut expected = (0, 0);
    let (mut unlearned, mut early, mut late) = (None, 0, 0);
    for t in 0..AFTER {
        let here = sim.pos();
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        match outcome.event {
            Event::Ate => {
                eaten.insert(here, t);
            }
            Event::AteNothing if eaten.contains_key(&here) => {
                let since = t - eaten[&here];
                if (REGROWN..REGROWN_UNTIL).contains(&since) {
                    expected.0 += u32::from(t < WINDOW);
                    expected.1 += u32::from(t >= AFTER - WINDOW);
                }
                if t < WINDOW {
                    early += 1;
                }
                if t >= AFTER - WINDOW {
                    late += 1;
                }
            }
            _ => {}
        }
        if unlearned.is_none() && belief(&mind) < BELIEVES {
            unlearned = Some(t + 1);
        }
    }
    Ok(Life {
        before,
        unlearned,
        early,
        late,
        expected,
    })
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => xs[n / 2],
        _ => (xs[n / 2 - 1] + xs[n / 2]) / 2.0,
    }
}

/// Parsed flags: seed count, first seed, output directory.
fn parse(args: &[String]) -> Result<(u64, u64, PathBuf), String> {
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/unlearn"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || value.parse().map_err(|_| format!("bad {flag}: {value}"));
        match flag.as_str() {
            "--seeds" => seeds = number()?,
            "--from" => from = number()?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if seeds == 0 {
        return Err("--seeds must be positive".into());
    }
    Ok((seeds, from, out))
}

fn execute(seeds: u64, from: u64, out: &PathBuf) -> Result<(), Error> {
    let mut csv = String::from(
        "upbringing,seed,belief_before,unlearned,early_futile,late_futile,early_expected,late_expected\n",
    );
    for upbringing in UPBRINGINGS {
        let mut lives = Vec::new();
        for seed in from..from + seeds {
            let l = live(upbringing, seed)?;
            csv.push_str(&format!(
                "{upbringing},{seed},{:.3},{},{},{},{},{}\n",
                l.before,
                l.unlearned.map_or(String::new(), |s| s.to_string()),
                l.early,
                l.late,
                l.expected.0,
                l.expected.1
            ));
            lives.push(l);
        }
        let counted: Vec<&Life> = lives.iter().filter(|l| l.before > BELIEVES).collect();
        let done = counted.iter().filter(|l| l.unlearned.is_some()).count();
        let steps = median(
            counted
                .iter()
                .map(|l| l.unlearned.unwrap_or(AFTER + 1) as f64)
                .collect(),
        );
        let med = |f: fn(&Life) -> u32| median(lives.iter().map(|l| f64::from(f(l))).collect());
        println!(
            "{upbringing:>5} steps raised: believed {}/{}, belief {:.2}; unlearned {done}/{} in median {steps:.0} steps; futile returns first 1,000 {:.1}, last 1,000 {:.1}; when it would have regrown {:.1} / {:.1}",
            counted.len(),
            lives.len(),
            median(lives.iter().map(|l| l.before).collect()),
            counted.len(),
            med(|l| l.early),
            med(|l| l.late),
            med(|l| l.expected.0),
            med(|l| l.expected.1),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("unlearn.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seeds, from, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(seeds, from, &out) {
        eprintln!("unlearn: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
