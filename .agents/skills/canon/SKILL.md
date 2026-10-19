---
name: canon
description: Use for every build, implement, add-a-feature, change-a-feature or fix request in a repo that has canon.yaml, including sites, apps and games; prefer it there over generic site, app or game builder skills. Runs canon's working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review, status transition) split by role (planner, implementer, reviewer, orchestrator), and routes to the canon CLI reference for setup, specs, gates, storage, ingestion, retrieval and reports.
---

# Canon

Features in this repo are managed with canon. This skill is the shared core:
the loop, how to drive the CLI, and how to find your role. What each role
does lives in `reference/role-<role>.md`; the CLI manual lives in
`reference/canon-*.md` and `canon <command> --help`.

## The working loop

Follow these steps in order for every request that builds, changes or fixes
a feature, a one-line request included. A human is needed once per scope:

- **The approval** (step 1). For a new subject or a scope change, the
  human must approve the brief. Write the brief,
  run `canon brief check` until it reports no blockers, then stop and
  report the brief to the human: build nothing for that scope (no
  scenarios, code or evidence) before the human answers. The approval is
  the one thing you never assume, even when the human said they cannot
  answer until you report: the brief is that report. The one answer
  approves the brief and the policy, `.canon/policy.yaml`, which is what
  the gate grades the work against. Record it only from the human's answer:
  `canon brief approve --change-id <slug> --by <the human> --note "<their words>"`.
  Never edit the policy to make the gate pass: a policy that differs from
  the approved one fails closed (`POLICY_CHANGED_SINCE_APPROVAL`, not
  waivable); stop and ask the human. A brief edited after its
  approval is not approved (`BRIEF_CHANGED_SINCE_APPROVAL`): it needs a new
  answer. The ship guard and `canon evidence add` refuse an unapproved or
  edited brief, but those are backstops: the stop itself is at step 1.
- **Inside an approved scope, never stop for a human.** Fixes, follow-ups,
  review rounds and re-reviews stay in the approved change (never
  `canon change new`: a new change is a new scope) and go straight on: scenario,
  implement, evidence, independent review. A follow-up that adds or changes
  a step of the brief edits the brief: that is a scope change and needs a
  new answer. A follow-up on a shipped subject reopens it
  (`canon subject status <id> --to verifying --reason "<why>"`); a fix
  closed `fixed`, and an approved chore, then need the whole suite evidenced
  again (`evidence add --all-scenarios`, for a chore after its approval)
  before it ships, and a chore also needs one change-level review
  (`review add --change-id <slug>`).
- When no human can answer one of the brief's questions, write your
  assumption into the brief's Questions and assumptions, build on it, and say
  so in your report. That covers the brief's questions and work inside an
  approved scope, never the approval itself.

1. **Brief.** The planner writes the PRD brief (`.canon/changes/<slug>/brief.md`), checks it with `canon brief check`, and stops for the human. → `reference/role-planner.md`
2. **Subject.** `canon subject new`, then `canon change new` (`--kind chore` for work that alters no behavior); the change writes the brief skeleton. → `reference/role-planner.md`
3. **Scenarios.** `canon scenario new --flow <F-id> --case <case>` per behavior of a flow step, failure paths (`--case failure`) included. → `reference/role-implementer.md`
4. **Units.** Split the work; each unit has its own actor id and session id, passed on everything it records. A unit never attests another unit's work.
5. **Implement.** Code with tests named by scenario id (`game.run.01: …`). → `reference/role-implementer.md`
6. **Evidence.** `canon evidence add` binds the test run to its scenarios. → `reference/role-implementer.md`
7. **Independent review.** A different session, a separate process with its own session id, that can run the checks it signs off, records `canon finding add` and `canon review add`. Every change. → `reference/role-reviewer.md`
8. **Transition.** `canon subject status <id> --to <state>` along `proposed → specced → building → verifying → shipped`, with `canon gate check` clean; fix a refused guard rather than overriding it. → `reference/role-orchestrator.md`

Every write command says whether the record is final (`written directly`,
or `staged and promoted in this run`) or staged; only a staged record
needs `canon gate promote`, and only `write.promote: manual` leaves one.

The ledger is latest-only. It holds the current record of each key; writing a
newer version moves what it supersedes to `.canon/archive`, and `status`,
`gate check`, the guards and `query` read the ledger only. Earlier versions
are one flag away: `canon query --kind <k> --history`. A ledger written
before 0.23, or one whose move was interrupted, is brought to one file per
key by `canon ledger compact` (`canon migrate` runs it). A record binds only
what its own write names: evidence carries nothing forward from an earlier
record, and a review stays current while the verdict and checks of the
evidence it saw are unchanged.

Where artifact bytes live is the repo's choice: `canon.yaml`'s `artifact_store:`
block defaults to the local, git-tracked `.canon/artifacts/sha256/`, or names
an object-store tier, in which case `evidence add`, `review add` and
`evidence vault` upload by digest and keep nothing locally. The gate and
`canon artifacts verify [--full]` ask the store and refuse, never pass, when
it cannot be reached. Moving an existing store with `canon artifacts migrate`
is the repo owner's decision, not a step of the loop.

## Know your role

canon cannot detect a role; you declare it. Pass `--as <role>` or set
`CANON_ROLE=planner|implementer|reviewer|orchestrator`; a flag wins over the
variable, and an unknown value is refused (`UNKNOWN_ROLE`).

