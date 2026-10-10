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
  indexed onto `Scenario.subject_id` by `canon inventory sync`. The
  Subject record carries no scenario list — the spec corpus is the only
  source, and the Subjects report panel, `spec_coverage.scope`, and the
  shipped gate all read it from there. To link a scenario, tag it and
  re-sync; there is no subject-side command for it.

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
  the `@subject:<subject-id>` tag; `canon inventory sync` maps the tag
  onto the scenario's `subject_id`. A malformed/absent tag leaves it
  unset (fail-soft), never an error. See `canon-authoring`.
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

`canon subject status <id> <state> [--override-reason <text>] [--actor-id <id>]`
performs a policy-gated transition.
The legal chain:

```
proposed → specced → building → verifying → shipped
```

plus **any state → retired** (a subject can be retired from anywhere).
Any other transition is rejected by failure class, exits 1, and leaves
the record UNCHANGED (fail closed).

```bash
canon subject status subject-domain-loop specced
canon subject status subject-domain-loop building
canon subject status subject-domain-loop verifying
canon subject status subject-domain-loop shipped   # gated — see below
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
leaves the record unchanged, and names `--override-reason`.

```bash
canon subject status demo-subject verifying \
  --override-reason "reviewer out until Monday" --actor-id lead
```

`--override-reason "<one line>"` waives ONLY `unreviewed-promotion` and
`open-blocker` — never a ship-gate verdict or case gap, never unreadable
routing. It prints the `waived ...` lines and `override recorded by
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
`require_review`, nothing is printed and nothing changes.

## Adopt flow

Planning docs become change/task records via `canon ingest plans` (see
`canon-plan-import`), but that stops at import. `canon subject adopt`
lifts an imported change into a managed Subject:

```bash
canon ingest plans --repo .                          # import → change/task rows
canon subject adopt subject-domain-loop-plan \
  --subject subject-domain-loop                        # link change → subject
```

`adopt` stamps the change's `subject_id` and adds it to the Subject's
`change_ids`, so `canon query --kind change --change-id …` and `canon
query --kind subject` agree on the link from both ends. It writes only
the side that lacks the link, so rerunning it on a linked pair writes
nothing and exits `0`.

The two records cannot be written atomically, so the subject is written
first: its `change_ids` is what the gate reads a subject's adopted
changes from (`open-blocker`, the status guard), so a half-done adoption
still has that change's open blockers counted. If the change write then
fails, `adopt` exits `2`, names the subject that now lists the change,
and prints `canon subject adopt <change_id> --subject <id>` to complete
it.

### Starting a new change: `canon change new`

For a change that does not exist yet, one command scaffolds it and
records it adopted:

```bash
canon change new add-login --subject auth --title "Add login"
```

- **Writes** `openspec/changes/<slug>/proposal.md` (the title is its
  `## Why`, which imports as the change summary) and `tasks.md` (no rows
  yet, so the change imports as `proposed`), under the repo's first
  `openspec` plans source. `canon init` configures that source
  (`{dialect: openspec, root: .}`), so `canon.yaml` needs no edit.
- **Reads** the files with the same `openspec` adapter `canon ingest
  plans` uses, then **records** the change already adopted, through the
  same write as `canon subject adopt`.
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
  exist, when the slug already has a change dir (active or archived) or a
  `change` record, or when `canon.yaml` has no `openspec` plans source.

Add task rows to `tasks.md` as `- [ ] <n> <title>`, rerun `canon ingest
plans`, and flip each with `canon gate task <slug>#<n>` once evidence
exists.

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
  how many carry a review that satisfies `require_review` (any review
  when it is off; with `distinct_actor`, a self-review does not count),
  open blocker findings on its adopted changes, and each feature surface
  missing a `require_cases` case. `(N due)` marks reviews the next
  transition will require.
- **Unowned** — scenarios with no `@subject:` tag.
- **`next:`** — at most five commands, each with a one-line reason; the
  rest are counted. Rules, in order: no `canon.yaml` → `canon init`; an
  unreadable one → `canon init --check-config`; a scenario tagged with a
  subject that has no record → `canon subject new <that id>`; no subject
  at all → `canon subject new`; subjects but no scenario records →
  `canon inventory sync`. Then per subject, verifying first, then
  building, specced, proposed, shipped: no scenario → `canon scenario new
  … --subject <id>`; a missing case → `canon scenario new <surface>.<next
  nn> … --case <case>`; (building and later) an unevidenced or divergent
  scenario → `canon evidence add --scenario-id …`; an open blocker →
  `canon finding close …`; a due, unreviewed, evidenced scenario →
  `canon review add …` from another session; none of these →
  `canon subject status <id> <next status>`. Last, unowned scenarios →
  tag them and `canon inventory sync`. With more than one `specs.roots[]`
  entry, every suggested `canon scenario new` carries `--project`: the
  owning root for a missing case, `<root-id>` (the reason lists the
  roots) for a subject that owns no scenario yet.

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
  "statusVersion": 1,
  "canonVersion": "0.14.0",
  "policy": { "present": true, "spec_coverage": { "require_evidence": true, "scope": [], "exclude_lanes": [], "require_cases": ["failure"], "require_review": { "scope": ["verifying", "shipped"], "distinct_actor": true, "block_on_findings": true } } },
  "warnings": [],
  "subjects": [
    { "id": "cart", "title": "Cart", "status": "building", "scenarios": 2, "excluded": 0, "evidenced": 2, "divergent": 0,
      "reviewed": 1, "reviewDue": true, "openBlockers": 1, "missingCases": [] }
  ],
  "unowned": ["cart.promo.01"],
  "next": [
    { "command": "canon finding close --change-id c-cart --round 1 --seq 1 --disposition fixed --resolution-sha <sha> --actor-id <unit> --session-id <session>", "why": "cart: 1 open blocker finding(s) (first: …); fix it, commit, then close it" },
    { "command": "canon review add --project-id root --scenario-id cart.add.02 --reviewer <reviewer> --actor-id <reviewer> --session-id <review-session> --role reviewer --pin <sha> --original-spec-ref \"<feature file>\"", "why": "cart: 1 of 2 scenario(s) lack a qualifying review; …" },
    { "command": "canon inventory sync", "why": "1 scenario(s) carry no @subject tag (first: cart.promo.01); …" }
  ],
  "nextOmitted": 0
}
```

Each `missingCases` entry is `{ "projectId", "surface", "case" }`: the
spec root, the `<area>.<surface>`, and the required case it lacks.

`policy.spec_coverage` has the shape `canon context --json` prints
(`null` when absent, `{"invalid": …}` when unusable). `statusVersion`
changes only when a field changes meaning or is removed.

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