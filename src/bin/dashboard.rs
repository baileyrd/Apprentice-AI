//! Phase 3 dashboard: one apprentice life, replayable in a browser.
//!
//! The curious apprentice lives in a world where food grows back in place.
//! Half the walls move at `WALLS_AT`, all the food moves at `FOOD_AT`. Every
//! step is recorded (real map, believed map, position, energy, surprise) and
//! written into a self-contained HTML page with no external dependencies.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::model::Rates;
use apprentice::sim::{Event, Sim};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: dashboard [--seed N] [--out FILE]

Lives one curious apprentice (world seed N, default 6) for 4,500 steps: half
the walls move at step 1,500 and all the food moves at step 3,000. Writes a
self-contained replay page to FILE (default runs/dashboard.html).";

const STEPS: usize = 4_500;
const WALLS_AT: usize = 1_500;
const FOOD_AT: usize = 3_000;
const TEMPLATE: &str = include_str!("dashboard.html");

type Error = Box<dyn std::error::Error>;

/// A map that holds from step `t` until the next entry.
#[derive(Serialize)]
struct Frame {
    t: usize,
    map: String,
}

/// A moment the world changed.
#[derive(Serialize)]
struct Mark {
    t: usize,
    label: &'static str,
}

/// Everything the page replays.
#[derive(Serialize)]
struct Life {
    seed: u64,
    width: i32,
    height: i32,
    marks: Vec<Mark>,
    real: Vec<Frame>,
    believed: Vec<Frame>,
    /// Flat `x, y` per step, after the step.
    pos: Vec<i32>,
    energy: Vec<u32>,
    /// Surprise per step, in ten-thousandths.
    surprise: Vec<u32>,
    /// One letter per step: m moved, b bumped, h hurt, e ate, n ate nothing,
    /// r rested, s slept.
    events: String,
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

fn real_map(sim: &Sim) -> String {
    sim.grid()
        .positions()
        .map(|p| glyph(Some(sim.grid().get(p))))
        .collect()
}

fn believed_map(sim: &Sim, mind: &impl Mind) -> String {
    let start = sim.start();
    sim.grid()
        .positions()
        .map(|p| glyph(mind.believes(p.shift(-start.x, -start.y))))
        .collect()
}

fn letter(event: Event, slept: bool) -> char {
    if slept {
        return 's';
    }
    match event {
        Event::Moved => 'm',
        Event::Bumped => 'b',
        Event::Hurt => 'h',
        Event::Ate => 'e',
        Event::AteNothing => 'n',
        Event::Rested => 'r',
    }
}

/// Appends `map` unless it equals the last frame.
fn keep(frames: &mut Vec<Frame>, t: usize, map: String) {
    if frames.last().is_some_and(|f| f.map == map) {
        return;
    }
    frames.push(Frame { t, map });
}

fn live(seed: u64) -> Result<Life, Error> {
    let setup = Setup::for_change(Change::Food);
    let mut sim = Sim::new(setup.config, seed)?;
    let mut mind = Apprentice::new(Drives::default(), Rates::default(), seed);
    let walls = sim.grid().positions_of(Cell::Wall).len();
    let to_move = (walls as f64 * setup.wall_fraction).round() as usize;
    let mut life = Life {
        seed,
        width: sim.grid().width(),
        height: sim.grid().height(),
        marks: vec![
            Mark {
                t: WALLS_AT,
                label: "walls move",
            },
            Mark {
                t: FOOD_AT,
                label: "food moves",
            },
        ],
        real: Vec::new(),
        believed: Vec::new(),
        pos: Vec::with_capacity(2 * STEPS),
        energy: Vec::with_capacity(STEPS),
        surprise: Vec::with_capacity(STEPS),
        events: String::with_capacity(STEPS),
    };
    for t in 0..STEPS {
        if t == WALLS_AT {
            sim.move_walls(to_move);
        }
        if t == FOOD_AT {
            sim.move_food();
        }
        let action = mind.act(&sim.observe());
        let outcome = sim.step(action);
        mind.feel(&outcome);
        let Pos { x, y } = sim.pos();
        life.pos.extend([x, y]);
        life.energy.push(outcome.energy);
        life.surprise
            .push((mind.surprise() * 10_000.0).round() as u32);
        life.events.push(letter(outcome.event, outcome.slept));
        keep(&mut life.real, t, real_map(&sim));
        keep(&mut life.believed, t, believed_map(&sim, &mind));
    }
    Ok(life)
}

fn parse(args: &[String]) -> Result<(u64, PathBuf), String> {
    let (mut seed, mut out) = (6, PathBuf::from("runs/dashboard.html"));
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
    let life = live(seed)?;
    let page = TEMPLATE.replace("/*LIFE*/null", &serde_json::to_string(&life)?);
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(out, page)?;
    println!(
        "wrote {} ({} real frames, {} believed frames)",
        out.display(),
        life.real.len(),
        life.believed.len()
    );
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
        eprintln!("dashboard: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
