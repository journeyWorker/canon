# canon-subject

A **Subject** is canon's first-class handle on the durable product unit a
team plans, designs, builds, verifies, and ships across MANY changes and
MANY scenarios. This skill covers the subject-write-first contract every
domain agent follows.

## Vocabulary rule (non-negotiable)

- **"feature docs" / ".feature files" = Gherkin behavior specs** — the
  `.feature` corpus `canon format`/`canon inventory sync` validate. "Read
  the feature docs" ALWAYS means the Gherkin files, never a Subject.
- **"subject" = the management record** authored by `canon subject new`.
  A Subject is the product unit; its `.feature` files are its behavior
  specs. Never the same word.

A Subject links, but does not contain, its work. Two joins, each
authored in one place:

- **Changes**: `change_ids` on the Subject, appended by `canon subject
  adopt`.
- **Scenarios**: the `@subject:<id>` tag on each Gherkin scenario,
  indexed onto `Scenario.subject_id` by the scenario index (rebuilt by
  `canon inventory sync`, and refreshed by every command that reads it).
  The Subject record carries no scenario list — the spec corpus is the
  only source, and the Subjects report panel, `spec_coverage.scope`, and
  the shipped gate all read it from there. To link a scenario, tag it;
  the next `canon status` or `canon gate check` indexes the tag. There is
  no subject-side command for it.

## The 4-step loop

Every substantial piece of work starts at a subject write.

### 1. Context → author or update the Subject FIRST

Write (or update) the Subject before touching anything else, so every
downstream artifact has a `subject_id` to pin to.

```bash
canon context --repo .            # record kinds, enum domains, policy requirements
canon subject new subject-domain-loop \
  --domain dev --title "Subject domain loop" \
  --summary "The durable product unit + its per-domain loop" \
  --owner-role implementer
```

