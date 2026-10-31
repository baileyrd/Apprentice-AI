//! Home-test extras diagnosis (docs/home-extras): R, RU, RS, RUS per trial.
//! usage: home_why FIRST LAST > trials.csv
use std::thread;

use apprentice::apprentice::{Drives, Mind};
use apprentice::home::{Place, at_home_as, step, world};
use apprentice::life::{ASKED, STRANGERS, Setting, TRIAL_STEPS};
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
    let places = [Place::Home, Place::Changed].into_iter().chain((0..STRANGERS).map(Place::Stranger));
    let mut out = Vec::new();
    let (raised, home, home_map) = at_home_as(s.recall, s.drives, seed).expect("home");
    for place in places {
        let mut mind = raised.clone();
        let maps_home = mind.model().contexts();
        let mut sim = world(place, &home, seed);
        let mut first_no = 0;
        let mut doubt_peak = 0;
        let mut row = String::new();
        for t in 1..=TRIAL_STEPS {
            step(&mut sim, &mut mind);
            let yes = mind.map_in_use() == home_map;
            if !yes && first_no == 0 {
                first_no = t;
            }
            doubt_peak = doubt_peak.max(mind.model().doubt().0);
            if t == ASKED {
                let m = mind.model();
                let fits = m.fits();
                let f = |i: Option<usize>| {
                    i.and_then(|i| fits.get(i).copied().flatten()).map_or(-1.0, |x| x)
                };
                row = format!(
                    "{name},{seed},{},{},{},{maps_home},{},{},{:.3},{:.3},{}",
                    place.name(),
                    u8::from(yes != place.is_home()),
                    home_map.unwrap_or(99),
                    m.contexts(),
                    m.doubt().0,
                    f(home_map),
                    f(mind.map_in_use()),
                    m.held(),
                );
            }
        }
        out.push(format!("{row},{first_no},{doubt_peak}"));
    }
    out
}

fn main() {
    let a: Vec<u64> = std::env::args().skip(1).map(|x| x.parse().expect("seed")).collect();
    let seeds: Vec<u64> = (a[0]..=a[1]).collect();
    let arms = [("R", arm(false, false)), ("RU", arm(true, false)), ("RS", arm(false, true)), ("RUS", arm(true, true))];
    let jobs: Vec<(usize, u64)> = seeds.iter().flat_map(|&s| (0..arms.len()).map(move |i| (i, s))).collect();
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = jobs.len().div_ceil(workers);
    let rows: Vec<String> = thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                let arms = &arms;
                sc.spawn(move || part.iter().flat_map(|&(i, s)| trials(arms[i].0, arms[i].1, s)).collect::<Vec<_>>())
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    });
    println!("arm,seed,place,wrong,home_map,maps_home,maps_100,doubt_100,home_fit_100,active_fit_100,held_100,first_no,doubt_peak");
    for r in rows {
        println!("{r}");
    }
}
