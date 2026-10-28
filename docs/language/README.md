# Language: words for what a square will become

Nano's pick for the language phase. Until now the mentor's words named what
a square is. Now it also has two words for what a square will do: one for
"food grows back here", one for "it won't". It says one of them about a
square right after the apprentice eats there. Like every word, they mean
nothing to the apprentice at first.

The question: patterns are shared by every world, so they cannot tell a
world where eaten bushes grow back from one where they don't. Can a learned
word settle that, square by square, in a world it has never seen? The word
never says which world it is in; it says what one square will do.

## How it works

- A word said about a square joins that square's state (how it last changed
  and since when) until the square next looks different.
- In sleep, each replayed sighting of such a square teaches two patterns: the
  general one, as before, and one keyed on the word. So a word comes to mean
  what the squares it was said about went on to become.
- When the word's pattern has enough samples, it answers for that square
  before the general pattern. The general pattern learns as if no word had
  been said, so words add knowledge and take none away.
- Nothing is built in: which word is which is learned only from what the
  apprentice later sees. No new learning rule: the same delta rule as patterns.

## Setup

- **Upbringing**, 4,000 steps: four 12×12 worlds of 1,000 steps each,
  alternating a bush world (an eaten bush grows back in place 30 steps
  later) and a barren world (an eaten bush never grows back; new food springs
  up on a random empty square), starting with a bush world.
- **Minds**: memory with patterns (as in the unlearning test), raised and
  tested with no mentor; with a mentor that says the true word after every
  meal; or with a mentor that says either word at random. The mentor never
  fades.
- **Tests**, each from a fresh copy of the raised apprentice: 2,000 steps in a
  new barren world, and 4,000 steps in a new bush world.
- A **futile return** is trying to eat on a square where it ate before in the
  same world, and finding nothing.

## Rules fixed before any run

Settings may be tuned on seeds 101–110; the final check is one run on seeds
1–10.

1. **Words are learned.** With the true mentor, after the upbringing, the
   "grows back" word gives a square it ate from 32–63 steps ago at least 0.8
   chance of food, and the "stays gone" word at most 0.2, in at least 9 of 10
   seeds.
2. **Words pay.** In the new barren world, median futile returns in the first
   1,000 steps are at least 50% below no mentor.
3. **No harm.** In the new bush world, median sleeps in the 4,000 steps are no
   more than 10% above no mentor.
4. **Noise is ignored.** With the random mentor, median futile returns in the
   new barren world's first 1,000 steps are no more than 10% above no mentor.

One change from the proposal: rule 4 was "a mentor with the two words
swapped". Words are arbitrary symbols, so a mentor that always swaps them is
not lying, it just has a different vocabulary, and the apprentice would learn
it just as well. A mentor whose words carry no information is the real test
of whether bad words get ignored.

## Tuning, disclosed

On seeds 101–110, before the final run, I found two things and changed one
setting. The rules above are unchanged.

- **The barren test had nothing to fix.** Patterns learn by a delta rule, so
  the general pattern mostly says what the last world taught. With the
  upbringing as first planned (ending in a barren world), the general pattern
  already gave an eaten square no chance of food when it reached the new
  barren world, with or without words. **Change:** each test now gets its own
  upbringing, ending in the other kind of world: bush, barren, bush, barren
  before the bush test (as planned), and barren, bush, barren, bush before
  the barren test. So in both tests, what it last learned is wrong. Rule 1 is
  read after the first of these, as planned.
- **Most futile returns are not hopes.** Trying to eat on an empty square
  costs the same as resting, so the apprentice often "eats" where it stands
  while idle. Nearly all futile returns happened when it gave the square no
  chance of food. Rule 2 still counts them, as written, but I added one
  diagnostic, not a rule: **false hopes**, the times a square it ate from
  comes into view empty when it gave that square more than even odds of food
  just before. That is the mistake a word should prevent.

Reproduce:

```sh
cargo run --release --bin foretell -- --out docs/language
```

Per-seed numbers are in [foretell.csv](foretell.csv). All numbers are
medians over seeds 1–10, one run.

## Results

