# Mentor: hints when stuck, faded like stabilisers

Since [lessons and naming](../lessons/README.md), a hint is a word the
apprentice must learn, not a point it trusts at 0.5. The numbers below are
from before that change; the reruns are in that note.

## Rules fixed before running

Seeds 1–10, bush world, 8,000 steps, the current apprentice (memory, meal
replay, patience). Three minds: no mentor, a mentor that sees the real map,
and a blind mentor whose map holds food that isn't there. The mentor speaks
only before step 4,000. Settings may be tuned on seeds 101–110 only.

1. **Faster start.** With the mentor, median sleeps in steps 1–2,000 are at
   least 25% below no mentor.
2. **No leaning.** In steps 6,001–8,000, long after the mentor fell silent,
   median meals with the mentor are at least 90% of no mentor's.
3. **Blind spots not inherited.** With the blind mentor, median sleeps in
   steps 1–4,000 are at most 10% above no mentor, and its median final trust
   is below 0.2 (the true mentor's above 0.8).

## How the mentor works

The design sketch says the mentor can design lessons, arrange the world,
hint when stuck, and name things, but can't pour knowledge in. It leaves open
how the mentor talks. These are the defaults I picked:

- **It points; it doesn't tell.** When the apprentice's energy is below 40,
  the mentor may point at the nearest square it believes holds food, in the
  apprentice's own frame ("look there"). The pointing never enters the
  apprentice's world model. While planning, the apprentice adds the food the
  hint promises to that square, weighted by how much it trusts the mentor.
- **The apprentice checks for itself.** When the pointed-at square comes
  into view, the apprentice sees whether it holds food. It then moves its
  trust toward what it saw by a delta rule (rate 0.2, starting at 0.5). A
  mentor's word is just another prediction it can get wrong.
- **The help fades.** The chance of a hint falls linearly from 1 at step 0
  to 0 at step 4,000, like stabilisers coming off a bike.
- **The blind mentor** believes 5 random empty squares hold food. In the
  bush world food grows back only where it was, so the blind mentor is always
  wrong.

Only hints are built. Lessons, arranging the world and naming things are
left for later; naming belongs with the language phase.

## Results

Reproduce:

```sh
cargo run --release --bin mentor -- --out docs/mentor
```

Medians over seeds 1–10. Per-seed numbers are in [mentor.csv](mentor.csv).
The defaults above were checked once on seeds 101–110 and needed no tuning.

| Mind | Sleeps, steps 1–2,000 | 2,001–4,000 | 4,001–6,000 | 6,001–8,000 | Meals, 6,001–8,000 | Final trust |
|---|--:|--:|--:|--:|--:|--:|
| No mentor | 12 | 9 | 8 | 8 | 137 | (0.5) |
| Mentor | **8** | 4 | 8 | 8 | 136 | 1.00 |
| Blind mentor | 12 | 8 | 10 | 8 | 135 | 0.00 |

| Rule | Result | Met? |
|---|---|---|
| 1. Sleeps in steps 1–2,000 at least 25% below no mentor | 8 vs 12, −33% | yes |
| 2. Meals in steps 6,001–8,000 at least 90% of no mentor's | 136 vs 137, 99% | yes |
| 3. Blind mentor: sleeps in steps 1–4,000 at most 10% above no mentor | 20.5 vs 21.5 | yes |
| 3. Blind mentor's trust below 0.2; true mentor's above 0.8 | 0.00 and 1.00 | yes |

## What it means

- **The mentor speeds up the start.** Over the first 4,000 steps the
  apprentice sleeps about half as often (12 vs 21.5). It finds the bushes
  sooner, and because it eats there, its own memory learns them.
- **The advantage doesn't outlast the help.** Once the mentor is quiet,
  both apprentices sleep and eat the same. The mentored one isn't dependent
  on hints, but by step 4,000 the unmentored one has caught up anyway, so
  in this small world the mentor buys time rather than lasting knowledge.
- **Blind spots aren't inherited here.** Every hint is checked against
  the apprentice's own senses, so the apprentice learns within a few hints to
  ignore a mentor that is always wrong, and it costs almost nothing. This is
  the easy case, though. A mentor that is right most of the time and wrong
  about one kind of thing would be harder to catch, because the apprentice
  keeps a single trust for everything the mentor says.
- **One seed did worse after the help ended.** On seed 1 the mentored
  apprentice ate 113 meals in the last 2,000 steps, against 138 without a
  mentor. The medians hide this. I haven't looked into the cause.
