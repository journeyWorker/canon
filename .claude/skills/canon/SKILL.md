---
name: canon
description: The provider-neutral Canon companion skill for agents using the canon CLI. Routes setup, authoring, validation, storage, ingestion, learning, retrieval, reporting, and provider-specific lazy references.
---

# Canon

Use this single skill when working with Canon in a consumer repository. Canon's CLI remains explicitly modular; choose the command that matches the operation rather than asking an implicit router to guess.

## Setup and provider projection

```bash
canon init
canon skills install                         # detects .claude/.agents|.codex/.omp/.pi; if none exists, installs Claude+Codex
canon skills install --providers=claude,codex,omp,pi
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics; never deletes user files
```

The canonical bundle is read-only. Claude receives
`.claude/skills/canon/SKILL.md`; Codex receives
`.agents/skills/canon/SKILL.md`; OMP receives `.omp/skills/canon/SKILL.md`; and
Pi receives `.pi/skills/canon/SKILL.md`. Every selected provider receives
matching `reference/**` and `scripts/**` sidecars under its `canon` bundle. A
legacy `.codex/skills/canon*` projection from canon 0.13.0 or earlier is
migrated by `canon skills install` and reported by `check`/`doctor`. OMP and Pi
projections are project-local passive skill bundles: the
`scripts/canon-retrieve-pre-dispatch.sh` file is available as a sidecar, not
installed as a native hook. Load only the reference needed for the current
task. `canon/skills-dev` is a separate developer-only legacy source and is
installed explicitly with `--source canon/skills-dev`.

## Route by task

- **Start product work / pin a durable unit:** read `reference/canon-subject.md`, then run `canon context` and `canon subject`.
- **Write or reorganize `.feature` specs:** read `reference/canon-authoring.md` before editing; use the scaffold.
- **Understand available fields/enums/policy bindings:** read `reference/canon-context.md` and `reference/canon-vocab.md`.
- **Capture replayable run inputs / version prompts:** read `reference/canon-context.md`, then use `canon context-pack create|show|verify` and `canon prompt register|show` with explicit repository-relative JSON manifests.
- **Validate or index specs:** read `reference/canon-fmt.md` and `reference/canon-inventory.md`.
- **Run evidence gates, flip tasks, install hooks:** read `reference/canon-gate.md`.
- **Finish a subject (claim work done):** read `reference/canon-review.md`. Order: test-run evidence (`canon evidence add`) → independent review of the adopted change (`canon finding add` per issue, fix, `canon finding close --disposition fixed --resolution-sha <sha>`) → `canon review add` per scenario by a reviewer other than the evidence actor → `canon subject status`. Commit each record with `canon gate promote`.
- **Ingest artifacts or sessions:** read `reference/canon-artifact-ingest.md`, `reference/canon-session-ingest.md`, or `reference/canon-plan-import.md`.
- **Configure storage, policy, plugins, or strategy learning:** read `reference/canon-storage.md`, `reference/canon-policy.md`, `reference/canon-plugins.md`, or `reference/canon-learn.md`.
- **Retrieve dispatch guidance:** read `reference/canon-retrieve.md` and load `scripts/canon-retrieve-pre-dispatch.sh` when wiring hooks.
- **Read generated reports and dashboards:** read `reference/canon-report-dashboard.md`.

## Safety contract

`canon format --check`, `canon skills check`, and `canon gate check` are read-only. Do not hand-edit generated reports, provenance lines, locks, or task evidence. Inspect `canon context` before authoring typed artifacts, and treat every failure class as actionable evidence rather than bypassing the gate.
