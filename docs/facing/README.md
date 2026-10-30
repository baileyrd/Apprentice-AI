# Facing: a five-box world seen one box at a time

Nano's question: today's apprentice gets a labelled top-down 3×3 view and
labelled moves. Can a learner that sees only what it faces, as raw noisy
features, through motors that mean nothing until tried, learn which way it
is facing?

## The world

Five boxes in a plus: the apprentice stands in the centre box, with one box
on each side. It faces one of the four side boxes.

- **Senses.** Only the box it faces, as 8 numbers: that box's look (a fixed
  random pattern of ±1, drawn per seed) plus noise of standard deviation
  0.3 on every number, fresh each step. No label, no kind.
- **Motors.** Three bare outputs. What each one does is shuffled per seed:
  turn left, turn right, do nothing.
- **Life.** The body stares (fires do-nothing) for a random while, then
  turns a random way. Stare lengths are geometric, 0, 1, 2… steps, mean 3,
  median 2, about 4% longer than 10. The learner watches and predicts.

Stages, in order (walking into boxes and walls come in later PRs):

1. **Turning.** Four boxes, four different looks, fixed order. Solvable by a
   lookup of (what it sees, which motor), so it proves motor learning only.
2. **Matching.** The north and south boxes look the same. Facing either,
   what comes after a turn depends on which one it faces: the real test of
   heading. After a long stare, only a memory that carries heading can know.

## The learner

The predictive-coding network from `src/network.rs`, unchanged: one hidden
layer of 16 units, local updates only. Input is what it senses now plus the
motor it fires (one unit per motor). Output is its prediction of what it
will sense next. The network keeps no state between steps, so three
memories are compared:

- **None.** Input is now and the motor only.
- **Recent 3.** Adds what it sensed and fired on each of the last 3 steps
  (at least the median stare). A control: a longer lookup key, not a
  heading; it can't see past a stare longer than 2.
- **Context.** Adds a copy of its own hidden layer from the step before
  (context units). Learned locally, as plain input; no backprop through
  time.

