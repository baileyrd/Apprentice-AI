# Network: a mind that learns its own features (world physics)

Until now the apprentice's knowledge has been tables: one belief per square
(the maps), and one belief per square state (the patterns). A table knows
only what it was told about each entry, so nothing it learns about one
square carries to another, and in a new world it knows almost nothing.

This phase starts replacing the tables with a **network with hidden
layers** that finds its own features in what the apprentice sees. It still
learns by **predictive coding**: every connection changes by a local rule
(the error at one end times the activity at the other). Nothing is trained
end to end, and there is no backprop. Backprop appears only as a comparison
baseline, as the pre-registered fallback rule allows.

This slice (after the memory gate, PR #19) is **world physics**: from what it sees now (its 3×3 view) and what
it does (one of six actions), the network predicts what it will see next and
how its energy will change. To do that it must discover, without being
told:

- moving shifts the whole view one square the other way;
- moving into a wall does nothing (a **bump**), so the view stays put;
- eating food leaves the square empty and gives energy; stepping on a
  hazard costs energy.

None of this is about a particular place, so if it is learned it should
work at once in a world the apprentice has never seen. That is the test.

In this slice the network only watches. A body with random actions lives
in the world (it covers every action, including walking into walls and
eating where there is nothing), and the network learns from its life. The
apprentice itself, its maps and its sleep are untouched. Later slices plug
the network into the apprentice, behind the active-memory gate: sleep
consolidation into its weights, curiosity as learning progress, and a
readable concept layer on top.

## Words used here

- **"Surprise"**: prediction error. For one step, the squared difference
  between the predicted view and the view that came, per square, as in
  every phase since phase 1. It is how the idea might show up in code, not
  a claim that the apprentice feels anything.
- **Hidden layer**: units between what it sees and what it predicts. No
  one says what each unit should stand for; whatever they stand for, they
  learned.

## The network

- **In** (42 units, held fixed to the senses): the 3×3 view, one unit per
  square per kind of square (9 × 4), and the action (6).
- **Two hidden layers** of units (sizes tuned on seeds 101–110).
- **Out** (37 units): the next view (9 × 4) and the energy change (in
  units of a meal).
- **Predicting.** Each layer predicts the layer above it from its own
  activity. A forward sweep gives the prediction before the next view
  arrives; that is what "surprise" is measured on.
- **Learning (predictive coding).** When the next view arrives, the
  output is held to it, and the hidden units settle for a few rounds to
  reduce every layer's own prediction error: each unit moves to better
  predict the layer above while staying close to what the layer below
  predicted for it. Then each connection moves by its own layer's error
  times the activity feeding it. All updates are local.

## Compared with

All three learn online from the same life, one step at a time.

1. **Today's map** (phase 1's predictive-coding table, the one every
   memory layer since is built on). In the new world it starts as a blank
   map with the prior it learned in the first world, as the memory does
   when it opens a new map. It predicts the next view by looking up the
   squares it expects to stand among next: one step over for a move,
   unless it believes that square is a wall.
2. **No hidden layer.** The same inputs and outputs connected directly,
   learned by a delta rule. It can learn "after moving east, this square
   tends to look like that", but not "this square looks like its east
   neighbour *if* I moved east and *if* that was not a wall".
3. **Backprop.** The same network, trained by the gradient of the output
   error. Allowed only as a comparison.

## Test

For each seed: a world (as in phase 0, food grows back anywhere) where the
body lives 3,000 steps while each learner learns, then a **new world** from
another seed, never seen, where it lives 1,000 more steps and the learners
keep learning.

Steps where the body ran out of energy and woke at its start square are
left out of every measure (the view jumps somewhere else, which no learner
can foresee from the view alone).

## Rules fixed before any run

Tuning on seeds 101–110 only. Judged once on seeds 1–100 and 111–210, each
comparison paired seed by seed, read from the 90% interval as met, missed
or unclear with `apprentice::judge` (the method of PR #18). **New-world
surprise** is the total "surprise" over the first 200 steps in the new
world (a sum, so it behaves like the counts the judging method was built
for).

1. **It carries its knowledge to a new world.** The network's new-world
   surprise is at least 25% below today's map's (typical change ≤ −25%).
2. **The hidden layers matter.** The network's new-world surprise is at
   least 25% below the no-hidden-layer learner's (≤ −25%).
3. **Local learning keeps up with backprop.** The network's new-world
   surprise is at most 10% above backprop's (≤ +10%).
4. **It grows a unit that knows a bump.** In at least 80% of seeds, some
   single hidden unit separates the moves that bump from the moves that go
   through (area under the curve at least 0.9 over the moves of the first
   1,000 new-world steps, unit read before learning from that step). A
   bump depends on both the action and the square in that direction, so
   such a unit has to combine the two; nothing builds it in.

**No regressions.** This slice adds a module and a test program and
changes no existing code path, so every earlier test gives the numbers it
gave on main; checked by the full test suite.

Reported but not judged: surprise in the first world's last 200 steps,
for all four learners (there the map knows every square, so it should
win), and the error of the energy prediction.

My guess before running: rules 1 and 2 met by a wide margin (around −50%),
rule 3 met, and rule 4 the most likely miss, since a random body bumps on
only about one move in five and the unit that handles it may be spread over
several.
