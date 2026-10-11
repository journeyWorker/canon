---
name: canon
description: Use for every build, implement, add-a-feature, change-a-feature or fix request in a repo that has canon.yaml, including sites, apps and games; prefer it there over generic site, app or game builder skills. Runs canon's working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review, status transition) and routes to the canon CLI reference for setup, specs, gates, storage, ingestion, retrieval and reports.
---

# Canon

Features in this repo are managed with canon. This skill's job is the
working loop below; the CLI manual lives in `reference/*.md` and
`canon <command> --help`. Start every session with `canon status`: it says
where each subject stands and lists the next commands.

## The working loop

Follow these steps in order for every request that builds, changes or fixes
a feature, a one-line request included. A human is needed once per scope:

- **The approval** (step 1). A new subject, or a change to an approved
  subject's scope, needs the human to approve its brief: the request, your
  assumptions, your open questions. In the same answer the human approves
  the policy, `.canon/policy.yaml` (written by `canon init`), which is what
  the gate grades the work against. When the human answers, record both at
  once: an `Approved: <who> <when> — "<the human's words>"` line in the
  change's `proposal.md`, and
  `canon policy approve --by <the human> --note "<their words>"`. Never
  approve without a human answer, and never edit the policy to make the
  gate pass.
- **Inside an approved scope, never stop for a human.** Fixes, follow-ups,
  review rounds and re-reviews go straight on: add or extend the scenario,
  implement, gather evidence, and get an independent review in another
  session. Independent review happens on every change. A fix or follow-up
  stays in the approved change: add it as tasks in that change's
  `tasks.md`, never with `canon change new`, because a new change is a new
  scope and needs the human's approval.

When no human can answer, write the question into the brief, proceed on the
assumption you state there, and say so in your final report.

1. **Brief.** For a new subject or a scope change, write down the request
   verbatim, your assumptions, and the questions you could not ask. It
   becomes the change's `openspec/changes/<slug>/proposal.md`, which
   `canon change new <slug> --subject <id> --title "…"` scaffolds once the
   subject exists (step 2). Ask the human once for the brief and the
   policy, and record the answer as above. Inside an approved scope there
   is no new brief: go to step 3 under the existing subject and change.
2. **Subject.** `canon subject new <id> --domain <domain> --title "…"`, then
   `canon change new <slug> --subject <id> --title "…"`, and put the brief in
   the change's `proposal.md`.
3. **Scenarios.** `canon feature new <area>.<surface> --title "…"`, then one
   `canon scenario new <area>.<surface>.<nn> --subject <id> --case <case> --title "…"`
   per behavior, with failure paths (`--case failure`) as well as `happy`
   and `edge`. When the gate asks for a missing case (`require_cases`), add
   a new scenario; never retag an existing one to satisfy it. Check the
   corpus with `canon format specs` and index it with `canon inventory sync`.
4. **Units.** Split the work into units (for example specs, domain code,
   assets, review). Give each unit its own actor id and session id, and pass
   them on everything the unit records (`--actor-id <unit> --session-id <session>`),
   so the ledger shows which unit did what. A delegated unit attests its own
   work; never attest another unit's work under your id.
5. **Implement.** Write the code with tests whose names carry the scenario
   id (`game.run.01: …`), so a test report binds to its scenario. Record a
   defect you find in your own work as a finding the moment you find it,
   before review:
   `canon finding add --change-id <slug> --round <n> --seq <n> --severity <s> --reviewer <me> --actor-id <me> --session-id <session> --summary "…"`.