- `<id>` is a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`) — the durable
  `subject_id` join key, never renumbered once assigned.
- `--domain <d>` is checked twice at write: SHAPE (a kebab-case slug),
  then MEMBERSHIP against the domain set THIS repo has activated.
  Canon ships no opinion about how a team slices its work — the set is
  resolved from the vocabulary, so a studio declaring
  `combat`/`economy`/`live-ops` authors against those and canon's base
  set (`planning`, `design`, `dev`, `data`, `test`, from `canon.core`)
  carries no authority there. Extend or replace it in your own
  `.canon/vocab/<id>/enums.yaml` under the `domain` key, activated by a
  `canon.project.yaml` profile (see `canon-vocab`) — never a canon code
  change. A non-member is refused with the legal set named
  (`expected one of: …`), so a typo cannot silently mint a category that
  then splits every `--domain`-filtered read. A repo that declares no
  `domain` enum has no constraint and accepts any slug. Run
  `canon context` to see the set in force.
- `--summary` is optional; `--owner-role` defaults to `implementer`;
  `--actor-id` defaults to `canon`. A freshly-authored Subject with no
  adopted changes and no tagged scenarios is a valid minimal record and
  starts at status `proposed`. `--json` prints the written record.

### 2. Retrieve subject-scoped strategy memory BEFORE working

```bash
canon retrieve --role dev --domain dev --subject subject-domain-loop --k 5
```

- `--regime <key>` XOR the derived pair `--domain <d> [--subject <id>]`:
  give the full regime key directly, OR let retrieval derive it. Never both.
- With `--domain`/`--subject`, retrieval tries `<domain>-<subject_id>`
  first, then `<domain>` — a subject's own lessons win, falling back to
  the domain's shared memory when the subject is new.
- Fail-soft: an empty/missing store degrades to an empty guidance list,
  never a nonzero exit. See `canon-retrieve` for the full contract.

### 3. Work → pin every artifact to `subject_id`

- **Scenarios**: `canon scenario new <tag> --subject <subject-id>` emits
  the `@subject:<subject-id>` tag; the scenario index maps the tag onto
  the scenario's `subject_id` the next time a command reads it (no sync
  step; see `canon-inventory`). A malformed/absent tag leaves it unset
  (fail-soft), never an error. See `canon-authoring`.
- **Changes**: adopt imported plan changes under the Subject (see
  [Adopt flow](#adopt-flow)).
- **Reviews / evidence / divergences**: authored as usual (`canon review
  add`, `canon finding add`, `canon gate`, `canon divergence …`) against
  the Subject's scenarios and adopted changes — the join spine carries
  them back through `scenario_id` and `change_id`. See `canon-review`
  for the finish-a-subject order.

### 4. Verdicts / trajectories → learn (knowledge out)

As work is reviewed, verdicts ingest under the subject-scoped regime
(`canon ingest artifacts`), so `canon learn` distills strategies keyed to
this subject and domain — the exact memory step 2 retrieves next time.

## Status lifecycle + the shipped evidence gate

`canon subject status <id> --to <state> [--override-reason <text>] [--reason <text>] [--actor-id <id>]`
performs a policy-gated transition.
The legal chain:

```
proposed → specced → building → verifying → shipped
```

plus **shipped → verifying** to reopen a shipped subject (with
`--reason`, below) and **any state → retired** (a subject can be retired
from anywhere). Any other transition is rejected by failure class, exits
1, and leaves the record UNCHANGED (fail closed).

```bash
canon subject status subject-domain-loop --to specced
canon subject status subject-domain-loop --to building
canon subject status subject-domain-loop --to verifying
canon subject status subject-domain-loop --to shipped   # gated — see below
```

On success the status updates in place; `--json` prints the updated
record. On a gate block it prints violations by failure class to stderr,
exits 1, and the record is unchanged.

**Corpus-wide, the same join:** `canon gate check` with
`.canon/policy.yaml`'s opt-in `spec_coverage` section generalizes this
gate past one transition — it left-joins the WHOLE Scenario corpus
against evidence, so an unimplemented or mismatched spec is reported
whether or not a Subject links it. `spec_coverage.scope` narrows
blocking to subjects in named statuses, and `@subject:<id>` tagging is
what brings a scenario into that scope. See `canon-gate`.

**The `verifying → shipped` evidence gate:** shipping additionally
requires that the Subject OWNS at least one scenario (one whose latest
synced generation carries `@subject:<id>`), and that EVERY owned
scenario carries a latest, non-`Divergent` verdict in the ledger (the
same last-wins rule `canon gate check` uses; a record keyed by both a
task and the scenario counts for the scenario). With
`spec_coverage.require_cases` set, every feature surface the Subject
owns must also carry a scenario of each required `@case:` (e.g. one
`@case:failure`) — attested golden-path scenarios alone do not ship. A
Subject with no tagged scenario is refused rather than shipped on an
empty set; so is one whose `scenario` records route away from the
gate's rung. Each refusal prints by failure class (`uncovered-cell`),
exits 1, and the status stays `verifying`. `retired` is not gated.

**The brief (0.16, 0.17, 0.19):** `verifying → shipped` also refuses where a brief
exists and is not approved, read exactly as `canon status` reads it (one
shared check, so status says `ship` only when this passes): the subject's
newest adopted change has a canon `.canon/changes/<change>/brief.md`
with no `Approved:` line outside HTML comments and code fences, whose
`Approved:` line carries a digest the brief's text no longer matches
(`BRIEF_CHANGED_SINCE_APPROVAL`; the brief was edited after the human
answered), or has no canon `brief.md` yet but comes from an external
openspec plan with a `proposal.md` under the plan sources. It also
refuses while `canon brief check` has blocker findings for that change,
which include a flow step with no scenario or no failure scenario (the
coverage rules gate shipping, never approval). It runs with or without
a policy. The refusal is an `unreviewed-promotion` naming the subject, and
says what to do:

```
canon subject status: unreviewed-promotion demo-subject — verifying → shipped: brief not approved — .canon/changes/c-demo/brief.md has no `Approved:` line; a human must answer the brief, and only from that answer the agent will record their words with `canon brief approve --change c-demo --by <the human> --note "<their words>"`
canon subject status: refused — get the human's answer to the brief and record it with `canon brief approve`, or pass --override-reason <text> to move anyway; the waiver is recorded on the subject
```

For an external change with no canon brief yet, the reason reads
``.canon/changes/<c>/brief.md does not exist yet: change `<c>` comes from
the external plan <proposal>, which canon reads and never edits, so its
approval lives in that canon file`` and the agent will ``create
.canon/changes/<c>/brief.md holding a `Source: <proposal>` line, then
record their words with `canon brief approve --change <c> --by <the human> --note "<their words>"` ``.

Never record the approval without the human's answer. When there is no brief to
answer (no adopted change, or a newest change with neither a canon
`brief.md` nor an external `proposal.md`), the move is not refused: it
prints ``canon subject status: advisory: <id>'s brief is not approved —
its newest adopted change `<c>` has no brief (no
.canon/changes/<c>/brief.md, no openspec proposal.md), so there is no
brief to answer and shipping is not refused`` (or `— <id> adopts no
change, so …`), and status shows the brief as not approved, as an
advisory. `subjects[].brief.enforced` in `canon status --json` says
which case applies. A brief that says the human ships changes who runs
the transition, not the check.

This refusal is a backstop, and so is the evidence one: shipping is not
the first thing an unapproved brief stops. The agent writes the brief,
runs `canon brief check` until it has no blockers, then stops and reports
it, and nothing is built or attested for the subject until the human
answers. `canon evidence add` refuses (exit 1, nothing staged) a
scenario whose `@subject` owner, or a task whose change
an adopting subject, has an enforced, unapproved brief, through the same
check; see `canon-gate`.

### The review guard (`spec_coverage.require_review`, opt-in)

When `.canon/policy.yaml` sets `spec_coverage.require_review` (see
`canon-review`), every status write also runs the review checks and
prints on stderr which it ran or skipped:

```
canon subject status: review guard for building → verifying (spec_coverage.require_review, scope: verifying, shipped)
  ran unreviewed-promotion — every owned scenario needs a review by an actor, and from a session, other than its evidence's
  ran open-blocker — 1 adopted change(s) checked for open blocker findings
