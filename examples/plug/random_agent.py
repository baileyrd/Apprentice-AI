"""A plug-in agent for the apprentice's world, in plain Python (stdlib only).

Run: cargo run --release --bin plug -- python3 examples/plug/random_agent.py

It reads one JSON message per line on stdin and answers each "sense" with one
line on stdout (see docs/port/README.md). This one eats food underfoot,
otherwise walks at random, and never names a place, so it has no home-test
answer. Its seed is its last argument; it must be deterministic given it.
"""

import json
import random
import sys

ACTIONS = ["North", "East", "South", "West", "Eat", "Rest"]
CENTER = 4  # the agent's own square in the 3x3 view


def choose(sense: dict, rng: random.Random) -> str:
    """The action for one view."""
    if sense["view"][CENTER] == "Food":
        return "Eat"
    return rng.choice(ACTIONS)


def main() -> None:
    rng = random.Random(int(sys.argv[-1]))
    for line in sys.stdin:
        msg = json.loads(line)
        if "sense" in msg:
            print(json.dumps({"act": choose(msg["sense"], rng)}), flush=True)
        # "felt" and "heard" carry what it could learn from; it ignores them.


if __name__ == "__main__":
    main()
