//! Costs for the false-hope mark of docs/sweep (Nano, 2026-10-30), measured
//! on today's default apprentice in the barren test world of the
//! barren-first life, tuning seeds 101-110.
//!
//! - W: steps wasted per false hope. For each false hope (an eaten square
//!   in view, empty, given more than even odds of food, in the first 1,000
//!   steps), the steps that closed on that square: the run of steps just
//!   before the sighting in which its distance to the square fell each
//!   step, and the steps after it while the distance still fell. Each step
//!   counts once per square. W = those steps / false hopes.
//! - S: steps lost per sleep. A sleep takes no steps here; it wakes on the
//!   start square, so S = the shortest walk (around walls) from the start
//!   square back to where it fell asleep, averaged over sleeps.
use std::collections::{HashSet, VecDeque};
use std::thread;

use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::life::{
    BARREN_STEPS, HOPE, Learner, UPBRINGING, WINDOW, WORLD_STEPS, barren, bushes, live,
};
use apprentice::memory::{Memory, Recall};
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::sim::{Event, Sim};
use apprentice::world::{Cell, Grid, Pos};

fn dist(a: Pos, b: Pos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Shortest walk from `from` to `to` around walls; `None` if cut off.
fn walk(grid: &Grid, from: Pos, to: Pos) -> Option<u32> {
    let mut seen = HashSet::from([from]);
    let mut queue = VecDeque::from([(from, 0)]);
    while let Some((p, d)) = queue.pop_front() {
        if p == to {
            return Some(d);
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let q = p.shift(dx, dy);
            let inside = q.x >= 0 && q.y >= 0 && q.x < grid.width() && q.y < grid.height();
            if inside && grid.get(q) != Cell::Wall && seen.insert(q) {
                queue.push_back((q, d + 1));
            }
        }
    }
    None
}

/// (false hopes, wasted steps, sleeps, steps lost to sleeps) in one life.
fn costs(seed: u64) -> (u32, u32, u32, u32) {
    let memory = Memory::new(Rates::default(), Recall::default());
    let mut mind: Learner = Apprentice::with_model(Drives::default(), memory, seed);
    for i in 0..UPBRINGING {
        let c = if i % 2 == 0 { barren() } else { bushes() };
        live(&mut mind, c, seed + 100 * i, WORLD_STEPS).expect("sim");
    }
    let mut sim = Sim::new(barren(), seed + 5_000).expect("sim");
    let mut eaten: HashSet<Pos> = HashSet::new();
    let mut path = Vec::new();
    let mut teleported = Vec::new();
    let mut hopes: Vec<(usize, Pos)> = Vec::new();
    let (mut sleeps, mut lost) = (0, 0);
    for t in 0..BARREN_STEPS {
        let here = sim.pos();
        path.push(here);
        if t < WINDOW {
            let start = sim.start();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let at = here.shift(dx, dy);
                    if !eaten.contains(&at) || sim.grid().get(at) == Cell::Food {
                        continue;
                    }
                    let b = mind.model().predict(at.shift(-start.x, -start.y));
                    if b[kind(Cell::Food)] > HOPE {
                        hopes.push((t, at));
                    }
                }
            }
        }
        let out = sim.step(mind.act(&sim.observe()));
        mind.feel(&out);
        teleported.push(out.slept);
        if out.slept {
            sleeps += 1;
            lost += walk(sim.grid(), sim.start(), here).unwrap_or(0);
        }
        if out.event == Event::Ate {
            eaten.insert(here);
        }
    }
    path.push(sim.pos());
    let mut wasted: HashSet<(usize, Pos)> = HashSet::new();
    for &(t, q) in &hopes {
        let mut k = t;
        while k > 0 && !teleported[k - 1] && dist(path[k - 1], q) > dist(path[k], q) {
            wasted.insert((k, q));
            k -= 1;
        }
        let mut k = t;
        while k + 1 < path.len() && !teleported[k] && dist(path[k + 1], q) < dist(path[k], q) {
            wasted.insert((k + 1, q));
            k += 1;
        }
    }
    (hopes.len() as u32, wasted.len() as u32, sleeps, lost)
}

fn main() {
    let seeds: Vec<u64> = (101..=110).collect();
    let all: Vec<(u32, u32, u32, u32)> = thread::scope(|s| {
        let hs: Vec<_> = seeds
            .chunks(3)
            .map(|p| s.spawn(move || p.iter().map(|&sd| costs(sd)).collect::<Vec<_>>()))
            .collect();
        hs.into_iter()
            .flat_map(|h| h.join().expect("join"))
            .collect()
    });
    let sum = |f: fn(&(u32, u32, u32, u32)) -> u32| all.iter().map(f).sum::<u32>();
    let (h, w, sl, lo) = (sum(|c| c.0), sum(|c| c.1), sum(|c| c.2), sum(|c| c.3));
    println!(
        "false hopes {h}, wasted steps {w}, W = {:.2}",
        f64::from(w) / f64::from(h.max(1))
    );
    println!(
        "sleeps {sl}, steps lost {lo}, S = {:.2}",
        f64::from(lo) / f64::from(sl.max(1))
    );
}