```

A target outside `require_review.scope` prints `skipped
unreviewed-promotion — `specced` is not in require_review.scope`. A
transition INTO an in-scope status with gaps — an owned scenario with no
qualifying review (`unreviewed-promotion`), or an open blocker finding
on an adopted change (`open-blocker`) — prints the violations, exits 1,
leaves the record unchanged, and names `--override-reason`. `shipped`
also refuses a `false-success` (a scenario whose latest audit is
`refuted`) and, when `require_audit.scope` covers `shipped`, an owed
sampled audit; see `canon-gate`.

```bash
canon subject status demo-subject --to verifying \
  --override-reason "reviewer out until Monday" --actor-id lead
```

`--override-reason "<one line>"` waives ONLY `unreviewed-promotion` (the
review gaps and the brief check) and `open-blocker` — never a ship-gate
verdict or case gap, never unreadable routing. A waived brief is recorded
as `{ class: unreviewed-promotion, subject: <subject id> }`. It prints
the `waived ...` lines and `override recorded by
`lead` for the N violation(s) above: <reason>`, and records on the new
Subject record exactly the violations it let through:

```yaml
status_override:
  to: verifying
  reason: reviewer out until Monday
  waived:
    - { class: unreviewed-promotion, subject: world.demo.01 }
  actor: { agent_id: lead }   # from --actor-id, default `canon`
```

A blank or multi-line reason exits 2. The waiver is cleared on the
subject's next status write, so it never outlives the status it was
granted for. While it stands, `canon gate check` lists the recorded gaps
as advisories instead of failing; a gap it did not record (a blocker
raised later, a scenario tagged later) still fails the gate. Without
`require_review`, nothing is printed and nothing changes, except that
`shipped` still runs the brief check above.

### Reopening a shipped subject (0.18)

A follow-up on a shipped subject (a fix, a review round, a scenario its
gate now misses) reopens it: reopen, add tasks to the approved change,
scenario, evidence, review, ship.

```bash
canon subject status demo-subject --to verifying \
  --reason "follow-up: the retry path leaks a file handle" --actor-id impl
```

```
canon subject status: reopened `demo-subject` from shipped, recorded by `impl`: follow-up: the retry path leaks a file handle; keep the follow-up in its approved change `c-demo`, then scenario, evidence and review; shipping again runs every guard
canon subject status: demo-subject → verifying — written directly; nothing to promote
```

- `--reason "<one line>"` is required for `shipped → verifying` and
  refused on any other transition (exit 2, as is a blank one). The new
  record carries it with the actor and the change the work goes into:
  `reopen: { reason: …, change: c-demo, actor: { agent_id: impl } }`. The
  next status write clears it, as it clears a waiver.
- Inside the approved scope it needs no human: the subject's newest
  adopted change's brief is approved, or there is no brief to answer.
  Otherwise it refuses, exit 1, naming the brief to approve, and
  `--override-reason` does not waive it:
  ``unreviewed-promotion demo-subject — shipped → verifying: reopening needs the approved scope, and the brief is not approved — .canon/changes/c-next/brief.md has no `Approved:` line; …``.
- The review guard does not run on a reopen (it prints `skipped … — a
  reopen moves the subject back to fix its gaps; shipping again runs
  it`). `verifying → shipped` afterwards runs every guard above again.
- `canon status` flags a shipped subject whose gate went red (whatever
  `verifying → shipped` would now refuse it on) and its only step is
  this reopen.

## Adopt flow

Planning docs written outside canon become change/task records via
`canon ingest plans` (see `canon-plan-import`), but that stops at import.
`canon subject adopt` lifts an imported change into a managed Subject:

```bash
canon ingest plans --repo .                          # import → change/task rows
canon subject adopt subject-domain-loop-plan \
  --subject subject-domain-loop                        # link change → subject
