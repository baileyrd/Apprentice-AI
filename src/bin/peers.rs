//! Peers experiment: do two apprentices who share words settle into a new
//! world faster than one alone?
//!
//! Each apprentice grows up in four worlds, bush and barren in turn, alone or
//! beside a sibling in a twin copy of each world. When one eats, the other
//! says what it expects that square to become, in its own voice. Then a
//! newcomer, from a fresh copy, lives in a new barren world and in a new bush
//! world: alone, arriving with its sibling, after its sibling has been there
//! a while, or after a stranger has.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::body::{Said, Voice};
use apprentice::memory::{Memory, Recall};
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::peer;
use apprentice::sim::{Config, Event, Regrowth, Sim, SimError};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: peers [--seeds N] [--from SEED] [--lead L] [--world-steps S] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10). Raises a newcomer alone or with a
sibling, then times how it settles into a new barren and a new bush world:
alone, arriving with its sibling, after its sibling has had L steps there
(default 1,000), or after a stranger has; prints medians and writes
DIR/peers.csv (default DIR: runs/peers). Each upbringing world lasts S
steps (default 1,000).";

type Error = Box<dyn std::error::Error>;
type Learner = Apprentice<Memory>;

/// The voice each apprentice hears its companion in.
const COMPANION: Voice = Voice(1);
const UPBRINGING: u64 = 4;
const WORLD_STEPS: usize = 1_000;
const BARREN_STEPS: usize = 2_000;
const BUSH_STEPS: usize = 4_000;
const WINDOW: usize = 1_000;
/// Chance of food above which a square counts as hoped for.
const HOPE: f64 = 0.5;
/// Offset to the companion's own seed.
const SIBLING: u64 = 1_000_000;

/// How the newcomer and its companion grew up.
#[derive(Clone, Copy, PartialEq)]
enum Raised {
    /// The newcomer alone; no companion.
    Alone,
    /// Side by side, hearing each other.
    Together,
    /// Each alone, so they are strangers.
    Apart,
}

/// A way to arrive: how they grew up, and whether the companion arrives in
/// the new world first.
struct Case {
    name: &'static str,
    raised: Raised,
    ahead: bool,
}

const CASES: [Case; 4] = [
    Case {
        name: "alone",
        raised: Raised::Alone,
        ahead: false,
    },
    Case {
        name: "together",
        raised: Raised::Together,
        ahead: false,
    },
    Case {
        name: "elder ahead",
        raised: Raised::Together,
        ahead: true,
    },
    Case {
        name: "stranger ahead",
        raised: Raised::Apart,
        ahead: true,
    },
];

fn bushes() -> Config {
    Setup::for_change(Change::Food).config
}

fn barren() -> Config {
    Config {
        regrowth: Regrowth::Anywhere,
        ..bushes()
    }
}

fn newborn(seed: u64) -> Learner {
    let memory = Memory::new(Rates::default(), Recall::default());
    Apprentice::with_model(Drives::default(), memory, seed)
}

/// What a stretch in one world showed.
#[derive(Default)]
struct Stay {
    /// False hopes in the first `WINDOW` steps: a square it ate from comes
    /// into view empty when it gave it more than even odds of food.
    hopes: u32,
    /// Futile returns in the first `WINDOW` steps: eating nothing on a
    /// square it ate from.
    futile: u32,
    sleeps: u32,
}

/// One apprentice living in its own copy of a world.
struct Body {
    mind: Learner,
    sim: Sim,
    eaten: HashSet<Pos>,
    stay: Stay,
}

impl Body {
    fn new(mind: Learner, config: Config, seed: u64) -> Result<Self, SimError> {
        Ok(Self {
            mind,
            sim: Sim::new(config, seed)?,
            eaten: HashSet::new(),
            stay: Stay::default(),
        })
    }

