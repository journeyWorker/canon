---
name: canon
description: Use for every build, implement, add-a-feature, change-a-feature or fix request in a repo that has canon.yaml, including sites, apps and games; prefer it there over generic site, app or game builder skills. Runs canon's working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review, status transition) split by role (planner, implementer, reviewer, orchestrator). The full guide is `canon skills show canon`.
---

# Canon

Features in this repo are managed with canon. This is the shared core guide:
the loop, how to drive the CLI, and how to find your role. The guide is built
into the `canon` binary, so it always matches the version you run:

- `canon skills show canon` prints this guide.
- `canon skills show <name>` prints one topic or role guide (the table at the
  end lists every name); `canon skills show canon --full` prints this guide
  followed by every topic.
- Your role guide's **Commands** block lists every option of every leaf the
  role uses. Ask `canon schema <command>` (or `--help`) only for a leaf it does
  not list, or after a `USAGE` refusal. Never guess an option name.

The `SKILL.md` your harness loaded is only a stub that sends you here.

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
  the gate grades the work against. The human may also approve it on the `canon dashboard` briefs page; you never use that form.
  The human records the answer, never you:
  `canon brief approve <slug> --approver <their name> --note "<their words>"`
  refuses every agent role (`HUMAN_ONLY`). Report that exact call, slug filled in, and
  wait; never remove or unset your role (`env -u CANON_ROLE`, dropping `--as`) to run it yourself.
  Never edit the policy to make the gate pass: a policy that differs from
  the approved one fails closed (`POLICY_CHANGED_SINCE_APPROVAL`, not
  waivable); stop and ask the human. A brief edited after its
  approval is not approved (`BRIEF_CHANGED_SINCE_APPROVAL`): it needs a new
  answer. The ship guard and `canon evidence add` refuse an unapproved or
  edited brief, but those are backstops: the stop itself is at step 1.
- **Approval is scoped.** Evidence for a scenario needs the approval of the
  changes its `@flow:<change>.<F-id>` tags name, so a new unapproved change
  on a live subject waits on its own scenarios only; `change new` and
  `subject adopt` warn when the subject is live. `canon subject unadopt <subject-id> --change-id <change>`
  takes a draft change back off (refused once approved). The subject's scope
  is its newest adopted change, walking back past pending drafts (brief not
  approved, no scenario of the subject tagged `@flow:<change>.*`): reopen,
  evidence and ship read the newest approved or tagged change, and the
  transition prints one line naming every draft set aside; a draft a scenario
  is tagged for blocks until it is approved. `canon skills show canon-subject`
- **Inside an approved scope, never stop for a human.** Fixes, follow-ups,
  review rounds and re-reviews stay in the approved change (never
  `canon change new`: a new change is a new scope) and go straight on: scenario,
  implement, evidence, independent review. A follow-up that adds or changes
  a step of the brief edits the brief: that is a scope change and needs a
  new answer. A follow-up on a shipped subject reopens it
  (`canon subject move <id> --to verifying --reason "<why>"`); a fix
  closed `fixed`, and an approved chore, then need evidence again for every
  scenario whose code changed (see "Evidence follows the code" below) before
  it ships, and a chore also needs one change-level review
  (`canon review change <slug>`).
- When no human can answer one of the brief's questions, write your
  assumption into the brief's Questions and assumptions, build on it, and say
  so in your report. That covers the brief's questions and work inside an
  approved scope, never the approval itself.

1. **Brief.** The planner writes the PRD brief (`.canon/changes/<slug>/brief.md`), checks it with `canon brief check`, and stops for the human. `canon skills show role-planner`
2. **Subject.** `canon subject new`, then `canon change new` (`--change-kind chore` for work that alters no behavior); the change writes the brief skeleton. `canon skills show role-planner`
3. **Scenarios.** `canon scenario new --flow <F-id> --case <case>` per behavior of a flow step, failure paths (`--case failure`) included; many at once with `--input` (below). `canon skills show role-implementer`
4. **Units.** Split the work; one unit is one flow step's slice with a named verify command, and each unit has its own actor id and session id, passed on everything it records. A unit never attests another unit's work.
5. **Implement.** Code with tests named by scenario id (`game.run.01: …`). `canon skills show role-implementer`
6. **Evidence.** `canon evidence add` binds the test run to its scenarios; one `--input` array records many at once. `canon skills show role-implementer`
7. **Independent review.** A different session, a separate process with its own session id, that can run the checks it signs off, records `canon finding add` and `canon review scenario`. Every change. `canon skills show role-reviewer`
8. **Transition.** `canon subject move <id> --to <state>` along `proposed → specced → building → verifying → shipped`, with `canon gate check` clean; fix a refused guard rather than overriding it. `canon skills show role-orchestrator`

