//! Facing experiment: can a learner that sees only the box it faces learn
//! its heading? Each memory lives the same lives; rules are in
//! docs/facing/README.md.

use std::process::ExitCode;
use std::thread;

use apprentice::facing::{
    Ceiling, EpropLife, Life, Memory, Score, Stage, ceiling, live, live_eprop,
};
use apprentice::gru::Training;

const USAGE: &str = "\
usage: facing [--stage turning|matching|walking] [--ring] [--gru] [--eprop]
              [--seeds N]
              [--from SEED] [--judge] [--steps S] [--rate R]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
seeds 1-20. Runs every memory on the stage (default turning) for S steps
(default: the stage's budget) at learning rate R (default 0.05), prints
each memory's scores and verdict. With --ring every learner also gets the
innate heading ring. With --gru, runs the GRU ceiling check instead (always
with the ring); with --eprop, the online e-prop network (always with the
ring, rate R).";

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
    ring: bool,
    gru: bool,
    eprop: bool,
}

fn budget(stage: Stage) -> usize {
    match stage {
        Stage::Turning => 8_000,
        Stage::Matching => 20_000,
        Stage::Walking => 40_000,
    }
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        stage: Stage::Turning,
        seeds: (101..111).collect(),
        steps: None,
        rate: 0.05,
        ring: false,
        gru: false,
        eprop: false,
    };
    let (mut n, mut from) = (10, 101);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--ring" {
            a.ring = true;
            continue;
        }
        if flag == "--gru" {
            a.gru = true;
            continue;
        }
        if flag == "--eprop" {
            a.eprop = true;
            continue;
        }
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
                    "walking" => Stage::Walking,
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
        Memory::Gated => "gated".into(),
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
        Stage::Matching | Stage::Walking => at_least(s.after_stare, PASS_LONG),
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

fn report(stage: Stage, label: &str, seeds: &[u64], scores: &[Score]) -> usize {
    let col = |f: fn(&Score) -> Option<f64>| median(scores.iter().filter_map(f).collect());
    let passed = scores.iter().filter(|s| passes(stage, s)).count();
    println!(
        "{:<10} {:>6.2} {:>6.2} {:>8.2} {:>6.2} {:>9.2} {:>8.2} {:>6.2} {:>7.2}   {passed}/{}",
        label,
        col(|s| s.one),
        col(|s| s.two),
        col(|s| s.after_stare),
        col(|s| s.probe),
        col(|s| s.probe_matching),
        col(|s| s.majority_matching),
        col(|s| s.probe_place),
        col(|s| s.probe_centre),
        scores.len(),
    );
    let missed: Vec<u64> = seeds
        .iter()
        .zip(scores)
        .filter(|(_, s)| !passes(stage, s))
        .map(|(&seed, _)| seed)
        .collect();
    if !missed.is_empty() && missed.len() < seeds.len() {
        println!("           missed seeds {missed:?}");
    }
    passed
}

/// The GRU ceiling check: 32 units, 20 epochs of BPTT in 40-step chunks,
/// then 4,000 frozen steps scored. Always with the ring.
fn execute_ceiling(a: &Args) -> Result<(), String> {
    let c = Ceiling {
        stage: a.stage,
        steps: a.steps.unwrap_or(budget(a.stage)),
        eval: 4_000,
        hidden: 32,
        training: Training {
            epochs: 20,
            chunk: 40,
            rate: 0.003,
            clip: 1.0,
        },
        window: WINDOW,
    };
    println!(
        "GRU ceiling, stage {:?}, {} training steps, seeds {:?}..={:?}",
        c.stage,
        c.steps,
        a.seeds.first(),
        a.seeds.last()
    );
    println!(
        "learner    one    two  stare5+  probe  probe NS  most NS  place  centre   passed (medians)"
    );
    let handles: Vec<_> = a
        .seeds
        .iter()
        .map(|&seed| thread::spawn(move || ceiling(&c, seed)))
        .collect();
    let scores = handles
        .into_iter()
        .map(|h| h.join().map_err(|_| "a life panicked".to_string()))
        .collect::<Result<Vec<Score>, String>>()?;
    verdict(a, "gru", &scores);
    Ok(())
}

/// E-prop, online: 64 leaky units, leaks 0.5-0.95, 400,000 steps unless
/// --steps says otherwise. Always with the ring.
fn execute_eprop(a: &Args) -> Result<(), String> {
    let life = EpropLife {
        stage: a.stage,
        steps: a.steps.unwrap_or(400_000),
        units: 64,
        leaks: (0.5, 0.95),
        rate: a.rate,
        window: WINDOW,
    };
    println!(
        "e-prop, stage {:?}, {} steps, rate {}, seeds {:?}..={:?}",
        life.stage,
        life.steps,
        life.rate,
        a.seeds.first(),
        a.seeds.last()
    );
    println!(
        "learner    one    two  stare5+  probe  probe NS  most NS  place  centre   passed (medians)"
    );
    let handles: Vec<_> = a
        .seeds
        .iter()
        .map(|&seed| thread::spawn(move || live_eprop(&life, seed)))
        .collect();
    let scores = handles
        .into_iter()
        .map(|h| h.join().map_err(|_| "a life panicked".to_string()))
        .collect::<Result<Vec<Score>, String>>()?;
    verdict(a, "e-prop", &scores);
    Ok(())
}

fn verdict(a: &Args, label: &str, scores: &[Score]) {
    let passed = report(a.stage, label, &a.seeds, scores);
    let need = (PASS_SHARE * a.seeds.len() as f64).ceil() as usize;
    let verdict = if passed >= need { "met" } else { "missed" };
    println!("           -> {verdict} (need {need})");
}

fn execute(a: &Args) -> Result<(), String> {
    if a.gru {
        return execute_ceiling(a);
    }
    if a.eprop {
        return execute_eprop(a);
    }
    let steps = a.steps.unwrap_or(budget(a.stage));
    println!(
        "stage {:?}, ring {}, {steps} steps, seeds {:?}..={:?}",
        a.stage,
        a.ring,
        a.seeds.first(),
        a.seeds.last()
    );
    println!(
        "memory     one    two  stare5+  probe  probe NS  most NS  place  centre   passed (medians)"
    );
    let need = (PASS_SHARE * a.seeds.len() as f64).ceil() as usize;
    for memory in [
        Memory::None,
        Memory::Recent(RECENT),
        Memory::Context,
        Memory::Gated,
    ] {
        let life = Life {
            stage: a.stage,
            memory,
            ring: a.ring,
            steps,
            rate: a.rate,
            window: WINDOW,
        };
        let scores = run(life, &a.seeds)?;
        let passed = report(a.stage, &name(memory), &a.seeds, &scores);
        let leak = a.stage == Stage::Matching
            && !a.ring
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