    /// One step; the square it ate from, relative to its wake square, if it
    /// ate.
    fn step(&mut self) -> Option<Pos> {
        let t = self.sim.t() as usize;
        let here = self.sim.pos();
        if t < WINDOW {
            self.stay.hopes += self.false_hopes();
        }
        let obs = self.sim.observe();
        let outcome = self.sim.step(self.mind.act(&obs));
        self.mind.feel(&outcome);
        self.stay.sleeps += u32::from(outcome.slept);
        match outcome.event {
            Event::Ate => {
                self.eaten.insert(here);
                let start = self.sim.start();
                return Some(here.shift(-start.x, -start.y));
            }
            Event::AteNothing if t < WINDOW && self.eaten.contains(&here) => {
                self.stay.futile += 1;
            }
            _ => {}
        }
        None
    }

    /// Squares it ate from, now in view and empty, to which it gave more
    /// than even odds of food just before looking.
    fn false_hopes(&self) -> u32 {
        let (pos, start) = (self.sim.pos(), self.sim.start());
        let mut hopes = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let at = pos.shift(dx, dy);
                if !self.eaten.contains(&at) || self.sim.grid().get(at) == Cell::Food {
                    continue;
                }
                let belief = self.mind.model().predict(at.shift(-start.x, -start.y));
                hopes += u32::from(belief[kind(Cell::Food)] > HOPE);
            }
        }
        hopes
    }
}

/// `speaker` saw `listener` eat at `at`, and says what it expects there.
fn comment(speaker: &Learner, listener: &mut Learner, at: Pos) {
    if let Some(word) = peer::foretell(speaker.model()) {
        let said = Said {
            voice: COMPANION,
            word,
        };
        listener.hear(at, said);
    }
}

/// `minds` live in twin copies of a world seeded `seed`. Every mind but the
/// first has `lead` steps there alone; then all step together, each
/// commenting on the others' meals, for `steps`. Returns the minds and what
/// the first one's stay showed.
fn live(
    minds: Vec<Learner>,
    config: &Config,
    seed: u64,
    steps: usize,
    lead: usize,
) -> Result<(Vec<Learner>, Stay), SimError> {
    let mut bodies = minds
        .into_iter()
        .map(|m| Body::new(m, config.clone(), seed))
        .collect::<Result<Vec<_>, _>>()?;
    for _ in 0..lead {
        for body in bodies.iter_mut().skip(1) {
            body.step();
        }
    }
    for _ in 0..steps {
        for i in 0..bodies.len() {
            let Some(at) = bodies[i].step() else {
                continue;
            };
            for j in (0..bodies.len()).filter(|&j| j != i) {
                let (speaker, listener) = pair(&mut bodies, j, i);
                comment(&speaker.mind, &mut listener.mind, at);
            }
        }
    }
    let mut bodies = bodies.into_iter();
    let first = bodies.next().ok_or(SimError::NoRoomForAgent)?;
    let mut minds = vec![first.mind];
    minds.extend(bodies.map(|b| b.mind));
    Ok((minds, first.stay))
}

/// Shared and exclusive borrows of two different bodies.
fn pair(bodies: &mut [Body], speaker: usize, listener: usize) -> (&Body, &mut Body) {
    if speaker < listener {
        let (a, b) = bodies.split_at_mut(listener);
        (&a[speaker], &mut b[0])
    } else {
        let (a, b) = bodies.split_at_mut(speaker);
        (&b[0], &mut a[listener])
    }
}

/// An upbringing: bush and barren worlds in turn, ending in the kind unlike
/// `next`, so what they last learned is wrong there.
fn raise(
    minds: Vec<Learner>,
    seed: u64,
    next: &Config,
    steps: usize,
) -> Result<Vec<Learner>, SimError> {
    let (last, other) = if next.regrowth == Regrowth::Anywhere {
        (bushes(), barren())
    } else {
        (barren(), bushes())
    };
    let mut minds = minds;
    for i in 0..UPBRINGING {
        let config = if (UPBRINGING - i) % 2 == 1 {
            &last
        } else {
            &other
        };
        minds = live(minds, config, seed + 100 * i, steps, 0)?.0;
    }
    Ok(minds)
}

