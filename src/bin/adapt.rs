//! Adaptation experiment (phases 2-4): after walls or food move, across seeds and minds.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{self, Change, Recovery, Setup, Trace};
use apprentice::agent::ReflexAgent;
use apprentice::apprentice::{Apprentice, Drives, Watched};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::report::{self, Line};
use apprentice::sim::Regrowth;
use apprentice::sim::SimError;

const USAGE: &str = "\
usage: adapt [--change walls|food] [--bushes] [--seeds N] [--out DIR]

Lives seeds 1..=N (default 10) with four minds: reflex (with a world model
watching), drive-only apprentice, drive + curiosity apprentice, and that
apprentice with memory. After the warm-up, half the walls move (default) or
all the food moves. --bushes makes food grow back in place (always so for
food). Prints adaptation times and writes DIR/adaptation.csv and
DIR/adaptation.svg (default DIR: runs/<change>).";

type Error = Box<dyn std::error::Error>;

const MINDS: [&str; 4] = ["reflex", "drive only", "drive + curiosity", "+ memory"];
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
        "drive + curiosity" => {
            adaptation::run(setup, &mut Apprentice::new(Drives::default(), rates, seed))
        }
        _ => {
            let memory = Memory::new(rates, Recall::default());
            adaptation::run(
                setup,
                &mut Apprentice::with_model(Drives::default(), memory, seed),
            )
        }
    }
}

struct Args {
    change: Change,
    bushes: bool,
    seeds: u64,
    out: PathBuf,
}

fn parse(mut raw: impl Iterator<Item = String>) -> Result<Args, Error> {
    let (mut change, mut seeds, mut out, mut bushes) = (Change::Walls, 10, None, false);
    while let Some(flag) = raw.next() {
        if flag == "--bushes" {
            bushes = true;
            continue;
        }
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--change" => {
                change = match value()?.as_str() {
                    "walls" => Change::Walls,
                    "food" => Change::Food,
                    other => return Err(format!("unknown change: {other}").into()),
                }
            }
            "--seeds" => seeds = value()?.parse()?,
            "--out" => out = Some(value()?.into()),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    let name = if change == Change::Walls {
        "walls"
    } else {
        "food"
    };
    let out = out.unwrap_or_else(|| PathBuf::from("runs").join(name));
    Ok(Args {
        change,
        bushes,
        seeds,
        out,
    })
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

fn execute(args: &Args) -> Result<(), Error> {
    let (seeds, out) = (args.seeds, &args.out);
    let mut base = Setup::for_change(args.change);
    if args.bushes {
        base.config.regrowth = Regrowth::InPlace {
            delay: adaptation::BUSH_DELAY,
        };
    }
    let horizon = base.after as f64;
    print!("seed");
    for name in MINDS {
        print!("  {name:>20}");
    }
    println!();
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
        let meal = median(
            runs.iter()
                .map(|(t, _)| t.first_meal.map_or(horizon, |s| s as f64))
                .collect(),
        );
        println!(
            "{name:>18}: median adaptation {:.0} steps, recovered {recovered}/{}, map target {:.1}%, first meal after {meal:.0}, meals {meals:.0}, sleeps {sleeps:.0}, mean energy {energy:.1}",
            median(both),
            runs.len(),
            100.0 * pre,
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("adaptation.csv"), csv(&all))?;
    let surprise = curves(&all, |t| trailing(&t.surprise, base.trail));
    let map = curves(&all, |t| t.map.clone());
    let figure = match args.change {
        Change::Walls => report::Figure {
            title: "Adapting after half the walls move",
            map_title: "Share of the believed map that is right (walls, hazards, open floor)",
            map_range: (0.8, 1.0),
        },
        Change::Food => report::Figure {
            title: "Adapting after all the food moves",
            map_title: "Share of old food beliefs checked and cleared",
            map_range: (0.0, 1.0),
        },
    };
    let times: Vec<(&str, Vec<Option<usize>>)> = MINDS
        .iter()
        .zip(&all)
        .map(|(name, runs)| (*name, runs.iter().map(|(_, r)| r.both).collect()))
        .collect();
    let range = (WINDOW.0 as f64, WINDOW.1 as f64);
    let svg = report::adaptation_svg(&figure, &surprise, &map, &times, range, base.after);
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
            let series: Vec<(usize, Vec<f64>)> = runs
                .iter()
                .map(|(t, _)| (t.changed_at, measure(t)))
                .collect();
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
        "mind,seed,adapt_steps,surprise_steps,map_steps,map_target,stale_food,first_meal,meals,sleeps,mean_energy\n",
    );
    let cell = |v: Option<usize>| v.map_or(String::new(), |s| s.to_string());
    for (name, runs) in MINDS.iter().zip(all) {
        for (i, (t, r)) in runs.iter().enumerate() {
            out.push_str(&format!(
                "{name},{},{},{},{},{:.4},{},{},{},{},{:.2}\n",
                i + 1,
                cell(r.both),
                cell(r.surprise),
                cell(r.map),
                r.baseline_accuracy,
                t.stale,
                cell(t.first_meal),
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
    match parse(raw.into_iter()).and_then(|args| execute(&args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
