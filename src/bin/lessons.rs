//! Lessons experiment: does a course of easier worlds prepare the apprentice
//! for a hard one better than the same time spent in hard worlds?
//!
//! Three minds take the same exam, a fresh hard world, for 4,000 steps:
//! - **no schooling**: the exam is its first world;
//! - **hard worlds**: it first lives as many steps as the schooled one did,
//!   lesson for lesson, but every world is already hard;
//! - **lessons**: it first takes the mentor's course of easier worlds.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::BUSH_DELAY;
use apprentice::apprentice::{Apprentice, Drives};
use apprentice::lessons::{Course, Lesson, live, study};
use apprentice::memory::{Memory, Recall};
use apprentice::metrics::Summary;
use apprentice::model::Rates;
use apprentice::sim::{Config, Regrowth, SimError};
use apprentice::world::Layout;

const USAGE: &str = "\
usage: lessons [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10). Each mind sits a 4,000-step exam in a
fresh hard world: with no schooling, after the same time in hard worlds, or
after the mentor's course of easier worlds. Prints median exam sleeps, meals
and hazards stepped on, and writes DIR/lessons.csv (default DIR: runs/lessons).";

type Error = Box<dyn std::error::Error>;

const EXAM: u64 = 4_000;
const MINDS: [&str; 3] = ["no schooling", "hard worlds", "lessons"];

/// A bush world of the given size and danger.
fn world(size: i32, wall_density: f64, hazard_density: f64, food_count: usize) -> Config {
    Config {
        layout: Layout {
            width: size,
            height: size,
            wall_density,
            hazard_density,
            food_count,
        },
        regrowth: Regrowth::InPlace { delay: BUSH_DELAY },
        ..Config::default()
    }
}

/// The exam's kind of world: full size, few bushes, many hazards.
fn hard() -> Config {
    world(12, 0.12, 0.15, 3)
}

/// The mentor's course: small and safe, then middling.
fn course() -> Course {
    Course {
        lessons: vec![world(6, 0.05, 0.0, 5), world(9, 0.1, 0.08, 5)],
        mastery: 300,
        cap: 3_000,
    }
}

/// One mind's schooling and exam.
struct Sitting {
    lessons: Vec<Lesson>,
    exam: Summary,
}

fn sit(name: &str, seed: u64) -> Result<Sitting, SimError> {
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
    let course = course();
    let first = seed * 10;
    let lessons = match name {
        "lessons" => study(&mut mind, &course, first)?,
        "hard worlds" => {
            // The same steps the course took on this seed, in hard worlds.
            let schedule = study(
                &mut Apprentice::with_model(
                    Drives::default(),
                    Memory::new(Rates::default(), Recall::default()),
                    seed,
                ),
                &course,
                first,
            )?;
            for (i, lesson) in schedule.iter().enumerate() {
                live(&mut mind, &hard(), first + i as u64, lesson.steps)?;
            }
            schedule
        }
        _ => Vec::new(),
    };
    let exam = live(&mut mind, &hard(), 1_000 + seed, EXAM)?;
    Ok(Sitting { lessons, exam })
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => xs[n / 2],
        _ => (xs[n / 2 - 1] + xs[n / 2]) / 2.0,
    }
}

/// Parsed flags: seed count, first seed, output directory.
fn parse(args: &[String]) -> Result<(u64, u64, PathBuf), String> {
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/lessons"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || value.parse().map_err(|_| format!("bad {flag}: {value}"));
        match flag.as_str() {
            "--seeds" => seeds = number()?,
            "--from" => from = number()?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if seeds == 0 {
        return Err("--seeds must be positive".into());
    }
    Ok((seeds, from, out))
}

fn execute(seeds: u64, from: u64, out: &PathBuf) -> Result<(), Error> {
    let mut csv =
        String::from("mind,seed,exam_sleeps,exam_meals,exam_hurts,school_steps,lessons_mastered\n");
    for name in MINDS {
        let mut sittings = Vec::new();
        for seed in from..from + seeds {
            let s = sit(name, seed)?;
            let steps: u64 = s.lessons.iter().map(|l| l.steps).sum();
            let mastered = s.lessons.iter().filter(|l| l.mastered).count();
            csv.push_str(&format!(
                "{name},{seed},{},{},{},{steps},{mastered}\n",
                s.exam.sleeps, s.exam.meals, s.exam.hurts
            ));
            sittings.push(s);
        }
        let med = |f: &dyn Fn(&Sitting) -> f64| median(sittings.iter().map(f).collect());
        let all_mastered = sittings
            .iter()
            .filter(|s| !s.lessons.is_empty() && s.lessons.iter().all(|l| l.mastered))
            .count();
        println!(
            "{name:>12}: exam sleeps {:.1}, meals {:.0}, hazards {:.0}; schooling {:.0} steps, every lesson mastered in {all_mastered}/{}",
            med(&|s| s.exam.sleeps as f64),
            med(&|s| s.exam.meals as f64),
            med(&|s| s.exam.hurts as f64),
            med(&|s| s.lessons.iter().map(|l| l.steps).sum::<u64>() as f64),
            sittings.len(),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("lessons.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seeds, from, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(seeds, from, &out) {
        eprintln!("lessons: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
