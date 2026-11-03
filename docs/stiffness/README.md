# Stiffness: can a local per-weight brake stop walking from erasing turning?

Follow-up to `docs/retention` (card #707): walking cut the turning test
from 1.000 to 0.850 (−0.150), and 78% of the misses were shared-weight
interference. This tests the smallest consolidation that fits the facing
learner (Nano, 2026-11-03: stiffness before adding units). Marks are
pinned here before any run.

## Rule

Each weight and bias keeps its own **stiffness** Ω, the sum of the sizes of
its own updates (|Δw|) during phase A, turning. Ω is then frozen. In
phase B, walking, every update to that weight is scaled by
`1 / (1 + λ · Ω / Ω̄)`, where Ω̄ is the mean Ω over the whole net, so λ has
no units. A weight that did much of turning's learning moves less later.
The rule is local: each weight reads only its own history. No backprop and
no replay. λ = 0 is the plain learner, byte-identical.

## Set-up

Same as `docs/retention`: walking world with the ring, PC learner with
context units, rate 0.05. Phase A is 20,000 turning-only steps, phase B is
40,000 walking steps, and the frozen 2,000-step turning test runs after
each. Arms on the same seeds: **P** (plain, λ = 0) and **S** (stiffness, λ
tuned).

## Tuning (disclosed)

λ ∈ {1, 3, 10, 30} on seeds 101–110 only. Pick the λ with the smallest
mean turning drop among those whose walking one-ahead (end of B) is at most
0.02 below P's. If none qualifies, take the smallest λ and say so.

## Marks (pinned)

Judged once on fresh seeds 1201–1400, paired seed by seed, 90% intervals
(`apprentice::judge`). Drop = turning test after A − after B.

1. **Held.** S's mean drop: met if the interval's top is ≤ 0.05.
2. **Halved.** Paired (S drop − ½ · P drop): met if the top is ≤ 0.
3. **No regression.** Paired (P walking one-ahead − S walking one-ahead):
   met if the top is ≤ 0.02. Why: a brake that protects turning by
   stopping walking is not consolidation.

Reading, fixed now:

- 1 and 3 met: stiffness is enough here. Propose it for #707.
- 2 and 3 met, 1 not: it helps but isn't enough. Next is adding units
  (expand).
- 3 missed: it protects turning only by slowing walking. That's a trade,
  not a fix. Report it, and don't propose it.
- Otherwise: say what was met; no proposal.

Reported only: P's drop (should repeat about −0.150), `|B − A|` per arm,
and the share of misses that guess a view the centre never shows.

## Tuning (seeds 101–110), before the judging run

| Strength λ | Turning drop | Walking one-ahead | \|B − A\| |
|---|---|---|---|
| plain | 0.166 | 0.800 | 10.81 |
| 1 | 0.040 | 0.825 | 9.86 |
| 3 | 0.029 | 0.827 | 8.70 |
| **10** | **0.014** | 0.819 | 7.75 |
| 30 | 0.037 | 0.802 | 6.68 |

Every strength keeps walking within 0.02 of plain, and λ = 10 has the
smallest drop, so the rule picks **λ = 10**. That λ is fixed for the
judging run.

Run: `cargo run --release --bin stiffness -- --strength 10 --from 1201 --seeds 200`.
