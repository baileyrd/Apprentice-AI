# Selective sleep for the network

PR #23 gave sleep a replay budget for the map: it replays only the most
"surprising" diary entries into the map, and lost nothing. The network
that holds the patterns (PR #21, the default since PR #22) was left out:
it still learns from every diary entry in every sleep. This slice puts the
network on a budget too, as Nano asked, and mixes in older memories so a
budget of today's surprises does not overwrite what earlier days taught.

## Words used here

- **"Surprise"** of a sighting: its prediction error just before it was
  seen, as in every phase. A plain number, not a feeling.
- **Pattern sample**: one thing the network learns from: a square's state
  and age, and what it was then seen to be.
- **Rehearsal**: pattern samples from earlier sleeps, replayed again.

## What changes

`Recall::net_replay: Option<NetReplay>` (default `None`: the network
learns from every entry, as now). With `NetReplay { budget, rehearsal }`:

- Each sleep teaches the network the `budget` most "surprising" of the
  day's settled diary entries that carry a square state, in the order seen.
- First it replays `rehearsal` older samples, spread evenly over a store of
  512 kept from earlier sleeps (every 8th of each day's samples; the oldest
  drop out). Older first, so today stays the most recent, as with the
  gate's confirmations.
- Confirmations counted by the gate are not affected.

The map's own budget (PR #23) is separate; both can be on.

## Test

The life of PRs #19–#23 (`apprentice::life`). Four settings per seed, gate
off in all:

- **A**: today's default: the network and the map learn from everything.
- **E**: network budget with rehearsal.
- **F**: network budget, no rehearsal.
- **G**: E plus the map's budget of 144 (PR #23): all of sleep selective.

## Rules fixed before any run

Budget and rehearsal tuned on seeds 101–110 only. Judged once on seeds
1–100 and 111–210, each comparison paired seed by seed, read from the 90%
interval as met, missed or unclear with `apprentice::judge` (PR #18).

1. **The network does less work.** Pattern samples learned in the bush
   world, E against A: at most −50%.
2. **Nothing that matters is lost.** E against A, each at most +10%: bush
   sleeps, barren sleeps, barren false hopes, home-test wrong answers after
   100 steps.
3. **Rehearsal is what keeps it.** Barren false hopes, E against F: at most
   −20%.
4. **All of sleep can be selective.** G against A, each at most +10%: the
   same four as rule 2.

Reported but not judged: upbringing sleeps, meals, map replays.

My guess before running: rule 1 met by construction. Rule 2 at risk on
false hopes: PR #19 found that teaching patterns only surprises biases them
toward change, so I expect false hopes to rise without rehearsal (F) and
rehearsal to win back some of it (rule 3 unclear or met). Rule 4 follows
rule 2, since the map's budget lost nothing on its own.

## Tuning (seeds 101–110)

Budget 72, 144 and 288 with 64 rehearsed samples, and budget 144 with
256. Every setting made the life worse in the same way: barren false hopes
rose from 0.6 to between 14 and 99 per barren world, bush sleeps rose
37–116%, while barren sleeps fell by a quarter to a half and barren meals
roughly tripled. More rehearsal did not help reliably (256: 71 false hopes;
64: 45). The smallest budgets were the worst. **Chosen: budget 144,
rehearsal 64**, the setting with the least harm that meets rule 1. Before
judging, rules 2–4 looked likely to miss.

The shape is the memory gate's (PR #19, PR #23): fewer barren sleeps,
three times the meals, many more false hopes. That is a lead on the gate:
its harm may come from the patterns learning mostly from surprises, not
from the map.

## Results (seeds 1–100 and 111–210, one run, budget 144, rehearsal 64)

`cargo run --release --bin sleep_net -- --judge` prints the verdicts
([judging.txt](judging.txt)) and writes [sleep_net.csv](sleep_net.csv).
About 75 minutes on 4 cores.

| Rule | Result | Verdict |
|---|---|---|
| 1. Patterns taught in the bush world, E vs A ≤ −50% | −59% [−59%, −58%] | **met** |
| 2a. Bush sleeps, E vs A ≤ +10% | +41% [+29%, +54%] | **missed** |
| 2b. Barren sleeps, E vs A ≤ +10% | −52% [−54%, −49%] | **met** |
| 2c. Barren false hopes, E vs A ≤ +10% | +1986% [+1570%, +2505%] | **missed** |
| 2d. Home-test wrong (100 steps), E vs A ≤ +10% | −0% [−6%, +6%] | **met** |
| 3. Barren false hopes, E vs F ≤ −20% | −18% [−38%, +7%] | **unclear** |
| 4a. Bush sleeps, G vs A ≤ +10% | +48% [+36%, +60%] | **missed** |
| 4b. Barren sleeps, G vs A ≤ +10% | −49% [−52%, −47%] | **met** |
| 4c. Barren false hopes, G vs A ≤ +10% | +1358% [+1051%, +1747%] | **missed** |
| 4d. Home-test wrong (100 steps), G vs A ≤ +10% | −2% [−7%, +4%] | **met** |

Means (A, E, F, G): barren false hopes 1.7, 66.3, 75.5, 52.0; bush
sleeps 14.2, 19.4, 15.2, 20.0; barren sleeps 56.3, 28.7, 26.3, 30.6;
barren meals 32.5, 97.1, 102.9, 92.7.

## What it shows

- **The network must not learn only from surprises.** Taught a budget of
  the day's most "surprising" entries, it comes to expect eaten squares
  to grow back where they never do: 66 false hopes per barren world
  against 1.7. Surprises in these worlds are mostly changes (a bush that
  regrew, a square that emptied), so a network fed mostly on changes
  learns that things change. PR #19 found the same with the table.
- **Rehearsal helps a little, not enough.** 64 older samples per sleep
  cut false hopes by 18% against none (unclear on the rule). The kept
  samples are a fair sample of past days, but 64 against 144 surprises
  still leaves the mix tilted toward change.
- **It has the gate's shape.** Fewer barren sleeps (−52%), three times
  the barren meals, many more false hopes, bush sleeps up: the same
  pattern as the memory gate in PRs #19 and #23. That is a strong lead
  for the gate: its harm is likely the patterns seeing mostly surprises,
  not the map. The gate already tallies confirmations to counter this;
  this result suggests the tally does not give the network enough of
  the unsurprising.
- **The map's budget stays harmless** (PR #23); adding it on top (G)
  changes little.

Defaults unchanged: `net_replay` stays `None`. What would make a budget
work is choosing samples to keep the mix fair, not by "surprise": for
example a budget split evenly between changes and non-changes, or
rehearsal sized to match the day's budget.