| Mind | Words learned | Barren: futile returns | Barren: false hopes | Barren: meals | Bush: sleeps | Bush: meals |
|---|--:|--:|--:|--:|--:|--:|
| No mentor | – | 29 | 27.5 | 23 | 8.5 | 269 |
| True mentor | 7/10 | 31 | **0** | 28.5 | **5** | 275 |
| Random mentor | 0/10 | 38.5 | 4 | 29.5 | 11.5 | 266 |

Barren columns are the first 1,000 steps of 2,000 (meals over all 2,000);
bush columns are 4,000 steps.

| Rule | Result | Met? |
|---|---|---|
| 1. Words learned in ≥ 9/10 | 7/10. "Stays gone" ≤ 0.03 in all 10; "grows back" 0.49, 0.51, 0.74 in seeds 8, 10, 1 | **no** |
| 2. Futile returns ≥ 50% below no mentor | 31 vs 29, +7% | **no** |
| 3. Bush sleeps ≤ 10% above no mentor | 5 vs 8.5, −41% | yes |
| 4. Random mentor's futile returns ≤ 10% above | 38.5 vs 29, +33% | **no** |

## What it shows

- **The words do what they were for.** Raised by the true mentor, the
  apprentice had no false hopes in the new barren world in any seed (one
  seed had one), against a median of 27.5 without words. And in the new bush
  world, where its general pattern had just learned that eaten food stays
  gone, the "grows back" word sent it back to bushes: 41% fewer sleeps.
- **Rule 2 measured the wrong thing.** Futile returns barely moved because
  nearly all of them are idle "eating" on an empty square, which costs the
  same as resting. Fixing that tie is outside this phase, so the rule
  misses. This also explains the "aimless futile eating" left open after the
  unlearning test.
- **"Grows back" is a softer truth than "stays gone".** A bush doesn't
  regrow while the apprentice stands on it, so a square it was told grows
  back can still look empty at 32–63 steps. In 3 seeds that pulled the
  meaning to 0.5–0.74.
- **Noise is not ignored: it is learned as "empty".** The random mentor's
  words came to mean nearly no food. Why: a barren square gives an "empty"
  sample every time it is seen while it stays eaten, but a bush gives one
  "food" sample when it regrows, and then its state (and word) ends. So any
  pattern fed by both kinds of square leans hard toward empty. With the
  random mentor that sent 3 seeds into long starvation in the bush world
  (73–109 sleeps), because its words overrode the general pattern. The
  same lean explains why the general pattern reads near 0 after every
  upbringing here. Counting one sample per change instead of one per
  sighting would remove it; that is a change to patterns, so it's not done
  here.
- **Earlier results shift slightly.** Hint and naming words now also teach
  word patterns. On seeds 1–10 the mentor experiment moves by at most 1.5
  sleeps over steps 1–4,000 (blind mentor 8 → 9.5).

## Is the upbringing too short?

Nano's question after the run: the home test needed 3,000 steps to learn one
world, so are 1,000 steps per world enough here? Same seeds 1–10, each
upbringing world lasting 1,000, 3,000 or 9,000 steps (4,000, 12,000 or
36,000 steps in all). This was run after the results above, so it is a
check, not a judged run.

| Steps per world | Words learned | Barren false hopes, none / true | Barren futile returns, none / true / random | Bush sleeps, none / true / random |
|--:|--:|--:|--:|--:|
| 1,000 | 7/10 | 27.5 / 0 | 29 / 31 / 38.5 | 8.5 / 5 / 11.5 |
| 3,000 | 8/10 | 43.5 / 0 | 24.5 / 16 / 17 | 11.5 / 3 / 22 |
| 9,000 | 8/10 | 39 / 0 | 13.5 / 18.5 / 19 | 10 / 2.5 / 11.5 |

More time sharpens the gain where the words work: the bush-world benefit
grows from −41% to −75%, and false hopes stay at 0. It does not fix the
misses. "Grows back" stays near 0.5 in two seeds even after 36,000 steps,
futile returns swing both ways with no trend, and the random mentor still
does harm. Those come from how the world and patterns work (idle eating
costs the same as resting; a bush doesn't regrow under it; an unchanged
square is sampled on every sighting), not from too little learning.

```sh
cargo run --release --bin foretell -- --world-steps 9000 --out runs/foretell-9k
```
