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
