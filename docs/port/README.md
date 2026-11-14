# Port: plugging other AI models into the world

Nano's ask (2026-11-14): test other AI models in the world sim. Separate the
agent's decisions from the simulation behind a strict boundary: the world is
a stream of events, the agent a black-box controller.

**Status: approved by Nano 2026-11-14 (D43); built.** A is PR #50, B is
PR #51 (with generic `life::behave` and `life::home_trial` for any port), C
adds `plug.rs` and `bin/plug`. The apprentice's outputs stayed
byte-identical throughout (D30; `tests/golden.rs`).

**Try it:** `cargo run --release --bin plug -- python3 examples/plug/random_agent.py`
runs the stdlib example beside the apprentice on spent seeds 101-102.

## Short answer

The physics is already decoupled. `Sim::step(Action) -> Outcome` is a pure
state machine with no I/O, and `Sim::observe()` builds the only view the agent
gets. The tight coupling is on the **agent and harness** side:

1. The port is split in two and incomplete. Words never cross it.
2. The step loop is hand-written about 24 times, and the order of speech
   around it is implicit.
3. Every test of the apprentice (home, false hopes, relearn) is typed to the
   concrete `Apprentice<Memory>` and reads its insides.

The fix is small: one trait with three events, one fallible runner that owns
the order, measures whose type bounds say what they need (so no agent fakes
a probe), and an out-of-process adapter so a Python or LLM agent can plug in without Rust.

## What is coupled today

| # | Coupling | Where | Effect on a foreign model |
|---|---|---|---|
| 1 | Two ports for one job: `Agent` (act, feel) and `Mind` (act, feel, plus `surprise`, `believes`, `map_in_use`). Control and introspection are mixed in `Mind`. | src/agent.rs:11, src/apprentice.rs:35 | It must implement introspection to be run at all. |
| 2 | `hear(at, said)` is an inherent method on `Apprentice`, called on the concrete type. | src/apprentice.rs:231; bin/mentor.rs:88, bin/foretell.rs:161, bin/peers.rs:190 | **It can't hear words.** The language, trust and peers tests can't include it. |
| 3 | The step loop `sim.step(mind.act(&sim.observe())); mind.feel(&o)` is copied about 24 times. Speech happens at two points: `Mentor::say` before sensing, `Mentor::foretell` after feeling. Each harness encodes this by hand. | life.rs:93, home.rs:64, adaptation.rs:180, relearn.rs, lessons.rs, experiment.rs, 11 bins | Order is part of what was judged. It is easy to get wrong in a new harness. |
| 4 | Measures read the apprentice's insides: false hopes via `mind.model().predict`, the home answer via `map_in_use() == home_map`, memory traffic via `model().traffic()`. `life`/`home` take `Learner = Apprentice<Memory>`. | life.rs:20, 115-128, 214-219; home.rs:18, 106 | The home test and the life test can't run anything else. |
| 5 | Experiments reach in to change the mind: `model_mut`, `forget_values`, `forget_maps`. | relearn.rs, adaptation.rs, bins | A black box can't take these. Those experiments stay apprentice-only. |
| 6 | The home test rebuilds the mind for each trial by replaying the whole home life. | life.rs:214, home.rs:96-107 | A foreign agent must be deterministic given its seed and relaunched per trial. |
| 7 | `Checkpoint` holds only the closed `Baseline` enum. | lib.rs:55 | Foreign agents can't be checkpointed. Not needed for v1. |

Not coupling, and left alone:
- `Sim` holds both the world and the body (`pos`, `energy`, `start`). With one
  body per world that is fine; peers already use twin worlds. Splitting it
  would be speculative.
- `Sim::move_walls` and `move_food` are the experimenter changing the world
  mid-life. That is world-side and stays.
- The mentor reads only the `Sim` (mentor.rs:149, 172), never the mind. It is
  part of the world.
- The facing (five-box) world has its own learner API. Out of scope.

