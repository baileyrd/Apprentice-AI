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
- **Motors.** Bare outputs `0..k`. What each one does is shuffled per seed.
  Effects: turn left, turn right, and (stage 2, if added) do nothing.
- **Life.** Motors are fired at random. The learner watches and predicts.

Stages, in order (walking into boxes and walls come in later PRs):

1. **Turning.** Four boxes, four different looks, fixed order. Solvable by a
   lookup of (what it sees, which motor), so it proves motor learning only.
2. **Matching.** The north and south boxes look the same. Facing either,
   what comes after a turn depends on which one it faces: the real test of
   heading.

## The learner

The predictive-coding network from `src/network.rs`, unchanged: one hidden
layer of 16 units, local updates only. Input is what it senses now plus the
motor it fires (one unit per motor). Output is its prediction of what it
will sense next. The network keeps no state between steps, so three
memories are compared:

- **None.** Input is now and the motor only.
- **Last step.** Adds what it sensed and fired one step before. A control:
  a longer lookup key, not a heading.
- **Context.** Adds a copy of its own hidden layer from the step before
  (context units). Learned locally, as plain input; no backprop through
  time.

## How it is scored

A prediction is decoded to the nearest look. It is right if that is the
look of the box it then faces.

- **One-ahead:** predicting the next view.
- **Two-ahead:** predicting the view two steps on, given both motors,
  feeding its own first prediction (and memory) back in. After two turns
  the same way, that is the box behind it.

## Rules fixed before any run

Settings may be tuned on seeds 101–110; the check is one run on seeds 1–20.
Accuracy is over the last 100 steps of the budget.

| Stage    | Budget       | One-ahead | Two-ahead | Seeds passing |
|----------|--------------|-----------|-----------|---------------|
| Turning  | 2,000 steps  | ≥ 95%     | ≥ 90%     | ≥ 19 of 20    |
| Matching | 5,000 steps  | ≥ 95%     | ≥ 90%     | ≥ 19 of 20    |

Plus one control rule:

- **Matching needs memory.** With no memory, the matching stage must fail
  (expected ceiling about 75% one-ahead: half the steps face a matching box,
  and from there a turn is a coin flip). If it passes, the stage doesn't
  test heading.

Known weakness, stated before running: with turning only, two matching
boxes are never seen twice in a row, so the last-step control can also pass
matching. A "do nothing" motor would fix that; it is offered to Nano, not
yet in.
