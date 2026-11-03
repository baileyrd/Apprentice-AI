# Open questions — apprentice-ai
Claude Code: append here instead of stopping when a question isn't blocking.
Format: `- [ ] Q<n> (<date>, blocking? y/n): <question> — interim choice: <what you did>`
Resolved items are checked and point to the D#/QA entry that answers them.

- [x] Q1 (2026-11-03, y for card #633): Is local learning a principle or just where it started? Nano: "Still deciding" (AUQ 2026-11-03); then "It is both" (project thread 2026-11-03 21:34Z). — resolved: D33 (D2 unchanged).
- [ ] Q2 (2026-11-03, n): Conflict. mem_3b207fe1 says the Excuse::Never default is due for review 2026-11-21; the only review on record (card #631) is for revisit. — interim: treat 2026-11-21 as the revisit review only.
- [ ] Q3 (2026-11-03, n): Conflict. mem_e96be534 item 2 says time-unseen and path-counting stay off by default; PR #33 deleted them. — interim: code wins (D22).
- [ ] Q4 (2026-11-03, n): Conflict. mem_551981d3 says narrower excuse becomes default; PR #37 made Excuse::Never default and #40 deleted the excuse. — interim: code wins (D25).
- [ ] Q5 (2026-11-03, n): Nano left marks-first (D13), the home-test limit (D15), never-tell-the-world (D16) and bush-judges-learning (D24) unlocked. Meant as revisable, or just not confirmed? — interim: [default]; changing one is still a default change under D1 if it alters behaviour.
- [ ] Q6 (2026-11-03, n): Does the network apprentice still take strangers for home more than the table, now the excuse is gone? (card #544, seeds 701+)
- [ ] Q7 (2026-11-03, n): Delete the gate (off, missed marks, last time-unseen split user) or judge it once more on fresh seeds?
- [ ] Q8 (2026-11-03, n): Should barren worlds get learnable structure (clustered or seasonal regrowth) so they test learning?
- [x] Q9 (2026-11-03, n): Retention next step: test per-weight stiffness against the −0.150 decay (card #707)? Not picked. — resolved: tested, all 3 marks met, proposed for #707, off by default → D34
- [x] Q10 (2026-11-03, n): Conflict. D14 lists spent seeds as 1–700 main and 1–160/201–210 facing; docs/retention/README.md (PR #41) also spent 1001–1200 (and smoked on 101–102). — interim: treat 1001–1200 as spent; D32 allocates 701–910 to stiffness. — resolved: spent list now adds 1001–1200 and 1201–1400; 701–1000 fresh → D34
- [ ] Q11 (2026-11-03, y for #633 only if local rules fail): Would Nano sign off a non-local main learner (e.g. GRU+BPTT) for holding a fact across steps, given D27 found local learners fail and GRU+BPTT passes? "It is both" (D33) does not say. — interim: D2/D33 hold; #633 runs local rules only, GRU+BPTT as baseline; no non-local main learner without his sign-off. Nano, told of Q11 (project thread 2026-11-03 22:42Z): "That's a decision I would need to make" — not a yes or no; reserved to him (D35). Still open: only Nano answers it; escalate if local rules miss their marks.
