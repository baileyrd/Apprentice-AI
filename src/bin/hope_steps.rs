//! The second false-hope mark of docs/sweep (Nano, 2026-10-30): steps
//! wasted on false hopes against steps lost to sleeps, both measured in
//! each arm, in the barren test world of the barren-first life. Lives each
//! seed's barren-first life under A (today's default) and S (the sweep
//! fix), step by step.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::judge::Interval;
use apprentice::life::{
    BARREN_STEPS, HOPE, Learner, Setting, UPBRINGING, WINDOW, WORLD_STEPS, barren, bushes, live,
};
use apprentice::memory::{Memory, Recall};
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::patterns::PatternNet;
use apprentice::sim::{Event, Sim, SimError};
use apprentice::world::{Cell, Grid, Pos};

const USAGE: &str = "\
usage: hope_steps [--seeds N] [--from SEED] [--judge] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110), or with --judge the judging seeds
1-100 and 111-210. Prints the mark's verdict and each arm's steps per false
hope (W) and per sleep (S), and writes DIR/hope_steps.csv (default DIR:
runs/sweep).";

type Error = Box<dyn std::error::Error>;

/// What one arm showed in the barren test world.
#[derive(Debug, Default, Clone, Copy)]
struct Steps {
    /// False hopes in the first `WINDOW` steps.
    hopes: u32,
    /// Steps wasted on them: taken toward the hoped-for square, not toward
    /// the next square it ate from.
    wasted: u32,
    sleeps: u32,
    /// Steps lost to sleeps: the walk from the start square, where it
    /// wakes, back to where it fell asleep.
    lost: u32,
}

/// Steps from `from` to every square it can walk to, around walls.
fn walks(grid: &Grid, from: Pos) -> HashMap<Pos, u32> {
    let mut far = HashMap::from([(from, 0)]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        let d = far[&p];
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let q = p.shift(dx, dy);
            let inside = q.x >= 0 && q.y >= 0 && q.x < grid.width() && q.y < grid.height();
            if inside && grid.get(q) != Cell::Wall && !far.contains_key(&q) {
                far.insert(q, d + 1);
                queue.push_back(q);
            }
        }
    }
    far
}

/// One barren-first life under `setting`, measured in its test world.
fn barren_life(setting: Setting, seed: u64) -> Result<Steps, SimError> {
    let memory = Memory::new(Rates::default(), setting.recall);
    let mut mind: Learner = Apprentice::with_model(setting.drives, memory, seed);
    for i in 0..UPBRINGING {
        let world = if i % 2 == 0 { barren() } else { bushes() };
        live(&mut mind, world, seed + 100 * i, WORLD_STEPS)?;
    }
    let mut sim = Sim::new(barren(), seed + 5_000)?;
    let mut out = Steps::default();
    let (mut path, mut woke) = (Vec::new(), Vec::new());
    let (mut hopes, mut meals) = (Vec::new(), Vec::new());
    let mut eaten: HashSet<Pos> = HashSet::new();
    for t in 0..BARREN_STEPS {
        let here = sim.pos();
        path.push(here);
        if t < WINDOW {
            hopes.extend(false_hopes(&mind, &sim, &eaten).into_iter().map(|q| (t, q)));
        }
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        woke.push(outcome.slept);
        if outcome.slept {
            out.sleeps += 1;
            out.lost += walks(sim.grid(), sim.start())
                .get(&here)
                .copied()
                .unwrap_or(0);
        }
        if outcome.event == Event::Ate {
            eaten.insert(here);
            meals.push((t, here));
        }
    }
    path.push(sim.pos());
    out.hopes = hopes.len() as u32;
    out.wasted = wasted(sim.grid(), &path, &woke, &hopes, &meals);
    Ok(out)
}

/// Eaten squares in view, empty, given more than even odds of food.
fn false_hopes(mind: &Learner, sim: &Sim, eaten: &HashSet<Pos>) -> Vec<Pos> {
    let start = sim.start();
    let mut hoped = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let at = sim.pos().shift(dx, dy);
            if !eaten.contains(&at) || sim.grid().get(at) == Cell::Food {
                continue;
            }
            let belief = mind.model().predict(at.shift(-start.x, -start.y));
            if belief[kind(Cell::Food)] > HOPE {
                hoped.push(at);
            }
        }
    }
    hoped
}

/// Steps toward a hoped-for square that a straight route to the next meal
/// would not have taken. Step `k` goes from `path[k - 1]` to `path[k]`;
/// those around a false hope at `t` that close on its square (the run just
/// before `t` and after it in which the walk to the square shrinks every
/// step) are wasted unless they also shrink the walk to the next square it
/// eats from. With no later meal, every closing step is wasted. Each step
/// counts once per square.
fn wasted(
    grid: &Grid,
    path: &[Pos],
    woke: &[bool],
    hopes: &[(usize, Pos)],
    meals: &[(usize, Pos)],
) -> u32 {
    let mut steps: HashSet<(usize, Pos)> = HashSet::new();
    for &(t, q) in hopes {
        let to_q = walks(grid, q);
        let to_goal = meals
            .iter()
            .find(|(m, _)| *m >= t)
            .map(|&(_, g)| walks(grid, g));
        let far = |map: &HashMap<Pos, u32>, p: Pos| map.get(&p).copied().unwrap_or(u32::MAX);
        let closes = |k: usize| !woke[k - 1] && far(&to_q, path[k]) < far(&to_q, path[k - 1]);
        let aimless = |k: usize| {
            to_goal
                .as_ref()
                .is_none_or(|g| far(g, path[k]) >= far(g, path[k - 1]))
        };
        let mut k = t;
        while k > 0 && closes(k) {
            if aimless(k) {
                steps.insert((k, q));
            }
            k -= 1;
        }
        let mut k = t + 1;
        while k < path.len() && closes(k) {
            if aimless(k) {
                steps.insert((k, q));
            }
            k += 1;
        }
    }
    steps.len() as u32
}

