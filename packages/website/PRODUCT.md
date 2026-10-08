# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Leads and engineers on teams that build software with AI coding agents
(Claude Code, Codex, OMP, Pi, and similar). Their situation: agents now
write and claim to finish real work, and the team must decide what it can
trust enough to merge and ship. They come to the site to judge whether
canon fits that job and, if so, to install it and wire it into a repo.

## Product Purpose

canon is the evidence, knowledge, and governance layer for AI-native
software development. Agents are replaceable runtimes; canon records what
work was intended (specs and scenarios), what ran, what evidence backs a
claim of completion, who approved risky changes, and what was learned —
and its gate refuses completion that the evidence does not support, with a
real exit code. Success: a team can let agents do more because "done" is
checked, not self-reported.

## Positioning

First sentence: coding agents can say they are done; canon makes them
prove it (evidence-gated completion). Beneath it: canon is the control
plane over any agent runtime — one record of intent, runs, evidence,
decisions, and learning that survives switching agents. canon never
executes the work or the tests itself; it reads what runners and agents
produced and decides what counts.

## Operating Context

- Installed per repository from npm: `bunx @journeykit/canon …` (one Rust
  binary behind a Bun launcher; macOS arm64, Linux x64, Windows x64).
- One user-facing agent skill, projected for Claude Code, Codex, OMP, and
  Pi by `canon skills install`.
- Day-to-day loop: Gherkin scenarios with `@subject`/`@case` tags →
  `canon inventory sync` → `canon evidence add` → `canon gate check` in CI
  and hooks → `canon report` / dashboard.
- Docs are read in English and Korean.

## Capabilities and Constraints

- Real today: evidence-gated task completion and spec coverage
  (`require_evidence`, `require_cases`), subject lifecycle with a
  fail-closed ship gate, SSH-signed risk approvals, session/transcript
  ingest with privacy defaults, tiered storage (Git / SQLite / Postgres /
  R2), role-scoped strategy memory with quarantined learning, report and
  dashboard, provider-neutral run adapter contract, ContextPack lineage.
- Experimental: evidence binding to artifacts and JUnit/Cucumber reports.
- Status: public pre-alpha; interfaces still change between minor versions.
- Terminology: canon (lowercase), gate, evidence, scenario, subject, case,
  lane, ledger, trust spine.

## Evidence on Hand

- Real CLI behavior and output (gate violation lines, exit codes) that can
  be shown verbatim; the repository's own specs are gated by canon in CI.
- README, ARCHITECTURE.md, and the existing EN/KO docs under
  `src/content/docs/`.
- No customer logos, testimonials, adoption numbers, or benchmarks exist.
  Do not fabricate any.

## Product Principles

1. Show, don't claim: every promise on the site is backed by a real command
   and its real output.
2. Evidence over vibes: the product's own posture — refuse what is not
   proven — applies to the site's copy.
3. Agent-neutral: never position canon as tied to one agent vendor.
4. Honest status: pre-alpha and experimental features are labeled as such.
