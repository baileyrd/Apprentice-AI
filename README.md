# Apprentice AI

A small, non-transformer learner that earns its knowledge through its own
trial and error in a grid world. Failure costs time and energy, never
existence: when energy hits zero the apprentice sleeps and wakes at its start
square with everything it has learned intact.

Design sketch: <https://claude.ai/artifact/NGoii1LqBsYDEWPomT7mSQ>

## Status: phase 3

The world, the body, the yardstick, a world model that learns, an
apprentice that chooses its own actions from drive and curiosity, and a
replay dashboard that puts what it believes next to what is there.
Results: [phase 1](docs/p1/README.md), [phase 2](docs/p2/README.md),
[phase 3](docs/p3/README.md).

| Module      | What it holds                                                   |
|-------------|-----------------------------------------------------------------|
| `world`     | Grid of `Empty`/`Wall`/`Food`/`Hazard`; out of bounds is a wall  |
| `body`      | 3×3 view, energy, path-integrated offset; `Move`, `Eat`, `Rest`  |
| `sim`       | Energy rules, food regrowth (anywhere or in place), sleep; pure |
| `agent`     | `Agent` port and two baselines: `random` and `reflex`           |
| `metrics`   | Meals, sleeps, bumps, hurts, mean energy, mean time to food     |
| `model`     | World models: two-layer predictive coding; backprop baseline    |
| `apprentice`| The learner: learned valence and volatility, drive + curiosity  |
| `adaptation`| Move half the walls or all the food, time the recovery          |
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

```sh
cargo run --release --bin adapt -- --change walls --out docs/p2
cargo run --release --bin adapt -- --change food --out docs/p3
```

Runs the adaptation experiments (walls move, or food moves) across 10 seeds
and three minds.

```sh
cargo run --release --bin dashboard -- --out docs/p3/dashboard.html
```

Writes a self-contained page replaying one life: real map against believed
map, surprise and energy, as the walls and then the food move.

## Roadmap

1. **P0** world, body, baselines, step log, checkpoints (done)
2. **P1** predictive-coding world model; surprise curve (done)
3. **P2** action from homeostatic drive + curiosity (done)
4. **P3** dashboard (believed vs real map) and food-move adaptation (this)
5. **P4** episodic diary, sleep replay, consolidation and decay
6. Later: mentor, language, multiple apprentices
