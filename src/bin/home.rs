//! Home experiment: does the apprentice know its own world?
//!
//! It lives 3,000 steps in a small bush world, its home. Then, as it is at
//! that moment, it is put in a world for 300 steps: home as it left it, home
//! with one wall moved, or one of three strangers built to the same recipe.
//! It is never told which. "Is this home?" is answered from its own catalog:
//! yes if the map it is using is the one it used at home.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::apprentice::Mind;
use apprentice::home::{Place, at_home, step, world};
use apprentice::sim::SimError;

const USAGE: &str = "\
usage: home [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10), one home each. Prints how often it said
yes at home, no to a stranger, and yes to home with a wall moved, after 100
and 300 steps, and writes DIR/home.csv (default DIR: runs/home).";

type Error = Box<dyn std::error::Error>;

const TRIAL_STEPS: usize = 300;
/// Steps into a trial at which the answer is judged, and a later look.
const ASKED: usize = 100;
const LATER: usize = 300;
const STRANGERS: u64 = 3;

/// How one trial went.
struct Trial {
    place: Place,
    /// Its answer, "is this home?", after each step.
    answers: Vec<bool>,
}

impl Trial {
    fn says_home(&self, step: usize) -> bool {
        self.answers[step - 1]
    }

    fn right(&self, step: usize) -> bool {
        self.says_home(step) == self.place.is_home()
    }

    /// Steps until it first said no; `None` if it never did.
    fn first_no(&self) -> Option<usize> {
        self.answers.iter().position(|&yes| !yes).map(|i| i + 1)
    }
}

fn trial(place: Place, seed: u64) -> Result<Trial, SimError> {
    let (mut mind, home, home_map) = at_home(seed)?;
    let mut sim = world(place, &home, seed);
    let answers = (0..TRIAL_STEPS)
        .map(|_| {
            step(&mut sim, &mut mind);
            mind.map_in_use() == home_map
        })
        .collect();
    Ok(Trial { place, answers })
}

fn places() -> Vec<Place> {
    let strangers = (0..STRANGERS).map(Place::Stranger);
    [Place::Home, Place::Changed]
        .into_iter()
        .chain(strangers)
        .collect()
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
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/home"));
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
    let mut csv = String::from("seed,place,answer_100,answer_300,first_no\n");
    let mut trials = Vec::new();
    for seed in from..from + seeds {
        for place in places() {
            let t = trial(place, seed)?;
            let answer = |s| if t.says_home(s) { "yes" } else { "no" };
            csv.push_str(&format!(
                "{seed},{},{},{},{}\n",
                place.name(),
                answer(ASKED),
                answer(LATER),
                t.first_no().map_or(String::new(), |s| s.to_string())
            ));
            trials.push(t);
        }
    }
    for name in ["home", "changed home", "stranger"] {
        let kind: Vec<&Trial> = trials.iter().filter(|t| t.place.name() == name).collect();
        let right = |s| kind.iter().filter(|t| t.right(s)).count();
        println!(
            "{name:>13}: right after {ASKED} steps {}/{n}, after {LATER} steps {}/{n}",
            right(ASKED),
            right(LATER),
            n = kind.len(),
        );
    }
    let first_no: Vec<f64> = trials
        .iter()
        .filter(|t| !t.place.is_home())
        .map(|t| t.first_no().unwrap_or(TRIAL_STEPS + 1) as f64)
        .collect();
    println!(
        "steps before it first said no to a stranger: median {:.0}",
        median(first_no)
    );
    fs::create_dir_all(out)?;
    fs::write(out.join("home.csv"), csv)?;
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
        eprintln!("home: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stranger_is_judged_right_only_when_it_says_no() {
        let t = Trial {
            place: Place::Stranger(0),
            answers: vec![true, true, false, false],
        };
        assert!(!t.right(2) && t.right(3));
        assert_eq!(t.first_no(), Some(3));
    }
}
