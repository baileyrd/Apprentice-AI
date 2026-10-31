# Why did time-unseen and path-counting hurt the home test?

Open question from docs/revisit: revisit + time-unseen + path-counting
(RUS) gave +0.24 [+0.13, +0.35] more wrong answers per home test than
revisit alone (R), on seeds 1–100 and 141–240. Both extras are deleted
(PR #33); this diagnosis runs at their last commit, c16c8bc. Nothing here
changes the apprentice or its defaults.

## Plan, fixed before running

**Arms**, each a whole home test (3,000 steps at home, then five 300-step
trials: home, changed home, three strangers), answers read at 100 steps:

- **R**: today's default (revisit 1/128).
- **RU**: R + time-unseen patterns (`PatternNet::unseen`).
- **RS**: R + path-counting plans (`Drives::sweep`).
- **RUS**: both.

**Seeds.** 241–300 for looking around (mechanism probes; reported only,
never a verdict). 301–500 (200 fresh seeds) for the attribution below,
run once.

**Measures**, per seed: wrong answers (0–5), split into strangers taken
as home (0–3) and home or changed home taken as a stranger (0–2).

**Attribution reading** (raw paired difference against R, 90% interval,
`apprentice::judge`; % change reported next to it):

1. A part **raises wrong answers** if its arm (RU or RS) against R has the
   whole interval above 0.
2. The harm **is mostly strangers taken as home** if, in RUS against R,
   the stranger difference is at least two thirds of the total difference
   (point estimates), and **mostly home taken as a stranger** if that part
   is at least two thirds; otherwise both.
3. Whether RUS reproduces at all on fresh seeds is read the same way as
   (1).

Mechanism probes (what changes in a trial: doubt, map fit, how much of
the view is judged by regrowth, where it walks) are chosen after the
attribution, on seeds 241–300, and reported only.

## How to rerun

The probes need the deleted code, so they run at c16c8bc:

```sh
git worktree add /tmp/c16 c16c8bc && cd /tmp/c16
git apply <repo>/docs/home-extras/probes.patch   # probe counters, Apprentice: Clone
cp <repo>/docs/home-extras/home_*.rs examples/
cargo run --release --example home_why -- 301 500 > attribution.csv
cargo run --release --example home_probe -- 241 300 > probes.csv
python3 <repo>/docs/home-extras/attribution.py attribution.csv
```

The patch only adds counters and switches that are off unless a probe
sets them; `home_why` output is byte-identical with and without it
(checked on seeds 241–244).

## Results

All results below were measured at c16c8bc, where path-counting and
time-unseen still existed, not on main.

### Attribution (seeds 301–500, one run, the reading fixed above)

Wrong answers per home test (0–5) at 100 steps. Every extra wrong answer,
in every arm, is a stranger taken as home: home and changed home were
answered right in all 800 trials of each arm.

| Arm | Wrong per test | Against R, raw | Against R, % | Reading |
|---|---|---|---|---|
| R | 0.49 | | | |
| RU (time-unseen) | 0.45 | −0.04 [−0.12, +0.04] | −2% [−7%, +3%] | no detectable effect |
| RS (path-counting) | 0.65 | **+0.16 [+0.06, +0.25]** | +8% [+2%, +14%] | **raises** |
| RUS (both) | 0.70 | +0.21 [+0.11, +0.31] | +12% [+6%, +18%] | reproduces |

1. **Established: path-counting adds wrong answers.** RS against R has
   the whole interval above 0. **Time-unseen: no detectable effect**; its
   interval includes zero.
2. **Mostly strangers taken as home**: 100% of RUS's extra wrong answers.
3. **RUS reproduces on fresh seeds** (+0.21 against +0.24 on the old
   seeds). RUS − RS (+0.06) is within noise; no interaction is claimed.

### Mechanism: leads, not findings (seeds 241–300, exploratory)

Probes on the 180 stranger trials per arm, first 100 steps:

| Arm, trial variant | Strangers taken as home (of 180) |
|---|---|
| R | 23 |
| R, regrowth-judged fit off in the trial | 11 |
| R, path-counting on in the trial only | 30 |
| RS | 39 |
| RS, regrowth-judged fit off in the trial | 17 |
| RS, path-counting off in the trial only | 34 |
| RU / RUS | 17 / 33 (fit off: 11 / 14) |

What a wrong stranger trial looks like (R and RS alike): the home map
really does fit what it sees (home-map error on map-judged squares 0.23
against 0.52 in right trials), it walks over a few fewer squares (24
against 29), and **far more of the view is excused as "eaten, not grown
back yet"**: about 80 sightings judged by patterns instead of the map,
against about 45 in right trials, and those judged sightings carry low
error (0.30–0.46 against 0.65).

Everything below is exploratory: one look-around run on 60 seeds, probes
chosen after seeing the data, no pinned reading. These are leads to test
on fresh seeds, not results.

The suggested chain:

1. **Lead: regrowth-judged fit (PR #11) is the door.** A square it ate from at
   home stays in state "ate" while it looks empty. In a stranger, most of
   those squares *are* empty (only 3 bushes in 36 squares), so they look
   exactly like a bush not yet grown back, and the patterns' soft
   "maybe regrown" belief scores far better than the map's "food" would.
   The misfit that should build doubt is explained away. Turning that
   judging off during the trial halves strangers taken as home in every
   arm (R 23 → 11, RS 39 → 17), at a cost of 2 wrong answers in 120 home
   and changed-home trials.
2. **Lead: path-counting walks more traffic through that door.** RS has more
   pattern-judged sightings per trial (57 against 46) with lower error
   (0.57 against 0.63), so more of a stranger is excused. Likely, not
   checked: it ends home life with more recently eaten squares, and its
   patterns lean more toward "still empty". With the
   door shut, RS's excess over R falls from 16 to 6.
3. **Lead: most of it is what it learned at home, not how it walks in
   the trial.** Switching path-counting off for the trial only takes RS from
   39 to 34; switching it on for R takes 23 to 30. So roughly a third of
   the harm comes from walking differently in the trial, two thirds from
   the mind raised with it (noisy: 60 seeds).

Not checked: how many squares are in state "ate" at the start of each
trial (pattern-judged sightings stand in for it), and which pattern
outputs moved.

### Link to the network's cost (card "strangers taken for home"): a hypothesis

Hypothesis, not tested here: the same door explains the patterns
network's own cost. Regrowth-judged
fit off halves strangers taken as home for today's default (R) too, as
the PR #22 check found on seeds 1–30. A fix would have to keep the
excuse for home (where eaten squares do grow back) without granting it
in a world whose map does not otherwise fit, for example by excusing an
eaten square only while the map's other squares fit, or for at most the
regrowth time the patterns learned. That is a design change with its own
marks; nothing here changes the apprentice.

## Decision (Nano, 2026-10-31)

Merged as docs only. Path-counting and the regrowth excuse both stay for
now. Next: a narrower excuse (an eaten square is excused only while the
rest of the map fits), with marks and thresholds written before running,
judged on fresh seeds (501+) against today's default and against the
excuse-off arm.

Update (Nano, 2026-10-31): that test runs on main, where path-counting is
gone, with three arms: main's default, excuse off, narrower excuse.
Bringing path-counting back would be its own card, opened only if someone
wants it, and it would have to pass the home test with the narrower
excuse in place.