**Done when:** `canon gate check` exits 0 with no violations, `canon subject show <id>` reports status `shipped`, and `canon status` names no next step for the subject (a human step left in `meta.next` is the one thing you report instead of doing).

Every write command says whether the record is final (`written directly`,
or `staged and promoted in this run`) or staged; only a staged record
needs `canon gate promote`, and only `write.promote: manual` leaves one.

### Commits and uncommitted code

Records that are about code take `--rev <REV>`: any git revision (`HEAD~1`,
a branch, a tag, a short sha), default `HEAD`, so you never copy a sha out of
git. `finding add` also takes `--resolution-rev` (default HEAD when the
finding is `fixed`) and `--introduced-rev` (no default: say where the defect
came from). Evidence, a review or a finding of uncommitted work is allowed,
never refused: the record carries `codeTree` (the git tree id of the code the
command saw: `.canon/`, the spec roots and the `code.exclude` globs are left
out, so editing a `.feature` file or a brief never changes it) and
`worktreeDirty`. The commit made from a working tree has the same `codeTree`
the record carries, so a review of a working tree survives the commit made
from it; a review is judged by what it pins (the scenario and what its
evidence says), never by the commit or the code tree. Outside a git work
tree, or in a repo with no commits, a command that needs a commit refuses
`NO_COMMIT`.

### Evidence follows the code

After a chore approval or a reopen, a scenario whose latest evidence was
recorded on the current `codeTree` needs nothing new: a docs-only chore then
needs its change-level review alone. What counts as code is `code.exclude` in
`canon.yaml` (globs, default `docs/**` and `*.md`). When code did change,
`canon status`, `meta.next` and the refusal hint lead with one batch over
every report, for example
`canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`
(with the real report paths, and `--project-id <id>` when the repo has several
spec roots), never a loop per scenario. With a `--report`, `--class` defaults
to `test-run` and `--ref` to the report paths, and the bound cases decide the
verdict: any failed case `divergent`, a skipped one and none failed
`not-applicable`, else `faithful`.

### State

The ledger is latest-only. It holds the current record of each key; writing a
newer version moves what it supersedes to `.canon/archive`, and `status`,
`gate check`, the guards and `query` read the ledger only. Earlier versions
are one flag away: `canon query --kind evidence --history`. A ledger written
before 0.23, or one whose move was interrupted, is brought to one file per
key by `canon ledger compact` (`canon migrate` runs it). A record binds only
what its own write names: evidence carries nothing forward from an earlier
record, and a review stays current while the verdict and checks of the
evidence it saw are unchanged and the code tree is the one it saw.

Where artifact bytes live is the repo's choice: `canon.yaml`'s `artifact_store:`
block defaults to the local, git-tracked `.canon/artifacts/sha256/`, or names
an object-store tier, in which case `evidence add`, `review scenario`, `review change` and
`artifacts store` upload by digest and keep nothing locally. The gate and
`canon artifacts verify` (`--full` re-hashes every blob) ask the store and
refuse, never pass, when it cannot be reached. Moving an existing store with
`canon artifacts move --tier <tier>` is the repo owner's decision, not a step
of the loop.

## Know your role

canon cannot detect a role; you declare it. Pass `--as <role>` or set
`CANON_ROLE=planner|implementer|reviewer|orchestrator`; a flag wins over the
variable, and an unknown value is refused (`UNKNOWN_ROLE`).

- **A role is set, or you were spawned as one:** load only
  `canon skills show role-<role>` and do that role's steps. `canon status`
  and `meta.next` then show only your role's steps, and count the others in
  `waitingOn`; `canon schema --as <role>` lists only your role's commands.
  Do not load another role's guide.
- **No role (one agent doing everything):** follow `canon status` and load
  each role guide as its phase starts: planner for steps 1–2,
  implementer for 3–6, reviewer for 7 (in a separate session), orchestrator
  for 8.

