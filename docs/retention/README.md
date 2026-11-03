# Retention: does learning to walk erase turning?

Card #707 proposed consolidation between stages, in case learning a new
stage overwrites the last one. The facing track never measured that: each
stage was trained from scratch. This is the cheap check first (Nano,
2026-11-03). If turning holds up, consolidation is dropped and only replay
practice stays on the card.

Marks are pinned here before any run.

## Set-up

One world and one body throughout: the walking five-box world with the
innate heading ring (`Stage::Walking`, 4 motors), so the motors, looks and
percept stay the same across phases and only the experience changes.

- **Learner.** The facing PC learner with context units
  (`Memory::Context`), rate 0.05, unchanged.
- **Phase A, turning (20,000 steps).** The driver never fires the forward
  motor, so it turns and stares in the centre. With the ring this is the
  matching stage, which every learner passed.
- **Phase B, walking (40,000 steps).** The normal walking driver, forward
  included. Its stall at about 0.8 one-ahead is known.
- **Control C (40,000 steps).** From the same end-of-A learner, more
  turning instead of walking.
- **Turning test.** A frozen copy of the learner (no learning, memory
  zeroed at the start) predicts the view after each turn on one fixed
  2,000-step turning-only episode in the centre: the same world copy,
  noise and driver every time it is run. Score = share of turns predicted
  right (nearest look). Run after A, after B, and after C.
- **Weight change.** L2 distance over every weight and bias of the net:
  `|B − A|` (walking), `|C − A|` (control), and `|A − start|` (what
  learning turning took).

## Marks (pinned)

Seeds 1001–1200, paired seed by seed, 90% intervals (`apprentice::judge`).
No tuning: every setting comes from the facing track. Smoke runs on 101–102
only.

1. **Decay (D).** Drop = turning test after A − turning test after B, raw
   accuracy, mean over seeds. **Held** if the interval's top is ≤ 0.05;
   **decayed** if its bottom is > 0.05; unclear otherwise. Why 0.05: the
   facing pass bar is ≥ 95% one-ahead, so a 5-point drop is enough to take
   a passing learner out of it.
2. **Touched (T).** Ratio = `|B − A|` / `|C − A|` per seed, typical value
   as the geometric mean with a 90% interval on its log. **Touched** if the
   interval's bottom is ≥ 2 (walking moved the weights at least twice as
   far as more turning did); **untouched** if its top is < 2; unclear
   otherwise. Nano's point: walking stalls, so it might barely move the
   learner, and then no decay would mean nothing.

Reading, fixed now:

- D held and T touched: turning held up. Drop consolidation from #707.
- D decayed: forgetting is real here; consolidation stays on the card.
- D held and T untouched or unclear: no decay, but walking may not have
  moved it enough to tell. Not evidence either way.
- D unclear: say so; no reading.

Reported only, not judged: control drop (after A − after C), walking's
one-ahead at the end of B, and `|B − A|` / `|A − start|`.

Run: `cargo run --release --bin retention -- --from 1001 --seeds 200`.