/// The two arms: today's default, and the sweep fix.
fn arms() -> [Setting; 2] {
    let a = Setting::from(Recall::default());
    let s = Setting {
        recall: Recall {
            pattern_net: Some(PatternNet {
                unseen: true,
                ..PatternNet::default()
            }),
            ..Recall::default()
        },
        drives: Drives {
            sweep: true,
            ..Drives::default()
        },
    };
    [a, s]
}

/// One seed and what each arm showed.
type Row = (u64, [Steps; 2]);

fn measure(seeds: &[u64]) -> Result<Vec<Row>, Error> {
    let [a, s] = arms();
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let parts: Vec<Result<Vec<Row>, SimError>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&seed| Ok((seed, [barren_life(a, seed)?, barren_life(s, seed)?])))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(SimError::NoRoomForAgent)))
            .collect()
    });
    let mut all = Vec::new();
    for part in parts {
        all.extend(part?);
    }
    Ok(all)
}

/// Per seed, per 1,000 steps of the barren test world: extra steps wasted
/// on false hopes, less half the steps saved on sleeps. At most 0 passes.
fn excess(row: &[Steps; 2]) -> f64 {
    let [a, s] = row;
    let thousands = BARREN_STEPS as f64 / 1_000.0;
    let extra = f64::from(s.wasted) - f64::from(a.wasted);
    let saved = (f64::from(a.lost) - f64::from(s.lost)) / thousands;
    extra - 0.5 * saved
}

fn report(rows: &[Row]) -> Result<(), Error> {
    let x: Vec<f64> = rows.iter().map(|r| excess(&r.1)).collect();
    let i = Interval::mean(&x)?;
    println!(
        "3b'. extra steps wasted on false hopes less half the sleep steps saved, per 1,000 steps: {:+.2} [{:+.2}, {:+.2}]: {}",
        i.mid,
        i.lo,
        i.hi,
        i.at_most(0.0)
    );
    for (arm, name) in ["A", "S"].iter().enumerate() {
        let sum =
            |f: fn(&Steps) -> u32| -> f64 { rows.iter().map(|r| f64::from(f(&r.1[arm]))).sum() };
        let (h, w, sl, lo) = (
            sum(|s| s.hopes),
            sum(|s| s.wasted),
            sum(|s| s.sleeps),
            sum(|s| s.lost),
        );
        let n = rows.len() as f64;
        println!(
            "   {name}: false hopes {:.2}, wasted {:.2}, W {:.2} steps per false hope; sleeps {:.1}, lost {:.1}, S {:.2} steps per sleep (means per life)",
            h / n,
            w / n,
            w / h.max(1.0),
            sl / n,
            lo / n,
            lo / sl.max(1.0)
        );
    }
    Ok(())
}

fn csv(rows: &[Row]) -> String {
    let mut out = String::from("seed,setting,hopes,wasted,sleeps,lost\n");
    for (seed, arms) in rows {
        for (name, s) in ["A", "S"].iter().zip(arms) {
            out.push_str(&format!(
                "{seed},{name},{},{},{},{}\n",
                s.hopes, s.wasted, s.sleeps, s.lost
            ));
        }
    }
    out
}

fn parse(args: &[String]) -> Result<(Vec<u64>, PathBuf), String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut out = PathBuf::from("runs/sweep");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--judge" {
            judge = true;
            continue;
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad {flag}: {value}");
        match flag.as_str() {
            "--seeds" => count = value.parse().map_err(|_| bad())?,
            "--from" => from = value.parse().map_err(|_| bad())?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if count == 0 {
        return Err("--seeds must be positive".into());
    }
    let seeds = if judge {
        (1..=100).chain(111..=210).collect()
    } else {
        (from..from + count).collect()
    };
    Ok((seeds, out))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seeds, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let run = || -> Result<(), Error> {
        let rows = measure(&seeds)?;
        report(&rows)?;
        fs::create_dir_all(&out)?;
        fs::write(out.join("hope_steps.csv"), csv(&rows))?;
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("hope_steps: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_steps_toward_the_hope_and_away_from_the_meal_are_wasted() {
        let grid = Grid::empty(5, 1);
        let at = |x: i32| Pos { x, y: 0 };
        // Walks 0 -> 1 -> 2 toward a hope at 2, seen at step 2, then eats at 0.
        let path = [at(0), at(1), at(2), at(1), at(0)];
        let woke = [false; 4];
        let hopes = [(2, at(2))];
        assert_eq!(wasted(&grid, &path, &woke, &hopes, &[(4, at(0))]), 2);
        // Passing the hope on the way to a meal beyond it wastes nothing.
        assert_eq!(
            wasted(&grid, &path[..3], &woke[..2], &hopes, &[(2, at(4))]),
            0
        );
    }

    #[test]
    fn excess_weighs_extra_waste_against_half_the_sleep_steps_saved() {
        let a = Steps {
            wasted: 2,
            lost: 40,
            ..Steps::default()
        };
        let s = Steps {
            wasted: 5,
            lost: 20,
            ..Steps::default()
        };
        assert_eq!(excess(&[a, s]), 3.0 - 0.5 * 10.0);
    }
}
