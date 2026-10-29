//! Language experiment: can a mentor's word for what a square will become
//! settle, square by square, what shared patterns cannot?
//!
//! Each apprentice grows up in four worlds, alternating bush worlds (eaten
//! bushes grow back) and barren worlds (they never do), with no mentor, a
//! mentor that says its true word for what an eaten square will do, or one
//! that says either word at random. Two more minds hear both mentors, taking
//! turns meal by meal, either in one voice or in two it can tell apart. Then, from a
//! fresh copy, it lives in a new barren world and in a new bush world.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::body::{Said, Voice, Word};
use apprentice::memory::{Memory, Recall};
use apprentice::mentor::{Foretell, GROWS_BACK, Mentor, STAYS_GONE, Teaching};
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::patterns::Last;
use apprentice::sim::{Config, Event, Regrowth, Sim, SimError};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: foretell [--seeds N] [--from SEED] [--world-steps S] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10). Raises apprentices with no mentor, a
true foretelling mentor, a random one, and both at once in one voice or two;
prints what its words came to mean, how far it trusts each voice,
futile returns in a new barren world and sleeps in a new bush world; writes
DIR/foretell.csv (default DIR: runs/foretell). Each upbringing world lasts S
steps (default 1,000).";

type Error = Box<dyn std::error::Error>;
type Learner = Apprentice<Memory>;

/// Each mind's mentors: how each foretells, and its voice.
type Mentors = &'static [(Foretell, u8)];
const HONEST: Voice = Voice(0);
const OTHER: Voice = Voice(1);
const MINDS: [(&str, Mentors); 5] = [
    ("no mentor", &[]),
    ("true mentor", &[(Foretell::Truly, 0)]),
    ("random mentor", &[(Foretell::AtRandom, 0)]),
    (
        "both, one voice",
        &[(Foretell::Truly, 0), (Foretell::AtRandom, 0)],
    ),
    (
        "both, two voices",
        &[(Foretell::Truly, 0), (Foretell::AtRandom, 1)],
    ),
];
const UPBRINGING: u64 = 4;
const BARREN_STEPS: usize = 2_000;
const BUSH_STEPS: usize = 4_000;
const WINDOW: usize = 1_000;
/// Age at which a word's meaning is read: a square eaten 32–63 steps ago,
/// when a bush has grown back.
const AGE: u64 = 32;
/// Chance of food above which a square counts as hoped for.
const HOPE: f64 = 0.5;

fn bushes() -> Config {
    Setup::for_change(Change::Food).config
}

fn barren() -> Config {
    Config {
        regrowth: Regrowth::Anywhere,
        ..bushes()
    }
}

/// A mind and the mentors that raise it.
struct Life {
    mind: Learner,
    mentors: Vec<Mentor>,
}

/// What a stretch in one world showed.
#[derive(Default)]
struct Stay {
    /// Futile returns in the first `WINDOW` steps.
    futile: u32,
    /// False hopes in the first `WINDOW` steps: a square it ate from comes
    /// into view empty when it gave it more than even odds of food.
    hopes: u32,
    sleeps: u32,
    meals: u32,
}

impl Life {
    /// A newborn mind with `mentors`.
    fn new(mentors: Mentors, seed: u64) -> Self {
        let memory = Memory::new(Rates::default(), Recall::default());
        let mentor = |i: usize, &(foretells, voice): &(Foretell, u8)| {
            let teaching = Teaching {
                fade: u64::MAX,
                foretells,
                ..Teaching::default()
            };
            Mentor::new(teaching, seed + i as u64).with_voice(Voice(voice))
        };
        Self {
            mind: Apprentice::with_model(Drives::default(), memory, seed),
            mentors: mentors
                .iter()
                .enumerate()
                .map(|(i, m)| mentor(i, m))
                .collect(),
        }
    }

    /// The upbringing: bush and barren worlds in turn, ending in the kind
    /// unlike `next`, so what it last learned is wrong there.
    fn raised(mentors: Mentors, seed: u64, next: &Config, steps: usize) -> Result<Self, SimError> {
        let mut life = Self::new(mentors, seed);
        let last = if next.regrowth == Regrowth::Anywhere {
            bushes()
        } else {
            barren()
        };
        let other = if next.regrowth == Regrowth::Anywhere {
            barren()
        } else {
            bushes()
        };
        for i in 0..UPBRINGING {
            let config = if (UPBRINGING - i) % 2 == 1 {
                last.clone()
            } else {
                other.clone()
            };
            life.live(config, seed + 100 * i, steps)?;
        }
        Ok(life)
    }

    fn live(&mut self, config: Config, seed: u64, steps: usize) -> Result<Stay, SimError> {
        let mut sim = Sim::new(config, seed)?;
        let mut eaten: HashSet<Pos> = HashSet::new();
        let mut stay = Stay::default();
        for t in 0..steps {
            let here = sim.pos();
            let obs = sim.observe();
            if t < WINDOW {
                stay.hopes += self.false_hopes(&sim, &eaten);
            }
            let outcome = sim.step(self.mind.act(&obs));
            self.mind.feel(&outcome);
            // Mentors take turns: one speaks about each meal.
            let turn = stay.meals as usize % self.mentors.len().max(1);
            if let Some((at, said)) = self
                .mentors
                .get_mut(turn)
                .and_then(|m| m.foretell(&sim, &outcome))
            {
                self.mind.hear(at, said);
            }
            stay.sleeps += u32::from(outcome.slept);
            match outcome.event {
                Event::Ate => {
                    stay.meals += 1;
                    eaten.insert(here);
                }
                Event::AteNothing if t < WINDOW && eaten.contains(&here) => stay.futile += 1,
                _ => {}
            }
        }
        Ok(stay)
    }

