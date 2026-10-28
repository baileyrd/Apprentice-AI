//! Phase 4 experiment: world A, then B, then A again, with and without memory.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup as AdaptSetup};
use apprentice::apprentice::{Apprentice, Drives};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::relearn::{self, Learning, Setup, Trace, mixed_up};
use apprentice::report::{self, Line};
use apprentice::sim::SimError;

const USAGE: &str = "\
usage: relearn [--seeds N] [--from SEED] [--phase STEPS] [--out DIR]

Lives seeds FROM..FROM+N-1 (default 1..10) through world A, world B (seed + 1000), then
A again, each for STEPS (default 3000), in the bush world. Two minds: the
phase 3 apprentice and the apprentice with memory. Prints learning times and
mix-ups (more than 100 steps in B on the map it ended A with), and writes DIR/relearn.csv and DIR/relearn.svg (default DIR: runs/relearn).";

type Error = Box<dyn std::error::Error>;

const MINDS: [&str; 2] = ["apprentice", "apprentice + memory"];
const STRIDE: usize = 25;
/// Steps in B on A's map beyond which a seed counts as mixed up.
const MIX_UP: usize = 100;

fn live(mind: &str, setup: &Setup) -> Result<Trace, SimError> {
    let (rates, seed) = (Rates::default(), setup.seed);
    if mind == "apprentice" {
        return relearn::run(setup, &mut Apprentice::new(Drives::default(), rates, seed));
    }
    let memory = Memory::new(rates, Recall::default());
    relearn::run(
        setup,
        &mut Apprentice::with_model(Drives::default(), memory, seed),
    )
}

struct Args {
    seeds: u64,
    from: u64,
    phase: usize,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        seeds: 10,
        from: 1,
        phase: 3_000,
        out: PathBuf::from("runs/relearn"),
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || {
            value
                .parse::<usize>()
                .map_err(|_| format!("bad {flag}: {value}"))
        };
        match flag.as_str() {
            "--seeds" => parsed.seeds = number()? as u64,
            "--from" => parsed.from = number()? as u64,
            "--phase" => parsed.phase = number()?,
            "--out" => parsed.out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if parsed.seeds == 0 || parsed.phase == 0 {
        return Err("--seeds and --phase must be positive".into());
    }
    Ok(parsed)
}

fn median(mut xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

fn fmt_steps(v: Option<usize>) -> String {
    v.map_or("never".into(), |s| s.to_string())
}

/// Mean of the last `n` values at each step (fewer at the very start).
fn trailing(xs: &[f64], n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(xs.len());
    let mut sum = 0.0;
    for (i, &x) in xs.iter().enumerate() {
        sum += x;
        if i >= n {
            sum -= xs[i - n];
        }
        out.push(sum / (i + 1).min(n) as f64);
    }
    out
}

/// Median across seeds of `measure`, per mind, every `STRIDE` steps.
fn curves<'a>(
    all: &[Vec<(Trace, Learning)>],
    measure: impl Fn(&Trace) -> Vec<f64>,
) -> Vec<Line<'a>> {
    MINDS
        .iter()
        .zip(all)
        .map(|(name, runs)| {
            let series: Vec<Vec<f64>> = runs.iter().map(|(t, _)| measure(t)).collect();
            let len = series.first().map_or(0, Vec::len);
            let points = (0..len)
                .step_by(STRIDE)
                .map(|i| (i as f64, median(series.iter().map(|s| s[i]).collect())))
                .collect();
            Line { name, points }
        })
        .collect()
}

fn csv(all: &[Vec<(Trace, Learning)>], from: u64) -> String {
    let mut out = String::from(
        "mind,seed,target,first_a,b,second_a,on_return,savings,mixed_up,meals,sleeps\n",
    );
    let cell = |v: Option<usize>| v.map_or(String::new(), |s| s.to_string());
    for (name, runs) in MINDS.iter().zip(all) {
        for (i, (t, l)) in runs.iter().enumerate() {
            out.push_str(&format!(
                "{name},{},{:.4},{},{},{},{:.4},{},{},{},{}\n",
                from + i as u64,
                l.target,
                cell(l.first_a),
                cell(l.b),
                cell(l.second_a),
                l.on_return,
                l.savings().map_or(String::new(), |v| format!("{v:.3}")),
                mixed_up(t),
                t.summary.meals,
                t.summary.sleeps,
            ));
        }
    }
    out
}

fn execute(args: &Args) -> Result<(), Error> {
    let config = AdaptSetup::for_change(Change::Food).config;
    println!("seed  {:>34}  {:>34}", MINDS[0], MINDS[1]);
    let mut all: Vec<Vec<(Trace, Learning)>> = vec![Vec::new(); MINDS.len()];
    for seed in args.from..args.from + args.seeds {
        let setup = Setup {
            config: config.clone(),
            seed,
            phase: args.phase,
        };
        let mut row = format!("{seed:>4}");
        for (m, name) in MINDS.iter().enumerate() {
            let trace = live(name, &setup)?;
            let l = relearn::learning(&trace);
            row.push_str(&format!(
                "  {:>34}",
                format!(
                    "A {} / B {} / A again {} ({:.0}%)",
                    fmt_steps(l.first_a),
                    fmt_steps(l.b),
                    fmt_steps(l.second_a),
                    100.0 * l.on_return
                )
            ));
            all[m].push((trace, l));
        }
        println!("{row}");
    }
    let horizon = args.phase as f64;
    println!(
        "\n(steps from each phase start to 95% of first-A accuracy; % = map right on return to A)\n"
    );
    for (name, runs) in MINDS.iter().zip(&all) {
        let time = |f: fn(&Learning) -> Option<usize>| {
            median(
                runs.iter()
                    .map(|(_, l)| f(l).map_or(horizon, |s| s as f64))
                    .collect(),
            )
        };
        let savings = median(runs.iter().filter_map(|(_, l)| l.savings()).collect());
        let back = runs.iter().filter(|(_, l)| l.second_a.is_some()).count();
        let on_return = median(runs.iter().map(|(_, l)| l.on_return).collect());
        let sleeps = median(runs.iter().map(|(t, _)| t.summary.sleeps as f64).collect());
        let mixed: Vec<String> = runs
            .iter()
            .zip(args.from..)
            .filter(|((t, _), _)| mixed_up(t) > MIX_UP)
            .map(|((t, _), seed)| format!("{seed}:{}", mixed_up(t)))
            .collect();
        println!(
            "{name:>20}: median A {:.0}, B {:.0}, A again {:.0} steps (relearned {back}/{}); savings {savings:.2}; map right on return {:.0}%; sleeps {sleeps:.0}; mix-ups {} [{}]",
            time(|l| l.first_a),
            time(|l| l.b),
            time(|l| l.second_a),
            runs.len(),
            100.0 * on_return,
            mixed.len(),
            mixed.join(" "),
        );
    }
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("relearn.csv"), csv(&all, args.from))?;
    let surprise = curves(&all, |t| trailing(&t.surprise, 100));
    let map = curves(&all, |t| t.map.clone());
    let p = args.phase as f64;
    let svg = report::relearn_svg(
        "Learning world A, then B, then A again",
        &surprise,
        &map,
        (0.0, 3.0 * p),
        &[(0.0, "A"), (p, "B"), (2.0 * p, "A again")],
    );
    fs::write(args.out.join("relearn.svg"), svg)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let parsed = match parse(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&parsed) {
        eprintln!("relearn: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
