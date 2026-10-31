# Why did time-unseen and path-counting hurt the home test?

Open question from docs/revisit: revisit + time-unseen + path-counting
(RUS) gave +0.24 [+0.13, +0.35] more wrong answers per home test than
revisit alone (R), on seeds 1–100 and 141–240. Both extras are deleted
(PR #33); this diagnosis runs at their last commit, c16c8bc. Nothing here
changes the apprentice or its defaults.

## Plan, fixed before running

**Arms**, each a whole home test (3,000 steps at home, then five 300-step
trials: home, changed home, three strangers), answers read at 100 steps:

- **R**: today's default (revisit 1/128).
- **RU**: R + time-unseen patterns (`PatternNet::unseen`).
- **RS**: R + path-counting plans (`Drives::sweep`).
- **RUS**: both.

**Seeds.** 241–300 for looking around (mechanism probes; reported only,
never a verdict). 301–500 (200 fresh seeds) for the attribution below,
run once.

**Measures**, per seed: wrong answers (0–5), split into strangers taken
as home (0–3) and home or changed home taken as a stranger (0–2).

**Attribution reading** (raw paired difference against R, 90% interval,
`apprentice::judge`; % change reported next to it):

1. A part **raises wrong answers** if its arm (RU or RS) against R has the
   whole interval above 0.
2. The harm **is mostly strangers taken as home** if, in RUS against R,
   the stranger difference is at least two thirds of the total difference
   (point estimates), and **mostly home taken as a stranger** if that part
   is at least two thirds; otherwise both.
3. Whether RUS reproduces at all on fresh seeds is read the same way as
   (1).

Mechanism probes (what changes in a trial: doubt, map fit, how much of
the view is judged by regrowth, where it walks) are chosen after the
attribution, on seeds 241–300, and reported only.