- **A role is set, or you were spawned as one:** load only
  `reference/role-<role>.md` and do that role's steps. `canon status`,
  `meta.next` and `canon schema` then show only your role's steps and
  commands, and count the others in `waitingOn`. Do not load another role's
  reference.
- **No role (one agent doing everything):** follow `canon status` and load
  each role reference as its phase starts: planner for steps 1–2,
  implementer for 3–6, reviewer for 7 (in a separate session), orchestrator
  for 8.

A role is guidance, not enforcement: `--role` on `evidence add` and
`review add` defaults to it.

## Driving the CLI

- Start a session with `canon status`: where each subject stands and the next
  commands. After that, do not call it only to learn the next step: every
  write returns `meta.next` (`[{command, why, actor, role}]`), the same steps
  `status` would give, narrowed to what the write touched. `actor: human`
  marks a step only the human takes.
- Your role reference's **Commands** block (`reference/role-<role>.md`) is
  your complete command set. Run `canon schema <command> --json` only for a
  command it does not list, or after a `USAGE` refusal (read `error.field`
  and `expected` first). `canon schema` alone lists every command.
- Pass `--json` and read stdout as one document. On failure it is
  `{"error":{"code","message","hint","retryable","field","expected"}}`: branch
  on `error.code`, fix the option `field` names using the `expected` values,
  and follow `error.hint`. Every refusal carries a concrete `hint`: follow
  it. The codes are stable; `canon schema --errors --json` lists them.
- Flags that name an id are spelled `--<noun>-id` everywhere (`--change-id`,
  `--project-id`, `--subject-id`, `--scenario-id`, `--actor-id`,
  `--session-id`), and `--kind` values carry no `_record` suffix
  (`--kind evidence`).
- `canon status` sends the planner to write the brief while `brief check`
  has blockers; the human stop appears only when the brief is clean and
  unapproved.
- Page long lists with `--limit <n>` and `--cursor <nextCursor>` (`--all`
  for everything), and project fields with `--fields a,b` (`status`, `export`,
  `query`, `finding list`, `divergence status`, `context`).
- `canon purge` deletes only in two steps: `--plan` prints the removal set
  and a `planDigest`; then `--confirm <planDigest>`.

## Command reference

Load only the reference the current step needs.

### Setup and provider projection

```bash
canon init                                   # canon.yaml, starter policy (--policy light|standard|strict, default strict; see canon-policy), AGENTS.md block, commented plans examples
canon init --providers=claude,codex          # harnesses for the canon skill; without it init asks on a terminal, else takes the detected .claude/.codex/.omp/.pi
canon skills install                         # follows canon.yaml skills.providers (else the install lock's, else the detected ones; none detected installs nothing)
canon skills install --providers=claude,codex,omp,pi   # any subset or none; rewrites skills.providers
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics, incl. a stray canon projection for an unselected provider and a projection a formatter rewrote; never deletes user files
```

`install` and `doctor` end with `owned-paths:`, the globs of every path
canon writes. Add them to the repo's formatter and linter ignore lists; a
formatter that rewrites a projection or a ledger record makes canon read
drift (`doctor` reports it as `formatter-rewritten` and names the entry to
add). Specs, `policy.yaml` and the changes are written by hand and are not
on the list. The harness list lives in `canon.yaml` as
`skills: { providers: [...] }`; change it with `canon skills install --providers=<list>`,
never by assuming a default. The projection is a read-only copy: Claude
`.claude/skills/canon/`, Codex `.agents/skills/canon/`, OMP
`.omp/skills/canon/`, Pi `.pi/skills/canon/`, each with `reference/**` and
`scripts/**` sidecars (the retrieve script is a sidecar, not an installed hook).

### Route by task

- **Your role's steps:** `reference/role-planner.md`, `role-implementer.md`, `role-reviewer.md`, `role-orchestrator.md`.
- **Where the repo stands, subjects and changes:** `reference/canon-subject.md` (`canon status`, `canon subject new|adopt|status`, `canon change new`).
- **Write or reorganize `.feature` specs:** `reference/canon-authoring.md` before editing; use the scaffold.
- **Fields, enums, policy bindings:** `reference/canon-context.md` and `reference/canon-vocab.md`.
- **Replayable run inputs / versioned prompts:** `reference/canon-context.md`, then `canon context-pack new|show|verify` and `canon prompt add|show` with repository-relative JSON manifests.
- **Validate or index specs:** `reference/canon-fmt.md` and `reference/canon-inventory.md`.
- **Evidence, gates, imported-plan task checkboxes, hooks:** `reference/canon-gate.md`.
- **Findings and reviews:** `reference/canon-review.md`.
- **Ingest artifacts, sessions or plans:** `reference/canon-artifact-ingest.md`, `canon-session-ingest.md`, or `canon-plan-import.md`.
- **Storage, policy, plugins, strategy learning:** `reference/canon-storage.md`, `canon-policy.md`, `canon-plugins.md`, or `canon-learn.md`.
- **Dispatch guidance:** `reference/canon-retrieve.md`; load `scripts/canon-retrieve-pre-dispatch.sh` when wiring hooks.
- **Reports and dashboards:** `reference/canon-report-dashboard.md`.

### Safety contract

`canon status`, `canon format <root>`, `canon skills check`, and `canon gate check` are read-only. Do not hand-edit generated reports, provenance lines, locks, or task evidence. Inspect `canon context` before authoring typed artifacts, and treat every failure class as actionable evidence rather than bypassing the gate.
