# canon-subject

> The subject-write-first agent contract — how to author/update a Subject (canon's durable product unit) FIRST, retrieve subject-scoped strategy memory with canon retrieve --domain/--subject, pin every downstream artifact (scenarios, tasks, reviews, verdicts) to its subject_id, and drive its status lifecycle proposed→specced→building→verifying→shipped (+ any→retired) through the evidence-gated CLI. Use when starting substantial work on a product unit, adopting an imported plan change into a subject, transitioning a subject's status, or reading the per-domain subject rollup. Vocabulary rule: "feature docs" = Gherkin .feature files; "subject" = the management record.

# canon-subject

A **Subject** is canon's first-class handle on the durable product unit a
team plans, designs, builds, verifies, and ships across MANY changes and
MANY scenarios. This skill covers the subject-write-first contract every
domain agent follows.

## Vocabulary rule (non-negotiable)

- **"feature docs" / ".feature files" = Gherkin behavior specs** — the
  `.feature` corpus `canon fmt`/`canon inventory sync` validate. "Read
  the feature docs" ALWAYS means the Gherkin files, never a Subject.
- **"subject" = the management record** authored by `canon subject new`.
  A Subject is the product unit; its `.feature` files are its behavior
  specs. Never the same word.

A Subject links, but does not contain, its work: `change_ids` (adopted
plan units) and `scenario_ids` (behavior specs) are join links.

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
- `--domain <d>` is a kebab-case slug validated at write. The closed base
  vocabulary (`planning`, `design`, `dev`, `data`, `test`) is extended in
  the `.canon/vocab` plugin (see `canon-vocab`); run `canon
  context` to see the domains a repo has activated.
- `--summary` is optional; `--owner-role` defaults to `implementer`;
  `--actor-id` defaults to `canon`. A freshly-authored Subject with no
  `change_ids`/`scenario_ids` is a valid minimal record and starts at
  status `proposed`. `--json` prints the written record.

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

- **Scenarios**: tag a `.feature` scenario `@subject:<subject-id>`;
  `canon inventory sync` maps the tag onto the scenario's `subject_id`.
  A malformed/absent tag leaves it unset (fail-soft), never an error.
- **Changes**: adopt imported plan changes under the Subject (see
  [Adopt flow](#adopt-flow)).
- **Reviews / evidence / divergences**: authored as usual (`canon review
  add`, `canon gate`, `canon divergence …`) against the Subject's
  scenarios — the join spine carries them back through `scenario_id`.

### 4. Verdicts / trajectories → learn (knowledge out)

As work is reviewed, verdicts ingest under the subject-scoped regime
(`canon ingest artifacts`), so `canon learn` distills strategies keyed to
this subject and domain — the exact memory step 2 retrieves next time.

## Status lifecycle + the shipped evidence gate

`canon subject status <id> <state>` performs a policy-gated transition.
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

**The `verifying → shipped` evidence gate:** shipping additionally
requires that EVERY linked `scenario_ids` entry carries a latest,
non-`Divergent` verdict in the ledger (the same last-wins rule `canon
gate check` uses). If any linked scenario is uncovered or its latest
verdict is `Divergent`, the transition prints the violating scenarios by
failure class, exits 1, and the status stays `verifying`. `retired` is
not gated.

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