```

A change made with `canon change new` (below) is recorded and adopted
already. A `tasks.md` in it is an optional work note nothing gates on
(`canon change new` no longer writes one); an existing one is still read:
`canon evidence add --task` and `canon gate task` read the file as it is,
and a flip re-imports that change itself. Run `canon ingest plans` for a
plan written outside canon, or when `canon query --kind task` must list
rows nobody has flipped yet.

`adopt` adds the subject to the change's `subject_ids` and the change to
the Subject's `change_ids`, so `canon query --kind change --change-id …`
and `canon query --kind subject` agree on the link from both ends. It
writes only the side that lacks the link, so rerunning it on a linked
pair writes nothing and exits `0`.

A change may belong to several subjects (0.18). Adopting it under a
second subject adds that subject, never re-points the change, and says
so:

```
canon subject adopt: linked change `refactor-core` to subject `editing` as well; the change now belongs to `perception` and `editing` — written directly; nothing to promote
```

Every reader joins a subject to its changes from both sides (the
subject's `change_ids`, and every change whose `subject_ids` names it):
the open-blocker rule, the status guard, `canon status`, the brief and
the evidence refusal. An open blocker on a shared change gates each
subject it belongs to. A change written before 0.18 names one subject as
`subject_id`; it reads, and `canon query --kind change --json` shows it,
as a one-element `subject_ids`.

The two records cannot be written atomically, so the subject is written
first: a half-done adoption still has that change's open blockers
counted. If the change write then fails, `adopt` exits `2`, names the
subject that now lists the change, and prints `canon subject adopt
<change_id> --subject <id>` to complete it.

### Starting a new change: `canon change new`

For a change that does not exist yet, one command writes it and records
it adopted:

```bash
canon change new add-login --subject auth --title "Add login"
```

```
canon change new: wrote .canon/changes/add-login/brief.md
canon change new: recorded change `add-login` adopted into subject `auth` — written directly; nothing to promote
```

- **Writes** canon's own change dir, `.canon/changes/<slug>/`, holding
  `brief.md` only: the PRD skeleton (`# <title>`, then the sections intent,
  desired outcome, user flow, step detail, acceptance, questions and
  assumptions, out of scope, each opened by a
  `<!-- canon:section <key> role=… -->` marker and a hint comment, with
  the headings in the repo's `language:`), and a comment naming
  `canon brief approve` as where the human's answer is recorded, so the
  file starts unapproved. The intent section's first paragraph holds the
  title, which imports as the change summary. `--kind chore` writes the
  short chore brief instead (intent, category, invariants, checks, out of
  scope; see `canon brief` below). No `tasks.md` is written:
  `canon status` lists each flow step's scenarios instead.
  `.canon/changes/` is canon's native plan source: every plan reader
  reads it with no `plans:` entry, so `canon.yaml` needs no edit and no
  external plans source is needed (see `canon-plan-import`).
- **Reads** the files with the native `canon` plan adapter `canon ingest
  plans` uses, then **records** the change already adopted, through the
  same write as `canon subject adopt`. Its records carry the actor
  `canon-plan-import-canon` (canon-written); an external openspec
  change's carry `canon-plan-import-openspec`.
- **What a failure leaves behind.** The files are staged under `.canon/`
  and the change dir is moved into place only once they parse. A failure
  before any record is written (staging, publishing the dir, or the
  first record write) removes the change dir, the staged files and any
  directory created for them: nothing is left behind. The two records
  cannot be written atomically. If the change record fails after the
  subject record was written, the command exits `2`, keeps the change
  dir and that one subject record, and prints the repair, `canon ingest
  plans && canon subject adopt <slug> --subject <id>`, which completes
  the link and is safe to rerun.
- **Refuses** with exit `2`, writing nothing, when the subject does not
  exist, or when the slug already has a `.canon/changes/<slug>` dir
  (active or under `.canon/changes/archive/`) or a `change` record.

A `tasks.md` is an optional work note canon never writes and nothing
gates on. If one exists, rows added as `- [ ] <n> <title>` are flipped with
`canon gate task <slug>#<n>` once evidence exists; `canon evidence add
--task` and the flip read the file as it is, so a new row needs no
`canon ingest plans` run.

