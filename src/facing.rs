//! Facing: a five-box world seen one box at a time.
//!
//! The apprentice stands in the centre of five boxes laid out in a plus and
//! faces one of the four side boxes. It senses only the box it faces, as
//! raw noisy numbers with no label, and acts through bare motors whose
//! effects it must find out: turn left, turn right, or do nothing. A
//! [`Learner`] (the predictive-coding [`Net`], unchanged) predicts what it
//! will sense next. A [`Probe`] reads its heading off the hidden layer, for
//! scoring only. Rules are in `docs/facing/README.md`.
//!
//! Separate from the grid world: nothing here touches the main apprentice.

use crate::network::{Learning, Net, NetError};
use crate::rng::Rng;

/// Numbers in one sensing of a box.
pub const FEATURES: usize = 8;
/// Standard deviation of the noise on every sensed number.
pub const NOISE: f64 = 0.3;
/// Hidden units in the learner's network.
pub const HIDDEN: usize = 16;
/// Side boxes, in turning order: north, east, south, west.
pub const SIDES: usize = 4;
/// Chance a stare ends on any step: stares last 0, 1, 2… steps, mean 3.
pub const STARE_END: f64 = 0.25;

/// One raw sensing.
pub type Sense = [f64; FEATURES];

/// Which world it lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Four side boxes, four different looks.
    Turning,
    /// The north and south boxes look the same.
    Matching,
}

impl Stage {
    /// The look of each side box, by look id.
    fn sides(self) -> [usize; SIDES] {
        match self {
            Stage::Turning => [0, 1, 2, 3],
            Stage::Matching => [0, 1, 0, 2],
        }
    }
}

/// What a motor does. Hidden from the learner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Left,
    Right,
    Still,
}

/// The five boxes and the body in the centre.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    /// Each look's true pattern, by look id.
    looks: Vec<Sense>,
    sides: [usize; SIDES],
    heading: usize,
    /// What motor `i` does, shuffled per seed.
    motors: Vec<Effect>,
    rng: Rng,
}

impl World {
    /// A world for `stage`, its motors shuffled by `seed`.
    pub fn new(stage: Stage, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let sides = stage.sides();
        let kinds = sides.iter().max().map_or(0, |m| m + 1);
        let looks = (0..kinds).map(|_| pattern(&mut rng)).collect();
        let mut motors = vec![Effect::Left, Effect::Right, Effect::Still];
        shuffle(&mut motors, &mut rng);
        let heading = rng.below(SIDES);
        Self {
            looks,
            sides,
            heading,
            motors,
            rng,
        }
    }

    /// Number of motors.
    pub fn motors(&self) -> usize {
        self.motors.len()
    }

    /// The motor that does nothing. For the driver, not the learner.
    pub fn still_motor(&self) -> usize {
        self.motors
            .iter()
            .position(|&e| e == Effect::Still)
            .unwrap_or(0)
    }

    /// Every look's true pattern, by look id. For scoring only.
    pub fn looks(&self) -> &[Sense] {
        &self.looks
    }

    /// Look id of the box it faces. For scoring only.
    pub fn faced(&self) -> usize {
        self.sides[self.heading]
    }

    /// Which side box it faces, 0 = north. For scoring only.
    pub fn heading(&self) -> usize {
        self.heading
    }

    /// Senses the box it faces: its look plus fresh noise.
    pub fn sense(&mut self) -> Sense {
        let mut s = self.looks[self.faced()];
        for x in &mut s {
            *x += NOISE * gaussian(&mut self.rng);
        }
        s
    }

    /// Fires `motor`. A motor it doesn't have is wired to nothing.
    pub fn fire(&mut self, motor: usize) {
        self.heading = match self.motors.get(motor) {
            Some(Effect::Left) => (self.heading + SIDES - 1) % SIDES,
            Some(Effect::Right) => (self.heading + 1) % SIDES,
            Some(Effect::Still) | None => self.heading,
        };
    }
}

/// The body's driver: stares a random while, then turns a random way.
/// Stare lengths are geometric (0, 1, 2…, mean 3), so some last past 10.
#[derive(Debug, Clone, PartialEq)]
pub struct Driver {
    rng: Rng,
    still: usize,
    turns: Vec<usize>,
    /// Stills left in this stare.
    left: u32,
    /// Stills fired since the last turn.
    stared: u32,
}

