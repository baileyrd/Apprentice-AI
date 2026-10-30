//! Facing experiment: can a learner that sees only the box it faces learn
//! its heading? Each memory lives the same lives; rules are in
//! docs/facing/README.md.

use std::process::ExitCode;
use std::thread;

use apprentice::facing::{Life, Memory, Score, Stage, live};

const USAGE: &str = "\
usage: facing [--stage turning|matching] [--seeds N] [--from SEED] [--judge]
              [--steps S] [--rate R]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
seeds 1-20. Runs every memory on the stage (default turning) for S steps
(default: the stage's budget) at learning rate R (default 0.05), prints
each memory's scores and verdict.";

/// Turns at the end over which accuracy is scored.
const WINDOW: usize = 100;
/// Steps of history the recent-views control keeps: at least the median
/// stare (2).
const RECENT: usize = 3;
const PASS_ONE: f64 = 0.95;
const PASS_TWO: f64 = 0.90;
const PASS_LONG: f64 = 0.90;
/// Seeds that must pass, out of every 20.
const PASS_SHARE: f64 = 19.0 / 20.0;
/// Above this, no memory solved matching: a leak.
const NO_MEMORY_CEILING: f64 = 0.80;

struct Args {
    stage: Stage,
    seeds: Vec<u64>,
    steps: Option<usize>,
    rate: f64,
}

fn budget(stage: Stage) -> usize {
    match stage {
        Stage::Turning => 8_000,
        Stage::Matching => 20_000,
    }
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        stage: Stage::Turning,
        seeds: (101..111).collect(),
        steps: None,
        rate: 0.05,
    };
    let (mut n, mut from) = (10, 101);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--judge" {
            a.seeds = (1..21).collect();
            continue;
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--stage" => {
                a.stage = match value.as_str() {
                    "turning" => Stage::Turning,
                    "matching" => Stage::Matching,
                    _ => return Err(format!("unknown stage {value}")),
                }
            }
            "--seeds" => n = value.parse().map_err(|_| bad())?,
            "--from" => from = value.parse().map_err(|_| bad())?,
            "--steps" => a.steps = Some(value.parse().map_err(|_| bad())?),
            "--rate" => a.rate = value.parse().map_err(|_| bad())?,
            _ => return Err(format!("unknown flag {flag}")),
        }
        a.seeds = (from..from + n).collect();
    }
    Ok(a)
}

fn name(m: Memory) -> String {
    match m {
        Memory::None => "none".into(),
        Memory::Recent(k) => format!("recent {k}"),
        Memory::Context => "context".into(),
    }
}

fn median(mut xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

/// Does a seed meet the stage's marks?
fn passes(stage: Stage, s: &Score) -> bool {
    let at_least = |x: Option<f64>, mark: f64| x.is_some_and(|x| x >= mark);
    let long = match stage {
        Stage::Turning => true,
        Stage::Matching => at_least(s.after_stare, PASS_LONG),
    };
    at_least(s.one, PASS_ONE) && at_least(s.two, PASS_TWO) && long
}

fn run(life: Life, seeds: &[u64]) -> Result<Vec<Score>, String> {
    let handles: Vec<_> = seeds
        .iter()
        .map(|&seed| thread::spawn(move || live(&life, seed)))
        .collect();
    handles
        .into_iter()
        .map(|h| {
            h.join()
                .map_err(|_| "a life panicked".to_string())?
                .map_err(|e| e.to_string())
        })
        .collect()
}

fn report(stage: Stage, memory: Memory, scores: &[Score]) -> usize {
    let col = |f: fn(&Score) -> Option<f64>| median(scores.iter().filter_map(f).collect());
    let passed = scores.iter().filter(|s| passes(stage, s)).count();
    println!(
        "{:<10} {:>6.2} {:>6.2} {:>8.2} {:>6.2}   {passed}/{}",
        name(memory),
        col(|s| s.one),
        col(|s| s.two),
        col(|s| s.after_stare),
        col(|s| s.probe),
        scores.len(),
    );
    passed
}

fn execute(a: &Args) -> Result<(), String> {
    let steps = a.steps.unwrap_or(budget(a.stage));
    println!(
        "stage {:?}, {steps} steps, seeds {:?}..={:?}",
        a.stage,
        a.seeds.first(),
        a.seeds.last()
    );
    println!("memory     one    two  stare5+  probe   passed (medians)");
    let need = (PASS_SHARE * a.seeds.len() as f64).ceil() as usize;
    for memory in [Memory::None, Memory::Recent(RECENT), Memory::Context] {
        let life = Life {
            stage: a.stage,
            memory,
            steps,
            rate: a.rate,
            window: WINDOW,
        };
        let scores = run(life, &a.seeds)?;
        let passed = report(a.stage, memory, &scores);
        let leak = a.stage == Stage::Matching
            && memory == Memory::None
            && median(scores.iter().filter_map(|s| s.one).collect()) > NO_MEMORY_CEILING;
        if leak {
            return Err("no-memory control beats the ceiling on matching: find the leak".into());
        }
        let verdict = if passed >= need { "met" } else { "missed" };
        println!("           -> {verdict} (need {need})");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let result = parse(&args).and_then(|a| execute(&a));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("facing: {e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
