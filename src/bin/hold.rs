//! Can a local learner hold a fact across steps? Runs the clone learner
//! (`apprentice::clones`) in the facing worlds and, with --judge, the best
//! earlier local learner on the same seeds, and judges the marks pinned in
//! `docs/hold/README.md`.

use std::process::ExitCode;
use std::thread;

use apprentice::facing::{
    ClonesLife, EpropLife, Life, Memory, Score, Stage, live, live_clones, live_eprop,
};
use apprentice::judge::{Interval, Verdict};

const USAGE: &str = "\
usage: hold [--stage turning|matching|walking] [--judge] [--from SEED] [--seeds N]
            [--steps S] [--clones C] [--recruit R] [--persist P] [--jitter J]

Seeds FROM..FROM+N-1 (default 101..102, smoke; tune on --from 201 --seeds 10).
Matching runs without the ring, walking with it. Without --judge, runs the
clone learner alone and prints its scores. With --judge, also runs the
baseline on the same seeds (gated context for matching and turning, gated
e-prop for walking; no memory too on matching, as the leak check) and judges
the marks (judge on --from 701 --seeds 200).";

/// Scored steps at the end over which accuracy is scored.
const WINDOW: usize = 100;
const PASS_ONE: f64 = 0.95;
const PASS_TWO: f64 = 0.90;
const PASS_LONG: f64 = 0.90;
/// Mark 1: share of seeds passing, whole interval at or above.
const PASS_SHARE: f64 = 0.95;
/// Mark 2: typical change in wrong one-ahead, whole interval at or below.
const FEWER_WRONG: f64 = -0.5;
/// Mark 3: above this median, no memory solved matching: a leak.
const NO_MEMORY_CEILING: f64 = 0.80;

struct Args {
    stage: Stage,
    seeds: Vec<u64>,
    judge: bool,
    life: ClonesLife,
}

fn budget(stage: Stage) -> usize {
    match stage {
        Stage::Turning => 8_000,
        Stage::Matching => 20_000,
        Stage::Walking => 400_000,
    }
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut from, mut n) = (101_u64, 2_u64);
    let mut stage = Stage::Matching;
    let mut steps = None;
    let mut life = ClonesLife {
        stage,
        ring: false,
        steps: 0,
        window: WINDOW,
        clones: 16,
        recruit: 4.0,
        persist: 1.0,
        jitter: 0.001,
    };
    let mut judge = false;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--judge" {
            judge = true;
            continue;
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--stage" => {
                stage = match value.as_str() {
                    "turning" => Stage::Turning,
                    "matching" => Stage::Matching,
                    "walking" => Stage::Walking,
                    _ => return Err(bad()),
                }
            }
            "--from" => from = value.parse().map_err(|_| bad())?,
            "--seeds" => n = value.parse().map_err(|_| bad())?,
            "--steps" => steps = Some(value.parse().map_err(|_| bad())?),
            "--clones" => life.clones = value.parse().map_err(|_| bad())?,
            "--recruit" => life.recruit = value.parse().map_err(|_| bad())?,
            "--persist" => life.persist = value.parse().map_err(|_| bad())?,
            "--jitter" => life.jitter = value.parse().map_err(|_| bad())?,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if life.clones == 0 {
        return Err("--clones must be at least 1".into());
    }
    life.stage = stage;
    life.ring = stage == Stage::Walking;
    life.steps = steps.unwrap_or(budget(stage));
    Ok(Args {
        stage,
        seeds: (from..from + n).collect(),
        judge,
        life,
    })
}

/// Lives `f` on every seed, one thread per seed.
fn across<F>(seeds: &[u64], f: F) -> Result<Vec<Score>, String>
where
    F: Fn(u64) -> Score + Send + Sync + Copy + 'static,
{
    let handles: Vec<_> = seeds
        .iter()
        .map(|&seed| thread::spawn(move || f(seed)))
        .collect();
    handles
        .into_iter()
        .map(|h| h.join().map_err(|_| "a life panicked".to_string()))
        .collect()
}

/// Does a seed meet the facing marks?
fn passes(stage: Stage, s: &Score) -> bool {
    let at_least = |x: Option<f64>, mark: f64| x.is_some_and(|x| x >= mark);
    let long = stage == Stage::Turning || at_least(s.after_stare, PASS_LONG);
    at_least(s.one, PASS_ONE) && at_least(s.two, PASS_TWO) && long
}

/// Wrong one-ahead predictions over the last `WINDOW` scored steps.
fn wrong(s: &Score) -> f64 {
    ((1.0 - s.one.unwrap_or(0.0)) * WINDOW as f64).round()
}

