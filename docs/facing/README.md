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
