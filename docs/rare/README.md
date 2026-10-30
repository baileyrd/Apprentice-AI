# Rare outcomes: learning regrowth faster without over-expecting it

PR #24 (fair mix) found a trade-off in the patterns' network. Fed mostly
on changes, it leans toward "it grows back": that costs false hopes in a
barren world but feeds it in a bush world. Fed fairly, the false hopes go,
and so does the bush advantage (bush sleeps +46% with the gate, +126% with
a budget). The network learns at one rate per sample, so a rare outcome
(a bush seen regrown) is learned slowly unless it is over-taught.

This slice lets a rare outcome teach more without the network coming to
over-expect it, as Nano agreed.

## Words used here

- **Outcome**: what a square was seen to be (food, empty, wall).
- **Rare**: seen less often than the others from the same state (eaten,
  seen empty, …), counted over the apprentice's whole life.

## What changes

`PatternNet::rare: Option<f64>` (default `None`: as now). With `Some(cap)`:

- Per state, the network's patterns keep a count of each outcome.
- A sample teaches at the network's rate times its outcome's **weight**:
  how much rarer it is than an even share, `(state's samples / kinds) /
  (outcome's count + 1)`, held between 1 and `cap`. Common outcomes teach
  as now; rare ones teach up to `cap` times as hard.
- Taught that way the network's answer settles on each outcome's chance
  times its weight, so its answer is **corrected back** before use: each
  outcome's share divided by its weight, then scaled to sum to 1. At the
  settled point that is the true chance again, so rare outcomes are learned
  faster without being over-expected.

This is still a local, online update (one weight per sample on the rate),
not backprop.

## Test

The life of PRs #19–#24 (`apprentice::life`). Four settings per seed:

- **A**: today's default.
- **R**: A with rare-outcome weighting.
- **G**: the gate on with fair mix (PR #24's H), with rare-outcome
  weighting.
- **B**: a network budget of 144 with 64 rehearsed and fair mix (PR #24's
  K), with rare-outcome weighting.

## Rules fixed before any run

The cap tuned on seeds 101–110 only. Judged once on seeds 1–100 and
111–210, each comparison paired seed by seed, read from the 90% interval
as met, missed or unclear with `apprentice::judge` (PR #18).

1. **It learns regrowth faster.** Bush sleeps, R against A: at most −10%.
2. **It costs nothing.** R against A, each at most +10%: barren sleeps,
   barren false hopes, home-test wrong answers after 100 steps.
3. **The gate can go on.** G against A, each at most +10%: bush sleeps,
   barren sleeps, barren false hopes, home-test wrong answers after 100
   steps.
4. **A budget can go on.** B against A, the same four, each at most +10%.

Reported but not judged: upbringing sleeps, meals, futile returns, diary
writes, patterns taught.

My guess before running: rule 2 met; rule 1 unclear (the default already
sees every regrowth); rule 3 unclear, since PR #24's gate cost in the bush
world may not be only slow learning; rule 4 likely missed, as a budget
gives the network fewer regrowths to learn from at any weight.
