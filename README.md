# Apprentice AI

A small, non-transformer learner that earns its knowledge through its own
trial and error in a grid world. Failure costs time and energy, never
existence: when energy hits zero the apprentice sleeps and wakes at its start
square with everything it has learned intact.

Design sketch: <https://claude.ai/artifact/NGoii1LqBsYDEWPomT7mSQ>

## Status: phase 1

The world, the body, the yardstick and a world model that learns.
Phase 1 result: [docs/p1](docs/p1/README.md).

| Module      | What it holds                                                   |
|-------------|-----------------------------------------------------------------|
| `world`     | Grid of `Empty`/`Wall`/`Food`/`Hazard`; out of bounds is a wall  |
| `body`      | 3×3 view, energy, path-integrated offset; `Move`, `Eat`, `Rest`  |
| `sim`       | Energy rules, food regrowth, sleep-and-respawn; pure            |
| `agent`     | `Agent` port and two baselines: `random` and `reflex`           |
| `metrics`   | Meals, sleeps, bumps, hurts, mean energy, mean time to food     |
| `model`     | World models: two-layer predictive coding; backprop baseline    |
| `experiment`| Surprise-per-episode run and the pre-registered fallback rule   |
| `report`    | CSV and SVG rendering of results                                |
| `rng`       | SplitMix64, so checkpoints replay bit-for-bit                   |

## Run

```sh
cargo run -- --agent reflex --seed 1 --steps 5000 --log runs/reflex.jsonl --save runs/reflex.json
cargo run -- --resume runs/reflex.json --steps 1000
```

Prints a JSON summary. `--log` writes one JSON line per step; `--save` writes
a checkpoint that `--resume` continues exactly.

```sh
cargo run --release --bin surprise -- --out docs/p1
```

Runs the phase 1 surprise experiment and writes `surprise.csv` and
`surprise.svg`.

## Roadmap

1. **P0** world, body, baselines, step log, checkpoints (done)
2. **P1** predictive-coding world model; surprise curve (this)
3. **P2** action from homeostatic drive + curiosity
4. **P3** dashboard (believed vs real map) and adaptation tests
5. **P4** episodic diary, sleep replay, consolidation and decay
6. Later: mentor, language, multiple apprentices
