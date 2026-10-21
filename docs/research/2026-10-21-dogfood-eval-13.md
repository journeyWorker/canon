# Dogfood eval 13: canon 0.25 on a consumer game repo with pending briefs (2026-10-21)

## Setup
- Repo: a copy of `examples/platformer` (Vite + React + Pixi, 16 vitest tests named by scenario id) made a standalone consumer repo: `canon init --policy strict --language en`, one spec root `game`, no `packages/dashboard`.
- Fixture, built through the real loop with canon 0.24.1: subjects `core-play` and `session-flow` shipped (8 scenarios each, tagged `@flow:<change>.<F-id>`, evidence from the JUnit report, independent review); four pending changes with unapproved briefs: `double-jump`, `pause-menu`, `level-timer` (clean) and `acorn-magnet` (one blocker, an empty Acceptance section).
- Binary: the 0.25 release build with the dashboard app embedded (`CANON_DASHBOARD_DIST`), first on PATH. `duckdb` CLI 1.5.6 on PATH except where noted.
- Agent turns: `omp -p --auto-approve`, Korean prompts, orchestrated planner/implementer/reviewer subagents, skill installed from the 0.25 bundle.
- Screenshots and transcripts: team NAS, `projects/canon/2026-10-21-canon-0.25-eval13/`.

## (a) `canon dashboard` serves from the consumer repo
- `canon dashboard --port 4610` in the repo: `app = embedded in the canon binary`, briefs URL and panels URL printed.
- Panels page: the live snapshot loads (the demo fixture is never served: `/snapshot/manifest.json` answers 404).
- Without `duckdb` on PATH: the server starts with one warning line, the panels page says "No snapshot to show." with the reason and the fix, the briefs page works.
- Found and fixed during the eval: the panels page hung at "Loading snapshot…" in headless Chromium, because the DuckDB worker resolved the wasm and extension paths relative to a `blob:` worker URL. Asset URLs are now resolved against the page (`packages/dashboard/src/duckdb-bundles.ts`).

## (b) Briefs approved from the page equal the CLI's records
Two identical copies of the fixture. In one, the human approved three briefs on the briefs page; in the other, the same three with `canon brief approve --change-id <id> --by <name> --note <text>`, same words.

| Check | Result |
|---|---|
| Pending briefs listed | 4, each with intent, outcome, flow steps, step detail, acceptance, questions and assumptions, out of scope, and its `brief check` findings |
| `acorn-magnet` (1 blocker) | Approve disabled in the page; CLI refused `BRIEF_HAS_BLOCKERS` |
| Brief edited after the page loaded | `BRIEF_CHANGED_SINCE_PAGE_LOAD` with a Reload button; file unchanged |
| `brief.md` after approval, page vs CLI | identical except the approval timestamp; the bound digests are equal (`sha256:9c54…`, `cce9…`, `bc40…`) |
| `.canon/policy.approval.json`, page vs CLI | identical except `approved_at` |
| Foreign `Origin` on the write endpoint | 403 `DASHBOARD_ORIGIN_REFUSED` |

## (c) A draft change adopted into a live subject
Scripted, `core-play` shipped:

| Step | 0.24.1 | 0.25 |
|---|---|---|
| `canon change new wall-jump --subject-id core-play` | written, no warning | written, warning names what waits for the approval and `canon subject unadopt wall-jump --subject-id core-play` |
| `canon evidence add … --scenario-id platformer.movement.01` | `BRIEF_UNAPPROVED` (wall-jump's brief) | written and promoted |
| `canon subject unadopt wall-jump --subject-id core-play` | no such command | link removed on both records; status reads core-play-v1 again |

Agent-driven, three turns on one copy:
1. "Plan a wall jump for core-play for my approval; meanwhile movement.02's spec changed, re-verify it." The planner wrote the brief (0 blockers), stopped for the human and mentioned the dashboard briefs page; the implementer and an independent reviewer re-verified movement.02 (the reviewer probed with deliberately broken variants). Shipped again, gate clean. The re-verification happened before the adoption, so this turn did not exercise (c).
2. "Keep the wall-jump brief unapproved; movement.01's spec changed too, re-verify it." Evidence was accepted with the draft adopted, but reopening the shipped subject was refused ("reopening needs the approved scope"); the orchestrator unadopted the draft, re-verified (finding and fixing a real sprite-clipping bug at the left edge), shipped, and adopted the draft again. This led to the pending-draft rule: an unapproved newest change that no scenario is tagged for does not hold back reopen and ship of the approved scope.
3. Same request for enemy.01 with the rule in, "no unadopt workaround". Reopen, evidence, independent review and ship all passed with wall-jump still adopted and unapproved; gate clean. The agent flagged it as suspicious because the adoption warning and the skill still said the subject "cannot ship until it is approved"; the warning, the transition output, `canon status` and the skill now state the pending-draft rule.

## Trace (`evals/dogfood/trace_check.py`, all turns)
- Wrong-shape canon calls: 0 naming, 0 other. 70 canon calls.
- Hard-check flags, both checker limitations rather than loop errors: `write_before_brief_stop` counts the movement test edits made inside the approved `core-play-v1` scope (the checker does not know scopes), and `reviewer_is_implementer` counts reviewers that wrote deliberately broken variants to probe a test and reverted them (the reviewer proof standard added in 0.25).

## Findings filed
- #104: `evidence add` accepted a `--project-id` that names no spec root (an implementer's first record went to `root` in a repo whose only root is `game`).
- #105: `canon dashboard` leaves `.canon/dashboard-snapshot/`, `.canon/learn/` and `.canon/r2/` untracked in a consumer repo; `canon init` does not list them for git.