The learning rate is scaled by (now's inputs) / (all inputs), so a wider
input from memory doesn't make the delta rule's step bigger.

A **probe** reads heading (north, east, south, west) off the hidden layer
by a linear delta rule, trained alongside, for scoring only. With context
units the hidden layer is also what is carried forward, so the probe reads
both.

## How it is scored

A prediction is decoded to the nearest look. It is right if that is the
look of the box it then faces. Stills are trivial (the view doesn't
change), so only turns are scored.

- **One-ahead:** predicting the view after a turn.
- **Two-ahead:** predicting the view two steps on, where that step is a
  turn, given both motors, feeding its own first prediction (and memory)
  back in. After two turns the same way, that is the box behind it.
- **After long stares:** one-ahead on turns that end a stare of 5 or more,
  over the second half of the life.
- **Probe:** heading read off the hidden layer, on the same turns.

## Rules fixed before any run

Settings may be tuned on seeds 101–110; the check is one run on seeds 1–20.
Accuracy is over the last 100 turns of the budget.

| Stage    | Budget        | One-ahead | Two-ahead | After stares ≥ 5 | Seeds passing |
|----------|---------------|-----------|-----------|------------------|---------------|
| Turning  | 8,000 steps   | ≥ 95%     | ≥ 90%     | —                | ≥ 19 of 20    |
| Matching | 20,000 steps  | ≥ 95%     | ≥ 90%     | ≥ 90%            | ≥ 19 of 20    |

- **Turning:** both no memory and context units are expected to pass.
- **Matching needs memory.** With no memory, median one-ahead must stay
  at or below 80% (expected about 75%: half the turns start facing a
  matching box, and from there a turn is a coin flip). Above that, the run
  stops to find the leak.
- The probe is reported beside accuracy, not judged.

## Changes before judging, disclosed

The first rules (commit 6c5c03f) had turning only and budgets of 2,000 and
5,000 steps. Nano then approved the do-nothing motor and stares; since
about 3 in 4 steps are now stares, budgets were raised. On tuning seeds:

- Turning at 2,000 steps: no memory 0.88, context 0.73 one-ahead. At 8,000
  steps all three memories reach about 1.00. Rate 0.05 (0.1 is unstable).
- Matching: no memory 0.76 at every budget (no leak). Recent 3 and context
  also stay at 0.76–0.78 (recent 1, the simplest, too, after 400,000 steps
  with 64 hidden units), and
  0.75 after long stares. Plain backprop (same network) and a numpy
  backprop check do no better. A lookup table on (last view, last motor,
  view, motor) is exact, so the history holds the answer.

Why it is hard, stated before judging: left and right mirror each other,
so the box after a turn from a matching box flips with each of three
facts: which box it came from, which motor brought it there, and which
motor it fires now. Using history means learning a three-way parity, which
small online learners learn very slowly. A heading would cut that to a
two-way combination (heading and motor), like turning; but nothing trains
context units to carry heading through a stare, because learning stays
local (no backprop through time).

## A world fix after the first judging run, disclosed

The first judging run (seeds 1–20) missed turning at 17/20 for no memory
and context. Seeds 2 and 11 had two boxes with identical looks (random ±1
patterns can collide), so their "four distinct boxes" were three; seed 10
had two looks one number apart. That is a flaw in the world, not the
learner. Looks are now redrawn until every pair differs in at least 3 of 8
numbers, and both stages were judged again. First-run results are kept in
the results section.

## Results (seeds 1–20)

Medians over seeds; accuracy over the last 100 turns.

**Turning (8,000 steps): met for every memory.**

| Memory   | One-ahead | Two-ahead | After stares ≥ 5 | Probe | Seeds passing |
|----------|-----------|-----------|------------------|-------|---------------|
| None     | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |
| Recent 3 | 1.00      | 0.99      | 0.97             | 1.00  | 20/20         |
| Context  | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |

(First run, before the look fix: 17/20, 16/20, 17/20; the misses were the
seeds with colliding looks.)

**Matching (20,000 steps): missed for every memory.**

| Memory   | One-ahead | Two-ahead | After stares ≥ 5 | Probe | Seeds passing |
|----------|-----------|-----------|------------------|-------|---------------|
| None     | 0.73      | 0.73      | 0.75             | 0.80  | 0/20          |
| Recent 3 | 0.72      | 0.73      | 0.75             | 0.85  | 0/20          |
| Context  | 0.73      | 0.73      | 0.75             | 0.83  | 0/20          |

- **No leak.** The no-memory control sits at 0.73, under the expected 75% ceiling.
- **Memory doesn't help yet.** Neither the recent-3 control nor context
  units beat no memory. The probe reads a little more heading from them
  (0.83–0.85 against 0.80), so some trace of the past reaches the hidden
  layer, but the prediction doesn't use it.
- **Cause, as far as checked:** the three-way parity above. The control
  rules out "context units can't hold it" as the whole story: recent 3
  holds the answer verbatim for stares of 2 or less and still doesn't
  learn to use it within the budget.

Run: `cargo run --release --bin facing -- --stage turning|matching --judge`.

## Second try: motor-gated hidden weights (rules fixed before any run)

Nano's fix: a separate set of hidden weights per motor, with the motor
choosing which set applies; context units kept.

- **Gated.** The input is laid out in one block per motor, each holding
  [what it senses now, context units]. Only the fired motor's block is
  filled; the others are zero. So each motor has its own first-layer
  weights into the same 16 hidden units, and the output weights are
  shared. The motor's one-hot is kept too, for a per-motor bias. Same
  `Net`, same local learning, same rate scaling (by the active block).
  Why it might work: the motor no longer has to be combined with the rest
  by the hidden layer; it picks the weights, which takes one of the three
  parity facts out of the learning.
- **Same marks** as the matching row above: one-ahead ≥ 95%, two-ahead
  ≥ 90%, after stares ≥ 5 ≥ 90%, 20,000 steps, ≥ 19 of 20 seeds.
- **Fresh seeds.** Matching is judged once on seeds 41–60, all four
  memories. Turning is re-run on seeds 21–40 with the 3-of-8 look rule, all
  four memories. Tuning stays on 101–110.
- **Probe on matching-box steps.** Also reported: probe accuracy on turns
  that start facing a matching box (north or south in the matching
  stage), where heading can't be read off the view. Chance there is about
  50%.
- **Stop rule (Nano's).** If gating misses matching, stop: merge the world
  and the controls, and write up "memory isn't the bottleneck,
  computation is."

## Second-try results

**Turning, fresh seeds 21–40 (3-of-8 look rule): met for every memory.**

| Memory   | One-ahead | Two-ahead | After stares ≥ 5 | Probe | Seeds passing |
|----------|-----------|-----------|------------------|-------|---------------|
| None     | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |
| Recent 3 | 1.00      | 1.00      | 0.97             | 1.00  | 20/20         |
| Context  | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |
| Gated    | 1.00      | 1.00      | 1.00             | 0.82  | 20/20         |

**Matching, fresh seeds 41–60: missed for every memory, gating included.**

| Memory   | One-ahead | Two-ahead | After stares ≥ 5 | Probe | Probe, matching box | Seeds passing |
|----------|-----------|-----------|------------------|-------|---------------------|---------------|
| None     | 0.74      | 0.72      | 0.75             | 0.79  | 0.64                | 0/20          |
| Recent 3 | 0.75      | 0.75      | 0.75             | 0.86  | 0.70                | 0/20          |
| Context  | 0.74      | 0.73      | 0.75             | 0.85  | 0.67                | 0/20          |
| Gated    | 0.75      | 0.75      | 0.75             | 0.66  | 0.49                | 0/20          |

On tuning seeds gating reached 0.78 at most (rates 0.02–0.1, up to
100,000 steps), so the miss is not a budget the judging run cut short.

The probe on matching-box turns sits above 50% even with no memory
(0.64). The likely reason, not checked: over the last 100 such turns the
body tends to spend longer near one of the two, so always guessing that
one beats a coin. Read it against the no-memory row, not against 50%.
Against that row, recent 3 and context carry a little heading (+0.03 to
+0.06); gating carries less (its hidden code changes with each motor, which
a single linear probe reads poorly).

## Write-up: memory isn't the bottleneck, computation is

Per Nano's stop rule, gating missed, so this line of work stops here; the
world and the controls are merged as they are.

What the five-box world showed:

1. **Motor learning is easy.** With raw noisy senses and unlabelled,
   shuffled motors, every learner (no memory included) learns what each
   motor does and predicts the next view at 100% within 8,000 steps, on
   40 judged seeds.
2. **Heading is not.** When two opposite boxes look alike, no learner
   beats the no-memory ceiling of about 75%, on 40 judged seeds and up to
   400,000 tuning steps.
3. **The memory was there.** The recent-3 control holds the deciding
   history verbatim for every stare of 2 or less, and a lookup table on
   (last view, last motor, view, motor) is exact. Context units carry a
   trace too: the probe reads slightly more heading from them than from no
   memory. Yet prediction stays at chance on matching boxes.
4. **What is missing is the computation.** The next view after a matching
   box flips with three facts at once (where it came from, the motor that
   brought it, the motor it fires now): a three-way parity. Neither local
   predictive coding nor backprop (in the same network, and in a separate
   numpy check) learns it from this data within the budgets. Gating, which
   takes the current motor out of what the hidden layer has to combine,
   didn't change that either, because the other two facts still have to
   be combined across steps, and nothing teaches the hidden layer to keep
   heading (no backprop through time).

What would test this next, not built: a learner that keeps heading as its
own state, updated by the motor (path integration on a ring), so that
prediction needs only heading × motor, which every learner here already
solves. That would be giving the apprentice a heading sense, a design
choice for Nano, like the offset it already gets in the grid world.

## Third try: an innate heading ring (rules fixed before any run)

Nano's choice (2026-10-30): give the apprentice heading as an innate sense,
not something it must learn to keep.

- **The ring.** Four input units, one active. The active unit shifts by
  one each time the body really turns, one way for left and the other for
  right; it doesn't move on stills (or, in walking, on steps forward).
  It is unlabelled: per seed, the unit that is active facing north is
  random, and so is which way round the ring runs. The motor→turn mapping
  is still shuffled per seed, so which motor moves the ring which way is
  still learned. Like a vestibular sense, it reports the turn that
  happened, not the motor fired.
- **Learners.** Every memory from before (none, recent 3, context, gated)
  now also gets the ring as input. The probe still reads heading off the
  hidden layer.
- **Matching, again:** same marks (20,000 steps; one-ahead ≥ 95%,
  two-ahead ≥ 90%, after stares ≥ 5 ≥ 90%; ≥ 19 of 20 seeds), judged once
  on fresh seeds 61–80. Expected: all four pass, no memory included,
  because with the ring the next view is a two-way combination (ring and
  motor), like turning. The 80% no-memory ceiling no longer applies: the
  ring gives heading by design.

### Stage 3: walking (rules fixed before any run)

- **World.** The matching world (north and south alike), plus a fourth
  motor, *forward*, shuffled in with the others. The centre box gets its
  own look, and beyond the plus is dark (all zeros, plus the usual noise).
  - From the centre, forward steps into the box it faces.
  - From a side box, it faces the centre box when facing back the way it
    came, and dark the other three ways. Forward toward the centre steps
    back into it; forward into the dark does nothing.
  - Turning and the ring work as before; forward doesn't turn it.
- **Life.** Stares as before, then a random one of left, right, forward.
  Every step that isn't a stare is scored.
- **Learners.** All four memories, each with the ring.
- **Marks.** One-ahead ≥ 95%, two-ahead ≥ 90%, after stares ≥ 5 ≥ 90%
  (all over the last 100 scored steps, the long-stare one over the second
  half), ≥ 19 of 20 seeds, 40,000 steps. Tuning on 101–110; judged once on
  seeds 81–100.
- **Expected:** no memory fails. In a side box facing dark, the ring says
  where it faces but not which side box it is in, so whether a turn shows
  the centre or more dark depends on the heading it had when it stepped
  in: memory across steps again. Whether recent 3 or context can carry
  it is the question.

### Two checks Nano asked for

- **"Lingers near one of the pair."** For the no-memory probe at 0.64 on
  matching-box turns: the share of the more common of north and south in
  those same turns is recorded per seed. If its median is close to 0.64,
  the explanation holds.
- **Gated probe at 0.49.** Noted in the results below.

## Third-try results

### The two checks

- **"Lingers near one of the pair" doesn't hold.** On seeds 41–60 (no
  ring), the more common of north and south makes up a median 0.55 of the
  last 100 matching-box turns: always guessing it scores 0.55, not 0.64.
  The real cause was the probe itself: it kept learning online, so during
  a stare at, say, north it was trained on north several times just before
  being scored on the turn that ends that stare. Its own weights carried
  heading through the stare. With the probe frozen for the second half of
  each life (now the standard, for both probes), matching-box probe
  accuracy drops to 0.54 for no memory, 0.53 for recent 3, 0.50 for
  context and 0.48 for gated, all at the 0.55 majority. **Correction to
  the second-try results:** recent 3 and context carry no heading the
  probe can find; the +0.03 to +0.06 there was the same artefact.
- **Gated probe at 0.49 (0.48 frozen).** Gating gives each motor its own
  first-layer weights, so the hidden code for the same heading differs by
  motor. About 3 in 4 steps are stares, so the probe learns mostly the
  do-nothing motor's code and is then scored on turns. Checked: scored on
  stares instead, the gated probe reads heading at 0.71 overall, close to
  the other memories (0.75–0.77, where 0.75 is the most anyone can get
  with north and south alike); scored on turns, 0.59. So gating's low
  probe is a property of the probe meeting a motor-split code, not
  evidence that gating loses heading. With the ring, the same happens
  (0.19 overall).

### Matching with the ring, seeds 61–80: met for every memory

| Memory (+ ring) | One-ahead | Two-ahead | After stares ≥ 5 | Probe | Seeds passing |
|-----------------|-----------|-----------|------------------|-------|---------------|
| None            | 1.00      | 1.00      | 1.00             | 0.96  | 20/20         |
| Recent 3        | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |
| Context         | 1.00      | 1.00      | 1.00             | 1.00  | 20/20         |
| Gated           | 1.00      | 1.00      | 1.00             | 0.19  | 20/20         |

As expected: given heading, the next view is a two-way combination (ring
and motor), and every learner, no memory included, solves it. Which motor
turns the ring which way was still learned per seed.

### Walking with the ring, seeds 81–100: missed for every memory

| Memory (+ ring) | One-ahead | Two-ahead | After stares ≥ 5 | Place probe | Seeds passing |
|-----------------|-----------|-----------|------------------|-------------|---------------|
| None            | 0.77      | 0.75      | 0.76             | 0.22        | 0/20          |
| Recent 3        | 0.77      | 0.75      | 0.76             | 0.24        | 0/20          |
| Context         | 0.79      | 0.77      | 0.78             | 0.30        | 0/20          |
| Gated           | 0.86      | 0.85      | 0.86             | 0.26        | 0/20          |

- **The same wall, one level up.** With heading given, what is left
  unknown is *where* it stands: in a side box facing the dark, the view
  and ring don't say which side box, and whether a turn shows the centre
  depends on the heading it had when it stepped in. That is memory across
  steps again, and again no learner keeps it: the place probe (centre or
  which side box, frozen) reads 0.22–0.30.
- **Gating helps here** (0.86 against 0.77, the only memory clearly above
  no memory), but nowhere near the mark; on tuning seeds it stayed at
  0.85 at 120,000 steps.
- **Read across stages:** every time the world asks for a fact the
  current percept doesn't carry (heading in matching, place in walking),
  the learners fail; every time it is in the percept (views in turning,
  the ring in matching), they pass at 100%. The innate ring moved the
  wall from heading to place; it didn't teach the learners to keep state.

The obvious next step would be an innate place sense too (path
integration of steps, like the ring for turns). That would make walking
a two-way combination again, and probably pass, but it would also mean
giving the apprentice, built in, every fact it fails to keep. Whether to
do that or to look for a learner that keeps state is Nano's call.

## A learner that keeps state, step 1: GRU ceiling check (rules fixed before any run)

Nano's direction (2026-10-30): no innate place sense; find a learner that
keeps state. First, a ceiling check: can any recurrent network, trained
the usual way, do walking? If not, stop and report.

- **Learner.** A small GRU, 32 hidden units, written here (no ML library),
  comparison only like `Learning::Backprop`. Input: the percept (8 sensed
  numbers + the 4 ring units) and the motor one-hot (4). Output: the next
  percept, read out linearly from the new hidden state. Trained **offline
  with backprop through time**: the 40,000-step walking life is recorded,
  then replayed in order for 20 epochs, in chunks of 40 steps (hidden
  state carried across chunks, gradients cut at chunk edges), Adam at rate
  0.003, gradients clipped to norm 1. Squared error on the next percept.
- **World and life.** Walking + ring, as judged above. After training, the
  same world goes on for 4,000 more steps with the GRU frozen, and that is
  what is scored.
- **Scoring.** As before, on non-stare steps of the 4,000: one-ahead and
  two-ahead over the last 100, after stares ≥ 5 over all of them. Two-ahead
  feeds its own predicted percept (ring included) back in.
- **Place probe.** A linear probe for where it stands (centre or which side
  box), trained by delta rule on the hidden states of one frozen replay of
  the training life, then scored on the last 100 scored steps.
- **Marks** (same as walking): one-ahead ≥ 95%, two-ahead ≥ 90%, after
  stares ≥ 5 ≥ 90%, in ≥ 19 of seeds 101–120.
- **Tuning**: if needed, only on seeds 201–205, disclosed. A gradient check
  against finite differences is a test.
- **Stop rule (Nano's):** if it misses, stop and report. Only if it passes
  comes step 2: a recurrent net with e-prop-style eligibility traces, online
  and local, with its own rules committed first and fresh seeds.

### Step 1 results: met

**Walking + ring, GRU trained offline by BPTT, seeds 101–120: met, 20/20.**

| Learner | One-ahead | Two-ahead | After stares ≥ 5 | Place probe | Centre-direction probe | Seeds passing |
|---------|-----------|-----------|------------------|-------------|------------------------|---------------|
| GRU     | 1.00      | 1.00      | 1.00             | 0.39        | 0.90                   | 20/20         |

So a network that keeps state can learn walking from this data; the task
is not too hard, and the local learners' miss is about how they learn.

The place probe (which side box, in compass terms) reads only 0.39, though
the GRU predicts perfectly. A second probe, added after seeing that and
reported for every learner from now on, asks where the centre lies *from
where it faces* (ahead, right, behind, left, or it is in the centre): 0.90.
The GRU keeps place egocentrically, relative to its heading, not on the
compass. For the local learners on seeds 81–100 that probe reads 0.37–0.49
(no memory 0.39), so they keep neither.

## Step 2: e-prop, online and local (rules fixed before any run)

- **Learner.** A recurrent network of 64 leaky tanh units, no self
  connections: `h(t) = α·h(t−1) + (1−α)·tanh(Win·x(t) + Wrec·h(t−1) + b)`,
  with each unit's leak α fixed and spread evenly over 0.5–0.95 (fast to
  slow memory). Output `y = Wout·h(t) + bout`, the next percept. Input:
  percept (8 + ring 4) and motor (4), as for the GRU.
- **Learning: e-prop** (Bellec et al., 2020), online, every step, no
  replay. Each input and recurrent weight keeps an eligibility trace,
  `ε_ij(t) = α_j·ε_ij(t−1) + (1−α_j)·tanh′(a_j(t))·z_i(t)`, where `z_i` is
  its presynaptic activity (input, or the unit's activity at `t−1`). A
  learning signal `L_j(t) = Σ_k Wout_kj·(y_k − y*_k)` is broadcast to each
  unit (symmetric feedback), and each weight moves by `−η·L_j(t)·ε_ij(t)`.
  `Wout` learns by the delta rule. Every update uses only that synapse's
  own trace and its unit's learning signal: local in space and time.
- **World and scoring.** Walking + ring, scored like the other online
  learners: last 100 scored steps of the life, after stares ≥ 5 over the
  second half, both probes frozen for the second half.
- **Budget: 400,000 steps**, one online pass. The GRU saw 40,000 steps 20
  times (800,000 step-updates); an online learner can't replay, so it
  gets a longer life instead, half the GRU's updates.
- **Marks** (same): one-ahead ≥ 95%, two-ahead ≥ 90%, after stares ≥ 5
  ≥ 90%, in ≥ 19 of 20 fresh seeds, **121–140**. Tuning (rate, hidden
  size, leak range) only on seeds 201–210, disclosed.
