//! Mentor experiment: does a mentor's pointing speed the apprentice up, and
//! does the apprentice lean on it or inherit its blind spots?
//!
//! Each mind lives 8,000 steps in the bush world, seeds FROM..FROM+N-1, with
//! no mentor, a mentor that sees the real map, or a blind mentor whose map
//! holds food that isn't there. Sleeps and meals are counted per 2,000 steps.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::memory::{Memory, Recall};
use apprentice::mentor::{Mentor, Teaching};
use apprentice::metrics::Metrics;
use apprentice::model::Rates;
use apprentice::sim::{Sim, SimError};

const USAGE: &str = "\
usage: mentor [--seeds N] [--from SEED] [--out DIR]

Lives seeds FROM..FROM+N-1 (default 1..10) for 8,000 steps in the bush world
with no mentor, a true mentor and a blind mentor. Prints median sleeps and
meals per 2,000 steps and the final trust, and writes DIR/mentor.csv
(default DIR: runs/mentor).";

type Error = Box<dyn std::error::Error>;

const STEPS: usize = 8_000;
const WINDOW: usize = 2_000;
const WINDOWS: usize = STEPS / WINDOW;
/// Squares the blind mentor wrongly believes hold food (the world has 5).
const PHANTOMS: usize = 5;
const MINDS: [&str; 3] = ["no mentor", "mentor", "blind mentor"];

/// One life's numbers.
struct Life {
    sleeps: [u64; WINDOWS],
    meals: [u64; WINDOWS],
    trust: f64,
    hints: u32,
}

fn live(name: &str, seed: u64) -> Result<Life, SimError> {
    let mut sim = Sim::new(Setup::for_change(Change::Food).config, seed)?;
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
    let teaching = Teaching::default();
    let mut mentor = match name {
        "mentor" => Some(Mentor::new(teaching, seed)),
        "blind mentor" => Some(Mentor::blind(teaching, &sim, PHANTOMS, seed)),
        _ => None,
    };
    let mut windows = vec![Metrics::default(); WINDOWS];
    let mut hints = 0;
    for t in 0..STEPS {
        if let Some(at) = mentor.as_mut().and_then(|m| m.hint(&sim)) {
            mind.hear(at);
            hints += 1;
        }
        let action = mind.act(&sim.observe());
        let outcome = sim.step(action);
        mind.feel(&outcome);
        windows[t / WINDOW].record(&outcome);
    }
    let count =
        |f: fn(&Metrics) -> u64| -> [u64; WINDOWS] { std::array::from_fn(|i| f(&windows[i])) };
    Ok(Life {
        sleeps: count(|m| m.summary().sleeps),
        meals: count(|m| m.summary().meals),
        trust: mind.trust(),
        hints,
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
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/mentor"));
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
    let mut csv = String::from("mind,seed,window,sleeps,meals,trust,hints\n");
    for name in MINDS {
        let mut lives = Vec::new();
        for seed in from..from + seeds {
            let life = live(name, seed)?;
            for w in 0..WINDOWS {
                csv.push_str(&format!(
                    "{name},{seed},{},{},{},{:.3},{}\n",
                    w + 1,
                    life.sleeps[w],
                    life.meals[w],
                    life.trust,
                    life.hints
                ));
            }
            lives.push(life);
        }
        let med = |f: &dyn Fn(&Life) -> f64| median(lives.iter().map(f).collect());
        let sleeps: Vec<String> = (0..WINDOWS)
            .map(|w| format!("{:.0}", med(&|l| l.sleeps[w] as f64)))
            .collect();
        let meals: Vec<String> = (0..WINDOWS)
            .map(|w| format!("{:.0}", med(&|l| l.meals[w] as f64)))
            .collect();
        println!(
            "{name:>12}: sleeps per 2,000 steps [{}], meals [{}], sleeps 1-4,000 {:.1}, trust {:.2}, hints {:.0}",
            sleeps.join(", "),
            meals.join(", "),
            med(&|l| (l.sleeps[0] + l.sleeps[1]) as f64),
            med(&|l| l.trust),
            med(&|l| f64::from(l.hints)),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("mentor.csv"), csv)?;
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
        eprintln!("mentor: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
