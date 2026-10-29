# Counting: one piece of evidence per change, not per glance

Found in the language phase ([docs/language](../language/README.md)):
patterns learned from every sighting. A square that stays empty is seen
again and again while it stays empty, so it teaches "stays empty" many
times. A bush that grows back teaches "food again" once, and then its state
ends. So any pattern fed by both kinds of square leaned hard toward empty,
and a mentor's random words were learned as "empty" rather than as noise.

## The change

A square in one state now teaches patterns at most once per age group
(1, 2–3, 4–7, … steps old), plus once whenever it changes. A square empty
for an hour is one piece of evidence per age, not one per glance. Nothing
else changes: same delta rule, same rates, same everything else.

## Rules fixed before any run

Seeds 1–10, one run each, compared with main (PR #14) on the same seeds.
Nothing may be tuned: the change has no settings.

1. **Noise reads as noise.** With the random mentor (language test), both
   words give a square eaten 32–63 steps ago between 0.25 and 0.75 chance of
   food, in at least 8 of 10 seeds. On main: 0 of 10.
2. **Noise does no harm.** With the random mentor, median sleeps in the new
   bush world are no more than 10% above no mentor. On main: 11.5 vs 8.5.
3. **Patterns anticipate.** In the transfer test, the chance of food given
   the first regrown bush is at least 0.2 higher with patterns than without.
   Open since the home test: 0.88 vs 0.74.
4. **No regressions.** Every rule met on main still holds: the home test
   (rules 1–3), the transfer test (anticipation with patterns ≥ 0.5; bush
   sleeps with patterns no more than 10% above without), and language rule 3
   (bush sleeps with the true mentor no more than 10% above no mentor).

Also reported, not judged: the four language rules again, A/B/A savings and
mix-ups, and how long a disproven belief takes to unlearn. My prediction for
unlearning: little change, since a bush world never piled up "food again"
samples; the pile was always on the side of "empty".
