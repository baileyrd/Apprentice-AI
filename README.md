# Apprentice AI

A small, non-transformer learner that earns its knowledge through its own
trial and error in a grid world. Failure costs time and energy, never
existence: when energy hits zero the apprentice sleeps and wakes at its start
square with everything it has learned intact.

Design sketch: <https://claude.ai/artifact/NGoii1LqBsYDEWPomT7mSQ>

## Status: patterns

The world, the body, the yardstick, a world model that learns, an
apprentice that chooses its own actions from drive and curiosity, a replay
dashboard, and a memory: a diary replayed into per-world maps in sleep, with
consolidation and decay. A mentor speaks in words the apprentice
learns from what it sees, names things, hints when it is stuck, and can set
lessons in easier worlds. A slowest
memory learns in sleep, from every world, how squares change, so a pattern
seen before is anticipated in a new world.
Results: [phase 1](docs/p1/README.md), [phase 2](docs/p2/README.md),
[phase 3](docs/p3/README.md), [phase 4](docs/p4/README.md),
[energy fix](docs/energy/README.md), [mentor](docs/mentor/README.md),
[lessons and naming](docs/lessons/README.md),
[patterns](docs/patterns/README.md).

| Module      | What it holds                                                   |
|-------------|-----------------------------------------------------------------|
| `world`     | Grid of `Empty`/`Wall`/`Food`/`Hazard`; out of bounds is a wall  |
| `body`      | 3×3 view, energy, path-integrated offset; `Move`, `Eat`, `Rest`  |
| `sim`       | Energy rules, food regrowth (anywhere or in place), sleep; pure |
| `agent`     | `Agent` port and two baselines: `random` and `reflex`           |
| `metrics`   | Meals, sleeps, bumps, hurts, mean energy, mean time to food     |
| `model`     | World models: two-layer predictive coding; backprop baseline    |
| `memory`    | Diary over per-world maps; sleep replay, stiffening, decay      |
| `patterns`  | How squares change, by state and age; learned in sleep, any world |
| `apprentice`| The learner: learned valence and volatility, drive + curiosity  |
| `mentor`    | Words about squares: hints when stuck, names; fades; can be blind |
| `lessons`   | A course of worlds, easiest first, moving on at mastery         |
| `adaptation`| Move half the walls or all the food, time the recovery          |
| `relearn`   | World A, then B, then A again: does B erase A?                  |
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
and four minds. `--bushes` makes food grow back in place for the wall test.

```sh
cargo run --release --bin relearn -- --out docs/p4
```

Runs the phase 4 test: world A, then B, then A again, with and without
memory.

```sh
cargo run --release --bin energy -- --out docs/energy
```

Compares sleeps and meals against an agent that sees the real map, and
breaks down the energy fix (meal replay, patience).

```sh
cargo run --release --bin mentor -- --out docs/lessons/naming
```

Lives with no mentor, a mentor that sees the map, and a blind mentor,
with and without naming; counts sleeps and meals per 2,000 steps and what
the mentor's words came to mean.

```sh
cargo run --release --bin lessons -- --out docs/lessons
```

Sits an exam in a hard world after no schooling, after time in hard worlds,
or after the mentor's lessons.

```sh
cargo run --release --bin transfer -- --out docs/patterns
```

Lives three bush worlds then a fresh one, with patterns on and off: does it
anticipate regrowth there, settle faster, and sleep less?

```sh
cargo run --release --bin dashboard -- --out docs/p3/dashboard.html
```

Writes a self-contained page replaying one life: real map against believed
map, surprise and energy, as the walls and then the food move.
`--story return --seed 6` replays world A, then B, then A again with memory
and patterns, showing which map it is using
([docs/patterns/return.html](docs/patterns/return.html)).

## Roadmap

1. **P0** world, body, baselines, step log, checkpoints (done)
2. **P1** predictive-coding world model; surprise curve (done)
3. **P2** action from homeostatic drive + curiosity (done)
4. **P3** dashboard (believed vs real map) and food-move adaptation (done)
5. **P4** episodic diary, sleep replay, consolidation and decay (done)
6. **Mentor** hints when stuck, faded, checked against its own senses (done)
7. **Mentor lessons and naming** words learned from experience; a course of worlds (done)
8. **Patterns** a slowest memory of how squares change, carried across worlds (this)
9. Later: language, multiple apprentices
