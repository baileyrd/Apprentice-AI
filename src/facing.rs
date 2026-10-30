//! Facing: a five-box world seen one box at a time.
//!
//! The apprentice stands in the centre of five boxes laid out in a plus and
//! faces one of the four side boxes. It senses only the box it faces, as
//! raw noisy numbers with no label, and acts through bare motors whose
//! effects it must find out: turn left, turn right, do nothing, and in the
//! walking stage step forward. It may also have an innate heading ring
//! ([`World::ring`]). A [`Learner`] (the predictive-coding [`Net`], unchanged) predicts what it
//! will sense next. A [`Probe`] reads its heading off the hidden layer, for
//! scoring only. Rules are in `docs/facing/README.md`.
//!
//! Separate from the grid world: nothing here touches the main apprentice.

use crate::eprop::Eprop;
use crate::gru::{Gru, Sample, Training};
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
    /// Matching, plus a forward motor that steps between the centre and
    /// the side boxes; beyond the plus is dark.
    Walking,
}

impl Stage {
    /// The look of each side box, by look id.
    fn sides(self) -> [usize; SIDES] {
        match self {
            Stage::Turning => [0, 1, 2, 3],
            Stage::Matching | Stage::Walking => [0, 1, 0, 2],
        }
    }
}

/// Look id of the centre box, in the walking stage.
const CENTRE: usize = 3;
/// Look id of the dark beyond the plus, in the walking stage.
const DARK: usize = 4;

/// What a motor does. Hidden from the learner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Left,
    Right,
    Still,
    Forward,
}

/// The five boxes and the body in the centre.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    /// Each look's true pattern, by look id.
    looks: Vec<Sense>,
    sides: [usize; SIDES],
    heading: usize,
    /// The side box it stands in; `None` is the centre.
    at: Option<usize>,
    /// What motor `i` does, shuffled per seed.
    motors: Vec<Effect>,
    /// The ring unit active facing north, and whether the ring runs
    /// against the compass. Drawn per seed, so the ring is unlabelled.
    ring_phase: usize,
    ring_flipped: bool,
    rng: Rng,
}

