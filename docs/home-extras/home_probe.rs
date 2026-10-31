//! Home-test extras probes (docs/home-extras), reported only.
//! Each arm is raised at home as set, then each trial runs in a variant:
//! base, regrowth-judged fit off ("nofit"), or path-counting flipped ("flip").
//! usage: home_probe FIRST LAST > probes.csv
use std::collections::HashSet;
use std::thread;

use apprentice::apprentice::{Drives, Mind};
use apprentice::home::{Place, at_home_as, step, world};
use apprentice::life::{ASKED, STRANGERS, Setting};
use apprentice::memory::Recall;
use apprentice::model::WorldModel;
use apprentice::patterns::PatternNet;

fn arm(unseen: bool, sweep: bool) -> Setting {
    let recall = Recall {
        pattern_net: Some(PatternNet { unseen, ..PatternNet::default() }),
        ..Recall::default()
    };
    Setting { recall, drives: Drives { sweep, ..Drives::default() } }
}

fn trials(name: &str, s: Setting, seed: u64) -> Vec<String> {
    let (raised, home, home_map) = at_home_as(s.recall, s.drives, seed).expect("home");
    let hm = home_map.unwrap_or(0);
    let mut out = Vec::new();
    for variant in ["base", "nofit", "flip"] {
        let places = [Place::Home, Place::Changed].into_iter().chain((0..STRANGERS).map(Place::Stranger));
        for place in places {
            let mut mind = raised.clone();
            match variant {
                "nofit" => mind.model_mut().set_regrowth_fit(false),
                "flip" => mind.drives_mut().sweep = !s.drives.sweep,
                _ => {}
            }
            mind.model_mut().reset_probe();
            let mut sim = world(place, &home, seed);
            let mut visited = HashSet::new();
            let mut first_no = 0;
            for t in 1..=ASKED {
                step(&mut sim, &mut mind);
                visited.insert(sim.pos());
                if mind.map_in_use() != home_map && first_no == 0 {
                    first_no = t;
                }
            }
            let m = mind.model();
            let p = m.probe(hm);
            let fit = m.fits().get(hm).copied().flatten().map_or(-1.0, |x| x);
            let yes = mind.map_in_use() == home_map;
            out.push(format!(
                "{name},{variant},{seed},{},{},{},{:.4},{},{:.4},{fit:.3},{},{first_no}",
                place.name(),
                u8::from(yes != place.is_home()),
                p.0,
                if p.0 > 0 { p.1 / p.0 as f64 } else { 0.0 },
                p.2,
                if p.2 > 0 { p.3 / p.2 as f64 } else { 0.0 },
                visited.len(),
            ));
        }
    }
    out
}

fn main() {
    let a: Vec<u64> = std::env::args().skip(1).map(|x| x.parse().expect("seed")).collect();
    let seeds: Vec<u64> = (a[0]..=a[1]).collect();
    let arms = [("R", arm(false, false)), ("RU", arm(true, false)), ("RS", arm(false, true)), ("RUS", arm(true, true))];
    let jobs: Vec<(usize, u64)> = seeds.iter().flat_map(|&s| (0..arms.len()).map(move |i| (i, s))).collect();
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let rows: Vec<String> = thread::scope(|sc| {
        let hs: Vec<_> = (0..workers)
            .map(|w| {
                let (arms, jobs) = (&arms, &jobs);
                sc.spawn(move || {
                    jobs.iter().skip(w).step_by(workers).flat_map(|&(i, s)| trials(arms[i].0, arms[i].1, s)).collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    });
    println!("arm,variant,seed,place,wrong,pat_n,pat_err,map_n,map_err,home_fit,visited,first_no");
    for r in rows {
        println!("{r}");
    }
}