/// The newcomer and its companion, if any, raised for a test in `next`.
fn raised(
    raised: Raised,
    seed: u64,
    next: &Config,
    steps: usize,
) -> Result<Vec<Learner>, SimError> {
    let newcomer = newborn(seed);
    let companion = newborn(seed + SIBLING);
    match raised {
        Raised::Alone => raise(vec![newcomer], seed, next, steps),
        Raised::Together => raise(vec![newcomer, companion], seed, next, steps),
        Raised::Apart => {
            let mut minds = raise(vec![newcomer], seed, next, steps)?;
            minds.extend(raise(vec![companion], seed + SIBLING, next, steps)?);
            Ok(minds)
        }
    }
}

/// What one seed showed for one case.
struct Outcome {
    /// The newcomer's trust in its companion's voice after the upbringing,
    /// and after the barren and bush tests.
    trust: (f64, f64, f64),
    barren: Stay,
    bush: Stay,
}

fn run(case: &Case, seed: u64, args: &Args) -> Result<Outcome, SimError> {
    let lead = if case.ahead { args.lead } else { 0 };
    let steps = args.world_steps;
    let trust = |minds: &[Learner]| minds[0].model().trust(COMPANION);
    let minds = raised(case.raised, seed, &barren(), steps)?;
    let raised_trust = trust(&minds);
    let (minds, barren) = live(minds, &barren(), seed + 5_000, BARREN_STEPS, lead)?;
    let barren_trust = trust(&minds);
    let minds = raised(case.raised, seed, &bushes(), steps)?;
    let (minds, bush) = live(minds, &bushes(), seed + 6_000, BUSH_STEPS, lead)?;
    Ok(Outcome {
        trust: (raised_trust, barren_trust, trust(&minds)),
        barren,
        bush,
    })
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

/// Parsed flags.
struct Args {
    seeds: u64,
    from: u64,
    lead: usize,
    /// Steps in each upbringing world.
    world_steps: usize,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut seeds, mut from, mut lead) = (10, 1, WORLD_STEPS);
    let mut world_steps = WORLD_STEPS;
    let mut out = PathBuf::from("runs/peers");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || value.parse().map_err(|_| format!("bad {flag}: {value}"));
        match flag.as_str() {
            "--seeds" => seeds = number()?,
            "--from" => from = number()?,
            "--lead" => lead = number()? as usize,
            "--world-steps" => world_steps = number()? as usize,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if seeds == 0 {
        return Err("--seeds must be positive".into());
    }
    Ok(Args {
        seeds,
        from,
        lead,
        world_steps,
        out,
    })
}

fn execute(args: &Args) -> Result<(), Error> {
    let mut csv = String::from(
        "case,seed,trust_raised,trust_barren,trust_bush,barren_hopes,barren_futile,barren_sleeps,bush_hopes,bush_futile,bush_sleeps\n",
    );
    for case in &CASES {
        let mut results = Vec::new();
        for seed in args.from..args.from + args.seeds {
            let r = run(case, seed, args)?;
            csv.push_str(&format!(
                "{},{seed},{:.3},{:.3},{:.3},{},{},{},{},{},{}\n",
                case.name,
                r.trust.0,
                r.trust.1,
                r.trust.2,
                r.barren.hopes,
                r.barren.futile,
                r.barren.sleeps,
                r.bush.hopes,
                r.bush.futile,
                r.bush.sleeps,
            ));
            results.push(r);
        }
        let med = |f: fn(&Outcome) -> f64| median(results.iter().map(f).collect());
        println!(
            "{:>14}: trust raised {:.2}, after barren {:.2}, after bush {:.2}; barren false hopes {:.1}, futile {:.1}, sleeps {:.1}; bush false hopes {:.1}, sleeps {:.1}",
            case.name,
            med(|r| r.trust.0),
            med(|r| r.trust.1),
            med(|r| r.trust.2),
            med(|r| f64::from(r.barren.hopes)),
            med(|r| f64::from(r.barren.futile)),
            med(|r| f64::from(r.barren.sleeps)),
            med(|r| f64::from(r.bush.hopes)),
            med(|r| f64::from(r.bush.sleeps)),
        );
    }
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("peers.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&args) {
        eprintln!("peers: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