impl World {
    /// A world for `stage`, its motors shuffled by `seed`.
    pub fn new(stage: Stage, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let sides = stage.sides();
        let walking = stage == Stage::Walking;
        // Patterned looks: the side boxes', and in walking the centre's.
        let kinds = match stage {
            Stage::Turning => 4,
            Stage::Matching => 3,
            Stage::Walking => CENTRE + 1,
        };
        let mut looks = distinct_patterns(kinds, &mut rng);
        let mut motors = vec![Effect::Left, Effect::Right, Effect::Still];
        if walking {
            looks.push([0.0; FEATURES]);
            motors.push(Effect::Forward);
        }
        shuffle(&mut motors, &mut rng);
        let heading = rng.below(SIDES);
        // Its own stream, so adding the ring changes nothing else.
        let mut ring_rng = Rng::new(seed ^ 0x0219);
        Self {
            looks,
            sides,
            heading,
            at: None,
            motors,
            ring_phase: ring_rng.below(SIDES),
            ring_flipped: ring_rng.chance(0.5),
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

    /// Look id of what it faces. For scoring only.
    pub fn faced(&self) -> usize {
        match self.at {
            None => self.sides[self.heading],
            Some(side) if self.heading == opposite(side) => CENTRE,
            Some(_) => DARK,
        }
    }

    /// The side box it stands in; `None` is the centre. For scoring only.
    pub fn at(&self) -> Option<usize> {
        self.at
    }

    /// The innate heading ring: one unit active, shifting one way on a
    /// left turn and the other on a right turn. Which unit means north,
    /// and which way round it runs, are drawn per seed.
    pub fn ring(&self) -> [f64; SIDES] {
        let turned = if self.ring_flipped {
            SIDES - self.heading
        } else {
            self.heading
        };
        let mut units = [0.0; SIDES];
        units[(self.ring_phase + turned) % SIDES] = 1.0;
        units
    }

    /// What it senses, and the ring after it if `ring`.
    pub fn percept(&mut self, ring: bool) -> Vec<f64> {
        let mut p = self.sense().to_vec();
        if ring {
            p.extend(self.ring());
        }
        p
    }

    /// Which side box it faces, 0 = north. For scoring only.
    pub fn heading(&self) -> usize {
        self.heading
    }

    /// Whether the box it faces looks like another box. For scoring only.
    pub fn faces_matching(&self) -> bool {
        let faced = self.faced();
        self.sides.iter().filter(|&&l| l == faced).count() > 1
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
        match self.motors.get(motor) {
            Some(Effect::Left) => self.heading = (self.heading + SIDES - 1) % SIDES,
            Some(Effect::Right) => self.heading = (self.heading + 1) % SIDES,
            Some(Effect::Forward) => self.step_forward(),
            Some(Effect::Still) | None => {}
        }
    }

    /// From the centre, into the faced box; from a side box, back to the
    /// centre if facing it; into the dark, nowhere.
    fn step_forward(&mut self) {
        self.at = match self.at {
            None => Some(self.heading),
            Some(side) if self.heading == opposite(side) => None,
            stay => stay,
        };
    }
}

fn opposite(side: usize) -> usize {
    (side + 2) % SIDES
}

/// The body's driver: stares a random while, then fires a random other
/// motor (a turn, or in walking, a step forward).
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
    /// Context units, with a separate set of hidden weights per motor:
    /// the motor picks which set applies.
    Gated,
}

/// Predicts the next sensing from this one, the motor, and its memory.
#[derive(Debug, Clone, PartialEq)]
pub struct Learner {
    net: Net,
    memory: Memory,
    /// Numbers in one percept: the senses, then the ring if it has one.
    percept: usize,
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
    /// A learner for a body with `motors` motors and percepts of
    /// `percept` numbers. It predicts the next whole percept.
    pub fn new(
        memory: Memory,
        percept: usize,
        motors: usize,
        rate: f64,
        seed: u64,
    ) -> Result<Self, NetError> {
        let recall = match memory {
            Memory::None => 0,
            Memory::Recent(k) => k * (percept + motors),
            Memory::Context | Memory::Gated => HIDDEN,
        };
        // One block of [sense, recall] per motor when gated, else one.
        let blocks = if memory == Memory::Gated { motors } else { 1 };
        // Memory widens the input, and a delta rule's step grows with the
        // active input's size; scale the rate so every memory learns at the
        // same pace per step.
        let now = percept + motors;
        let net = Net::new(
            &[blocks * (percept + recall) + motors, HIDDEN, percept],
            rate * now as f64 / (now + recall) as f64,
            LEARNING,
            seed,
        )?;
        Ok(Self {
            net,
            memory,
            percept,
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

    /// [sense, motor, recall]; gated, [block per motor, motor], with only
    /// the fired motor's block holding [sense, recall].
    fn input(&self, recall: &[f64], sense: &[f64], motor: usize) -> Vec<f64> {
        if self.memory != Memory::Gated {
            let mut x = sense.to_vec();
            x.extend(one_hot(motor, self.motors));
            x.extend_from_slice(recall);
            return x;
        }
        let p = self.percept;
        let block = p + recall.len();
        let mut x = vec![0.0; self.motors * block];
        if let Some(active) = x.get_mut(motor * block..(motor + 1) * block) {
            active[..p].copy_from_slice(&sense[..p]);
            active[p..].copy_from_slice(recall);
        }
        x.extend(one_hot(motor, self.motors));
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
            Memory::Context | Memory::Gated => self.net.hidden(input),
        }
    }
}

/// A linear readout of one of `classes` facts (heading, or where it
/// stands) from hidden activity, learned by a delta rule. Scoring only: the
/// learner never sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    /// One row per class: a weight per hidden unit, then bias.
    rows: Vec<Vec<f64>>,
    rate: f64,
}

impl Probe {
    /// A probe for `classes` classes, reading `width` hidden units.
    pub fn new(classes: usize, width: usize, rate: f64) -> Self {
        Self {
            rows: vec![vec![0.0; width + 1]; classes],
            rate,
        }
    }

    /// The class it reads off `hidden`.
    pub fn read(&self, hidden: &[f64]) -> usize {
        let scores = self.scores(hidden);
        (0..scores.len())
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]))
            .unwrap_or(0)
    }

    /// Learns that `hidden` came with `class`.
    pub fn learn(&mut self, hidden: &[f64], class: usize) {
        let scores = self.scores(hidden);
        for (c, row) in self.rows.iter_mut().enumerate() {
            let e = if c == class { 1.0 } else { 0.0 } - scores[c];
            let width = row.len() - 1;
            for (w, a) in row.iter_mut().zip(hidden) {
                *w += self.rate * e * a;
            }
            row[width] += self.rate * e;
        }
    }

    fn scores(&self, hidden: &[f64]) -> Vec<f64> {
        self.rows
            .iter()
            .map(|row| {
                let (weights, bias) = row.split_at(row.len() - 1);
                bias[0] + weights.iter().zip(hidden).map(|(w, a)| w * a).sum::<f64>()
            })
            .collect()
    }
}