A change from an external plan (an openspec change listed under
`plans.sources`, imported by `canon ingest plans` and adopted with
`canon subject adopt`) gets its brief as a canon file too:
`.canon/changes/<change>/brief.md` holding a `Source: <path>` line that
points at the external document and, once the human answers, the
`Approved:` line that `canon brief approve` writes. Canon never edits the external document. Such a
pointer `brief.md` is not a change of its own: the native reader skips
it, so the external source keeps the change, its summary and its tasks.

### The brief: `canon brief`, `canon brief check`, `canon brief approve`

```bash
canon brief [--change <slug>]                 # state and the next one to three steps; never edits
canon brief check [--change <slug>] [--json]  # findings; exit 0 clean, 2 blockers, 1 error
canon brief approve --change <slug> --by <the human> --note "<their words>"
```

Without `--change`, the repo's only live change is read. The brief is the
PRD `canon change new` writes; sections are found by their
`<!-- canon:section <key> role=… -->` marker, never by heading text. A
brief without markers (written before 0.19) checks as `brief.legacy`, an
advisory, and is read as before.

- **Blockers (structure, `brief.*`)**: a missing, duplicated or
  out-of-order section; a flow with no steps or a duplicate step id;
  step detail that does not match the flow ids in order; leftover
  placeholders (a hint comment still there, an empty section,
  `TODO`/`TBD`); a malformed waiver; for a chore, a flow/detail/acceptance
  section, a category outside `refactor|dependency|build|ci|infra`, or
  invariants/checks that say only "none". Once the subject is `building`
  or later, a flow step no scenario points at (`brief.flow-uncovered`) or
  one with no failure scenario (`brief.flow-no-failure`): these two
  **gate shipping, never approval**.
- **Advisories (prose, `prose.*`)**: phrase lists, sentence and
  paragraph length, and key-sentence marking in `role=normative`
  sections, for `language: en` and `ko`. Never an authorship verdict.
- **Waivers**: `<!-- canon:waive <rule-id> <section-key> "<reason>" -->`,
  one rule in one section, reason required; listed in the output.
- **`approve`** records the human's answer: it writes
  `Approved: <who> <when> — "<words>" (brief sha256:<digest>)` into the
  brief and records the policy approval in the same call. It refuses
  (exit 2) while the check has structure blockers. Edit the brief
  afterwards and every reader says `BRIEF_CHANGED_SINCE_APPROVAL` until a
  human answers again and `approve` runs again. Run it only from the
  human's answer, with their words as `--note`.

The optional refinement passes that fix findings before the human stop are
listed in `role-planner.md` in this directory.

A **chore** (`canon change new --kind chore`) changes how the subject is
built or kept, never what it does. Its brief is intent, category,
invariants, checks, out of scope; it adds no scenarios, and its evidence
is the subject's suite run again at the chore commit with
`canon evidence add --all-scenarios --report <kind>:<path> …`. That
supersedes the old evidence, which makes the pinned reviews stale, so an
independent re-review follows. Work that changes behavior is a feature
change, never a chore.

## Where are we: `canon status`

```bash
canon status                 # subjects by status, their gaps, next commands
canon status --json          # the same report, stable shape
canon status --repo ../other
```

A read: it always exits 0 and writes nothing (no ledger record, no
`.canon/audit` line). Every count comes from the joins the gate itself
uses, so status and the gate cannot disagree about a gap:

- **Header** — the canon version, and the policy in force:
  `spec_coverage` (with `scope`/`exclude_lanes`), `require_cases` and
  `require_review`. When `.canon/policy.yaml` is absent, unreadable, has
  no `spec_coverage` section, or has an invalid one, a `warning:` line
  says the gate requires no evidence, failure cases or review (or refuses
  until it is fixed).
