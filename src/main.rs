//! CLI: run a baseline in the grid world, log every step, checkpoint, resume.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::Checkpoint;
use apprentice::agent::{Baseline, RandomAgent, ReflexAgent};
use apprentice::metrics::Metrics;
use apprentice::sim::{Config, Sim};

const USAGE: &str = "\
usage: apprentice [--agent random|reflex] [--seed N] [--steps N]
                  [--log FILE.jsonl] [--save FILE.json] [--resume FILE.json]

Runs an agent in the grid world and prints a JSON summary.
--resume continues a saved run; --agent and --seed are then ignored.";

type Error = Box<dyn std::error::Error>;

#[derive(Debug)]
struct Args {
    agent: String,
    seed: u64,
    steps: u64,
    log: Option<PathBuf>,
    save: Option<PathBuf>,
    resume: Option<PathBuf>,
}

fn parse(mut raw: impl Iterator<Item = String>) -> Result<Args, Error> {
    let mut args = Args {
        agent: "reflex".into(),
        seed: 0,
        steps: 1_000,
        log: None,
        save: None,
        resume: None,
    };
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--agent" => args.agent = value()?,
            "--seed" => args.seed = value()?.parse()?,
            "--steps" => args.steps = value()?.parse()?,
            "--log" => args.log = Some(value()?.into()),
            "--save" => args.save = Some(value()?.into()),
            "--resume" => args.resume = Some(value()?.into()),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    Ok(args)
}

fn start(args: &Args) -> Result<Checkpoint, Error> {
    if let Some(path) = &args.resume {
        return Ok(serde_json::from_reader(File::open(path)?)?);
    }
    let agent = match args.agent.as_str() {
        "random" => Baseline::Random(RandomAgent::new(args.seed)),
        "reflex" => Baseline::Reflex(ReflexAgent::new(args.seed)),
        other => return Err(format!("unknown agent: {other}").into()),
    };
    let sim = Sim::new(Config::default(), args.seed)?;
    Ok(Checkpoint {
        sim,
        agent,
        metrics: Metrics::default(),
    })
}

fn execute(args: &Args) -> Result<(), Error> {
    let mut run = start(args)?;
    let mut log = args
        .log
        .as_ref()
        .map(File::create)
        .transpose()?
        .map(BufWriter::new);
    let mut write_err = None;
    run.run(args.steps, |record| {
        let Some(out) = log.as_mut() else { return };
        if write_err.is_some() {
            return;
        }
        let line = serde_json::to_string(record).map_err(Error::from);
        write_err = line
            .and_then(|l| writeln!(out, "{l}").map_err(Error::from))
            .err();
    });
    if let Some(err) = write_err {
        return Err(err);
    }
    if let Some(out) = log.as_mut() {
        out.flush()?;
    }
    if let Some(path) = &args.save {
        serde_json::to_writer(BufWriter::new(File::create(path)?), &run)?;
    }
    println!("{}", serde_json::to_string_pretty(&run.metrics.summary())?);
    Ok(())
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