/// Where it stands as a probe class: a side box, or [`SIDES`] for the
/// centre.
fn place(at: Option<usize>) -> usize {
    at.unwrap_or(SIDES)
}

/// Which way the centre lies from where it faces, in turns to the right,
/// as a probe class; [`SIDES`] when it stands in the centre. Where it
/// stands, seen from its own heading.
fn centre_direction(world: &World) -> usize {
    world.at().map_or(SIDES, |side| {
        (opposite(side) + SIDES - world.heading()) % SIDES
    })
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
    /// Heading read off the hidden layer by a probe frozen for the second
    /// half of the life, on the last `window` turns.
    pub probe: Option<f64>,
    /// The same, on the last `window` turns that start facing a matching
    /// box, where the view can't tell heading.
    pub probe_matching: Option<f64>,
    /// Share of the more common heading among those same turns: what
    /// always guessing it would score.
    pub majority_matching: Option<f64>,
    /// Where it stands (centre or which side box), read off the hidden
    /// layer by a frozen probe, on the last `window` scored steps.
    pub probe_place: Option<f64>,
    /// Which way the centre lies from where it faces (ahead, right,
    /// behind, left, or it is in the centre), by a frozen probe likewise.
    pub probe_centre: Option<f64>,
}

/// A stare this long or longer counts as long.
pub const LONG_STARE: u32 = 5;

/// One life: settings shared by every seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Life {
    pub stage: Stage,
    pub memory: Memory,
    /// Whether it has the innate heading ring.
    pub ring: bool,
    pub steps: usize,
    pub rate: f64,
    /// Turns at the end over which it is scored.
    pub window: usize,
}

/// Lives `life` on `seed` and scores it.
pub fn live(life: &Life, seed: u64) -> Result<Score, NetError> {
    let world = World::new(life.stage, seed);
    let percept = FEATURES + if life.ring { SIDES } else { 0 };
    let learner = Learner::new(
        life.memory,
        percept,
        world.motors(),
        life.rate,
        seed ^ 0x5eed,
    )?;
    Ok(score(world, learner, life, seed))
}

/// Anything that learns online to predict the next percept, with a hidden
/// layer to probe.
trait Predictor {
    /// Hidden units the probes read.
    fn width(&self) -> usize;
    /// Its hidden activity on this step, and its predictions one and two
    /// steps on (the second given motor `after`). Changes nothing.
    fn foresee(&self, percept: &[f64], motor: usize, after: usize) -> [Vec<f64>; 3];
    /// Learns that `percept` and `motor` led to `next`, and moves on.
    fn learn(&mut self, percept: &[f64], motor: usize, next: &[f64]);
}

impl Predictor for Learner {
    fn width(&self) -> usize {
        HIDDEN
    }

    fn foresee(&self, percept: &[f64], motor: usize, after: usize) -> [Vec<f64>; 3] {
        let [one, two] = self.imagine(percept, motor, after);
        [self.hidden(percept, motor), one, two]
    }

    fn learn(&mut self, percept: &[f64], motor: usize, next: &[f64]) {
        Learner::learn(self, percept, motor, next);
    }
}

/// An e-prop network on the facing body: input is the percept and the
/// motor's one-hot.
#[derive(Debug, Clone, PartialEq)]
pub struct EpropLearner {
    net: Eprop,
    motors: usize,
    /// Whether the motor picks the recurrent weights.
    gated: bool,
}

impl EpropLearner {
    pub fn new(net: Eprop, motors: usize, gated: bool) -> Self {
        Self { net, motors, gated }
    }

    fn gate(&self, motor: usize) -> usize {
        if self.gated { motor } else { 0 }
    }

    fn input(&self, percept: &[f64], motor: usize) -> Vec<f64> {
        let mut x = percept.to_vec();
        x.extend(one_hot(motor, self.motors));
        x
    }
}

impl Predictor for EpropLearner {
    fn width(&self) -> usize {
        self.net.units()
    }