## What a foreign agent gets: the same as the apprentice, nothing more

Every channel the apprentice uses, and how it crosses the port:

| Channel | Today | Across the port |
|---|---|---|
| 3×3 view of cells | `Observation.view` | `sense` event |
| Energy | `Observation.energy`, `Outcome.energy` | `sense`, `felt` |
| Path integration (offset from the wake square) | `Observation.offset` | `sense` |
| What its action did (moved, bumped, hurt, ate, ate nothing, rested) | `Outcome.event` | `felt` |
| Sleep (energy hit zero, woke on its wake square) | `Outcome.slept` | `felt` |
| Words: a square (wake-relative) and a word in a voice | `Apprentice::hear`, off-port | `heard` event (new on the port) |
| Its own sleep replay, diary, maps, patterns | inside the apprentice | inside the agent; the world doesn't run it |

Never sent (D16, D42): which world it is in, a world or episode id, a
"new world" or reset message, the seed, grid size, step count `t`, and the
physiology constants. Moving to a new world looks to the agent exactly as it
does to the apprentice: the view changes.

Disclosed, not sent:
- **Designer priors.** The apprentice's `Drives` (setpoint 80 and so on) were
  chosen knowing `max_energy` is 100 (apprentice.rs:88-126). A foreign agent's
  author may tune with the same knowledge. Reports should say so.
- **Agent seed.** The apprentice's RNG takes the world seed (home.rs:82,
  life.rs:139). A foreign agent gets its own seed once at launch, derived from
  the world seed but not equal to it.

## Proposed API

### PR A: one port (no behaviour change)

```rust
// src/agent.rs
/// Anything that can live in the world, in process. The world talks to it
/// only through these events, in the order the runner fixes.
pub trait Agent {
    /// Chooses the next action from what it senses.
    fn act(&mut self, obs: &Observation) -> Action;
    /// Hears what its last action did.
    fn feel(&mut self, _outcome: &Outcome) {}
    /// Hears `said` about square `at`, relative to its wake square.
    fn hear(&mut self, _at: Pos, _said: Said) {}
    /// Its own name for the world it thinks it is in, if it keeps one.
    /// Read by the home test; never sent to it.
    fn place(&self) -> Option<usize> { None }
}

// src/apprentice.rs: control moves to Agent; Mind keeps only looking inside.
pub trait Mind: Agent {
    fn surprise(&self) -> f64;
    fn believes(&self, at: Pos) -> Option<Cell>;
    fn map_in_use(&self) -> Option<usize> { None }
}
```

`Apprentice` and `Watched` implement `Agent` (moving their `act`, `feel` and
`hear` bodies unchanged) and `Mind`. `Apprentice::place` returns
`map_in_use()`. `Mind` keeps its name and its methods, so no option is removed
(D1); retiring `map_in_use` in favour of `place` would be its own PR.

### PR B: one runner that owns the order and can fail

The runner talks to a **fallible** port. In-process agents get it for free
with an error type that cannot exist, so their callers never handle an
error that can't happen; a plugged agent's errors stop the life.

