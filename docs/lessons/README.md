# Mentor lessons and naming

## Rules fixed before running

Seeds 1–10. Settings could be tuned on seeds 101–110 only, and the tuning
runs were seen before these rules were written (noted under each part).

**Naming** (bush world, 8,000 steps, mentor silent from step 4,000):

1. **Words are learned.** With naming, by step 1,000 each of the mentor's four
   words most likely means its own kind of square, in at least 9 of 10 seeds.
2. **Words pay.** With naming, median sleeps in steps 1–2,000 are at least 25%
   below no mentor, and no more than the mentor without naming.
3. **Blind spots, with naming.** With a blind mentor that also names things,
   median sleeps in steps 1–4,000 are at most 10% above no mentor.
4. **Mentor phase rules still hold** for the mentor without naming, now that
   its pointing is a word to be learned: at least 25% fewer early sleeps;
   at least 90% of meals in steps 6,001–8,000; blind mentor at most 10% more
   sleeps in steps 1–4,000; the food word ends below 0.2 food for the blind
   mentor and above 0.8 for the true one.

**Lessons** (exam: a fresh hard world, 4,000 steps):

5. **Lessons prepare.** Median exam sleeps after the course are at least 20%
   below both no schooling and the same number of steps spent in hard worlds.
6. **Within reach.** Every lesson is mastered before its cap in at least 8 of
   10 seeds.

What the tuning seeds showed before these were fixed: naming learned all four
words in 10/10; the mentor without naming got 23% fewer early sleeps (so
rule 4 may now miss); blind + naming was 10% above no mentor (rule 3 at the
edge); lessons gave 62 exam sleeps against 62 with no schooling and 69 after
hard worlds (rule 5 likely to miss). I kept the rules where the design sketch
puts the bar rather than moving them to fit.

## How it works

The design sketch says the mentor can design lessons, arrange the world so
the next challenge is just within reach, and name things the apprentice has
already experienced. It doesn't say how naming works, what a lesson is, or
what counts as being within reach. These are the defaults I picked:

- **Words.** Everything the mentor says is now a word about a square. Its
  words are arbitrary symbols, shuffled per mentor, and mean nothing to the
  apprentice at first. When a square it was told about comes into view, the
  apprentice moves that word's meaning (a belief over the four kinds of
  square) toward what it sees, by a delta rule. While planning, a told square
  gains the food its word means. Told words never enter the world model.
  This replaces the mentor phase's built-in "a point means food, trusted
  0.5". Pointing at food is now the mentor saying its word for food, and that
  word has to be learned like any other.
- **Naming.** When the apprentice isn't stuck, a naming mentor says the
  right word for one of the 9 squares the apprentice is looking at. It
  speaks with the same fading chance as hints, so it is silent from step
  4,000.
- **Lessons.** A course is a list of worlds, easiest first: 6×6 with no
  hazards, then 9×9 with some, both with 5 bushes. A lesson is mastered after
  300 steps without running out of energy. The apprentice then moves on, or
  is moved on after a cap of 3,000 steps. The exam is a fresh hard world:
  12×12, 3 bushes, hazards on 15% of squares. Only the mentor picks the
  world; it never touches the apprentice's mind.

Reproduce:

```sh
cargo run --release --bin mentor -- --out docs/lessons/naming
cargo run --release --bin lessons -- --out docs/lessons
```

Per-seed numbers are in [naming/mentor.csv](naming/mentor.csv) and
[lessons.csv](lessons.csv). All numbers are medians over seeds 1–10.

## Naming: results

| Mind | Sleeps, steps 1–2,000 | 2,001–4,000 | 1–4,000 | Meals, 6,001–8,000 | Food word means food | All words known by 1,000 |
|---|--:|--:|--:|--:|--:|--:|
| No mentor | 12 | 9 | 21.5 | 137 | – | – |
| Mentor | 10 | 5 | 14.5 | 134 | 1.00 | 0/10 |
| Mentor + naming | 11 | **2** | **13.0** | 132 | 1.00 | **10/10** |
| Blind mentor | 13 | 10 | 22.5 | 132 | 0.00 | 0/10 |
| Blind + naming | 14 | 9 | 22.5 | 136 | 0.03 | 2/10 |

