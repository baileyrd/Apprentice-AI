//! Mind replay: what the apprentice knows about home, when it knows it, and
//! how it knows it.
//!
//! One home (seed N). For each trial (home, home with a wall moved, three
//! strangers) the page replays the last `BEFORE` steps at home and the first
//! `TRIAL_STEPS` in the trial world. Every step records the real world, what
//! it believes about each square and where that belief comes from (a
//! pattern, today's diary, the map in use, or nothing yet), what its home map
//! alone holds, each map's running error, its doubt, and its answer to "is
//! this home?".

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;

use apprentice::apprentice::Mind;
use apprentice::home::{HOME_STEPS, Learner, Place, at_home, newborn, world};
use apprentice::memory::{Memory, Recall, Source};
use apprentice::model::{Belief, most_likely};
use apprentice::sim::{Event, Outcome, Sim};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: mind [--seed N] [--out FILE]

Replays, for home seed N (default 3), the last 150 steps at home and the first
300 steps in each trial world (home, home with a wall moved, three strangers):
what it believes about each square and why, what its home map holds, each
map's fit, its doubt and its answer to \"is this home?\". Writes a
self-contained page to FILE (default runs/mind.html).";

const BEFORE: usize = 150;
const TRIAL_STEPS: usize = 300;
const TEMPLATE: &str = include_str!("mind.html");

type Error = Box<dyn std::error::Error>;

/// One step, as the page draws it. Square strings cover the grid plus a
/// one-square border, row by row.
#[derive(Serialize)]
struct Step {
    real: String,
    /// Most likely kind per square: `.` `#` `*` `^`, `?` if never seen.
    believed: String,
    /// Where each belief comes from: `p` pattern, `d` diary, `m` map, `u`
    /// never seen.
    source: String,
    /// How sure, per square: `0`–`9`.
    sure: String,
    /// What the home map alone holds, and how sure.
    home: String,
    home_sure: String,
    x: i32,
    y: i32,
    /// m moved, b bumped, h hurt, e ate, n ate nothing, r rested, s slept.
    event: char,
    /// Map in use.
    map: usize,
    /// Each map's running error in ten-thousandths; `null` while it does not
    /// count yet.
    fits: Vec<Option<u32>>,
    doubt: usize,
    /// Sightings held out of every map while in doubt.
    held: usize,
    home_answer: bool,
}

#[derive(Serialize)]
struct Trial {
    name: String,
    is_home: bool,
    steps: Vec<Step>,
}

#[derive(Serialize)]
struct Replay {
    seed: u64,
    width: i32,
    height: i32,
    /// Index of the first step in the trial world.
    trial_at: usize,
    home_map: Option<usize>,
    misfit: u32,
    settle: usize,
    trials: Vec<Trial>,
}

fn glyph(cell: Option<Cell>) -> char {
    match cell {
        None => '?',
        Some(Cell::Empty) => '.',
        Some(Cell::Wall) => '#',
        Some(Cell::Food) => '*',
        Some(Cell::Hazard) => '^',
    }
}

fn source(s: Source) -> char {
    match s {
        Source::Pattern => 'p',
        Source::Active => 'a',
        Source::Diary => 'd',
        Source::Map => 'm',
        Source::Unknown => 'u',
    }
}

/// How sure a belief is, from 0 (uniform) to 9 (certain).
fn sure(b: &Belief) -> char {
    let top = b.iter().copied().fold(0.0, f64::max);
    let level = ((top - 0.25) / 0.75 * 9.0).round().clamp(0.0, 9.0) as u32;
    char::from_digit(level, 10).unwrap_or('0')
}

fn letter(outcome: &Outcome) -> char {
    if outcome.slept {
        return 's';
    }
    match outcome.event {
        Event::Moved => 'm',
        Event::Bumped => 'b',
        Event::Hurt => 'h',
        Event::Ate => 'e',
        Event::AteNothing => 'n',
        Event::Rested => 'r',
    }
}