A role is guidance, not enforcement: `--role` on `evidence add` and
`review scenario` defaults to it. `CANON_ROLE` never filters or fails
`canon schema`, `canon context` or `canon skills show`; only `--as` filters.

## Driving the CLI

- Start a session with `canon status`: where each subject stands and the next
  commands. After that, do not call it only to learn the next step: every
  write returns `meta.next` (`[{command, argv, why, actor, role}]`), the same
  steps `status` would give, narrowed to what the write touched. `actor: human`
  marks a step only the human takes.
- Your role guide's **Commands** block (`canon skills show role-<role>`) is
  your complete command set, every option of every leaf included. Run `canon schema <command> --json` only for a
  command it does not list, or after a `USAGE` refusal (read `error.field`
  and `error.expected` first). `canon schema` alone lists every command.
- Pass `--json` and read stdout as one document, nothing on stderr. Success is
  `{"ok":true,"data":…,"meta":…}`; failure is
  `{"ok":false,"error":{…},"meta":…}` (a check that fails keeps its report in
  `data`). `meta` carries `command`, `schemaVersion`, `canonVersion`, `next`,
  `warnings` and, for a repeated write that changed nothing, `unchanged: true`.
  On failure branch on `error.code`, fix the option `error.field` names
  using the `error.expected` values (`field` is an option name, or a JSON
  pointer such as `/items/2/scenarioId` into an `--input` document), and
  follow `error.hint`: every refusal carries a working invocation. The codes
  are stable; `canon schema --errors --json` lists them and
  `canon schema --error <CODE>` explains one.
- **Exit codes:** 0 ok; 1 the command's own finding (a failed check, a
  violation, or `unverifiable` when nothing was examined); 2 refused (usage,
  `CONFIG_MISSING`, an id or file that does not exist, an invalid argument).
  A refusal leaves the tree unchanged.
- Repeating a write is safe: a record that already exists with identical
  content is reported `unchanged` (`meta.unchanged`), not duplicated.
- Flags that name an id are spelled `--<noun>-id` everywhere (`--change-id`,
  `--project-id`, `--subject-id`, `--scenario-id`, `--actor-id`,
  `--session-id`), the primary object of a command is its operand
  (`canon subject move <id> --to verifying`, `canon change new <slug>`), and
  `--kind` values carry no `_record` suffix (`--kind evidence`).
- `canon status` sends the planner to write the brief while `brief check`
  has blockers; the human stop appears only when the brief is clean and
  unapproved.
- Page long lists with `--limit <n>` and `--cursor <nextCursor>` (`--all`
  for everything), and project fields with `--fields a,b` (`status`, `export`,
  `query`, `finding list`, `divergence list`, `context`).
- `--override-reason <text>` is the only way to proceed past a waivable
  refusal; it is recorded, echoed in `data.overrides[]` and listed by `status`
  for the human. There is no `--force`. `brief approve`, `policy approve` and
  `divergence accept` are human-only and refuse every agent role: report the
  exact call, ids filled in, and wait; never unset your role to run one.

### Many records at once: `--input`