| Rule | Result | Met? |
|---|---|---|
| 1. All four words learned by step 1,000, ≥ 9/10 | 10/10 | yes |
| 2. Naming: early sleeps ≥ 25% below no mentor | 11 vs 12, −8% | **no** |
| 2. … and no more than the mentor without naming | 11 vs 10 | **no** |
| 3. Blind + naming: sleeps 1–4,000 ≤ 10% above no mentor | 22.5 vs 21.5, +5% | yes |
| 4. Mentor without naming: early sleeps ≥ 25% below no mentor | 10 vs 12, −17% (was −33%) | **no** |
| 4. Late meals ≥ 90% of no mentor's | 134 vs 137, 98% | yes |
| 4. Blind mentor: sleeps 1–4,000 ≤ 10% above no mentor | 22.5 vs 21.5, +5% | yes |
| 4. Food word: blind < 0.2, true > 0.8 | 0.00 and 1.00 | yes |

What it means:

- **The apprentice learns words from experience.** With naming it learns
  all four words in every seed by step 1,000, only from seeing what the
  mentor names.
- **Learning words costs the first 2,000 steps.** Pointing used to carry a
  built-in meaning. Now its word has to be learned first, so the mentor helps
  less at the start: 17% fewer early sleeps, against 33% in the mentor phase.
  This is a regression on a rule that passed before.
- **The help moved later, not away.** Over steps 1–4,000 the mentor still
  cuts sleeps by a third, and naming by 40% (13 against 21.5). Most of the
  gain comes in steps 2,001–4,000 (2 sleeps with naming, against 9 with no
  mentor), once the words mean something. The rule only looked at the first
  2,000 steps.
- **Naming didn't make a blind mentor dangerous.** A mentor that is right
  about names can still end up believed when it is wrong about where food is.
  Here it didn't happen: the blind mentor's food word ended up meaning "empty
  floor". Most likely its hints far outnumber the times food is in view to
  be named; I haven't checked that directly. The flip side: in 8 of 10 seeds that apprentice also never learned
  what the blind mentor's food word really means.
- **One meaning per word.** The apprentice keeps a single meaning per word,
  so it can't tell "this mentor's word for food is wrong" from "this mentor
  is wrong about where food is". A mentor that is mostly right would be
  harder to catch.

## Lessons: results

| Mind | Exam sleeps | Exam meals | Hazards stepped on | Steps before exam |
|---|--:|--:|--:|--:|
| No schooling | 75.5 | 148 | 19 | 0 |
| Same steps in hard worlds | 72.0 | 154 | 11 | 732 |
| Lessons | 74.5 | 150 | 16 | 732 |

| Rule | Result | Met? |
|---|---|---|
| 5. Exam sleeps after lessons ≥ 20% below both others | 74.5 vs 75.5 and 72 | **no** |
| 6. Every lesson mastered, ≥ 8/10 | 10/10 | yes |

What it means:

- **Lessons don't transfer, because there is almost nothing to carry over.**
  What this apprentice knows is mostly a map of one world. Between worlds it
  carries only about a dozen numbers: what each kind of square is worth, how
  fast worlds change, and a prior over kinds of square. It learns those
  within a few hundred steps in any world, so an easy start buys it nothing.
- **Hard worlds taught hazards better.** The apprentice from hard worlds
  stepped on 11 hazards in the exam, against 16 after lessons. The lessons
  were safe, so they taught little about hazards.
- **For lessons to help, the apprentice needs knowledge that generalises:**
  for example "food grows in clusters" or "hazards sit near walls", rather
  than "food is at (3, 4)". That is a change to the learner, not to the
  mentor, and I haven't made it.
