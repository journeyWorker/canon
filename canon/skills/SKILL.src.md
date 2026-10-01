---
name: canon
description: The provider-neutral Canon companion skill for agents using the canon CLI. Routes setup, authoring, validation, storage, ingestion, learning, retrieval, reporting, and provider-specific lazy references.
---

# Canon

Use this single skill when working with Canon in a consumer repository. Canon's CLI remains explicitly modular; choose the command that matches the operation rather than asking an implicit router to guess.

## Setup and provider projection

```bash
canon init
canon skills install                         # detects .claude/.codex; if neither exists, installs both
canon skills install --providers=claude,codex
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics; never deletes user files
```

The canonical bundle is read-only. Claude receives `.claude/skills/canon/SKILL.md`; Codex receives `.codex/skills/canon.md`. Both receive `reference/**` and `scripts/**` sidecars under their `canon` bundle. Load only the reference needed for the current task. `canon/skills-dev` is a separate developer-only legacy source and is installed explicitly with `--source canon/skills-dev`.

## Route by task

- **Start product work / pin a durable unit:** read `reference/canon-subject.md`, then run `canon context` and `canon subject`.
- **Write or reorganize `.feature` specs:** read `reference/canon-authoring.md` before editing; use the scaffold.
- **Understand available fields/enums/policy bindings:** read `reference/canon-context.md` and `reference/canon-vocab.md`.
- **Validate or index specs:** read `reference/canon-fmt.md` and `reference/canon-inventory.md`.
- **Run evidence gates, flip tasks, install hooks:** read `reference/canon-gate.md`.
- **Ingest artifacts or sessions:** read `reference/canon-artifact-ingest.md`, `reference/canon-session-ingest.md`, or `reference/canon-plan-import.md`.
- **Configure storage, policy, plugins, or strategy learning:** read `reference/canon-storage.md`, `reference/canon-policy.md`, `reference/canon-plugins.md`, or `reference/canon-learn.md`.
- **Retrieve dispatch guidance:** read `reference/canon-retrieve.md` and load `scripts/canon-retrieve-pre-dispatch.sh` when wiring hooks.
- **Read generated reports and dashboards:** read `reference/canon-report-dashboard.md`.

## Safety contract

`canon format --check`, `canon skills check`, and `canon gate check` are read-only. Do not hand-edit generated reports, provenance lines, locks, or task evidence. Inspect `canon context` before authoring typed artifacts, and treat every failure class as actionable evidence rather than bypassing the gate.