    fn foresee(&self, percept: &[f64], motor: usize, after: usize) -> [Vec<f64>; 3] {
        let gate = self.gate(motor);
        let (h1, one) = self
            .net
            .peek(self.net.state(), &self.input(percept, motor), gate);
        let two = self
            .net
            .peek(&h1, &self.input(&one, after), self.gate(after))
            .1;
        [h1, one, two]
    }

    fn learn(&mut self, percept: &[f64], motor: usize, next: &[f64]) {
        let gate = self.gate(motor);
        self.net.advance(&self.input(percept, motor), gate, next);
    }
}

/// E-prop's settings (see `docs/facing/README.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpropLife {
    pub stage: Stage,
    pub steps: usize,
    pub units: usize,
    pub leaks: (f64, f64),
    pub rate: f64,
    pub window: usize,
    /// One set of recurrent weights per motor, picked by the motor fired.
    pub gated: bool,
}

/// Lives `life` on `seed` with an e-prop network (always with the ring)
/// and scores it like [`live`].
pub fn live_eprop(life: &EpropLife, seed: u64) -> Score {
    let world = World::new(life.stage, seed);
    let percept = FEATURES + SIDES;
    let gates = if life.gated { world.motors() } else { 1 };
    let net = Eprop::new(
        percept + world.motors(),
        life.units,
        percept,
        gates,
        life.leaks,
        life.rate,
        seed ^ 0x5eed,
    );
    let learner = EpropLearner::new(net, world.motors(), life.gated);
    let as_life = Life {
        stage: life.stage,
        memory: Memory::None,
        ring: true,
        steps: life.steps,
        rate: life.rate,
        window: life.window,
    };
    score(world, learner, &as_life, seed)
}

/// Lives with random motors, `learner` predicting and learning online, and
/// scores it. Only `life`'s stage, ring, steps and window are read.
fn score<P: Predictor>(mut world: World, mut learner: P, life: &Life, seed: u64) -> Score {
    let width = learner.width();
    let mut driver = Driver::new(&world, seed ^ 0xd21e);
    let mut probe = Probe::new(SIDES, width, 0.02);
    let mut place_probe = Probe::new(SIDES + 1, width, 0.02);
    let mut centre_probe = Probe::new(SIDES + 1, width, 0.02);
    let (mut places, mut centres) = (vec![], vec![]);
    let (mut ones, mut twos, mut probes, mut long) = (vec![], vec![], vec![], vec![]);
    let (mut probes_matching, mut headings_matching) = (vec![], vec![]);
    let mut sense = world.percept(life.ring);
    let mut now = driver.drive();
    let mut two_ahead: Option<usize> = None;
    for t in 0..life.steps {
        let after = driver.drive();
        let (motor, stared) = now;
        let [hidden, one, two] = learner.foresee(&sense, motor, after.0);
        let heading = world.heading();
        let matching = world.faces_matching();
        let stands = place(world.at());
        let centre = centre_direction(&world);
        world.fire(motor);
        let next = world.percept(life.ring);
        let truth = world.faced();
        let looks = world.looks();
        if let Some(stared) = stared {
            let hit = nearest(looks, &one) == truth;
            ones.push(hit);
            let read = probe.read(&hidden) == heading;
            probes.push(read);
            places.push(place_probe.read(&hidden) == stands);
            centres.push(centre_probe.read(&hidden) == centre);
            if matching {
                probes_matching.push(read);
                headings_matching.push(heading);
            }
            if stared >= LONG_STARE && t >= life.steps / 2 {
                long.push(hit);
            }
            if let Some(guess) = two_ahead {
                twos.push(guess == truth);
            }
        }
        two_ahead = after.1.map(|_| nearest(looks, &two));
        // Frozen for the second half: a probe still learning would carry
        // heading through a stare in its own weights.
        if t < life.steps / 2 {
            probe.learn(&hidden, heading);
            place_probe.learn(&hidden, stands);
            centre_probe.learn(&hidden, centre);
        }
        learner.learn(&sense, motor, &next);
        sense = next;
        now = after;
    }
    Score {
        one: tail_rate(&ones, life.window),
        two: tail_rate(&twos, life.window),
        after_stare: tail_rate(&long, long.len()),
        probe: tail_rate(&probes, life.window),
        probe_matching: tail_rate(&probes_matching, life.window),
        majority_matching: majority(&headings_matching, life.window),
        probe_place: tail_rate(&places, life.window),
        probe_centre: tail_rate(&centres, life.window),
    }
}

