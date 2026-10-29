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