    /// Squares it ate from, now in view and empty, to which it gave more
    /// than even odds of food just before looking.
    fn false_hopes(&self, sim: &Sim, eaten: &HashSet<Pos>) -> u32 {
        let start = sim.start();
        let mut hopes = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let at = sim.pos().shift(dx, dy);
                if !eaten.contains(&at) || sim.grid().get(at) == Cell::Food {
                    continue;
                }
                let belief = self.mind.model().predict(at.shift(-start.x, -start.y));
                hopes += u32::from(belief[kind(Cell::Food)] > HOPE);
            }
        }
        hopes
    }

    /// Chance of food `word`, in the honest mentor's voice, gives a square
    /// eaten `AGE` steps ago; `None` until learned.
    fn meaning(&self, word: Word) -> Option<f64> {
        let said = Said {
            voice: HONEST,
            word,
        };
        let b = self.mind.model().foretold(Last::Ate, AGE, said)?;
        Some(b[kind(Cell::Food)])
    }

    /// The same, from the general pattern, with no word said.
    fn expected(&self) -> Option<f64> {
        let b = self.mind.model().expects(Last::Ate, AGE)?;
        Some(b[kind(Cell::Food)])
    }
}

/// What one seed showed for one mind.
struct Outcome {
    /// Trust in the first and second voice after the upbringing.
    trust: (f64, f64),
    general: Option<f64>,
    grows_back: Option<f64>,
    stays_gone: Option<f64>,
    barren: Stay,
    bush: Stay,
}

/// Raises the mind twice, once for each test, each upbringing ending in the
/// other kind of world.
fn run(mentors: Mentors, seed: u64, steps: usize) -> Result<Outcome, SimError> {
    let mut life = Life::raised(mentors, seed, &bushes(), steps)?;
    let (grows_back, stays_gone) = (life.meaning(GROWS_BACK), life.meaning(STAYS_GONE));
    let general = life.expected();
    let model = life.mind.model();
    let trust = (model.trust(HONEST), model.trust(OTHER));
    let bush = life.live(bushes(), seed + 6_000, BUSH_STEPS)?;
    let barren = Life::raised(mentors, seed, &barren(), steps)?.live(
        barren(),
        seed + 5_000,
        BARREN_STEPS,
    )?;
    Ok(Outcome {
        trust,
        general,
        grows_back,
        stays_gone,
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
    out: PathBuf,
    /// Steps in each upbringing world.
    world_steps: usize,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/foretell"));
    let mut world_steps = 1_000;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || value.parse().map_err(|_| format!("bad {flag}: {value}"));
        match flag.as_str() {
            "--seeds" => seeds = number()?,
            "--from" => from = number()?,
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
        out,
        world_steps,
    })
}

fn show(x: Option<f64>) -> String {
    x.map_or(String::new(), |v| format!("{v:.3}"))
}

fn execute(args: &Args) -> Result<(), Error> {
    let Args {
        seeds,
        from,
        ref out,
        world_steps,
    } = *args;
    let mut csv = String::from(
        "mind,seed,trust_0,trust_1,general,grows_back,stays_gone,barren_futile,barren_hopes,barren_sleeps,barren_meals,bush_futile,bush_hopes,bush_sleeps,bush_meals\n",
    );
    for (name, mentors) in MINDS {
        let mut results = Vec::new();
        for seed in from..from + seeds {
            let r = run(mentors, seed, world_steps)?;
            csv.push_str(&format!(
                "{name},{seed},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.trust.0,
                r.trust.1,
                show(r.general),
                show(r.grows_back),
                show(r.stays_gone),
                r.barren.futile,
                r.barren.hopes,
                r.barren.sleeps,
                r.barren.meals,
                r.bush.futile,
                r.bush.hopes,
                r.bush.sleeps,
                r.bush.meals,
            ));
            results.push(r);
        }
        let med =
            |f: fn(&Outcome) -> u32| median(results.iter().map(|r| f64::from(f(r))).collect());
        let learned = results
            .iter()
            .filter(|r| {
                r.grows_back.is_some_and(|p| p >= 0.8) && r.stays_gone.is_some_and(|p| p <= 0.2)
            })
            .count();
        println!(
            "{name:>16}: words learned {learned}/{}; trust {:.2} / {:.2}; barren futile returns (first 1,000) {:.1}, false hopes {:.1}, sleeps {:.1}, meals {:.1}; bush false hopes {:.1}, sleeps {:.1}, meals {:.1}",
            results.len(),
            median(results.iter().map(|r| r.trust.0).collect()),
            median(results.iter().map(|r| r.trust.1).collect()),
            med(|r| r.barren.futile),
            med(|r| r.barren.hopes),
            med(|r| r.barren.sleeps),
            med(|r| r.barren.meals),
            med(|r| r.bush.hopes),
            med(|r| r.bush.sleeps),
            med(|r| r.bush.meals),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("foretell.csv"), csv)?;
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
        eprintln!("foretell: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