/// Share of the most common value among the last `window` entries.
fn majority(values: &[usize], window: usize) -> Option<f64> {
    let tail = &values[values.len().saturating_sub(window)..];
    let most = (0..SIDES)
        .map(|v| tail.iter().filter(|&&x| x == v).count())
        .max()?;
    (!tail.is_empty()).then(|| most as f64 / tail.len() as f64)
}

/// The GRU ceiling check's settings (see `docs/facing/README.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ceiling {
    pub stage: Stage,
    /// Steps of the recorded life it trains on.
    pub steps: usize,
    /// Steps after training, frozen, that are scored.
    pub eval: usize,
    pub hidden: usize,
    pub training: Training,
    /// Scored steps at the end over which accuracy is scored.
    pub window: usize,
}

/// Records a life with the ring, trains a GRU on it offline by BPTT, then
/// scores it frozen on the steps that follow. Only `one`, `two`,
/// `after_stare` and `probe_place` are filled.
pub fn ceiling(c: &Ceiling, seed: u64) -> Score {
    let mut world = World::new(c.stage, seed);
    let n = world.motors();
    let mut driver = Driver::new(&world, seed ^ 0xd21e);
    let input = |p: &[f64], m: usize| -> Vec<f64> {
        let mut x = p.to_vec();
        x.extend(one_hot(m, n));
        x
    };
    let mut life = Vec::with_capacity(c.steps);
    let mut places = Vec::with_capacity(c.steps);
    let mut centres = Vec::with_capacity(c.steps);
    let mut percept = world.percept(true);
    for _ in 0..c.steps {
        let (motor, _) = driver.drive();
        places.push(place(world.at()));
        centres.push(centre_direction(&world));
        world.fire(motor);
        let next = world.percept(true);
        life.push(Sample {
            input: input(&percept, motor),
            target: next.clone(),
        });
        percept = next;
    }
    let mut gru = Gru::new(percept.len() + n, c.hidden, percept.len(), seed ^ 0x5eed);
    gru.train(&life, &c.training);

    let mut place_probe = Probe::new(SIDES + 1, c.hidden, 0.02);
    let mut centre_probe = Probe::new(SIDES + 1, c.hidden, 0.02);
    let mut h = gru.rest();
    for ((sample, &at), &centre) in life.iter().zip(&places).zip(&centres) {
        h = gru.step(&h, &sample.input).0;
        place_probe.learn(&h, at);
        centre_probe.learn(&h, centre);
    }

    let (mut ones, mut twos, mut long, mut placed) = (vec![], vec![], vec![], vec![]);
    let mut centred = vec![];
    let mut now = driver.drive();
    let mut two_ahead: Option<usize> = None;
    for _ in 0..c.eval {
        let after = driver.drive();
        let (motor, stared) = now;
        let (h1, one) = gru.step(&h, &input(&percept, motor));
        let two = gru.step(&h1, &input(&one, after.0)).1;
        let stands = place(world.at());
        let centre = centre_direction(&world);
        world.fire(motor);
        let next = world.percept(true);
        let truth = world.faced();
        let looks = world.looks();
        if let Some(stared) = stared {
            let hit = nearest(looks, &one) == truth;
            ones.push(hit);
            placed.push(place_probe.read(&h1) == stands);
            centred.push(centre_probe.read(&h1) == centre);
            if stared >= LONG_STARE {
                long.push(hit);
            }
            if let Some(guess) = two_ahead {
                twos.push(guess == truth);
            }
        }
        two_ahead = after.1.map(|_| nearest(looks, &two));
        h = h1;
        percept = next;
        now = after;
    }
    Score {
        one: tail_rate(&ones, c.window),
        two: tail_rate(&twos, c.window),
        after_stare: tail_rate(&long, long.len()),
        probe: None,
        probe_matching: None,
        majority_matching: None,
        probe_place: tail_rate(&placed, c.window),
        probe_centre: tail_rate(&centred, c.window),
    }
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

/// Two looks must differ in at least this many numbers.
pub const MIN_DIFFERENCE: usize = 3;

/// `n` random patterns of ±1, each differing from every other in at least
/// [`MIN_DIFFERENCE`] numbers, so different boxes really look different.
fn distinct_patterns(n: usize, rng: &mut Rng) -> Vec<Sense> {
    let mut looks: Vec<Sense> = Vec::with_capacity(n);
    while looks.len() < n {
        let p = pattern(rng);
        if looks.iter().all(|l| differing(l, &p) >= MIN_DIFFERENCE) {
            looks.push(p);
        }
    }
    looks
}

fn differing(a: &Sense, b: &Sense) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
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
    fn looks_differ_in_every_seed() {
        for seed in 0..200 {
            let w = World::new(Stage::Turning, seed);
            let l = w.looks();
            for i in 0..l.len() {
                for j in i + 1..l.len() {
                    assert!(differing(&l[i], &l[j]) >= MIN_DIFFERENCE, "seed {seed}");
                }
            }
        }
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
        let mut l = Learner::new(Memory::Recent(2), FEATURES, 3, 0.05, 0).expect("shape");
        let (a, b) = ([0.5; FEATURES], [-0.5; FEATURES]);
        l.learn(&a, 0, &b);
        l.learn(&b, 2, &a);
        let block = FEATURES + 3;
        assert_eq!(&l.recall[..FEATURES], &b);
        assert_eq!(&l.recall[FEATURES..block], &[0.0, 0.0, 1.0]);
        assert_eq!(&l.recall[block..block + FEATURES], &a);
    }

    #[test]
    fn gated_input_fills_only_the_fired_motors_block() {
        let l = Learner::new(Memory::Gated, FEATURES, 3, 0.05, 0).expect("shape");
        let x = l.input(&[0.25; HIDDEN], &[1.0; FEATURES], 1);
        let block = FEATURES + HIDDEN;
        assert_eq!(x.len(), 3 * block + 3);
        assert!(x[..block].iter().all(|&v| v == 0.0));
        assert_eq!(&x[block..block + FEATURES], &[1.0; FEATURES]);
        assert_eq!(&x[block + FEATURES..2 * block], &[0.25; HIDDEN]);
        assert!(x[2 * block..3 * block].iter().all(|&v| v == 0.0));
        assert_eq!(&x[3 * block..], &[0.0, 1.0, 0.0]);
    }

    #[test]
    fn only_north_and_south_match() {
        let mut w = World::new(Stage::Matching, 8);
        let right = motor(&w, Effect::Right);
        let mut matching = [false; SIDES];
        for _ in 0..SIDES {
            matching[w.heading()] = w.faces_matching();
            w.fire(right);
        }
        assert_eq!(matching, [true, false, true, false]);
        assert!(!World::new(Stage::Turning, 8).faces_matching());
    }

    #[test]
    fn ring_shifts_with_turns_only_and_comes_back() {
        let mut w = World::new(Stage::Walking, 9);
        let start = w.ring();
        assert_eq!(start.iter().sum::<f64>(), 1.0);
        w.fire(motor(&w, Effect::Right));
        let turned = w.ring();
        assert_ne!(turned, start);
        w.fire(motor(&w, Effect::Forward));
        w.fire(w.still_motor());
        assert_eq!(w.ring(), turned);
        for _ in 0..3 {
            w.fire(motor(&w, Effect::Right));
        }
        assert_eq!(w.ring(), start);
    }

    #[test]
    fn walking_steps_into_a_box_and_back() {
        let mut w = World::new(Stage::Walking, 10);
        let (forward, right) = (motor(&w, Effect::Forward), motor(&w, Effect::Right));
        let side = w.heading();
        let ahead = w.faced();
        w.fire(forward);
        assert_eq!(w.at(), Some(side));
        assert_eq!(w.faced(), DARK);
        w.fire(forward);
        assert_eq!(w.at(), Some(side), "the dark doesn't let it through");
        w.fire(right);
        w.fire(right);
        assert_eq!(w.faced(), CENTRE);
        w.fire(forward);
        assert_eq!(w.at(), None);
        assert_eq!(w.faced(), w.sides[opposite(side)]);
        assert!(w.sides.contains(&ahead));
    }

    #[test]
    fn dark_and_centre_decode_apart_from_the_side_boxes() {
        let w = World::new(Stage::Walking, 11);
        assert_eq!(w.looks().len(), 5);
        assert_eq!(nearest(w.looks(), &[0.0; FEATURES]), DARK);
        assert_eq!(nearest(w.looks(), &w.looks()[CENTRE]), CENTRE);
    }

    #[test]
    fn majority_reads_the_window() {
        assert_eq!(majority(&[0, 2, 2, 0, 0], 4), Some(0.5));
        assert_eq!(majority(&[], 4), None);
    }

    #[test]
    fn probe_learns_a_separable_heading() {
        let mut p = Probe::new(SIDES, HIDDEN, 0.05);
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
            ring: false,
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
