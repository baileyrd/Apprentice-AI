//! Golden outputs: what the library's lives report, pinned on spent seeds.
//!
//! Refactors of the agent port and the step loop (docs/port) must leave
//! every byte of these unchanged. A hash is of the `Debug` text, which prints
//! floats exactly, so any change in behaviour shows.

use apprentice::adaptation::{self, Change};
use apprentice::agent::Agent;
use apprentice::apprentice::{Apprentice, Drives};
use apprentice::episode::{After, Step, run_agent};
use apprentice::experiment;
use apprentice::home;
use apprentice::lessons::{self, Course};
use apprentice::life::{self, Learner};
use apprentice::memory::{Memory, Recall};
use apprentice::mentor::{Foretell, Mentor, Teaching};
use apprentice::model::{PredictiveCoding, Rates};
use apprentice::relearn;
use apprentice::sim::{Config, Sim};

/// Spent seed (smoke range); judging seeds are untouched.
const SEED: u64 = 101;

/// FNV-1a, 64 bit: stable across Rust releases, unlike `DefaultHasher`.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn learner() -> Learner {
    Apprentice::with_model(
        Drives::default(),
        Memory::new(Rates::default(), Recall::default()),
        SEED,
    )
}

fn check(name: &str, output: impl std::fmt::Debug, want: u64) {
    let got = fnv(&format!("{output:?}"));
    assert_eq!(got, want, "{name}: output changed (hash {got:#018x})");
}

/// Minutes in a debug build, so CI skips it; run it with
/// `cargo test --release --test golden -- --include-ignored`.
#[test]
#[ignore = "slow in debug builds"]
fn whole_life_with_home_test() {
    check(
        "life",
        life::life(Recall::default(), SEED),
        0x86d8_d0dd_51d0_c62f,
    );
}

fn adaptation(change: Change, want: u64) {
    let mut setup = adaptation::Setup::for_change(change);
    setup.seed = SEED;
    (setup.warmup, setup.after) = (500, 500);
    check("adaptation", adaptation::run(&setup, &mut learner()), want);
}

#[test]
fn adaptation_walls_move() {
    adaptation(Change::Walls, 0x6a89_3a7d_7e16_cf4b);
}

#[test]
fn adaptation_food_moves() {
    adaptation(Change::Food, 0x1eaf_b411_f696_57bd);
}

#[test]
fn relearn_a_b_a() {
    let setup = relearn::Setup {
        config: life::bushes(),
        seed: SEED,
        phase: 200,
    };
    check(
        "relearn",
        relearn::run(&setup, &mut learner()),
        0x6d3f_c027_f2ce_3e8e,
    );
}

#[test]
fn lessons_course() {
    let course = Course {
        lessons: vec![home::config(), Config::default()],
        mastery: 300,
        cap: 2_000,
    };
    check(
        "lessons",
        lessons::study(&mut learner(), &course, SEED),
        0xbbb0_ca6b_b792_7e88,
    );
}

/// A mentor that names, hints and foretells: say before sensing, foretell
/// after feeling, as the mentor and foretell experiments do, through the
/// one runner.
#[test]
fn mentor_speaks_and_is_heard() {
    let mut sim = Sim::new(life::bushes(), SEED).expect("room");
    let mut mind = learner();
    let teaching = Teaching {
        names: true,
        foretells: Foretell::Truly,
        ..Teaching::default()
    };
    let mut mentor = Mentor::new(teaching, SEED);
    let mut log = Vec::new();
    let mut watch = After(|_: &Learner, sim: &Sim, step: &Step| {
        log.push((sim.pos(), step.outcome));
    });
    run_agent(&mut sim, &mut mind, &mut [&mut mentor], 600, &mut watch);
    check("mentor", (log, mind.place()), 0x2c6d_7b7f_5c9e_adf4);
}

/// A fixed reflex agent while a world model watches, episode by episode.
#[test]
fn experiment_episodes() {
    let setup = experiment::Setup {
        seed: SEED,
        episodes: 20,
        snapshot_at: vec![1, 20],
        ..experiment::Setup::default()
    };
    let model = PredictiveCoding::new(Rates::default());
    check(
        "experiment",
        experiment::run(&setup, model),
        0x1742_d43f_53dd_ad1e,
    );
}