/// A motor the driver fires, and for a turn, the stills just before it.
pub type Drive = (usize, Option<u32>);

impl Driver {
    /// A driver for `world`'s motors.
    pub fn new(world: &World, seed: u64) -> Self {
        let still = world.still_motor();
        let turns = (0..world.motors()).filter(|&m| m != still).collect();
        let mut d = Self {
            rng: Rng::new(seed),
            still,
            turns,
            left: 0,
            stared: 0,
        };
        d.left = d.stare_length();
        d
    }

    /// The next motor to fire; for a turn, also how long it stared first.
    pub fn drive(&mut self) -> Drive {
        if self.left > 0 {
            self.left -= 1;
            self.stared += 1;
            return (self.still, None);
        }
        self.left = self.stare_length();
        let stared = std::mem::take(&mut self.stared);
        (
            self.rng.pick(&self.turns).unwrap_or(self.still),
            Some(stared),
        )
    }

    fn stare_length(&mut self) -> u32 {
        let mut n = 0;
        while !self.rng.chance(STARE_END) {
            n += 1;
        }
        n
    }
}

/// What the learner carries from one step to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memory {
    /// Nothing: it predicts from now and the motor only.
    None,
    /// What it sensed and fired on each of the last `k` steps. A control.
    Recent(usize),
    /// Its own hidden layer from the step before (context units).
    Context,
}

/// Predicts the next sensing from this one, the motor, and its memory.
#[derive(Debug, Clone, PartialEq)]
pub struct Learner {
    net: Net,
    memory: Memory,
    motors: usize,
    /// What it carries into the next step.
    recall: Vec<f64>,
}

const LEARNING: Learning = Learning::PredictiveCoding {
    rounds: 20,
    step: 0.2,
    precision: 100.0,
};

impl Learner {
    /// A learner for a body with `motors` motors.
    pub fn new(memory: Memory, motors: usize, rate: f64, seed: u64) -> Result<Self, NetError> {
        let recall = match memory {
            Memory::None => 0,
            Memory::Recent(k) => k * (FEATURES + motors),
            Memory::Context => HIDDEN,
        };
        // Memory widens the input, and a delta rule's step grows with the
        // input's size; scale the rate so every memory learns at the same
        // pace per step.
        let now = FEATURES + motors;
        let net = Net::new(
            &[now + recall, HIDDEN, FEATURES],
            rate * now as f64 / (now + recall) as f64,
            LEARNING,
            seed,
        )?;
        Ok(Self {
            net,
            memory,
            motors,
            recall: vec![0.0; recall],
        })
    }

    /// Its hidden layer's activity on this step. For the probe.
    pub fn hidden(&self, sense: &[f64], motor: usize) -> Vec<f64> {
        self.net.hidden(&self.input(&self.recall, sense, motor))
    }

    /// Its predictions one and two steps on, given both motors, feeding
    /// its own first prediction and memory back in.
    pub fn imagine(&self, sense: &[f64], first: usize, second: usize) -> [Vec<f64>; 2] {
        let x = self.input(&self.recall, sense, first);
        let one = self.net.predict(&x);
        let recall = self.next_recall(&x, sense, first);
        let two = self.net.predict(&self.input(&recall, &one, second));
        [one, two]
    }

    /// Learns that `sense` and `motor` led to `next`, then steps its memory.
    pub fn learn(&mut self, sense: &[f64], motor: usize, next: &[f64]) {
        let x = self.input(&self.recall, sense, motor);
        let recall = self.next_recall(&x, sense, motor);
        self.net.learn(&x, next);
        self.recall = recall;
    }

    fn input(&self, recall: &[f64], sense: &[f64], motor: usize) -> Vec<f64> {
        let mut x = sense.to_vec();
        x.extend(one_hot(motor, self.motors));
        x.extend_from_slice(recall);
        x
    }

    /// What it would carry on after this step.
    fn next_recall(&self, input: &[f64], sense: &[f64], motor: usize) -> Vec<f64> {
        match self.memory {
            Memory::None => Vec::new(),
            Memory::Recent(_) => {
                let mut r = sense.to_vec();
                r.extend(one_hot(motor, self.motors));
                let keep = self.recall.len() - r.len();
                r.extend_from_slice(&self.recall[..keep]);
                r
            }
            Memory::Context => self.net.hidden(input),
        }
    }
}