```rust
// src/episode.rs (new)
/// What the runner talks to. `Error` is `Infallible` for in-process agents.
pub trait Port {
    type Error: std::error::Error;
    fn act(&mut self, obs: &Observation) -> Result<Action, Self::Error>;
    fn feel(&mut self, outcome: &Outcome) -> Result<(), Self::Error>;
    fn hear(&mut self, at: Pos, said: Said) -> Result<(), Self::Error>;
    fn place(&self) -> Option<usize>;
}

impl<A: Agent> Port for A {
    type Error = Infallible;
    // each method wraps the Agent method in Ok
}

/// Something in the world that speaks: a mentor today.
pub trait Speaker {
    /// Before the agent senses this step.
    fn say(&mut self, sim: &Sim) -> Option<(Pos, Said)>;
    /// After the step's outcome.
    fn foretell(&mut self, sim: &Sim, outcome: &Outcome) -> Option<(Pos, Said)>;
}

/// One step, as the experimenter sees it (the agent never sees `pos`).
pub struct Step { pub pos: Pos, pub obs: Observation, pub action: Action, pub outcome: Outcome }

/// The experimenter's two looks per step. Both default to doing nothing.
pub trait Watch<P> {
    /// Before anything is said or sensed this step: the agent as it was
    /// after the last step. False hopes are read here.
    fn before(&mut self, _agent: &P, _sim: &Sim) {}
    /// After the agent felt the step and heard any foretelling.
    fn after(&mut self, _agent: &P, _sim: &Sim, _step: &Step) {}
}

/// Where in the step the agent failed, and when.
pub struct RunError<E> { pub t: u64, pub during: Phase, pub cause: E }
pub enum Phase { Hear, Act, Feel }

/// Lives `steps` in `sim`. Each step, in this order:
/// watch.before; say -> hear; observe -> act; sim.step; feel;
/// foretell -> hear; watch.after.
pub fn run<P: Port>(
    sim: &mut Sim,
    agent: &mut P,
    speakers: &mut [&mut dyn Speaker],
    steps: usize,
    watch: &mut impl Watch<P>,
) -> Result<(), RunError<P::Error>>;
```

Failure rules: the first error ends the life and is returned with its step
and phase. A failed `act` returns **before** `sim.step`, so the world does
not move and nothing is felt. A failed `feel` or `hear` returns at once; the
step already taken stays taken and `watch.after` is not called for it. In
process, `RunError<Infallible>` cannot be built, so library callers unwrap
it with `match e.cause {}` and keep their current `Result<_, SimError>`
signatures.

`Mentor` implements `Speaker` (its existing two methods). `Speaker` has two
real users from the start: honest and blind mentors, and the foretelling
mentors that take turns. Peers stay as they are; a peer speaks from another
mind, which is a multi-agent runner, not this one.

**Measures, and why no agent fakes a probe.** Measures live in watchers,
and a watcher's type bound says what it needs:
- **Behaviour** (sleeps, meals, futile eats) is `impl<P: Port> Watch<P>`. It
  reads only `Step` and `Sim`. Every agent gets these. Marks reward meals and
  sleeps and only report false hopes (D23), so this is the core.
- **Home answer** reads `Port::place()`, which every port has. An agent whose
  place is `None` at the end of its home life gets "n/a" for the test. The
  apprentice always has one (memory.rs:866), so its answers don't change.
- **Probes** (false hopes, memory traffic, surprise) are watchers typed to the
  learner, `impl Watch<Learner>`, reading `model().predict(..)[Food]` in
  `before` and `model().traffic()` exactly as life.rs does today. A foreign
  agent can't be given them (it won't compile), so its report shows "n/a".
  No probe trait for foreign agents is proposed: none has a second user yet.

Migration, one library module per commit: `life`, `home`, `adaptation`,
`relearn`, `lessons`, `experiment`. Each keeps its public `Learner`-typed
functions as thin wrappers (no signature change) over a core generic in
`P: Port`. Bins move later, or not at all; nothing forces them.