- **Subjects grouped by status** (lifecycle order). Per subject:
  scenarios it owns through `@subject:` (after `exclude_lanes`, which
  are counted separately), how many carry a ledger verdict (the
  `verifying → shipped` gate's reading; divergent ones are counted too),
  how many carry a review that satisfies `require_review` (any current
  signed review when it is off; with `distinct_actor`, a self-review does
  not count), open blocker findings on its adopted changes, and each
  feature surface missing a `require_cases` case. `(N due)` marks reviews
  the next transition will require; `N stale review(s)` and `N unsigned
  review(s)` (printed when non-zero) count scenarios whose review went
  stale or whose latest reviews are all `indeterminate`/`unavailable`
  (see `canon-review`), and the `canon review add` step names both. With
  `require_review` on, a `warning:` line counts the in-effect reviews that
  predate review pinning (0.15), checked against the commit each pinned:
  current, stale (named) and undecidable.
- **Brief** (0.16, 0.17, 0.19) — for each `building`, `verifying` or `shipped`
  subject, and each `proposed` subject whose newest adopted change has a
  brief (a canon `brief.md`, or an external `proposal.md`), a `brief:`
  line under it says whether the brief is approved:
  `brief: approved in <brief> — <the recorded words>`, or
  `brief: not approved (…)`. Approval is an `Approved:` line in
  `.canon/changes/<change>/brief.md` for the subject's newest adopted
  change (the last one adopted; a live dir before an archived one), and
  only there: a line that starts with `Approved:` (indent allowed), says
  something after it, and sits outside HTML comments (`<!-- … -->`,
  multi-line included) and fenced code blocks (```` ``` ```` or `~~~`).
  `canon brief approve` writes it from the human's answer, as
  `Approved: <who> <when> — "<the human's words>" (brief sha256:<digest>)`,
  the digest covering the brief without its `Approved:` lines. A brief
  edited after that reads `brief: not approved (<brief> changed after its
  approval (BRIEF_CHANGED_SINCE_APPROVAL))` and names the re-approval
  command: the changed brief needs a new human answer. A line with no
  digest (written before 0.19) stays approved, with an `advisory:` to
  re-approve so it binds to the text. Fixes and follow-ups
  stay in the approved change, so a newly adopted change is a
  new scope: an older change's approval does not cover it. A change from
  an external plan has its brief in the same canon file, with a
  `Source: <path>` line pointing at the external document; an `Approved:`
  line inside an external `proposal.md` is not read. Before that canon
  file exists, an external openspec `proposal.md` under the plan sources
  (the first source holding the change; with no `plans:` section, the
  `openspec` source rooted at the repo that `canon gate task` also falls
  back to) still makes the brief one to answer, and the line reads
  ``brief: not approved (.canon/changes/<c>/brief.md does not exist yet:
  change `<c>` comes from the external plan <proposal>, which canon reads
  and never edits, so its approval lives in that canon file)``. With a
  canon brief and no line it reads
  ``brief: not approved (<brief> has no `Approved:` line)``.
  There is no record kind for it; the file is the record. For a brief in
  the PRD format, `brief check: <n> blocker(s), <n> advisory(ies)` follows
  (`canon brief` names the next step), then, per flow step of the
  change, `flow F1` (with ` — <gaps>` when it has no scenario or no
  failure scenario) and under it each scenario that carries
  `@flow:<change>.F1` with its title, `[failure]` when it is, its
  evidence (`yes`/`none`) and its review state.
- **Red shipped subject** (0.18) — a `shipped` subject that the
  `verifying → shipped` guard would now refuse, by the guard's own checks
  (owned scenarios, verdicts, required cases, audits, and, where
  `require_review` covers `shipped`, reviews and open blockers under
  `block_on_findings`; a gap its waiver recorded is not counted), prints
  ``gate: red — <n> <class> (first: <subject>), …; reopen with `canon subject status <id> --to verifying --reason "<why it reopens>" --actor-id <unit>` ``,
  carries `red: [...]` in `--json`, and that reopen is its only `next:`
  step. Open blockers count against every subject a change belongs to.
- **Unowned** — scenarios with no `@subject:` tag.
- **Audits** — how many scenarios are sampled for
  `spec_coverage.require_audit`, confirmed, owed (no audit or
  `indeterminate`) and refuted (`false-success`). An owed or
  refuted audit adds a `next:` step suggesting `canon evidence add
  --kind audit …` (recorded from an external evaluator; canon never runs
  one).
- **Dropped bindings** — `dropped bindings (N):` lists every recorded
  `--drop-attachment` (scenario, paths, reason); `drops[]` in `--json`.
- **Brief overrides** — a `warning:` line per evidence record attested
  with `canon evidence add --override-reason` while its subject's brief
  was not approved: what it attests, who, when, the subject and the
  reason (see `canon-gate`).
- **`next:`** — at most five steps, each with a one-line reason; the
  rest are counted. Rules, in order: no `canon.yaml` → `canon init`; an
  unreadable one → `canon init --check-config`; a scenario tagged with a
  subject that has no record → `canon subject new <that id>`; no subject
  at all → `canon subject new`; subjects and `.feature` scenarios but no
  scenario records → `canon inventory sync` (status refreshes the index
  itself first, so this means the refresh refused on a format violation
  or a duplicate id, which the sync names). Then per subject, verifying
  first, then building, specced, proposed, shipped: an unapproved brief
  (building and later, and proposed once its change has a brief)
  → the human step below, which is then the subject's
  only step; no scenario → `canon scenario new … --subject <id>` (and, when no
  `.feature` file holds a scenario yet, `canon feature new` first, unless
  every subject waits on its brief); a missing case → `canon scenario new
  <surface>.<next nn> … --case <case>`; (building and later) an
  unevidenced or divergent scenario → `canon evidence add --scenario-id
  …` (its reason adds "then `canon gate promote`" only under
  `write.promote: manual`); an open blocker → `canon finding close …`; a
  due, unreviewed, evidenced scenario → `canon review add …` from another
  session; none of these → `canon subject status <id> --to <next status>`.
  Last, unowned scenarios → `canon status`, after tagging them in their
  `.feature` files (status re-indexes the edit; no sync step). With more
  than one `specs.roots[]` entry, every suggested `canon scenario new`
  carries `--project`: the owning root for a missing case, `<root-id>`
  (the reason lists the roots) for a subject that owns no scenario yet.
- **The one human step.** Every step is the agent's to run except one:
  a subject whose brief is enforced (its newest change's canon
  `brief.md` has no `Approved:` line, or it has no canon `brief.md` yet
  and comes from an external openspec `proposal.md`) gets
  `human: stop and report the brief in <brief>; build and attest nothing for <id> until the human answers (one answer approves brief and policy)`,
  where `<brief>` is the canon `brief.md`, or the external `proposal.md`
  when no canon brief exists yet. Its reason reads ``<id>: <why
  unanswered> (an older change's approval does not cover a new scope),
  and the approval is never assumed: `canon evidence add` refuses <id>'s
  scenarios and tasks until it is recorded; once the human answers,
  <record> (one call records the brief and the policy approval)``, where
  `<record>` is ``record their words with `canon brief approve --change
  <c> --by <the human> --note "<their words>"` ``, or, for an external
  change, ``create <brief> holding a `Source: <proposal>` line, then
  record their words with `canon brief approve --change <c> --by <the
  human> --note "<their words>"` ``; for a brief edited after its
  approval, ``re-approve with `canon brief approve …` ``.
  It is the subject's only step, whatever its status: nothing suggests
  `canon feature new`, `canon scenario new`, `canon evidence add`, a
  review or a move-on for it before the human answers, and
  `canon subject status <id> --to shipped` refuses on the same check. The
  approval is never assumed. A subject with no brief to answer gets no human step;
  its `brief:` line says
  `not approved (no adopted change; advisory, nothing to approve yet)` or
  ``not approved (change `<id>` has no brief; advisory, nothing to
  approve yet)`` and nothing is withheld (a `proposed` subject with no
  brief, and a `specced` subject, show no `brief:` line at all). Once nothing blocks, a
  ship-ready verifying subject gets the agent's own
  `canon subject status <id> --to shipped`; status never asks a human to ship.

Status fails closed rather than suggest progress the gate would refuse:

- When `scenario`, `subject`, `review` or `finding` routes away from the
  `local` rung (the only rung status and the gate read), its records
  would read as empty. The only `next:` step is
  `canon init --check-config`, with the reason naming the
  `routing.<kind>: local` setting to restore.
- When `spec_coverage` is present but unusable, `canon gate check`
  refuses. The first `next:` step is `canon gate check`, with the reason
  naming the section to fix, and no subject is suggested to move to its
  next status until it parses.

`--json` prints one object:

```json
{
  "statusVersion": 3,
  "canonVersion": "0.17.0",
  "policy": { "present": true, "spec_coverage": { "require_evidence": true, "scope": [], "exclude_lanes": [], "require_cases": ["failure"], "require_review": { "scope": ["verifying", "shipped"], "distinct_actor": true, "block_on_findings": true } } },
  "warnings": [],
  "subjects": [
    { "id": "cart", "title": "Cart", "status": "building", "scenarios": 2, "excluded": 0, "evidenced": 2, "divergent": 0,
      "reviewed": 1, "staleReviews": 0, "unsignedReviews": 0, "reviewDue": true, "openBlockers": 1, "missingCases": [],
      "brief": { "approved": true, "enforced": false, "change": "c-cart", "brief": ".canon/changes/c-cart/brief.md", "briefExists": true, "approval": "Journey 2026-10-11 — \"approved\"" } },
    { "id": "promo", "title": "Promo", "status": "verifying", "scenarios": 1, "excluded": 0, "evidenced": 1, "divergent": 0,
      "reviewed": 1, "staleReviews": 0, "unsignedReviews": 0, "reviewDue": true, "openBlockers": 0, "missingCases": [],
      "brief": { "approved": false, "enforced": true, "change": "c-promo", "brief": ".canon/changes/c-promo/brief.md", "briefExists": false, "source": "openspec/changes/c-promo/proposal.md" } }
  ],
  "unowned": ["cart.promo.01"],
  "next": [
    { "command": "stop and report the brief in openspec/changes/c-promo/proposal.md; build and attest nothing for promo until the human answers (one answer approves brief and policy)", "why": "promo: .canon/changes/c-promo/brief.md does not exist yet: change `c-promo` comes from the external plan openspec/changes/c-promo/proposal.md, which canon reads and never edits, so its approval lives in that canon file (an older change's approval does not cover a new scope), and the approval is never assumed: … once the human answers, create .canon/changes/c-promo/brief.md holding a `Source: openspec/changes/c-promo/proposal.md` line, then record their words with `canon brief approve --change c-promo --by <the human> --note \"<their words>\"` (one call records the brief and the policy approval)", "actor": "human" },
    { "command": "canon finding close --change-id c-cart --round 1 --seq 1 --disposition fixed --resolution-sha <sha> --actor-id <unit> --session-id <session>", "why": "cart: 1 open blocker finding(s) (first: …); fix it, commit, then close it", "actor": "agent" },
    { "command": "canon review add --project-id root --scenario-id cart.add.02 --reviewer <reviewer> --actor-id <reviewer> --session-id <review-session> --role reviewer --pin <sha> --original-spec-ref \"<feature file>\"", "why": "cart: 1 of 2 scenario(s) lack a qualifying review; …", "actor": "agent" },
    { "command": "canon status", "why": "1 scenario(s) carry no @subject tag (first: cart.promo.01); add `@subject:<id>` above each in its .feature file, then check again: …", "actor": "agent" }
  ],
  "nextOmitted": 0
}
```

Each `missingCases` entry is `{ "projectId", "surface", "case" }`: the
spec root, the `<area>.<surface>`, and the required case it lacks.

`brief` is present for `building`, `verifying` and `shipped` subjects,
and for a `proposed` subject whose newest adopted change has a brief (a
canon `brief.md` or an external `proposal.md`). `change` is the newest
adopted change (omitted when the subject adopts none); `brief` its canon
`.canon/changes/<change>/brief.md`, the one file the approval is read
from (omitted when the change has no brief to answer); `briefExists`
(always present) whether that file exists (`false` for an external
change whose canon brief is still to be created); `source` the external
plan document the change comes from (the canon brief's `Source:` line,
or, before that file exists, the external `proposal.md`; omitted for a
canon-written change); `approval` the text after `Approved:`; and
`enforced` whether the missing approval blocks (a brief exists to
answer). The 0.16 `proposal` field is gone. `changed` (omitted when
false) says the brief was edited after its approval, `legacyApproval`
that the `Approved:` line carries no digest, `findings` is
`{ "blockers", "advisories" }` from `canon brief check` (PRD briefs
only), and `flow` lists each flow step as `{ "id", "change",
"scenarios": [{ id, title, failure, evidenced, review }], "gaps" }`.

Each `next` entry's `actor` is `agent` (its `command` is a canon command
line) or `human` (its `command` says what to get from the human).

`policy.spec_coverage` has the shape `canon context --json` prints
(`null` when absent, `{"invalid": …}` when unusable). `statusVersion`
changes only when a field changes meaning or is removed. It became `2`
in 0.16, when `next[].command` stopped always being a command line
(`actor` tells which), and `brief` was added alongside. It is `3` since
0.17, when `brief.proposal` was removed: the canon `brief.md` is
`brief.brief`, with `briefExists` and `source`.

`subject new`, `adopt` and `status` write their records directly; the
human-readable success line ends `— written directly; nothing to
promote` (`--json` prints the record and nothing else).

## Reading the per-domain management view

```bash
canon query --kind subject --domain dev --status building
canon report --repo .        # renders the "Subjects" panel
```

- `canon query --kind subject [--domain <d>] [--status <s>]` — the
  per-domain management view; `--domain`/`--status` are subject-only
  filters.
- `canon report`'s **Subjects** panel is a per-domain rollup: one row per
  subject (`domain`, `subject_id`, `title`, `status`, `scenario_count`,
  `covered_scenarios`), where `covered_scenarios` counts linked scenarios
  carrying a latest non-`Divergent` verdict — the same coverage the
  shipped gate enforces, surfaced read-only. Also exported by `canon
  report --snapshot` (see `canon-report-dashboard`).