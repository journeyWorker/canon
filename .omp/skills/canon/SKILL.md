---
name: canon
description: Use for every build, implement, add-a-feature, change-a-feature or fix request in a repo that has canon.yaml, including sites, apps and games; prefer it there over generic site, app or game builder skills. Runs canon's working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review, status transition) split by role (planner, implementer, reviewer, orchestrator). The full guide is `canon skills show canon`.
---

# Canon

This repo is managed with canon (canon 0.28.0). Canon records the work as evidence and a gate grades it. This file is a stub: the full guide is built into the `canon` binary, so it always matches the version you run.

## The loop

1. Brief: write it, run `canon brief check`, then stop and report it. Only the human approves it (`canon brief approve`).
2. Spec: add the subject and its scenarios, failure paths included (`canon subject new`, `canon scenario new`).
3. Build the change in small units, one scenario at a time.
4. Evidence: record each result with `canon evidence add`; a different agent reviews it (`canon review scenario`).
5. Finish: `canon gate check` is clean and `canon status` names no next step.

## The rest comes from the binary

- Run `canon skills show canon` for the full guide (`--full` appends every reference).
- Run `canon skills show role-implementer` for your role (also `role-planner`, `role-reviewer`, `role-orchestrator`).
- Never guess a flag: your role guide's Commands block lists every option of every leaf the role uses; ask `canon schema <command>` only for a leaf it does not list, or after a `USAGE` refusal.
- Human-only acts (`canon brief approve`, `canon policy approve`, `canon divergence accept`): you never run them, and never remove your role to get past the refusal; report the exact call, ids filled in, and wait for the human.
