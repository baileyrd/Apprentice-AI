# Port: plugging other AI models into the world

Nano's ask (2026-11-14): test other AI models in the world sim. Separate the
agent's decisions from the simulation behind a strict boundary: the world is
a stream of events, the agent a black-box controller.

**Status: proposal.** Nothing here is built. Code starts only after Nano
approves, and then in three PRs (A, B, C below), each on its own. The
apprentice's judged results must stay byte-identical throughout (D30).

## Short answer

The physics is already decoupled. `Sim::step(Action) -> Outcome` is a pure
state machine with no I/O, and `Sim::observe()` builds the only view the agent
gets. The tight coupling is on the **agent and harness** side:

1. The port is split in two and incomplete. Words never cross it.
2. The step loop is hand-written about 24 times, and the order of speech
   around it is implicit.
3. Every test of the apprentice (home, false hopes, relearn) is typed to the
   concrete `Apprentice<Memory>` and reads its insides.

The fix is small: one trait with three events, one runner that owns the
order, an optional probe trait for tests that need to look inside, and an
out-of-process adapter so a Python or LLM agent can plug in without Rust.

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
/// Anything that can live in the world. The world talks to it only through
/// these three events, in the order the runner fixes.
pub trait Agent {
    /// Chooses the next action from what it senses.
    fn act(&mut self, obs: &Observation) -> Action;
    /// Hears what its last action did.
    fn feel(&mut self, _outcome: &Outcome) {}
    /// Hears `said` about square `at`, relative to its wake square.
    fn hear(&mut self, _at: Pos, _said: Said) {}
}

// src/apprentice.rs: control moves to Agent; Mind keeps only looking inside.
/// Optional window into an agent, for measures that need its beliefs.
pub trait Mind: Agent {
    fn surprise(&self) -> f64;
    fn believes(&self, at: Pos) -> Option<Cell>;
    fn map_in_use(&self) -> Option<usize> { None }
}
```

`Apprentice` and `Watched` implement `Agent` (moving their `act`, `feel` and
`hear` bodies unchanged) and `Mind`. `Mind` keeps its name, so call sites
barely move, and no option is removed (D1). Proof: a golden test records the
`StepRecord` stream of several lives on spent seeds before the change and
asserts it is identical after.

### PR B: one runner that owns the order

```rust
// src/episode.rs (new)
/// Something in the world that speaks: a mentor today.
pub trait Speaker {
    /// Before the agent senses this step.
    fn say(&mut self, sim: &Sim) -> Option<(Pos, Said)>;
    /// After the step's outcome.
    fn foretell(&mut self, sim: &Sim, outcome: &Outcome) -> Option<(Pos, Said)>;
}

/// One step, as the experimenter sees it (the agent never sees `pos`).
pub struct Step { pub pos: Pos, pub obs: Observation, pub action: Action, pub outcome: Outcome }

/// Lives `steps` in `sim`. Each step: say -> hear, observe -> act, step ->
/// feel, foretell -> hear. `watch` sees every step after the agent felt it.
pub fn run<A: Agent>(
    sim: &mut Sim,
    agent: &mut A,
    speakers: &mut [&mut dyn Speaker],
    steps: usize,
    watch: impl FnMut(&A, &Sim, &Step),
);
```

`Mentor` implements `Speaker` (its existing two methods). `Speaker` has
two real users from the start: honest and blind mentors, and the foretelling
mentors that take turns. Peers stay as they are; a peer speaks from another
mind, which is a multi-agent runner, not this one.

Migration, one library module per commit, each checked byte-identical on
spent seeds: `life`, `home`, `adaptation`, `relearn`, `lessons`,
`experiment`. Then `life` and `home` become generic over `A: Mind` instead of
`Apprentice<Memory>`. Bins move later, or not at all; nothing forces them.

Measures then split in two:
- **Behaviour** (sleeps, meals, futile eats) needs only `Step`. Every agent
  gets these. Marks reward meals and sleeps and only report false hopes (D23), so this is the core.
- **Probe** (false hopes, surprise, home answer, memory traffic) needs `Mind`.
  An agent without a probe gets "n/a", never "wrong".

### PR C: out-of-process agents (the plug for other models)

Most other models (RL policies, LLMs) live in Python or behind an API. A
child process speaking JSON lines over stdin and stdout is the one adapter
that covers all of them. It uses `serde_json`, which is already a
dependency: no new crates. PyO3 or a network server would add dependencies
for no gain at one agent per process.

```rust
// src/plug.rs (new)
/// An agent in another process. Implements Agent (and Mind, when it reports
/// a place).
pub struct Plugged { child: Child, stdin: ChildStdin, stdout: BufReader<ChildStdout>, place: Option<usize> }

impl Plugged {
    /// Launches `cmd`, passing the agent's seed as its last argument.
    pub fn launch(cmd: &[String], seed: u64) -> Result<Self, PlugError>;
}
```

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

Errors: a bad line, an unknown action or a dead process is a
`PlugError` with the line and step attached, returned up through `run`
(which becomes `Result` for plugged agents). Never a panic, never a silent
`Rest`.

**Home test for a black box (D42).** `place` is any small integer the agent
picks to name the world it thinks it is in. It goes from agent to world, so it
tells the agent nothing. It is scored exactly as `map_in_use` is: the id at
the end of the home life against the id at step 100 and step 300 of each
trial. An agent that never sends one has no home answer (n/a). Open as Q12
for Nano.

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

1. Approve the plan and its order (A, then B, then C)? A and B are
   refactors with byte-identical outputs; C adds a module and a bin.
2. Q12: is a self-reported `place` id the right home-test answer for a black
   box, or should foreign agents skip the home test in v1?
