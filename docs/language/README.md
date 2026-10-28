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

Reproduce:

```sh
cargo run --release --bin foretell -- --out docs/language
```
