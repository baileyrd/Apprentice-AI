//! What the default apprentice does in each world of the barren-first
//! sequence: a diagnosis, not a judgement.
use std::collections::HashSet;
use std::thread;

use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::body::Action;
use apprentice::life::{Learner, barren, bushes};
use apprentice::memory::{Memory, Recall};
use apprentice::model::{Rates, WorldModel};
use apprentice::sim::{Config, Event, Sim};
use apprentice::world::Cell;

/// Counts per 1,000 steps (first 1,000 steps of each world).
#[derive(Default, Clone, Copy)]
struct Look {
    sleeps: f64,
    meals: f64,
    rests: f64,
    eat_nothing: f64,
    bumps: f64,
    hurts: f64,
    squares: f64,
    food_in_view: f64,
    volatility: f64,
    maps: f64,
    prior_food: f64,
    expect: [f64; 3],
}

fn world(mind: &mut Learner, config: Config, seed: u64) -> Look {
    let mut sim = Sim::new(config, seed).expect("sim");
    let mut l = Look::default();
    let mut seen: HashSet<_> = HashSet::new();
    for t in 0..1_000 {
        if t == 500 {
            let far = apprentice::world::Pos {
                x: 10_000,
                y: 10_000,
            };
            l.prior_food = mind.model().explain(far).0[2];
        }
        let obs = sim.observe();
        l.food_in_view += f64::from(u8::from(obs.view.contains(&Cell::Food)));
        seen.insert(sim.pos());
        let action = mind.act(&obs);
        l.rests += f64::from(u8::from(action == Action::Rest));
        let out = sim.step(action);
        mind.feel(&out);
        l.sleeps += f64::from(u8::from(out.slept));
        match out.event {
            Event::Ate => l.meals += 1.0,
            Event::AteNothing => l.eat_nothing += 1.0,
            Event::Bumped => l.bumps += 1.0,
            Event::Hurt => l.hurts += 1.0,
            _ => {}
        }
    }
    l.squares = seen.len() as f64;
    for (k, age) in [50u64, 200, 800].into_iter().enumerate() {
        let saw_empty = apprentice::patterns::Last::Saw(Cell::Empty);
        l.expect[k] = mind.model().expects(saw_empty, age).map_or(-1.0, |b| b[2]);
    }
    l.volatility = mind.volatility();
    l.maps = mind.model().contexts() as f64;
    l
}

fn sequence(seed: u64) -> Vec<Look> {
    let memory = Memory::new(Rates::default(), recall());
    let mut mind: Learner = Apprentice::with_model(Drives::default(), memory, seed);
    (0..5u64)
        .map(|i| {
            let c = if i % 2 == 0 { barren() } else { bushes() };
            world(&mut mind, c, seed + 100 * i)
        })
        .collect()
}

fn recall() -> Recall {
    let off = std::env::args().any(|a| a == "--no-patterns");
    let unseen = std::env::args().any(|a| a == "--unseen");
    let net = apprentice::patterns::PatternNet {
        unseen,
        ..Default::default()
    };
    Recall {
        patterns: !off,
        pattern_net: Some(net),
        ..Recall::default()
    }
}

/// A newborn in the same five worlds, each world on its own.
fn newborns(seed: u64) -> Vec<Look> {
    (0..5u64)
        .map(|i| {
            let memory = Memory::new(Rates::default(), recall());
            let mut mind: Learner = Apprentice::with_model(Drives::default(), memory, seed);
            let c = if i % 2 == 0 { barren() } else { bushes() };
            world(&mut mind, c, seed + 100 * i)
        })
        .collect()
}

fn main() {
    let fresh = std::env::args().any(|a| a == "--newborn");
    let seeds: Vec<u64> = (101..=110).collect();
    let all: Vec<Vec<Look>> = thread::scope(|s| {
        let hs: Vec<_> = seeds
            .chunks(10)
            .map(|p| {
                s.spawn(move || {
                    p.iter()
                        .map(|&sd| if fresh { newborns(sd) } else { sequence(sd) })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter()
            .flat_map(|h| h.join().expect("join"))
            .collect()
    });
    let n = all.len() as f64;
    println!(
        "world kind sleeps meals rests eat_nothing bumps hurts squares food_in_view volatility maps prior_food(t500)"
    );
    for w in 0..5 {
        let m = |f: fn(&Look) -> f64| all.iter().map(|v| f(&v[w])).sum::<f64>() / n;
        println!(
            "{} {} {:.1} {:.1} {:.0} {:.0} {:.0} {:.1} {:.1} {:.0} {:.4} {:.1} {:.3} {:.3}/{:.3}/{:.3}",
            w + 1,
            if w % 2 == 0 { "barren" } else { "bush" },
            m(|l| l.sleeps),
            m(|l| l.meals),
            m(|l| l.rests),
            m(|l| l.eat_nothing),
            m(|l| l.bumps),
            m(|l| l.hurts),
            m(|l| l.squares),
            m(|l| l.food_in_view),
            m(|l| l.volatility),
            m(|l| l.maps),
            m(|l| l.prior_food),
            m(|l| l.expect[0]),
            m(|l| l.expect[1]),
            m(|l| l.expect[2])
        );
    }
}
