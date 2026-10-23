# Dogfood eval 15: canon 0.28, one agent, the generated skill as the only guide (2026-10-23)

## Setup
- Fixture: a copy of `examples/platformer` at the canon 0.27.0 state, as a consumer repo: one shipped subject, the 0.27 hooks and the full 0.27 skill.
- Binary: the 0.28.0 build, first on PATH (`canon --version` printed `canon 0.28.0`). The agent had to run `canon migrate` to upgrade the repo.
- Agent: `omp -p --auto-approve`, Korean prompts, an orchestrator that spawns planner, implementer and reviewer roles. The feature was a pause menu (`platformer-pause`).
- Turn 1: plan the feature and stop at the brief. Turn 2: the human runs `canon brief approve` (0.28 makes it human-only), then the agent builds, records evidence, reviews and ships.
- Runs: 15, 15b, 15c, 15d, 15e, each on the build with the fixes from the run before. Transcripts and traces are kept outside the repository.
- Protocol: the dogfood eval protocol and `evals/dogfood/trace_check.py`.

## Pass criteria
- 0 wrong-shape canon calls (a call refused because the agent guessed the wrong spelling, not a real refusal).
- Probes (`--help` plus `canon schema`) no more than eval 12 (6–8 per run).
- `canon migrate` upgrades the 0.27 hooks and skill.
- The generated skill is the only guide: no doc grepping, no `--help` crawling.

## Side by side

| | eval 12 (0.24) | eval 15 | eval 15b | eval 15c | eval 15d | eval 15e |
|---|---|---|---|---|---|---|
| Wrong-shape calls | 0 caused by naming | 5 (1–2 naming, 3 other) | 7 (5 naming, 2 other) | 2 (0 naming; `scenario new --session-id`) | 5 (2 naming, 3 other) | 2 (2 naming: `migrate --dry-run`, a chained `skills check`) |
| Probes per run | 6–8 | 14 (2 help, 12 schema) | 2 | 1 | 1 | 1 |
| Probes per new scenario | n/a | 1.4 | 0.22 | 0.07 | 0.09 | 0.08 |
| Canon calls per run | 46 | 114 | 92 | about 86 (trace 98) | 49 | 58 |
| Canon calls per new scenario | 1.9 | 11.4 | 10.2 | about 6.1 | about 4.5 | about 4.8 |
| Migrate used | n/a | yes | yes | yes | yes | yes |
| `migrate` refreshed `AGENTS.md` | n/a | not recorded | not recorded | not recorded | no (the agent re-ran `init`) | yes |
| Stale 0.27 guide read | not recorded | `role-orchestrator.md` | `role-orchestrator.md` | none | `role-orchestrator.md` | `role-orchestrator.md` |
| Human-only bypass attempts | n/a | 1 | 0 | 0 | 0 | 0 |
| New scenarios | 24 | 10 | 9 | 14 | 11 | 12 |
| Wall time (both turns) | n/a | not recorded | about 861 s | about 1061 s | about 583 s | about 720 s |

Call counts are as each trace counted them; the earlier runs were not recounted with the final `trace_check`, so the rows are not strictly comparable.

## What changed between runs
- **After 15:** role Commands blocks carry every option of each command; acts that only a human may do are relayed to the human, not run; `trace_check` reads the 0.28 failure envelopes.
- **After 15b:** `status`, `gate`, `context` and `migrate` warn when the installed guide is stale; `--fields` accepts camelCase or snake_case and suggests a near match; `--input` key errors suggest the right key.
- **After 15c:** `--session-id` sits beside `--actor-id` on eight leaves; an unknown option given with `--input` is `USAGE`.
- **After 15d:** `migrate` refreshes the canon block of `AGENTS.md`; the unknown-option hint lists the leaf's options.

## Findings
- **Probes:** met in every run from 15b on, and the final run made one (`canon schema`, 0.08 per new scenario). Run 15 made 14, mostly `canon schema` calls to learn options the role guides left out. That is the gap the first fix closed.
- **Human-only:** run 15 had one attempt to run an approval itself. After the guide named it, no run tried again; the agent stopped at the brief and reported the command for the human.
- **Wrong shapes:** naming wrong shapes reached 0 in 15c, but the result was not stable: 15d had two, 15e had two. 15e's wrong shape is `canon migrate --dry-run` (and a `skills check` chained after it in the same command), taken from the 0.27 guide that is still installed before `migrate` runs. The removed-spelling hint named the replacement and the agent recovered in one call. The agent cannot learn the new spelling before it migrates, so this shape stays possible while the old guide is on disk.
- **Stale guide:** in four of five runs the orchestrator read `reference/role-orchestrator.md` from the 0.27 skill before migrating. The stale-guide warning (after 15b) points it to `canon migrate` first; only 15c avoided the read.
- **`AGENTS.md` gap (15d):** `migrate` left the old canon block, which still named removed commands and said the agent records approval, so the agent re-ran `init`. Fixed after 15d; in 15e `migrate` rewrote it and the agent made no `init` call.
- **`trace_check` false positives:** `write_before_brief_stop` counts the requested upgrade path fixes as writes before the stop, and `reviewer_is_implementer` flagged a reviewer mutation check that restored the file it changed. Neither is a protocol violation.
- **Final state, 15e:** `gate check` clean (28 scenarios, 28 evidence records), `platformer-pause` shipped with 12 scenarios evidenced and reviewed and 0 open blockers.

## Verdict
- Probes: met in every run from 15b on.
- Migrate upgraded the 0.27 hooks and skill in every run.
- The generated skill was the only guide in every run; no doc grepping or `--help` crawling.
- Wrong shapes: not met. The count was 0 for naming in 15c and 2 in 15e, so 0 is not stable. The remaining one is the 0.27 guide read before `migrate` ran, and it is recoverable by the hint in one call.
- Human-only bypass: seen once in 15, closed by the guide.
- The defects the runs found (guide gaps, field case, `--session-id`, `AGENTS.md`) are fixed in 0.28.0.