Every write leaf takes `--input` instead of flags: inline JSON, a file, or `-`
for stdin. The keys are the option names in camelCase, and `operands` holds
the positional arguments. An object is one invocation; a JSON array (or NDJSON
from a file or stdin) is a batch, run in order and answered with
`data.items[]` (`index`, `status` of `written`, `unchanged`, `failed` or
`skipped`, and that element's `data` or `error`). A batch is validated whole
before anything is written: an invalid element writes nothing and exits 2,
`error.field` pointing at `/items/<i>/<key>`; a write that fails stops there
(exit 1) and re-running the same batch reports the finished elements
`unchanged`. A typo in a key is refused (`INPUT_UNKNOWN_FIELD`), never dropped.
`canon schema <command> --json` shows the leaf's input schema and an example.

```text
canon scenario new --input '[{"operands":["auth.login.01"],"title":"Logs in","case":"happy"},{"operands":["auth.login.02"],"title":"Wrong password","case":"failure"}]'
canon evidence add --input '[{"scenarioId":"auth.login.01","class":"test-run","ref":"npm test","role":"implementer"},{"scenarioId":"auth.login.02","class":"test-run","ref":"npm test","role":"implementer"}]'
canon finding add --input '[{"changeId":"add-login","round":1,"seq":1,"severity":"note","actorId":"reviewer-1","summary":"Token is logged"},{"changeId":"add-login","round":1,"seq":2,"severity":"blocker","actorId":"reviewer-1","summary":"No rate limit"}]'
```

`--input` never carries `--commit` or `--plan-digest`, and the destructive
leaves below take one object only.

### Preview, then `--commit`

The five commands that delete or rewrite existing data (`canon purge`,
`canon tier age`, `canon ledger compact`, `canon artifacts move`,
`canon migrate`) act in two steps. The bare run previews: it writes nothing,
exits 0 and prints the effects and a `planDigest`. Read the effects, then act
with `--commit --plan-digest <planDigest>`. It refuses (`PLAN_CHANGED`) when
the plan differs from the one you previewed, so run the preview again. Appends
and state transitions (`gate promote`, `subject move`) run directly. Where a
preview helps elsewhere (`init`, `skills install`, `export`, `inventory sync`,
`subject adopt`, …) `--dry-run` validates fully and writes nothing.

```bash
canon purge --kind session --before 2026-01-01
```

## Upgrading a repo from 0.27

0.28 changed the CLI grammar with no aliases: an old spelling fails with exit 2
and an `error.hint` naming the replacement. `canon status`, `canon gate check`
and `canon context` warn when the repo holds 0.27-era state and put
`canon migrate --json` first in `meta.next`. Then:

1. `canon migrate` previews and prints the `planDigest`.
2. `canon migrate --commit --plan-digest <digest>` with that digest applies it.
   It moves the layout, rewrites canon's own hooks and scripts, compacts the
   ledger, replaces an old full skill with the stub, and backs up every file it
   rewrites under `.canon/migrate-backup/` (local and git-ignored; `--revert` needs it). It refuses uncommitted changes
   under a path it moves.
3. `canon skills install` refreshes the stub and the scripts; `canon skills check`
   confirms them.

**Done when:** `canon migrate` reports nothing left to move, `canon skills check` exits 0, and `canon status` and `canon gate check` no longer warn about 0.27 state.

## Setup and provider projection

```bash
canon init                                   # canon.yaml, starter policy (--policy light|standard|strict, default strict; see canon-policy), AGENTS.md block, commented plans examples
canon init --providers=claude,codex          # harnesses for the canon skill; without it init asks on a terminal, else takes the detected .claude/.codex/.omp/.pi
canon skills install                         # follows canon.yaml skills.providers (else the install lock's, else the detected ones; none detected installs nothing); prunes projected files the bundle no longer ships when their bytes match the lock, keeps a modified one and reports `kept:`
canon skills install --providers=claude,codex,omp,pi   # any subset or none; rewrites skills.providers
canon skills check                           # read-only drift check; exit 1 (SKILLS_DRIFT) names each orphan, a projected file the bundle no longer ships
canon skills doctor                          # diagnostics, incl. a stray canon projection for an unselected provider and a projection a formatter rewrote, and `orphan:` lines for projected files the bundle no longer ships; never deletes user files
```

`install` writes, per selected harness, a short `SKILL.md` stub (this file's
frontmatter plus a pointer to `canon skills show canon`) and the `scripts/**`
sidecars; the guides themselves are not copied, they come from the binary.
Claude gets `.claude/skills/canon/`, Codex `.agents/skills/canon/`, OMP
`.omp/skills/canon/`, Pi `.pi/skills/canon/` (the retrieve script is a sidecar,
not an installed hook). `check` reports a stub written by another canon
version; `install` rewrites it. The harness list lives in `canon.yaml` as
`skills: { providers: [...] }`; change it with `canon skills install --providers=<list>`,
never by assuming a default.

`install` and `doctor` end with `owned-paths:`, the globs of every path
canon writes. Add them to the repo's formatter and linter ignore lists; a
formatter that rewrites a projection or a ledger record makes canon read
drift (`doctor` reports it as `formatter-rewritten` and names the entry to
add). Specs, `policy.yaml` and the changes are written by hand and are not
on the list.

**Done when:** `canon skills check` exits 0 and `canon skills doctor` lists no `orphan:`, `stray:` or `outdated:` line.

## Route by task

- **Fix a bug:** the approved change if there is one, else the planner (`canon change new`); first a failing run of the scenario (`canon scenario new --case failure`, then `canon evidence add`), then the fix. `canon skills show role-implementer`.
- **Add or change a feature:** `canon subject new`, `canon change new`, then the brief. `canon skills show role-planner`.
- **Pure refactor, dependency, build, CI or infra:** `canon change new <slug> --subject-id <id> --change-kind chore`; data or interface changes are not chores. `canon skills show role-planner`.
- **Review someone's change:** `CANON_ROLE=reviewer`, `canon brief check <slug>`. `canon skills show role-reviewer`.
- **Resume:** `canon status --json`, then follow `meta.next`. `canon skills show role-orchestrator`.
- **Plan more work on a live subject:** `canon change new <slug> --subject-id <id>` (it warns until the brief is approved; `canon subject unadopt` parks a draft). `canon skills show canon-subject`, `canon skills show role-planner`.
- **Upgrade canon in a repo:** `canon migrate` (previews), then `canon migrate --commit --plan-digest <digest>`, then `canon skills install`. See "Upgrading a repo from 0.27" above and `canon skills show role-orchestrator`.

**Done when:** you have named your role and loaded its guide, and `canon status --json` lists your next step in `meta.next`.

## Safety contract

`canon status`, `canon format`, `canon skills check`, `canon skills show` and
`canon gate check` do not change the repo's records. Do not hand-edit generated
reports, provenance lines, locks, or task evidence. Inspect `canon context`
before authoring typed artifacts, and treat every failure class as actionable
evidence rather than bypassing the gate.

## Guides: `canon skills show <name>`

Every name below is a guide built into the binary. Load only the one the
current step needs.

| Name | What it is for |
| --- | --- |
| `canon` | This core guide: the loop, driving the CLI, routing. |
| `role-planner` | Planner steps 1–2: subject, change, PRD brief, the human stop. |
| `role-implementer` | Implementer steps 3–6: scenarios, units, code, evidence. |
| `role-reviewer` | Reviewer step 7: findings and signed reviews in a separate session. |
| `role-orchestrator` | Orchestrator step 8: dispatch, transitions, resuming, upgrading. |
| `brief-shape` | Brief pass: the structure `canon brief check` expects. |
| `brief-clarify` | Brief pass: turn open points into questions a human answers in one reply. |
| `brief-harden` | Brief pass: make flow and step detail agree, with failure paths. |
| `brief-align` | Brief pass: scenarios cover the brief's flow and the brief still says what they prove. |
| `brief-polish` | Brief pass: make the brief readable once (clears `prose.*` advisories). |
| `canon-subject` | Status, subjects and changes: `canon subject new|adopt|show|move`, `canon change new`. |
| `canon-authoring` | Write or reorganize `.feature` specs; the scaffold. |
| `canon-context` | `canon context`: fields, enums, policy bindings, context packs and prompts. |
| `canon-vocab` | Typed authoring vocabulary and the handoff template. |
| `canon-fmt` | `canon format`: validate a spec or artifact corpus. |
| `canon-inventory` | `canon inventory sync`: index specs into scenario records. |
| `canon-gate` | Evidence, gates, task checkboxes, hooks. |
| `canon-review` | Findings and reviews. |
| `canon-artifact-ingest` | `canon ingest records`: artifacts into verdict trajectories. |
| `canon-session-ingest` | `canon ingest sessions`: agent transcripts into session records. |
| `canon-plan-import` | `canon ingest plans`: plan state into change and task records. |
| `canon-storage` | Storage rungs, tiers, retention, export, purge. |
| `canon-policy` | The policy and the CEL expressions behind it. |
| `canon-plugins` | Plugin overlays on core records. |
| `canon-learn` | Strategy memory: promote, approve, roll back. |
| `canon-retrieve` | Dispatch-time guidance (`canon retrieve`); `scripts/canon-retrieve-pre-dispatch.sh` is the hook sidecar. |
| `canon-report-dashboard` | `canon report build|check|snapshot` and `canon dashboard`. |

**Done when:** `canon skills show <name>` prints the guide you needed; a name that is not in the table is a usage error (exit 2) whose `error.expected` lists the valid ones.