**Golden checks**, before the first commit and after each: on spent seeds,
the `StepRecord` stream of each harness **and** what it reports (`Stay`,
home answers, the judged bins' CSV and table output) are byte-identical.

### PR C: out-of-process agents (the plug for other models)

Most other models (RL policies, LLMs) live in Python or behind an API. A
child process speaking JSON lines over stdin and stdout is the one adapter
that covers all of them. It uses `serde_json`, which is already a
dependency: no new crates. PyO3 or a network server would add dependencies
for no gain at one agent per process.

```rust
// src/plug.rs (new)
/// An agent in another process. Implements `Port` (never `Agent`, whose
/// methods can't fail).
pub struct Plugged { child: Child, stdin: ChildStdin, stdout: BufReader<ChildStdout>, place: Option<usize> }

impl Plugged {
    /// Launches `cmd`, passing the agent's seed as its last argument.
    pub fn launch(cmd: &[String], seed: u64) -> Result<Self, PlugError>;
}

impl Port for Plugged {
    type Error = PlugError;
    // act: write `sense`, read one line, parse; keeps the last `place` sent.
    // feel/hear: write one line.
    // place: the last id the agent reported, or None if it never sent one.
}

/// What went wrong talking to the process; carries the offending line.
pub enum PlugError { Launch(io::Error), Write(io::Error), Read(io::Error), Closed, BadReply { line: String, why: String } }
```

`place` is decided per reply, not per type: a plugged agent always has
`place()`, and it is `None` until the agent first sends one.

Wire format, one JSON object per line:

```text
world -> agent   {"heard": {"at": {"x": 1, "y": -2}, "voice": 0, "word": 4}}
world -> agent   {"sense": {"view": ["Wall","Empty",...9], "energy": 47, "offset": {"x": 3, "y": 0}}}
agent -> world   {"act": "North"}            // or "East", "South", "West", "Eat", "Rest"
                 {"act": "Eat", "place": 2}  // place: optional, see below
world -> agent   {"felt": {"event": "Ate", "energy": 71, "slept": false}}
```

Only `sense` expects a reply. The process is launched per life (and per
home-test trial, see coupling 6) and closed when the life ends; there is no
end-of-world message.

Failure-path tests, pinned with the golden checks. Each uses a tiny scripted
child and asserts the `RunError` phase and step, that the world's `t` did not
advance on an `act` failure, and that nothing further was written:
- a reply that isn't JSON, or has no `act`, or names an unknown action;
- the child exits before replying to `sense`;
- the child closes stdin, so writing `felt` or `heard` fails;

One more test sits outside the run: a command that can't be launched makes
`Plugged::launch` return `PlugError::Launch` before any step.

**Home test for a black box (D42).** `place` is any small integer the agent
picks to name the world it thinks it is in. It goes from agent to world, so it
tells the agent nothing. It is scored exactly as `map_in_use` is: the id at
the end of the home life against the id at step 100 and step 300 of each
trial. An agent that never sends one has no home answer (n/a). Nano chose this
(Q12, D43).

A tiny `examples/plug/random_agent.py` (stdlib only) and a `bin/plug` that
runs a plugged agent through the life test and home test and prints the same
table as the apprentice will prove the loop end to end.

## Judging foreign models

Same standard as everything else (D12, D13, D14): marks pinned in writing
before any run, paired seed-by-seed against the apprentice with
`judge::Interval::change`, one judging run on a fresh block (next fresh:
901-1000, then 1401+). The home test limit (at most +0.25 wrong per home test,
upper 90%, raw) applies to any agent that reports a place.

Identity checks during the refactor rerun spent seeds (1-700, smoke on
101-102). No verdict is drawn, so they spend nothing.

A foreign model is a comparison subject, not the apprentice's learner, so
backprop inside it is fine (D2 allows it as a baseline). Swapping one in as
the apprentice's main learner would be Q11, Nano's call (D35).

## What stays the same

- Default settings, results, seeds and speech order: unchanged (D1, D30).
- `Sim`, `Grid`, `Observation`, `Action`, `Outcome`: unchanged types.
- No new dependencies.

## For Nano to decide

Both answered by Nano on 2026-11-14 ("Go", D43):

1. Approve the plan and its order (A, then B, then C)? **Approved.** A and B
   are refactors with byte-identical outputs; C adds a module and a bin. Each
   goes in its own PR.
2. Q12: is a self-reported `place` id the right home-test answer for a black
   box, or should foreign agents skip the home test in v1? **The optional
   `place` id**, scored as `map_in_use` is; an agent that never sends one is
   reported n/a (D42).
