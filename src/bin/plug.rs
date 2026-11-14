//! Plug: an agent in another process lives the apprentice's tests beside it.
//!
//! Each seed, both the apprentice and the plugged agent live a bush world
//! (sleeps and meals), then take the home test: 3,000 steps at home, then 300
//! in home as left, home with a wall moved, or one of three strangers, fresh
//! each trial. "Is this home?" is answered by the place each names
//! (docs/port): the apprentice names the map it is using; a plugged agent
//! names whatever it reports, or nothing (n/a). Neither is told the world.

use std::process::ExitCode;

use apprentice::apprentice::{Apprentice, Drives};
use apprentice::episode::Port;
use apprentice::life::{self, BUSH_STEPS, Learner};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::plug::{Plugged, agent_seed};

const USAGE: &str = "\
usage: plug [--seeds N] [--from SEED] -- COMMAND [ARGS...]

Seeds FROM..FROM+N-1 (default 101..102, spent smoke seeds). COMMAND is
launched once per life with its seed as the last argument and spoken to in
JSON lines (docs/port/README.md). Prints sleeps and meals in a bush world and
wrong home-test answers after 100 and 300 steps, for the apprentice and for
the plugged agent.";

type Error = Box<dyn std::error::Error>;

/// One agent's totals over all seeds.
#[derive(Debug, Default)]
struct Tally {
    sleeps: u32,
    meals: u32,
    /// Wrong answers after `ASKED` and `TRIAL_STEPS`, over answered trials.
    wrong: (u32, u32),
    answered: u32,
    /// Trials it gave no answer in: no place named at the end of home.
    unanswered: u32,
}

/// Every test for one seed, with `make` building a fresh agent per life.
fn tally<P: Port>(
    tally: &mut Tally,
    seed: u64,
    mut make: impl FnMut() -> Result<P, Error>,
) -> Result<(), Error>
where
    P::Error: 'static,
{
    let seen = life::behave(&mut make()?, life::bushes(), seed, BUSH_STEPS)?;
    tally.sleeps += seen.sleeps;
    tally.meals += seen.meals;
    for place in life::trial_places() {
        match life::home_trial(&mut make()?, place, seed)? {
            None => tally.unanswered += 1,
            Some(answers) => {
                tally.answered += 1;
                tally.wrong.0 += u32::from(!answers.asked);
                tally.wrong.1 += u32::from(!answers.last);
            }
        }
    }
    Ok(())
}

fn apprentice(seed: u64) -> Learner {
    let memory = Memory::new(Rates::default(), Recall::default());
    Apprentice::with_model(Drives::default(), memory, seed)
}

fn row(name: &str, t: &Tally, seeds: u64) -> String {
    let per = |x: u32| f64::from(x) / seeds as f64;
    let wrong = if t.answered == 0 {
        "n/a".to_string()
    } else {
        format!("{} / {} / {}", t.wrong.0, t.wrong.1, t.answered)
    };
    format!(
        "{name:<10} {:>8.1} {:>8.1}   {wrong:<16} {:>4}",
        per(t.sleeps),
        per(t.meals),
        t.unanswered
    )
}

/// Parsed flags: seed count, first seed, the agent's command.
fn parse(args: &[String]) -> Result<(u64, u64, Vec<String>), String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("missing -- COMMAND")?;
    let (flags, cmd) = (&args[..split], &args[split + 1..]);
    if cmd.is_empty() {
        return Err("missing COMMAND after --".into());
    }
    let (mut seeds, mut from) = (2, 101);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let n: u64 = value.parse().map_err(|_| format!("bad number {value:?}"))?;
        match flag.as_str() {
            "--seeds" => seeds = n,
            "--from" => from = n,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    Ok((seeds, from, cmd.to_vec()))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (seeds, from, cmd) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(why) => {
            eprintln!("{why}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match go(seeds, from, &cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("plug: {e}");
            let mut cause = e.source();
            while let Some(c) = cause {
                eprintln!("  because: {c}");
                cause = c.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn go(seeds: u64, from: u64, cmd: &[String]) -> Result<(), Error> {
    let (mut mine, mut theirs) = (Tally::default(), Tally::default());
    for seed in from..from + seeds {
        tally(&mut mine, seed, || Ok(apprentice(seed)))?;
        tally(&mut theirs, seed, || {
            Ok(Plugged::launch(cmd, agent_seed(seed))?)
        })?;
    }
    println!("seeds {from}..{}", from + seeds - 1);
    println!("per seed: sleeps and meals in {BUSH_STEPS} bush-world steps");
    println!("agent        sleeps    meals   home wrong@100/@300/asked  n/a");
    println!("{}", row("apprentice", &mine, seeds));
    println!("{}", row("plugged", &theirs, seeds));
    Ok(())
}
