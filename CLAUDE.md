# Apprentice AI

Design record: `docs/handoff/` (SPEC.md index, DECISIONS.md, OPEN_QUESTIONS.md).

## Project experts
Before asking the user any design, scope or behaviour question, ask the
expert for the paths involved. Never ask the user first.

| Paths | Expert |
|---|---|
| `**` | `apprentice-ai-expert` |

Call it with `subagent_type: "<expert>"`. **Fallback:** if that type isn't
available (cloud sessions can fail to load agents), read
`.claude/agents/<expert>.md` fresh at call time, spawn a general-purpose Agent
whose prompt is the file's body (everything after the frontmatter) followed by
the question. That counts as the expert. Never answer without it, and never
copy the file's text anywhere.
- ANSWER: act on it; the source is cited.
- ESCALATE: ask the user exactly that block: question, options,
  recommendation. Nothing more.
- When the user answers, pass the answer to the expert to record.
  Don't write DECISIONS.md or remind-me yourself; one writer keeps each
  answer recorded exactly once.
- Non-blocking? Take the expert's recommendation, keep going, and add the
  question to the pack's OPEN_QUESTIONS.md.
