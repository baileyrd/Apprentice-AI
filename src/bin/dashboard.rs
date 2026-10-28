//! Dashboard: one apprentice life, replayable in a browser.
//!
//! Two stories, both in a world where food grows back in place:
//! - **changes** (phase 3): the curious apprentice; half the walls move at
//!   `WALLS_AT`, all the food moves at `FOOD_AT`;
//! - **return**: the apprentice with memory and patterns lives in world A,
//!   then B, then A again, `PHASE` steps each; the page also shows which of
//!   its maps it is using.
//!
//! Every step is recorded (real map, believed map, position, energy,
//! surprise) and written into a self-contained HTML page.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::sim::{Event, Outcome, Sim};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: dashboard [--story changes|return] [--seed N] [--out FILE]

changes (default): one curious apprentice (world seed N, default 6) for 4,500
steps; half the walls move at step 1,500 and all the food moves at step 3,000.
return: the apprentice with memory and patterns lives world A (seed N), then
B (seed N + 1000), then A again, 3,000 steps each.
Writes a self-contained replay page to FILE (default runs/dashboard.html).";

const STEPS: usize = 4_500;
const WALLS_AT: usize = 1_500;
const FOOD_AT: usize = 3_000;
/// Steps in each world of the return story.
const PHASE: usize = 3_000;
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
    lede: &'static str,
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
    /// Memory map in use per step (0 is the first world's); empty when the
    /// mind has no memory.
    maps: Vec<usize>,
}

impl Life {
    fn new(seed: u64, lede: &'static str, sim: &Sim, marks: Vec<Mark>, steps: usize) -> Self {
        Self {
            seed,
            lede,
            width: sim.grid().width(),
            height: sim.grid().height(),
            marks,
            real: Vec::new(),
            believed: Vec::new(),
            pos: Vec::with_capacity(2 * steps),
            energy: Vec::with_capacity(steps),
            surprise: Vec::with_capacity(steps),
            events: String::with_capacity(steps),
            maps: Vec::new(),
        }
    }

    /// Records step `t`, just taken in `sim`.
    fn record(&mut self, t: usize, sim: &Sim, mind: &impl Mind, outcome: &Outcome) {
        let Pos { x, y } = sim.pos();
        self.pos.extend([x, y]);
        self.energy.push(outcome.energy);
        self.surprise
            .push((mind.surprise() * 10_000.0).round() as u32);
        self.events.push(letter(outcome.event, outcome.slept));
        keep(&mut self.real, t, real_map(sim));
        keep(&mut self.believed, t, believed_map(sim, mind));
    }
}

/// Which life to replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Story {
    Changes,
    Return,
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

fn changes(seed: u64) -> Result<Life, Error> {
    let setup = Setup::for_change(Change::Food);
    let mut sim = Sim::new(setup.config, seed)?;
    let mut mind = Apprentice::new(Drives::default(), Rates::default(), seed);
    let walls = sim.grid().positions_of(Cell::Wall).len();
    let to_move = (walls as f64 * setup.wall_fraction).round() as usize;
    let marks = vec![
        Mark {
            t: WALLS_AT,
            label: "walls move",
        },
        Mark {
            t: FOOD_AT,
            label: "food moves",
        },
    ];
    let lede = "One life of the curious apprentice. Half the walls move, then all the food moves. Watch what it believes catch up with what is there.";
    let mut life = Life::new(seed, lede, &sim, marks, STEPS);
    for t in 0..STEPS {
        if t == WALLS_AT {
            sim.move_walls(to_move);
        }
        if t == FOOD_AT {
            sim.move_food();
        }
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        life.record(t, &sim, &mind, &outcome);
    }
    Ok(life)
}

fn ret(seed: u64) -> Result<Life, Error> {
    let config = Setup::for_change(Change::Food).config;
    let mut a = Sim::new(config.clone(), seed)?;
    let mut b = Sim::new(config, seed + 1000)?;
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
    let marks = vec![
        Mark {
            t: PHASE,
            label: "moved to world B",
        },
        Mark {
            t: 2 * PHASE,
            label: "back to world A",
        },
    ];
    let lede = "The apprentice with memory and patterns lives in world A, then B, then A again. It is never told the world changed. Watch which of its maps it uses, and whether A's map survives B.";
    let mut life = Life::new(seed, lede, &a, marks, 3 * PHASE);
    for t in 0..3 * PHASE {
        let sim = if (PHASE..2 * PHASE).contains(&t) {
            &mut b
        } else {
            &mut a
        };
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        life.record(t, sim, &mind, &outcome);
        life.maps.push(mind.model().active());
    }
    Ok(life)
}

fn parse(args: &[String]) -> Result<(Story, u64, PathBuf), String> {
    let (mut story, mut seed, mut out) = (Story::Changes, 6, PathBuf::from("runs/dashboard.html"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--story" => {
                story = match value.as_str() {
                    "changes" => Story::Changes,
                    "return" => Story::Return,
                    _ => return Err(format!("unknown story: {value}")),
                }
            }
            "--seed" => seed = value.parse().map_err(|_| format!("bad seed: {value}"))?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    Ok((story, seed, out))
}

fn execute(story: Story, seed: u64, out: &PathBuf) -> Result<(), Error> {
    let life = match story {
        Story::Changes => changes(seed)?,
        Story::Return => ret(seed)?,
    };
    let title = match story {
        Story::Changes => "Apprentice Replay",
        Story::Return => "Apprentice Return Replay",
    };
    let page = TEMPLATE
        .replace(
            "<title>Apprentice Replay</title>",
            &format!("<title>{title}</title>"),
        )
        .replace("/*LIFE*/null", &serde_json::to_string(&life)?);
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
    let (story, seed, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(story, seed, &out) {
        eprintln!("dashboard: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