fn median(mut xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len().max(1) as f64
}

fn report(stage: Stage, label: &str, seeds: &[u64], scores: &[Score]) -> usize {
    let col = |f: fn(&Score) -> Option<f64>| median(scores.iter().filter_map(f).collect());
    let passed = scores.iter().filter(|s| passes(stage, s)).count();
    println!(
        "{label:<10} {:>6.2} {:>6.2} {:>8.2} {:>6.2} {:>9.2} {:>6.2} {:>7.2}   {passed}/{}",
        col(|s| s.one),
        col(|s| s.two),
        col(|s| s.after_stare),
        col(|s| s.probe),
        col(|s| s.probe_matching),
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
    if !missed.is_empty() && missed.len() <= 20 {
        println!("           missed seeds {missed:?}");
    }
    passed
}

const HEADER: &str =
    "learner      one    two  stare5+  probe  probe NS  place  centre   passed (medians)";

fn show(i: &Interval) -> String {
    format!("{:+.3} [{:+.3}, {:+.3}]", i.mid, i.lo, i.hi)
}

/// The best earlier local learner for `stage`, on the same settings as
/// docs/facing judged it.
fn baseline(stage: Stage, seeds: &[u64]) -> Result<(&'static str, Vec<Score>), String> {
    match stage {
        Stage::Walking => {
            let life = EpropLife {
                stage,
                steps: 400_000,
                units: 64,
                leaks: (0.5, 0.95),
                rate: 0.01,
                window: WINDOW,
                gated: true,
            };
            Ok(("gated e-p", across(seeds, move |s| live_eprop(&life, s))?))
        }
        Stage::Turning | Stage::Matching => {
            let life = memory_life(stage, Memory::Gated);
            Ok(("gated", context(&life, seeds)?))
        }
    }
}

fn memory_life(stage: Stage, memory: Memory) -> Life {
    Life {
        stage,
        memory,
        ring: false,
        steps: budget(stage),
        rate: 0.05,
        window: WINDOW,
    }
}

fn context(life: &Life, seeds: &[u64]) -> Result<Vec<Score>, String> {
    let life = *life;
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

fn judge(a: &Args, clones: &[Score]) -> Result<(), String> {
    let n = a.seeds.len();
    let passed = clones.iter().filter(|s| passes(a.stage, s)).count();
    let (name, base) = baseline(a.stage, &a.seeds)?;
    report(a.stage, name, &a.seeds, &base);
    let leak = if a.stage == Stage::Matching {
        let none = context(&memory_life(a.stage, Memory::None), &a.seeds)?;
        report(a.stage, "none", &a.seeds, &none);
        Some(median(none.iter().filter_map(|s| s.one).collect()))
    } else {
        None
    };
    let share = Interval::share(passed, n).map_err(|e| e.to_string())?;
    let (b, c): (Vec<f64>, Vec<f64>) = base
        .iter()
        .zip(clones)
        .map(|(b, c)| (wrong(b), wrong(c)))
        .unzip();
    let change = Interval::change(&b, &c).map_err(|e| e.to_string())?;
    let m1 = share.at_least(PASS_SHARE);
    let m2 = change.at_most(FEWER_WRONG);
    println!(
        "1 seeds passing: {passed}/{n}, share {}  (bottom >= {PASS_SHARE}): {m1}",
        show(&share)
    );
    println!(
        "2 fewer wrong:   change {}  (top <= {FEWER_WRONG}): {m2}; raw wrong/100 {name} {:.2} -> clones {:.2} ({:+.2})",
        show(&change),
        mean(&b),
        mean(&c),
        mean(&c) - mean(&b),
    );
    if let Some(none) = leak {
        let ok = if none <= NO_MEMORY_CEILING {
            "no leak"
        } else {
            "LEAK: stop"
        };
        println!(
            "3 leak check:    no-memory median one-ahead {none:.2} (<= {NO_MEMORY_CEILING}): {ok}"
        );
    }
    let held = matches!((m1, m2), (Verdict::Met, Verdict::Met));
    println!(
        "world verdict: {}",
        if held { "met" } else { "not met (see marks)" }
    );
    Ok(())
}

fn execute(a: &Args) -> Result<(), String> {
    let life = a.life;
    println!(
        "{life:?}, seeds {:?}..={:?}",
        a.seeds.first(),
        a.seeds.last()
    );
    println!("{HEADER}");
    let clones = across(&a.seeds, move |s| live_clones(&life, s))?;
    report(a.stage, "clones", &a.seeds, &clones);
    if a.judge {
        judge(a, &clones)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(&args).and_then(|a| execute(&a)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
