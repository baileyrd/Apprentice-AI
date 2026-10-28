//! Phase 1 experiment: surprise curve, believed maps and the fallback verdict.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::experiment::{self, Setup};
use apprentice::model::{Backprop, PredictiveCoding, Rates};
use apprentice::report::{self, Series};

const USAGE: &str = "\
usage: surprise [--seed N] [--episodes N] [--steps N] [--out DIR]

Lives one fixed world with the reflex body while the predictive-coding world
model learns, then applies the pre-registered fallback rule at episode 200:
if surprise has not halved, the backprop baseline runs on the same world.
Writes DIR/surprise.csv and DIR/surprise.svg (default DIR: runs/p1).";

/// Episode by which surprise must have halved (decided before any run).
const DEADLINE: usize = 200;

type Error = Box<dyn std::error::Error>;

fn parse(mut raw: impl Iterator<Item = String>) -> Result<(Setup, PathBuf), Error> {
    let mut setup = Setup::default();
    let mut out = PathBuf::from("runs/p1");
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => setup.seed = value()?.parse()?,
            "--episodes" => setup.episodes = value()?.parse()?,
            "--steps" => setup.steps_per_episode = value()?.parse()?,
            "--out" => out = value()?.into(),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    let last = setup.episodes;
    setup.snapshot_at.retain(|&e| e <= last);
    if !setup.snapshot_at.contains(&last) {
        setup.snapshot_at.push(last);
    }
    Ok((setup, out))
}

fn execute(setup: &Setup, out: &PathBuf) -> Result<(), Error> {
    let pc = experiment::run(setup, PredictiveCoding::new(Rates::default()))?;
    let verdict = experiment::verdict(&pc.surprise, DEADLINE);
    print_results("predictive coding", &pc);
    match verdict {
        None => println!("verdict: needs at least {DEADLINE} episodes"),
        Some(v) => println!(
            "verdict: first-10 mean {:.5} -> episodes {}-{DEADLINE} mean {:.5}, drop {:.1}% (need {:.0}%); fallback {}",
            v.opening,
            DEADLINE + 1 - experiment::WINDOW,
            v.closing,
            100.0 * v.drop,
            100.0 * experiment::REQUIRED_DROP,
            if v.fallback {
                "TRIGGERED"
            } else {
                "not triggered"
            },
        ),
    }

    let bp = match verdict {
        Some(v) if v.fallback => Some(experiment::run(setup, Backprop::new(Rates::default()))?),
        _ => None,
    };
    let mut series = vec![Series {
        name: "predictive coding",
        results: &pc,
    }];
    if let Some(bp) = &bp {
        print_results("backprop", bp);
        if let Some(v) = experiment::verdict(&bp.surprise, DEADLINE) {
            println!(
                "backprop: first-10 mean {:.5} -> closing mean {:.5}, drop {:.1}%",
                v.opening,
                v.closing,
                100.0 * v.drop
            );
        }
        series.push(Series {
            name: "backprop",
            results: bp,
        });
    }

    fs::create_dir_all(out)?;
    fs::write(out.join("surprise.csv"), report::csv(&series))?;
    fs::write(
        out.join("surprise.svg"),
        report::svg(&series, verdict, DEADLINE),
    )?;
    Ok(())
}

fn print_results(name: &str, results: &experiment::Results) {
    for s in &results.snapshots {
        let acc = s
            .accuracy()
            .map_or("n/a".into(), |a| format!("{:.1}%", 100.0 * a));
        println!(
            "{name} episode {:>4}: surprise {:.5}, seen {:.0}% of squares, believed map {acc} right",
            s.episode,
            results.surprise[s.episode - 1],
            100.0 * s.coverage(),
        );
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(raw.into_iter()).and_then(|(setup, out)| execute(&setup, &out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
