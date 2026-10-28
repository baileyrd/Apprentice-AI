//! Phase 2 experiment: adaptation after the walls move, across seeds and minds.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{self, Recovery, Setup, Trace};
use apprentice::agent::ReflexAgent;
use apprentice::apprentice::{Apprentice, Drives, Watched};
use apprentice::model::Rates;
use apprentice::report::{self, Line};
use apprentice::sim::SimError;

const USAGE: &str = "\
usage: adapt [--seeds N] [--out DIR]

Lives seeds 1..=N (default 10) with three minds: reflex (with a world model
watching), drive-only apprentice and drive + curiosity apprentice. Half the
walls move after the warm-up. Prints adaptation times and writes
DIR/adaptation.csv and DIR/adaptation.svg (default DIR: runs/p2).";

type Error = Box<dyn std::error::Error>;

const MINDS: [&str; 3] = ["reflex", "drive only", "drive + curiosity"];
/// Plotted window around the move, in steps, and the sampling stride.
const WINDOW: (i64, i64) = (-500, 3000);
const STRIDE: usize = 25;

fn live(mind: &str, setup: &Setup) -> Result<Trace, SimError> {
    let (rates, seed) = (Rates::default(), setup.seed);
    match mind {
        "reflex" => adaptation::run(setup, &mut Watched::new(ReflexAgent::new(seed), rates)),
        "drive only" => {
            let drives = Drives {
                curiosity: 0.0,
                ..Drives::default()
            };
            adaptation::run(setup, &mut Apprentice::new(drives, rates, seed))
        }
        _ => adaptation::run(setup, &mut Apprentice::new(Drives::default(), rates, seed)),
    }
}

fn parse(mut raw: impl Iterator<Item = String>) -> Result<(u64, PathBuf), Error> {
    let (mut seeds, mut out) = (10, PathBuf::from("runs/p2"));
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--seeds" => seeds = value()?.parse()?,
            "--out" => out = value()?.into(),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    Ok((seeds, out))
}

fn fmt_steps(steps: Option<usize>) -> String {
    steps.map_or("never".into(), |s| s.to_string())
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

fn execute(seeds: u64, out: &PathBuf) -> Result<(), Error> {
    let base = Setup::default();
    let horizon = base.after as f64;
    println!("seed  {:>20}  {:>20}  {:>20}", MINDS[0], MINDS[1], MINDS[2]);
    let mut all: Vec<Vec<(Trace, Recovery)>> = vec![Vec::new(); MINDS.len()];
    for seed in 1..=seeds {
        let setup = Setup {
            seed,
            ..base.clone()
        };
        let mut row = format!("{seed:>4}");
        for (m, name) in MINDS.iter().enumerate() {
            let trace = live(name, &setup)?;
            let r = adaptation::recovery(&trace, setup.floor_window, setup.trail);
            row.push_str(&format!(
                "  {:>20}",
                format!(
                    "{} ({}/{})",
                    fmt_steps(r.both),
                    fmt_steps(r.surprise),
                    fmt_steps(r.map)
                )
            ));
            all[m].push((trace, r));
        }
        println!("{row}");
    }
    println!("\n(both (surprise/map) steps after the move; 'never' within {horizon} steps)\n");
    for (m, name) in MINDS.iter().enumerate() {
        let runs = &all[m];
        let both = runs
            .iter()
            .map(|(_, r)| r.both.map_or(horizon, |s| s as f64))
            .collect();
        let recovered = runs.iter().filter(|(_, r)| r.both.is_some()).count();
        let pre = median(runs.iter().map(|(_, r)| r.baseline_accuracy).collect());
        let meals = median(runs.iter().map(|(t, _)| t.summary.meals as f64).collect());
        let sleeps = median(runs.iter().map(|(t, _)| t.summary.sleeps as f64).collect());
        let energy = median(runs.iter().map(|(t, _)| t.summary.mean_energy).collect());
        println!(
            "{name:>18}: median adaptation {:.0} steps, recovered {recovered}/{}, map before move {:.1}%, meals {meals:.0}, sleeps {sleeps:.0}, mean energy {energy:.1}",
            median(both),
            runs.len(),
            100.0 * pre,
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("adaptation.csv"), csv(&all))?;
    let surprise = curves(&all, |t| trailing(&t.surprise, base.trail));
    let accuracy = curves(&all, |t| t.accuracy.clone());
    let times: Vec<(&str, Vec<Option<usize>>)> = MINDS
        .iter()
        .zip(&all)
        .map(|(name, runs)| (*name, runs.iter().map(|(_, r)| r.both).collect()))
        .collect();
    let range = (WINDOW.0 as f64, WINDOW.1 as f64);
    let svg = report::adaptation_svg(&surprise, &accuracy, &times, range, base.after);
    fs::write(out.join("adaptation.svg"), svg)?;
    Ok(())
}

/// Mean of the last `n` values at each step (fewer at the very start).
fn trailing(xs: &[f64], n: usize) -> Vec<f64> {
    let mut sum = 0.0;
    (0..xs.len())
        .map(|i| {
            sum += xs[i];
            if i >= n {
                sum -= xs[i - n];
            }
            sum / (i + 1).min(n) as f64
        })
        .collect()
}

/// Median across seeds of `measure`, per mind, sampled over `WINDOW`.
fn curves(
    all: &[Vec<(Trace, Recovery)>],
    measure: impl Fn(&Trace) -> Vec<f64>,
) -> Vec<Line<'static>> {
    MINDS
        .iter()
        .zip(all)
        .map(|(name, runs)| {
            let series: Vec<(usize, Vec<f64>)> =
                runs.iter().map(|(t, _)| (t.change, measure(t))).collect();
            let points = (WINDOW.0..=WINDOW.1)
                .step_by(STRIDE)
                .filter_map(|rel| {
                    let values: Vec<f64> = series
                        .iter()
                        .filter_map(|(c, v)| v.get(usize::try_from(*c as i64 + rel).ok()?).copied())
                        .collect();
                    (!values.is_empty()).then(|| (rel as f64, median(values)))
                })
                .collect();
            Line { name, points }
        })
        .collect()
}

fn csv(all: &[Vec<(Trace, Recovery)>]) -> String {
    let mut out = String::from(
        "mind,seed,adapt_steps,surprise_steps,map_steps,map_before_move,meals,sleeps,mean_energy\n",
    );
    let cell = |v: Option<usize>| v.map_or(String::new(), |s| s.to_string());
    for (name, runs) in MINDS.iter().zip(all) {
        for (i, (t, r)) in runs.iter().enumerate() {
            out.push_str(&format!(
                "{name},{},{},{},{},{:.4},{},{},{:.2}\n",
                i + 1,
                cell(r.both),
                cell(r.surprise),
                cell(r.map),
                r.baseline_accuracy,
                t.summary.meals,
                t.summary.sleeps,
                t.summary.mean_energy,
            ));
        }
    }
    out
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(raw.into_iter()).and_then(|(seeds, out)| execute(seeds, &out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
