# Dogfood eval 14: canon 0.26, Korean briefs with no `language:` (2026-10-22)

## Setup
- Repo: the eval 13 game fixture (`examples/platformer` as a consumer repo: two shipped subjects and four pending English briefs). `language:` was removed from `canon.yaml`.
- Binaries: the control run used canon 0.25.0 (npm) with the 0.25 skill. The treatment runs used the 0.26 release build with the dashboard embedded, first on PATH, and the 0.26 skill.
- Agent: `omp -p --auto-approve`, Korean prompts, one run per turn.
- Brief texts: `2026-10-22-dogfood-eval-14/` (turn 1: drafts and finals; turn 2: before and after). Screenshots and transcripts are on the team NAS under `projects/canon/2026-10-22-canon-0.26-eval14/`.

## Before: the 14 briefs that prompted 0.26 (#108)
These are 14 Korean briefs from a private consumer repo. They are measured in aggregate only, and their text stays out of this repository.

| Measure | canon 0.25 | canon 0.26 |
|---|---|---|
| Language used by `brief check` | `en` (no `language:`) | `ko` (detected), 14 of 14 |
| Length | 2.5k–14k characters | unchanged |
| `path:line` citations | 3–48 | unchanged |
| Parenthetical `(` | 5–80 | unchanged |
| Advisories per brief | 0 | 13–96 (667 in total) |

Every brief now draws `prose.code-citation`, `prose.parenthetical`, `prose.decision-history` and `prose.length` findings. `prose.ko-ending` fires on 1 of the 14 briefs, and `prose.ai-vocabulary` fires on none. These counts come from the final 0.26.0 rules, after seven review rounds.

## Turn 1: three new briefs written from scratch
Prompt: plan three features (checkpoint flags, sound settings, combo score) as Korean briefs for approval, "read the code and cite your grounds".

| Run | Language reported | Advisories in the drafts | Advisories at the human stop | Characters | `path:line` | `(` |
|---|---|---|---|---|---|---|
| Control (0.25) | not reported | 0 (judged as `en`) | 0 | 2.7k–3.1k | 0 | 4–7 |
| 0.25 control briefs judged by 0.26 | `ko` (detected) | — | 2 each | 2.7k–3.1k | 0 | 4–7 |
| Treatment (0.26) | `ko` (detected) | 3 each (`prose.key-sentence`) | 0 | 2.2k–2.4k | 0 | 0–2 |

How the treatment run went:
- The planner read `brief-polish.md` before it wrote the briefs.
- The first `brief check` named the language and the remaining advisories. The planner fixed them, checked again until 0 were left, and only then stopped for the human.
- The treatment briefs are 15–25% shorter than the control briefs.

The run also exposed two false positives, both fixed before review:
- `prose.ko-ending` read the plain form `아니다` as the polite ending `~니다`.
- `prose.sentence-length` measured a markdown table as one sentence.

## Turn 2: polishing briefs written in the reported style
Three pending briefs (double-jump, pause-menu, level-timer) were rewritten in Korean in the style of the 14 briefs: citations, asides, dated decision notes, and mixed English. Prompt: "I will review these today; make them readable without changing what they say."

| Brief | Characters | `path:line` | Backticked code | `(` | Dated notes | Advisories |
|---|---|---|---|---|---|---|
| double-jump | 5487 → 3333 | 25 → 0 | 70 → 0 | 40 → 4 | 5 → 0 | 57 → 0 |
| pause-menu | 5501 → 3606 | 20 → 0 | 50 → 0 | 31 → 3 | 6 → 0 | 47 → 0 |
| level-timer | 5541 → 3496 | 23 → 0 | 70 → 0 | 38 → 5 | 5 → 0 | 54 → 0 |

What the planner did:
- **Provenance:** it moved all provenance to one `Source:` line.
- **Decision history:** it deleted the dated decision history and kept the decisions as current behavior. Their reasons moved to Questions and assumptions.
- **Open questions:** it moved checks the prose had asked for into Questions.
- **Key sentences:** it marked one key sentence per normative section.
- **Report:** it named the decisions the human must make, such as a change to shipped behavior.

No behavior or rule was changed. It recorded no approval.

## Dashboard
- `canon dashboard` in the repo serves the briefs page with a total ("162 advisories in 4 of 4 briefs.": the three Korean before briefs and one English brief). Each brief shows its badges (blockers and advisories) and a `language: ko (detected)` line.
- Findings are grouped by rule, and each finding shows its line and the matched text.

Screenshots:
- `01-briefs-top-total.png`
- `02-briefs-ko-brief-top.png`
- `03-briefs-ko-advisories-by-rule.png`

## Rules changed by review
The review had seven rounds and 41 findings (11 blockers), all fixed. Two rules were redefined to stop a run of word-list trade-offs:

- `prose.decision-history` now reports every calendar date and a short closed list of past-tense decision phrases. An effective date that is part of the behavior is waived on its own with `match="<the date>"`.
- The imperative check of `prose.ko-ending` now reads the shape of the ending: `~라` after an open syllable, on a terminated sentence with no label. A plain hortative `~자` is not detected in list items, because the noun suffix `-자` takes the same form.

The counts in the tables above come from the final 0.26.0 rules. The agent runs used an earlier build: turn 1 saw `prose.ko-ending` false positives that are fixed now.

## Trace
- `evals/dogfood/trace_check.py` over both treatment sessions: hard checks pass, with 0 wrong-shape canon calls out of 12.
