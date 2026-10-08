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
same last-wins rule `canon gate check` uses). With
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
  ran unreviewed-promotion — every owned scenario needs a review by an actor other than its evidence actor
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
query --kind subject` agree on the link from both ends.

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