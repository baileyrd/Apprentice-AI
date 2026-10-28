# Energy: why the apprentice ran out, and the fix

Reproduce:

```sh
cargo run --release --bin energy -- --out docs/energy
cargo run --release --bin relearn -- --out docs/energy/relearn
cargo run --release --bin adapt -- --change walls --bushes --out docs/energy/walls-bushes
cargo run --release --bin adapt -- --change walls --out docs/energy/walls
cargo run --release --bin adapt -- --change food --out docs/energy/food
```

All numbers are medians over seeds 1–10, in the bush world (food grows back
in place 30 steps after it is eaten), over 8,000 steps. Per-seed numbers
are in [energy.csv](energy.csv).

## Diagnosis

| Mind | Sleeps | Meals | Mean energy | Steps spent hungry |
|---|--:|--:|--:|--:|
| Sees the real map, walks to the nearest food | **0** | 890 | 88 | 0% |
| Phase 4 apprentice | 134 | 319 | 38 | 69% |

The reference agent shows the world can feed an apprentice without it ever
running out, so the shortfall is in the apprentice. Tracing its beliefs
pointed to two causes:

1. **It remembers bushes as mostly empty.** It holds a food belief of only
   about 10% for bush squares, even for bushes that hold food at that
   moment. The phase 4 memory stores how often a square looked a certain
   way, and the apprentice usually sees a bush just after eating from it
   while the bush grows back.
2. **It undervalues food more than a few steps away.** The planner discounted
   value by 0.9 per step, so food 10 steps away counted for about a third
   of its worth, and checking a nearby unknown square often won.

Curiosity was not the problem. Turning curiosity down made sleeps worse,
because looking around is how it finds bushes that have grown back.

## The fix

- **Meal replay.** When eating feeds it, the apprentice tells its memory.
  In sleep each meal is replayed 5 extra times, as rewarding moments are in
  animal sleep, so a square that fed it is remembered as food. This is still
  a local delta rule on the map, with no gradients.
- **Patience.** The planner's discount went from 0.9 to 0.97 per step.

Both settings were tuned on seeds 101–110 only. The rule below was then
checked once, on seeds 1–10.

## Rule fixed before the final run, and result

**Median sleeps in the bush world are at least halved (≤ 64, from 134), and
no phase 2–4 rule is broken.**

| Mind | Sleeps | Meals | Mean energy | Hungry |
|---|--:|--:|--:|--:|
| Phase 4 apprentice | 134 | 319 | 38 | 69% |
| + patience only | 132 | 334 | 39 | 66% |
| + meal replay only | 63 | 478 | 44 | 58% |
| **+ both** | **38** | 530 | 48 | 52% |

Met: sleeps fell by 72%. Meal replay does most of it. Patience adds little
on its own, but a good deal once the apprentice knows where food is.

## Earlier rules, rerun with the fix

| Rule | Before | After | Holds |
|---|---|---|---|
| P4: second visit to A ≤ half the first | 0.24 | 0.19 | yes |
| P4: ≥ 20% fewer sleeps in the bush world (missed before) | 166 / 132 sleeps | 40 / 34 | **now met** |
| P4: re-map moved walls when fed, ≥ 9/10 | 10/10 | 10/10 | yes |
| P4: phase 2 wall test ≤ 25% slower than 638 | 390 | 204 | yes |

## Side effects

The planner change also affects the apprentice without memory:

- **Phase 2 wall test.** Its median adaptation fell from 638 to 387 steps,
  but it now re-maps the walls in 9 of 10 seeds instead of 10. This test
  never had a pass rule for that mind, but it is a small regression and I'm
  flagging it.
- **Relearning A without memory.** The second visit to A now takes 0.84 of
  the first visit's time, against 0.51 before. Again, no rule applied.

The notes for phases 2–4 and the phase 3 dashboard record the settings of
their time; they were not rerun. The reruns above are in this folder.
