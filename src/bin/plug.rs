//! Plug: an agent in another process lives the apprentice's tests beside it.
//!
//! Each seed, both the apprentice and the plugged agent live a bush world
//! (sleeps and meals), then take the home test: 3,000 steps at home, then 300
//! in home as left, home with a wall moved, or one of three strangers, fresh
//! each trial. "Is this home?" is answered by the place each names
//! (docs/port): the apprentice names the map it is using; a plugged agent
//! names whatever it reports, or nothing (n/a). Neither is told the world.

use std::process::ExitCode;
use std::time::Duration;

use apprentice::apprentice::{Apprentice, Drives};
use apprentice::episode::Port;
use apprentice::home;
use apprentice::life::{self, BUSH_STEPS, Learner};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::plug::{DEADLINE, Plugged, agent_seed};

const USAGE: &str = "\
usage: plug [--seeds N] [--from SEED] [--deadline-ms MS] -- COMMAND [ARGS...]

Seeds FROM..FROM+N-1 (default 101..102, spent smoke seeds; N at least 1).
COMMAND is launched once per life with its seed as the last argument and
spoken to in JSON lines (docs/port/README.md); a write or reply that takes
over MS milliseconds (default 10,000) ends the run. Prints sleeps and meals
in a bush world and wrong home-test answers after 100 and 300 steps, for the
apprentice and for the plugged agent.";

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

/// What to run, checked.
#[derive(Debug, PartialEq, Eq)]
struct Args {
    from: u64,
    /// The last seed, at least `from`.
    last: u64,
    deadline: Duration,
    cmd: Vec<String>,
}

/// Parses and checks the flags, so no life starts on a bad seed range.
fn parse(args: &[String]) -> Result<Args, String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("missing -- COMMAND")?;
    let (flags, cmd) = (&args[..split], &args[split + 1..]);
    if cmd.is_empty() {
        return Err("missing COMMAND after --".into());
    }
    let (mut seeds, mut from, mut deadline) = (2, 101, DEADLINE);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let n: u64 = value.parse().map_err(|_| format!("bad number {value:?}"))?;
        match flag.as_str() {
            "--seeds" => seeds = n,
            "--from" => from = n,
            "--deadline-ms" => deadline = Duration::from_millis(n),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let last = seeds
        .checked_sub(1)
        .ok_or("--seeds must be at least 1")?
        .checked_add(from)
        .filter(|&last| last <= home::MAX_SEED)
        .ok_or(format!("seeds must end by {}", home::MAX_SEED))?;
    if deadline.is_zero() {
        return Err("--deadline-ms must be at least 1".into());
    }
    Ok(Args {
        from,
        last,
        deadline,
        cmd: cmd.to_vec(),
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&args) {
        Ok(parsed) => parsed,
        Err(why) => {
            eprintln!("{why}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match go(&args) {
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

fn go(args: &Args) -> Result<(), Error> {
    let (mut mine, mut theirs) = (Tally::default(), Tally::default());
    for seed in args.from..=args.last {
        tally(&mut mine, seed, || Ok(apprentice(seed)))?;
        tally(&mut theirs, seed, || {
            Ok(Plugged::launch(&args.cmd, agent_seed(seed), args.deadline)?)
        })?;
    }
    let seeds = args.last - args.from + 1;
    println!("seeds {}..{}", args.from, args.last);
    println!("per seed: sleeps and meals in {BUSH_STEPS} bush-world steps");
    println!("agent        sleeps    meals   home wrong@100/@300/asked  n/a");
    println!("{}", row("apprentice", &mine, seeds));
    println!("{}", row("plugged", &theirs, seeds));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(line: &str) -> Result<Args, String> {
        let args: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        parse(&args)
    }

    #[test]
    fn defaults_are_two_smoke_seeds() {
        let args = parse_str("-- agent x").expect("parses");
        let want = Args {
            from: 101,
            last: 102,
            deadline: DEADLINE,
            cmd: vec!["agent".into(), "x".into()],
        };
        assert_eq!(args, want);
    }

    #[test]
    fn flags_set_the_range_and_deadline() {
        let args = parse_str("--from 5 --seeds 1 --deadline-ms 250 -- a").expect("parses");
        assert_eq!((args.from, args.last), (5, 5));
        assert_eq!(args.deadline, Duration::from_millis(250));
    }

    #[test]
    fn the_last_seed_may_be_the_largest_home_seed() {
        let line = format!("--from {} --seeds 1 -- a", home::MAX_SEED);
        assert_eq!(parse_str(&line).map(|a| a.last), Ok(home::MAX_SEED));
    }

    #[test]
    fn bad_ranges_are_rejected_before_any_life() {
        let max = home::MAX_SEED;
        for line in [
            "--seeds 0 -- a".to_string(),
            format!("--from {max} --seeds 2 -- a"),
            format!("--from {} --seeds 1 -- a", max + 1),
            format!("--from {} --seeds 2 -- a", u64::MAX),
            "--deadline-ms 0 -- a".to_string(),
        ] {
            assert!(parse_str(&line).is_err(), "{line}");
        }
    }

    #[test]
    fn bad_commands_and_flags_are_rejected() {
        for line in [
            "agent",
            "--",
            "--seeds -- a",
            "--seeds x -- a",
            "--fast 1 -- a",
        ] {
            assert!(parse_str(line).is_err(), "{line}");
        }
    }
}