/// A linear readout of heading from hidden activity, learned by a delta
/// rule. Scoring only: the learner never sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    /// One row per heading: weights, then bias.
    rows: [[f64; HIDDEN + 1]; SIDES],
    rate: f64,
}

impl Probe {
    pub fn new(rate: f64) -> Self {
        Self {
            rows: [[0.0; HIDDEN + 1]; SIDES],
            rate,
        }
    }

    /// The heading it reads off `hidden`.
    pub fn read(&self, hidden: &[f64]) -> usize {
        let scores = self.scores(hidden);
        (0..SIDES)
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]))
            .unwrap_or(0)
    }

    /// Learns that `hidden` came with `heading`.
    pub fn learn(&mut self, hidden: &[f64], heading: usize) {
        let scores = self.scores(hidden);
        for (h, row) in self.rows.iter_mut().enumerate() {
            let e = if h == heading { 1.0 } else { 0.0 } - scores[h];
            for (w, a) in row.iter_mut().zip(hidden) {
                *w += self.rate * e * a;
            }
            row[HIDDEN] += self.rate * e;
        }
    }

    fn scores(&self, hidden: &[f64]) -> [f64; SIDES] {
        self.rows
            .map(|row| row[HIDDEN] + row.iter().zip(hidden).map(|(w, a)| w * a).sum::<f64>())
    }
}

/// Accuracies at the end of a life. Each is `None` when nothing counted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// Next view after a turn, over the last `window` turns.
    pub one: Option<f64>,
    /// View two steps on, where that step is a turn, over the last
    /// `window` such.
    pub two: Option<f64>,
    /// Next view after turns that end a stare of [`LONG_STARE`] or more,
    /// over the second half of the life.
    pub after_stare: Option<f64>,
    /// Heading read off the hidden layer, on the last `window` turns.
    pub probe: Option<f64>,
}

/// A stare this long or longer counts as long.
pub const LONG_STARE: u32 = 5;

/// One life: settings shared by every seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Life {
    pub stage: Stage,
    pub memory: Memory,
    pub steps: usize,
    pub rate: f64,
    /// Turns at the end over which it is scored.
    pub window: usize,
}

/// Lives `life` on `seed` and scores it.
pub fn live(life: &Life, seed: u64) -> Result<Score, NetError> {
    let mut world = World::new(life.stage, seed);
    let mut learner = Learner::new(life.memory, world.motors(), life.rate, seed ^ 0x5eed)?;
    let mut driver = Driver::new(&world, seed ^ 0xd21e);
    let mut probe = Probe::new(0.02);
    let (mut ones, mut twos, mut probes, mut long) = (vec![], vec![], vec![], vec![]);
    let mut sense = world.sense();
    let mut now = driver.drive();
    let mut two_ahead: Option<usize> = None;
    for t in 0..life.steps {
        let after = driver.drive();
        let (motor, stared) = now;
        let [one, two] = learner.imagine(&sense, motor, after.0);
        let hidden = learner.hidden(&sense, motor);
        let heading = world.heading();
        world.fire(motor);
        let next = world.sense();
        let truth = world.faced();
        let looks = world.looks();
        if let Some(stared) = stared {
            let hit = nearest(looks, &one) == truth;
            ones.push(hit);
            probes.push(probe.read(&hidden) == heading);
            if stared >= LONG_STARE && t >= life.steps / 2 {
                long.push(hit);
            }
            if let Some(guess) = two_ahead {
                twos.push(guess == truth);
            }
        }
        two_ahead = after.1.map(|_| nearest(looks, &two));
        probe.learn(&hidden, heading);
        learner.learn(&sense, motor, &next);
        sense = next;
        now = after;
    }
    Ok(Score {
        one: tail_rate(&ones, life.window),
        two: tail_rate(&twos, life.window),
        after_stare: tail_rate(&long, long.len()),
        probe: tail_rate(&probes, life.window),
    })
}

/// Look id whose pattern is closest to `v`.
pub fn nearest(looks: &[Sense], v: &[f64]) -> usize {
    let distance = |l: &Sense| -> f64 { l.iter().zip(v).map(|(a, b)| (a - b).powi(2)).sum() };
    (0..looks.len())
        .min_by(|&a, &b| distance(&looks[a]).total_cmp(&distance(&looks[b])))
        .unwrap_or(0)
}

