# Peers: several apprentices sharing words

To one apprentice, another apprentice is just another voice. Since PR #16 it
can learn, voice by voice, what a speaker's words mean and how far to believe
them. So the question for this phase: **do two apprentices who share words
settle into a new world faster than one alone?**

## The change

- **An apprentice can speak.** When its companion eats, it says one of the
  two foretelling words about that square: "grows back" when its own
  patterns expect food there again once a bush has had time to regrow (32
  steps), "stays gone" when they don't, and nothing until it has a pattern
  at all. How to speak is built in; *what* it says comes only from what it
  has learned itself, so it can be wrong.
- **The listener learns it like any voice.** Each apprentice hears the other
  in that other's own voice, and learns what those words mean and how far
  to trust them exactly as it does for a mentor. Nothing tells it the
  speaker is an apprentice, or a good one.
- **Twin worlds.** The two live in identical copies of each world (same
  layout, same rules), so they never compete for the same bush and their
  squares line up. Each one sees the other eat and comments on it. A single
  shared world with two bodies is a bigger change, left for later.

## Test

The language test's upbringing (bush and barren worlds in turn, four worlds
of 1,000 steps), then a new world of the kind it saw least recently: a
barren world (2,000 steps) and a bush world (4,000 steps), each from its own
fresh upbringing. Judged on one apprentice, the **newcomer**, in four ways:

1. **Alone.** Raised alone, arrives alone. The baseline.
2. **Arriving together.** Raised together with a sibling, both arrive in the
   new world at the same moment.
3. **An elder ahead.** Raised together; the sibling arrives 1,000 steps
   earlier and has started to learn the place when the newcomer arrives.
4. **A stranger ahead.** Both raised alone, so the newcomer has never heard
   this voice; the stranger arrives 1,000 steps earlier.

Measured as in the language test: **false hopes** (in the first 1,000 steps
of a barren world, how often a square it ate from comes into view empty when
it gave it better than even odds of food) and **sleeps** (times it ran out of
energy) in the bush world, plus sleeps in the barren world.

## Rules fixed before any run

Tuning on seeds 101–110 only; the final check is one run on seeds 1–10.
Medians over seeds. "Within bounds" means no more than 10% or 2 (whichever
is larger) above.

1. **Arriving together helps.** Barren false hopes at least 25% below alone,
   and bush sleeps at least 15% below alone.
2. **An elder who has started helps more.** Barren false hopes at least 50%
   below alone, and bush sleeps at least 25% below alone.
3. **A known voice is worth more than a stranger's.** On false hopes or on
   bush sleeps, the sibling elder is at least 30% below the stranger elder,
   and on the other within bounds of it.
4. **No harm and no regressions.** No condition's barren or bush sleeps
   are out of bounds against alone. Existing tests (language, mentor, home)
   give the same numbers as main.

Trust in the companion's voice is reported, not judged.

My guess before running: rule 1 is the hardest. Two newcomers know the same
things, so each one's words are mostly an echo of the listener's own
mistakes. Words should pay when the speaker knows something the listener
doesn't, which is what rules 2 and 3 check.

## Tuning, disclosed

None. On seeds 101–110 nothing was changed. One thing showed up there: a
sibling's words raised false hopes (0 alone, 26.5 together), because a
sibling raised beside you is wrong at the same moments you are.

## Results (seeds 1–10, one run)

`cargo run --release --bin peers -- --out docs/peers` writes `peers.csv`.

| Newcomer | Trust in companion: raised / after barren / after bush | Barren false hopes | Barren sleeps | Bush sleeps |
|---|--:|--:|--:|--:|
| Alone | – | 27.5 | 57 | 8.5 |
| Arriving together | 0.33 / 0.11 / 0.30 | 21.5 | 57.5 | 11.5 |
| Elder ahead | 0.33 / 0.58 / 0.54 | 24.5 | 56 | 10 |
| Stranger ahead | 0.50 / 0.50 / 0.50 | 27.5 | 57 | 13 |

| Rule | Result | Met? |
|---|---|---|
| 1. Together: false hopes −25%, bush sleeps −15% | −22%; +35% | **no** |
| 2. Elder: false hopes −50%, bush sleeps −25% | −11%; +18% | **no** |
| 3. Sibling elder ≥ 30% below stranger on one measure | −11% hopes, −23% sleeps | **no** |
| 4. No harm, no regressions | language test identical to main (same CSV); no existing code changed; but together (11.5) and stranger (13) bush sleeps are out of bounds against 8.5 | **no** |

## Two checks, not judged

**More seeds.** The same run on seeds 11–40 (`peers-more.csv`), 40 seeds in
all:

| Newcomer | Barren false hopes | Bush sleeps | Bush sleeps better / worse than alone, same seed |
|---|--:|--:|--:|
| Alone | 24 | 11 | – |
| Arriving together | 21.5 | 9 | 16 / 18 |
| Elder ahead | 21.5 | 8 | 21 / 18 |
| Stranger ahead | 24 | 13 | 10 / 21 |

**A longer upbringing**, 3,000 steps per world as in the trust check
(`peers-long.csv`, seeds 1–10):

| Newcomer | Trust: raised / after barren / after bush | Barren false hopes | Bush sleeps |
|---|--:|--:|--:|
| Alone | – | 43.5 | 11.5 |
| Arriving together | 0.04 / 0.11 / 0.04 | 41 | 12.5 |
| Elder ahead | 0.04 / **0.94 / 0.94** | **19** | **7** |
| Stranger ahead | 0.50 / 0.50 / 0.50 | 43.5 | 34 |

## What it shows

- **Two newcomers don't settle faster than one.** Over 40 seeds, arriving
  together is a coin toss against arriving alone. Each sibling's words are
  only what it has learned, and it learned the same things, so in a new
  world it is wrong exactly when the listener is. Raised together, the
  listener learns this: with the longer upbringing its trust in its sibling
  ends near 0 (0.04). That is the right call, not a failure of trust.
- **An elder who has been there is believed, and helps.** With the longer
  upbringing, the newcomer arrives distrusting its sibling (0.04), yet
  within the new world trust climbs to 0.94, because now its sibling's
  words disagree with its own expectations and turn out right. False hopes
  in the barren world fall 56% (43.5 to 19) and bush sleeps 39% (11.5 to 7).
  With the standard 1,000-step upbringing the same direction shows, but
  weakly (trust 0.58, false hopes −11%).
- **Bush sleeps are too noisy to judge on 10 seeds.** Per seed they run from
  2 to 133, and any small change in what an apprentice hears sends its life
  down a different path. A stranger never trusted or distrusted (0.50
  throughout) still moves the median from 8.5 to 13, and in the longer run
  to 34 (one seed's sleeps went from 15 to 27, another's from 35 to 58).
  Rules 1, 3 and 4 failed on this noise as much as on anything learned.
- **Rule 2 asked too much of a short upbringing.** Knowledge passes through
  words only once the listener has heard enough to judge the speaker; the
  same lesson as the trust phase.

So: words between apprentices pay when the speaker knows something the
listener doesn't, and the listener has had time to find that out. They don't
make two beginners wiser together.