/// Grid squares plus a one-square border, row by row.
fn squares(sim: &Sim) -> impl Iterator<Item = Pos> {
    let (w, h) = (sim.grid().width(), sim.grid().height());
    (-1..=h).flat_map(move |y| (-1..=w).map(move |x| Pos { x, y }))
}

fn record(sim: &Sim, mind: &Learner, outcome: &Outcome, home_map: Option<usize>) -> Step {
    let memory: &Memory = mind.model();
    let start = sim.start();
    let rel = |p: Pos| p.shift(-start.x, -start.y);
    let mut s = Step {
        real: String::new(),
        believed: String::new(),
        source: String::new(),
        sure: String::new(),
        home: String::new(),
        home_sure: String::new(),
        x: sim.pos().x,
        y: sim.pos().y,
        event: letter(outcome),
        map: memory.active(),
        fits: memory
            .fits()
            .iter()
            .map(|f| f.map(|v| (v * 10_000.0).round() as u32))
            .collect(),
        doubt: memory.doubt().0,
        held: memory.held(),
        home_answer: mind.map_in_use() == home_map,
    };
    for p in squares(sim) {
        s.real.push(glyph(Some(sim.grid().get(p))));
        let (belief, from) = memory.explain(rel(p));
        let seen = from != Source::Unknown;
        s.believed
            .push(glyph(if seen { most_likely(&belief) } else { None }));
        s.source.push(source(from));
        s.sure.push(sure(&belief));
        let held = home_map.and_then(|i| memory.recalls(i, rel(p)));
        s.home.push(glyph(held.and_then(|b| most_likely(&b))));
        s.home_sure.push(held.map_or('0', |b| sure(&b)));
    }
    s
}

fn trial(place: Place, seed: u64) -> Result<Trial, Error> {
    // Its life is deterministic: live it once to learn which map is home,
    // then again to record the run-up against that map.
    let (_, _, home_map) = at_home(seed)?;
    let (mut mind, mut sim) = newborn(seed)?;
    let mut steps = Vec::with_capacity(BEFORE + TRIAL_STEPS);
    for t in 0..HOME_STEPS {
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        if t >= HOME_STEPS - BEFORE {
            steps.push(record(&sim, &mind, &outcome, home_map));
        }
    }
    let mut there = world(place, &sim, seed);
    for _ in 0..TRIAL_STEPS {
        let outcome = there.step(mind.act(&there.observe()));
        mind.feel(&outcome);
        steps.push(record(&there, &mind, &outcome, home_map));
    }
    let name = match place {
        Place::Stranger(k) => format!("Stranger {}", k + 1),
        Place::Home => "Home".into(),
        Place::Changed => "Home, a wall moved".into(),
    };
    Ok(Trial {
        name,
        is_home: place.is_home(),
        steps,
    })
}

fn parse(args: &[String]) -> Result<(u64, PathBuf), String> {
    let (mut seed, mut out) = (3, PathBuf::from("runs/mind.html"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => seed = value.parse().map_err(|_| format!("bad seed: {value}"))?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    Ok((seed, out))
}

fn execute(seed: u64, out: &PathBuf) -> Result<(), Error> {
    let places = [
        Place::Home,
        Place::Changed,
        Place::Stranger(0),
        Place::Stranger(1),
        Place::Stranger(2),
    ];
    let trials = places
        .into_iter()
        .map(|p| trial(p, seed))
        .collect::<Result<Vec<_>, _>>()?;
    let (_, home) = newborn(seed)?;
    let recall = Recall::default();
    let replay = Replay {
        seed,
        width: home.grid().width() + 2,
        height: home.grid().height() + 2,
        trial_at: BEFORE,
        home_map: at_home(seed)?.2,
        misfit: (recall.misfit * 10_000.0).round() as u32,
        settle: recall.settle,
        trials,
    };
    let page = TEMPLATE.replace("/*REPLAY*/null", &serde_json::to_string(&replay)?);
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(out, page)?;
    println!("wrote {}", out.display());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seed, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(seed, &out) {
        eprintln!("mind: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
