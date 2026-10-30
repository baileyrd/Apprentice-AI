# Experience decline: which learned part makes barren worlds harder?

PR #26 left one question open: experience makes barren worlds harder. In a
barren-first life, later barren worlds cost more sleeps than the first, and
a newborn dropped into a later barren world does better than the apprentice
that grew up there (tuning seeds 101–110: newborn 19.9 and 21.5 sleeps per
1,000 steps against 25.8 and 25.9 for the experienced apprentice). This
slice asks which learned part carries the harm, by wiping one part at the
start of world 5 and seeing whether the apprentice then lives like a
newborn.

It is a diagnosis, not a change: nothing here alters the default, and no
rule is judged.

## The life

The barren-first life of `apprentice::life` with today's default settings:
barren, bush, barren, bush (1,000 steps each, seeds seed + 100·i), then
world 5, a new barren world of 2,000 steps (seed + 5000). Only world 5 is
measured.

## Arms, at the start of world 5

| Arm | What is wiped | What it keeps |
|---|---|---|
| **E** experienced | nothing | everything |
| **P** patterns | the patterns network, rebuilt as at birth | maps, diary, its own values |
| **M** memory | the catalog of maps (one fresh map, active), fast beliefs, the day's diary and meals, active memory, tallies, rehearsal samples, doubt | patterns, its own values |
| **V** values | the apprentice's own learned values: valence (enter, eat, rest), volatility, when it last saw each square, words heard and their meanings | patterns, maps |
| **F** full reset | P + M + V on the same apprentice | its random-number stream and clock |
| **N** newborn | a new apprentice with seed `seed`, straight into world 5 | nothing |

"Network" in the ask: the patterns are the only network the apprentice
runs (PR #22 made them a predictive-coding net), so patterns and network
are one arm (P). The third part wiped is the apprentice's own learned
values (V), the remaining place where experience is stored.

F is the control: if wiping all three parts does not bring the apprentice
back to a newborn, something not listed carries over and the single-part
arms cannot be read as a full account.

## Seeds

Tuning seeds 101–140 (40 seeds). No judging seeds are used.

## Measures (world 5 only)

- **Sleeps per 1,000 steps** (sleeps × 1,000 / 2,000). The main number.
- Meals per 1,000 steps, and false hopes in the first 1,000 steps: reported.

## Comparison

Each arm against N, paired seed by seed:

- typical change, `Interval::change` (geometric mean of
  (x + 1)/(newborn + 1)), 90% interval;
- raw difference in sleeps per 1,000 steps, `Interval::mean` of the per-seed
  differences, 90% interval;
- share of the gap closed: (E − arm)/(E − N) on means.

## How to read it

Fixed before the run:

- **The decline is real on these seeds** if E against N has a raw-difference
  interval wholly above 0. If not, the other arms are not read.
- **The control holds** if F against N has a raw-difference interval that
  contains 0.
- **A part carries the decline** if wiping it closes at least half the gap
  and its raw-difference interval against N contains 0. **A part
  contributes** if it closes at least a quarter of the gap. Parts may share
  it; the shares need not add to 1.

My guess before running: P carries most of it. Patterns are the only part
taught in every world of the life, so they average bush and barren
regrowth, and the bush worlds teach "eaten squares come back".

## Follow-up (Nano, 2026-10-30), fixed before its run

- Each arm's own mean now carries a 90% interval (`Interval::mean`).
- **PV**: patterns and own values wiped together; maps kept. Read by the
  same rules: it carries the decline if it closes at least half the gap and
  its raw-difference interval against N contains 0.

The seeds are the same (101–140), so the first run's arms are rerun
unchanged and must reproduce its numbers.

## Results (seeds 101–140, first run)

`cargo run --release --bin decline` prints [results.txt](results.txt) and
writes [decline.csv](decline.csv).

Sleeps per 1,000 steps in world 5 (newborn 25.0):

| Arm | Mean | vs newborn | Raw difference | Gap closed |
|---|---|---|---|---|
| E experienced | 28.4 | +13% [+9%, +18%] | +3.4 [+2.3, +4.4] | — |
| P patterns wiped | 26.5 | +6% [+2%, +10%] | +1.5 [+0.5, +2.5] | 56% |
| M maps wiped | 28.3 | +13% [+9%, +17%] | +3.3 [+2.4, +4.2] | 3% |
| V own values wiped | 27.4 | +9% [+6%, +13%] | +2.3 [+1.5, +3.1] | 31% |
| F all wiped | 24.5 | −2% [−6%, +2%] | −0.6 [−1.6, +0.5] | 116% |

Meals per 1,000 steps (newborn 20.7): E 16.0, P 20.3, M 15.7, V 16.8,
F 21.6. False hopes in the first 1,000 steps (newborn 24.3): E 0.7, P 21.4,
M 0.0, V 1.9, F 24.5.

Read by the rules above:

- **The decline is real on these seeds**: E costs 3.4 more sleeps per 1,000
  steps than a newborn, interval wholly above 0.
- **The control holds**: wiping all three parts brings it back to a newborn
  (raw −0.6, interval contains 0).
- **No single part carries it.** The patterns close 56% of the gap, but the
  apprentice is still 1.5 sleeps per 1,000 steps worse than a newborn
  (interval above 0). **Patterns and its own values both contribute** (56%
  and 31%). The maps do not (3%).

What the other numbers suggest, not checked: the experienced apprentice
eats less (16 against 21 meals per 1,000 steps), and wiping the patterns
alone restores the meals (20.3). Experienced patterns say eaten squares stay
gone (false hopes 0.7 against a newborn's 24), so it stops going back to
them; in a barren world food regrows anywhere, including where it ate, so
a newborn's naive hope may be what keeps it finding food. The values'
share has no lead yet: which value moved (eat, enter, rest or volatility)
is the next thing to look at.
