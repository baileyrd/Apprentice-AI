# Apprentice AI

A small, non-transformer learner that earns its knowledge through its own
trial and error in a grid world. Failure costs time and energy, never
existence: when energy hits zero the apprentice sleeps and wakes at its start
square with everything it has learned intact.

Design sketch: <https://claude.ai/artifact/NGoii1LqBsYDEWPomT7mSQ>

## Status: phase 0

The world, the body and the yardstick. No learning yet.

| Module      | What it holds                                                   |
|-------------|-----------------------------------------------------------------|
| `world`     | Grid of `Empty`/`Wall`/`Food`/`Hazard`; out of bounds is a wall  |
| `body`      | 3×3 egocentric view + energy; actions `Move(dir)`, `Eat`, `Rest` |
| `sim`       | Energy rules, food regrowth, sleep-and-respawn; pure            |
| `agent`     | `Agent` port and two baselines: `random` and `reflex`           |
| `metrics`   | Meals, sleeps, bumps, hurts, mean energy, mean time to food     |
| `rng`       | SplitMix64, so checkpoints replay bit-for-bit                   |

## Run

```sh
cargo run -- --agent reflex --seed 1 --steps 5000 --log runs/reflex.jsonl --save runs/reflex.json
cargo run -- --resume runs/reflex.json --steps 1000
```

Prints a JSON summary. `--log` writes one JSON line per step; `--save` writes
a checkpoint that `--resume` continues exactly.

## Roadmap

1. **P0** world, body, baselines, step log, checkpoints (this)
2. **P1** predictive-coding world model; surprise curve
3. **P2** action from homeostatic drive + curiosity
4. **P3** dashboard (believed vs real map) and adaptation tests
5. **P4** episodic diary, sleep replay, consolidation and decay
6. Later: mentor, language, multiple apprentices
