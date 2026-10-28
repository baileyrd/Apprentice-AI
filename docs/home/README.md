# Home: does it know its own world?

Nano's question: give the apprentice a small world it knows well, call it
home, then put it in a world and ask: is this your home?

## Setup

- **Home** is a small bush world: 6×6, wall density 0.12, hazard density
  0.05, 3 bushes that grow back in place. The apprentice (memory, meal
  replay, patterns, the recall margin) lives there 3,000 steps.
- **A trial** puts that apprentice, as it is at the end of those 3,000
  steps, in a world for 300 steps. It always wakes on home's wake square
  (its maps are drawn relative to where it wakes). Trials per home:
  - **home**: home itself, as it was left;
  - **changed home**: home with one wall moved;
  - **strangers**: three new worlds built to the same recipe.
- **Asking.** It is never told which world it is in. The question "is this
  home?" is answered from its own catalog: **yes** if the map it is using is
  the one it was using at the end of its life at home.

## Rules fixed before any run

Settings may be tuned on seeds 101–110 against these rules; the final check
is one run on seeds 1–10 (10 homes, 50 trials). Answers are read after 100
steps in the trial world.

1. **Knows home.** It says yes at home in at least 9 of 10.
2. **Knows a stranger.** It says no in at least 27 of 30 strangers.
3. **Home, changed, is still home.** It says yes in at least 8 of 10 changed
   homes.

Also reported, not judged: the answers after 300 steps, and how many steps
each stranger took before it first said no.

## What the first runs showed

On the spare seeds, before any change, it said yes at home in 7 of 10 and no
to only 17 of 30 strangers. Two things went wrong, and both are the problem
Nano described, that it never builds a stable sense of place:

- **It split its own home into many maps.** In one home it made 15 maps in
  3,000 steps. Every bush it ate from looked "wrong" to its map until it grew
  back, and in a small world bushes fill much of the view. So home kept
  failing to fit, and it kept deciding it was somewhere new.
- **It learned strangers into home's map.** In a stranger, home's map would
  misfit for a while, then fit again for a few steps, which reset all its
  doubt. Meanwhile its naps wrote the stranger into home's map, so home's
  map came to fit the stranger, and the stranger became "home".

## The change

Three changes to memory, each a local rule, with no world label or cue:

1. **Doubt builds up and drains away; it does not reset.** Steps that misfit
   add to its doubt, and steps that fit take away from it. A few familiar
   steps no longer erase the evidence of being elsewhere.
2. **While in doubt, it does not write into the map it is using.** What it
   senses stays in the diary until it decides. If it then starts a new map
   or recalls an old one, the held sightings go there. If the doubt drains
   away, they go into the map it was using. If the doubt alone fills the
   diary, they are written in anyway.
3. **An eaten bush growing back is expected.** For a square it ate from,
   the map is judged by what patterns say (it grows back after a while), not
   by the map alone.

**Tuning, disclosed.** On seeds 101–110 I also tried a higher misfit line
(0.35 to 0.5), a misfit line set by each map's usual error, and judging only
on squares that had proved stable (the landmarks idea). None met all three
rules. Each of the three changes above was needed: without (1) it said no to
26 of 30 strangers, without (2) 25 of 30, and without (3) it said yes at
home in only 8 of 10. No new settings were added.

Reproduce:

```sh
cargo run --release --bin home -- --out docs/home
```

## Results (seeds 1–10, one run)

| Rule | After 100 steps | Met? | After 300 steps |
|---|--:|---|--:|
| 1. Says yes at home, ≥ 9 of 10 | 10 of 10 | yes | 10 of 10 |
| 2. Says no to a stranger, ≥ 27 of 30 | 28 of 30 | yes | 28 of 30 |
| 3. Says yes to home with a wall moved, ≥ 8 of 10 | 10 of 10 | yes | 10 of 10 |

It took a median of 39 steps to first say no to a stranger. Per-trial
answers are in [home.csv](home.csv).

### Earlier tests, rerun

| Test | Before | After | Rule held? |
|---|--:|--:|---|
| A/B/A relearn savings (rule: ≤ 0.30) | 0.37 | **0.09** | yes |
| A/B/A mix-ups (rule: ≤ 2 of 10) | 3 | **0** | yes |
| B learned | 10 of 10 | 10 of 10 | yes |
| Anticipation with patterns (≥ 0.5) | 0.77 | 0.88 | yes |
| …and ≥ 0.2 above without patterns | 0.77 vs 0.13 | 0.88 vs 0.74 | **no** |
| Bush-world sleeps, patterns vs without (≤ 10% above) | 10.5 vs 38.5 | 10 vs 38.5 | yes |
| Walls re-mapped when fed, bush world (≥ 9 of 10) | 10 (333 steps) | 10 (252 steps) | yes |
| Phase 2 wall test (median ≤ 798 steps) | 427 | 495 | yes |
| Lessons exam sleeps: lessons vs no schooling | 55.5 vs 55 | 48 vs 53 | (not a rule here) |

Outputs are in [relearn/](relearn/), [transfer.csv](transfer.csv),
[walls-bushes/](walls-bushes/), [walls/](walls/), [energy/](energy/) and
[lessons/](lessons/).

## What it means

- **It knows its home.** It said yes at home every time, kept saying yes
  when a wall had moved, and said no to 28 of 30 strangers within about 40
  steps.
- **The same fix solved the A/B/A mix-up.** No seed now spends time in world
  B on A's map, and coming back to A takes a median of 8 steps against 104
  the first time (savings 0.09, the best of any phase). The earlier attempt
  (a margin before going back) treated the symptom. The cause was that doubt
  reset too easily and the strange world was written into the old map while
  it was still deciding.
- **One earlier rule now misses, from the other side.** The apprentice
  *without* patterns now anticipates a regrown bush in a fresh world at 0.74
  (it was 0.12), so patterns are no longer 0.2 ahead. Patterns themselves
  did not get worse (0.88). I have not checked why the apprentice without
  patterns improved. A likely cause is that it no longer writes the fresh
  world into an old map while in doubt.
