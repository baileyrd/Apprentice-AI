# Apprentice AI — spec index
Harvested: 2026-11-03 @ 709f965 · remind-me tag `apprentice_ai` · board project `apprentice_ai`
Design chat: unknown (decisions spread over many chats, voice sessions and PR threads; see DECISIONS.md Source column)
Tags: [stated] [default] [open]. Pointers only — open the listed docs; they are the source.

## Intent
A small, non-transformer learner that earns its knowledge by trial and error in a
grid world. Failure costs time and energy, never existence (D3). Learning is the
point: results should show what the apprentice learned, not only that our
scaffolding helps (D7, D28). README.md has the module map.

## Index

| Subsystem | Docs | Code | Owner | Status | Expert |
|---|---|---|---|---|---|
| World, body, energy | README.md, docs/energy | src/world.rs, body.rs, sim.rs | baileyrd | stable | this one |
| Planner and drives (revisit) | docs/revisit, docs/decline, docs/sweep (deleted fix) | src/apprentice.rs | baileyrd | active | this one |
| Memory: diary, maps, sleep, gate | docs/p4, docs/gate, docs/sleep, docs/sleep-net, docs/fair-mix, docs/rare | src/memory.rs | baileyrd | active | this one |
| Patterns (slowest memory) | docs/patterns, docs/patterns-net, docs/counting, docs/evidence, docs/unlearn | src/patterns.rs, network.rs | baileyrd | active | this one |
| Home test, recognition | docs/home, docs/recognition, docs/excuse, docs/home-extras | src/home.rs, life.rs | baileyrd | active | this one |
| Mentor, language, trust, peers | docs/mentor, docs/lessons, docs/language, docs/trust, docs/peers | src/mentor.rs, peer.rs, lessons.rs | baileyrd | parked (D4) | this one |
| World-physics network | docs/network | src/physics.rs, model.rs | baileyrd | stable | this one |
| Facing (five-box) and retention | docs/facing, docs/retention | src/facing.rs, gru.rs, eprop.rs, retention.rs | baileyrd | closed / active (#707) | this one |
| Judging method | docs/judging | src/judge.rs | baileyrd | stable | this one |
| Early phases | docs/p1, docs/p2, docs/p3 | src/experiment.rs, adaptation.rs, relearn.rs | baileyrd | stable | this one |

## Live work (Mill Board, project `apprentice_ai`)
- #631 review revisit default, due 2026-11-21 · #707 wake/sleep consolidation (retention decayed −0.150)
- #633 local rule that holds a fact across steps (blocked on Q1) · #544 network vs table strangers-for-home
- #444 learning-progress curiosity · #531 network phase (concept layer, network as map prior)

## Seeds
Used: 1–700 (main experiments), 1–160 and 201–210 (facing). Next fresh: 701+ (D14).