6. **Evidence.** Per scenario:
   `canon evidence add --project-id <root-id> --scenario-id <id> --kind test-run --role implementer --ref "<command>" --actor-id <unit> --session-id <session>`,
   with `--report junit:<path>` or `--artifact <path>` when files prove it.
   canon stores those bytes in `.canon/artifacts/sha256/` (commit it with the
   ledger) and `canon gate check` re-checks them. For a scenario about what
   users see, bind the screenshot or recording that shows it
   (`--artifact <path>`). Then `canon gate promote`.
   An external evaluator's audit is recorded with `canon evidence add --kind
   audit`; canon never generates one.
   Evidence for a plan task (`--task <id>`, before `canon gate task`) must
   bind the artifact or report that proves the task; a canon command
   (`canon status`, `canon gate check`) is never that proof.
7. **Independent review.** A different session from the one that wrote the
   code and evidence reviews the change. The reviewer records every defect
   the review finds as a finding:
   `canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <me> --actor-id <me> --session-id <session> --summary "…"`,
   then `canon gate promote`; once fixed,
   `canon finding close --change-id <slug> --round <n> --seq <n> --disposition fixed --resolution-sha <sha>`
   (a short sha is fine; canon resolves it)
   and `canon gate promote`. Every bug fix adds or extends a scenario. The
   reviewer signs off each scenario:
   `canon review add --project-id <root-id> --scenario-id <id> --reviewer <who> --actor-id <who> --role reviewer --session-id <review-session> --pin <sha> --upstream-ref <ref>`
   (written directly; nothing to promote). A reviewer of user-visible
   behavior attaches what it looked at with `--artifact <path>`
   (repeatable); canon stores and re-checks those bytes and never compares
   images.
8. **Transition.** `canon subject status <id> <state>` along
   `proposed → specced → building → verifying → shipped`. It refuses a move
   the policy does not yet allow (missing evidence, missing reviews, open
   blocker findings); fix the gap rather than overriding, and finish with
   `canon gate check` clean. Move the subject to `shipped` yourself once
   `canon subject status <id> shipped` passes its guard, unless the brief
   says the human ships. `canon status` shows whether each subject's brief
   is approved and names the next step.

Every write command says whether it staged the record (then
`canon gate promote`) or wrote it directly.

## Command reference

Load only the reference the current step needs.

### Setup and provider projection

```bash
canon init                                   # canon.yaml, starter policy (--policy light|standard|strict, default strict; see canon-policy), AGENTS.md block, plans home
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
installed as a native hook. `canon/skills-dev` is a separate developer-only
legacy source and is installed explicitly with `--source canon/skills-dev`.

### Route by task

- **Where the repo stands, subjects and changes (loop steps 1, 2, 8):** read `reference/canon-subject.md`; it covers `canon status`, `canon subject new|adopt|status` and `canon change new`.
- **Write or reorganize `.feature` specs (step 3):** read `reference/canon-authoring.md` before editing; use the scaffold.
- **Understand available fields/enums/policy bindings:** read `reference/canon-context.md` and `reference/canon-vocab.md`.
- **Capture replayable run inputs / version prompts:** read `reference/canon-context.md`, then use `canon context-pack create|show|verify` and `canon prompt register|show` with explicit repository-relative JSON manifests.
- **Validate or index specs:** read `reference/canon-fmt.md` and `reference/canon-inventory.md`.
- **Evidence, gates, task checkboxes, hooks (step 6):** read `reference/canon-gate.md`.
- **Findings and reviews (step 7):** read `reference/canon-review.md`.
- **Ingest artifacts, sessions or plans:** read `reference/canon-artifact-ingest.md`, `reference/canon-session-ingest.md`, or `reference/canon-plan-import.md`.
- **Configure storage, policy, plugins, or strategy learning:** read `reference/canon-storage.md`, `reference/canon-policy.md`, `reference/canon-plugins.md`, or `reference/canon-learn.md`.
- **Retrieve dispatch guidance:** read `reference/canon-retrieve.md` and load `scripts/canon-retrieve-pre-dispatch.sh` when wiring hooks.
- **Read generated reports and dashboards:** read `reference/canon-report-dashboard.md`.

### Safety contract

`canon status`, `canon format <root>`, `canon skills check`, and `canon gate check` are read-only. Do not hand-edit generated reports, provenance lines, locks, or task evidence. Inspect `canon context` before authoring typed artifacts, and treat every failure class as actionable evidence rather than bypassing the gate.