/// Share of `true` among the last `window` entries.
fn tail_rate(hits: &[bool], window: usize) -> Option<f64> {
    let tail = &hits[hits.len().saturating_sub(window)..];
    if tail.is_empty() {
        return None;
    }
    Some(tail.iter().filter(|&&h| h).count() as f64 / tail.len() as f64)
}

fn one_hot(i: usize, n: usize) -> Vec<f64> {
    (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()
}

/// A random pattern of ±1.
fn pattern(rng: &mut Rng) -> Sense {
    std::array::from_fn(|_| if rng.chance(0.5) { 1.0 } else { -1.0 })
}

/// Standard normal, by Box–Muller.
fn gaussian(rng: &mut Rng) -> f64 {
    let u = 1.0 - rng.unit();
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * rng.unit()).cos()
}

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motor(world: &World, effect: Effect) -> usize {
        world.motors.iter().position(|&e| e == effect).unwrap_or(0)
    }

    #[test]
    fn four_turns_one_way_come_back_past_four_boxes() {
        let mut w = World::new(Stage::Turning, 3);
        let right = motor(&w, Effect::Right);
        let start = w.faced();
        let mut seen: Vec<usize> = (0..4)
            .map(|_| {
                w.fire(right);
                w.faced()
            })
            .collect();
        assert_eq!(seen[3], start);
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn left_undoes_right_and_still_or_stray_motors_do_nothing() {
        let mut w = World::new(Stage::Turning, 4);
        let start = w.heading();
        w.fire(motor(&w, Effect::Right));
        w.fire(motor(&w, Effect::Left));
        w.fire(w.still_motor());
        w.fire(99);
        assert_eq!(w.heading(), start);
    }

    #[test]
    fn matching_boxes_are_opposite_and_look_the_same() {
        let w = World::new(Stage::Matching, 5);
        assert_eq!(w.sides[0], w.sides[2]);
        assert_ne!(w.sides[1], w.sides[3]);
        assert_eq!(w.looks().len(), 3);
    }

    #[test]
    fn noisy_senses_decode_to_the_faced_look() {
        let mut w = World::new(Stage::Turning, 6);
        let hits = (0..200)
            .filter(|_| {
                let s = w.sense();
                nearest(w.looks(), &s) == w.faced()
            })
            .count();
        assert!(hits >= 195, "{hits}/200");
    }

    #[test]
    fn stares_average_three_with_a_long_tail() {
        let w = World::new(Stage::Turning, 7);
        let mut d = Driver::new(&w, 7);
        let mut stares = vec![];
        for _ in 0..40_000 {
            if let (_, Some(stared)) = d.drive() {
                stares.push(stared);
            }
        }
        let mean = stares.iter().sum::<u32>() as f64 / stares.len() as f64;
        assert!((mean - 3.0).abs() < 0.2, "mean {mean}");
        assert!(stares.iter().any(|&s| s > 10));
    }

    #[test]
    fn recent_memory_keeps_the_last_k_steps_newest_first() {
        let mut l = Learner::new(Memory::Recent(2), 3, 0.05, 0).expect("shape");
        let (a, b) = ([0.5; FEATURES], [-0.5; FEATURES]);
        l.learn(&a, 0, &b);
        l.learn(&b, 2, &a);
        let block = FEATURES + 3;
        assert_eq!(&l.recall[..FEATURES], &b);
        assert_eq!(&l.recall[FEATURES..block], &[0.0, 0.0, 1.0]);
        assert_eq!(&l.recall[block..block + FEATURES], &a);
    }

    #[test]
    fn probe_learns_a_separable_heading() {
        let mut p = Probe::new(0.05);
        let code = |h: usize| -> Vec<f64> { (0..HIDDEN).map(|i| f64::from(i % 4 == h)).collect() };
        for t in 0..400 {
            p.learn(&code(t % 4), t % 4);
        }
        assert!((0..4).all(|h| p.read(&code(h)) == h));
    }

    #[test]
    fn learns_to_turn() {
        let life = Life {
            stage: Stage::Turning,
            memory: Memory::None,
            steps: 8_000,
            rate: 0.05,
            window: 100,
        };
        let score = live(&life, 101).expect("shape");
        assert!(score.one.unwrap_or(0.0) >= 0.95, "{score:?}");
    }

    #[test]
    fn tail_rate_reads_only_the_window() {
        assert_eq!(tail_rate(&[false, true, true], 2), Some(1.0));
        assert_eq!(tail_rate(&[], 5), None);
    }
}
