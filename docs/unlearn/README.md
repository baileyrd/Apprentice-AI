# Unlearning: how long does a disproven belief last?

Nano's question: patterns play a large part in how we understand the world,
which is why people respond so poorly to their beliefs being disproven. Does
the apprentice hold on to a pattern after the world stops following it? And
does a belief it has held longer take longer to unlearn?

## Setup

- **Raised on regrowth.** The apprentice (memory, meal replay, patterns)
  lives in the 12×12 bush world, where a bush it eats from grows back on the
  same square 30 steps later. Three upbringings, each its own apprentice:
  1,000, 3,000 or 9,000 steps.
- **The world stops following the pattern.** It then lives 4,000 steps in a
  new world of the same size where an eaten bush never grows back. Instead,
  new food springs up on a random empty square, so it can still eat.
- **The belief** is read straight from its patterns: the chance of food it
  gives a square it ate from 64–127 steps ago. Above 0.5 it believes the
  bush will have grown back.
- **The behaviour** is a futile return: trying to eat on a square where it
  ate before in the new world, and finding nothing.

## Rules fixed before any run

Settings may be tuned on seeds 101–110 against these rules; the final check
is one run on seeds 1–10. Only seeds whose belief is above 0.5 when they move
count for rules 1 and 3 (a belief it never formed cannot be unlearned).

1. **It does unlearn.** For every upbringing, the belief falls below 0.5
   within the 4,000 steps in at least 9 of 10 counted seeds.
2. **Its behaviour follows.** For every upbringing, median futile returns
   in the last 1,000 steps are at most half those in the first 1,000.
3. **Nano's question, measured.** Median steps to unlearn after 9,000 steps
   of regrowth, against after 1,000. This is a measurement, not a target; my
   prediction before running is that they will be about the same, because
   each pattern learns by a delta rule that keeps no count of how much
   evidence lies behind it. If so, that is a difference from people worth
   knowing.

## Tuning, disclosed

No settings were changed. On seeds 101–110 the futile returns rose over
time instead of falling, and the apprentice did not believe food was on any
square it had eaten from by the end. So I added one diagnostic, not a rule:
futile returns to a square eaten 31–127 steps earlier, when a bush would have
grown back. That is the behaviour the belief should drive. Rule 2 is still
judged as written.

Reproduce:

```sh
cargo run --release --bin unlearn -- --out docs/unlearn
```

## Results (seeds 1–10, one run)

| Upbringing | Seeds that believed | Belief when it moved | Steps to unlearn | Futile returns, first / last 1,000 | …when a bush would have regrown |
|---|--:|--:|--:|--:|--:|
| 1,000 steps | 2 of 10 | 0.00 (median) | 108 | 27.5 / 59 | 0 / 0 |
| 3,000 steps | 9 of 10 | 0.85 | 135 | 27 / 52.5 | 0 / 0 |
| 9,000 steps | 10 of 10 | 1.00 | 185 | 13.5 / 41.5 | 0 / 0 |

| Rule | Result | Met? |
|---|---|---|
| 1. Unlearns in ≥ 9 of 10 counted seeds | 2/2, 9/9, 10/10 | yes (only 2 seeds counted after 1,000 steps) |
| 2. Futile returns at most halve | they rose in every upbringing | **no** |
| 3. 9,000 against 1,000 steps (measured) | 185 against 108 steps; against 3,000, 185 against 135 | prediction wrong |

Per-seed numbers are in [unlearn.csv](unlearn.csv).

## What it means

- **It lets go fast.** Once eaten bushes stop growing back, the belief falls
  below even odds in a few hundred steps at most, in every seed that held
  it. Forming the belief took 1,000 to 3,000 steps. Unlearning is roughly ten
  times quicker than learning.
- **A belief held longer does take longer to unlearn, but not much.** After
  9,000 steps it took 185 steps, after 3,000 it took 135. My prediction of
  "about the same" was wrong. The reason is still mechanical rather than
  human-like: a longer upbringing pushes the belief closer to certainty
  (1.00 against 0.85), so it has further to fall. Past that point extra
  experience adds nothing, because each pattern keeps no count of the
  evidence behind it. A person's belief after 9,000 confirmations would
  likely be far more stubborn than after 3,000.
- **Its behaviour was never driven by the belief here.** It almost never
  went back to an eaten square when the bush would have regrown, early or
  late. The belief was gone before it mattered. The futile returns that
  rose are aimless eating on squares it once ate from. That rises over
  time in a world where food appears anywhere, for a reason I have not
  traced. Rule 2 measured the wrong thing.
- **So it is not stubborn the way people are.** To make it so, a pattern
  would need to weigh new evidence against how much it has already seen
  (a learning rate that shrinks with confirmations, for example). That is a
  design choice, and it cuts both ways: it would make the apprentice slower
  to adapt in exactly the tests we have been passing.
