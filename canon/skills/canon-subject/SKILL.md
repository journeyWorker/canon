---
name: canon-subject
description: The subject-write-first agent contract (s36 subject-domain-loop) — how to author/update a Subject (canon's durable product unit, the reviewed 13th record kind) FIRST, retrieve subject-scoped strategy memory with canon retrieve --domain/--subject, pin every downstream artifact (scenarios, tasks, reviews, verdicts) to its subject_id, and drive its status lifecycle proposed→specced→building→verifying→shipped (+ any→retired) through the evidence-gated CLI. Use when starting substantial work on a product unit, when adopting an imported plan change into a subject, when transitioning a subject's status, or when reading the per-domain subject rollup (canon query --kind subject / canon report's Subjects panel). Fixes the vocabulary rule: "feature docs" = Gherkin .feature files; "subject" = the management record.
---

# canon-subject

s36 (`s36-subject-domain-loop`) adds the ONE thing canon lacked: a
first-class handle on the durable product unit a team plans, designs,
builds, verifies, and ships across MANY changes and MANY scenarios. That
unit is a **Subject** (`RecordKind::Subject`, the reviewed 13th kind —
`canon_model::records::Subject`), named after a musical canon's subject
taken up by many voices, one per role. This skill fixes the
subject-write-first contract every domain agent follows.

## Vocabulary rule (non-negotiable)

- **"feature docs" / ".feature files" = Gherkin behavior specs** — the
  `.feature` corpus `canon fmt`/`canon inventory sync` validate. When an
  agent is told "read the feature docs", that ALWAYS means the Gherkin
  files, never a Subject.
- **"subject" = the management record** — the `Subject` record authored
  by `canon subject new`. A Subject is the product unit; its `.feature`
  files are its behavior specs. They are never the same word.

A Subject links, but does not contain, its work: `change_ids`
(imported plan units adopted under it) and `scenario_ids` (behavior
specs specced against it) are join links, not embedded bodies.

## The 4-step loop

Every substantial piece of work starts at a subject write. In order:

### 1. Context → author or update the Subject FIRST

Read the authoring surface, then write (or update) the Subject before
touching anything else — the Subject write is the agent's FIRST act, so
every downstream artifact has a `subject_id` to pin to.

```bash
canon context --repo .            # record kinds, enum domains, policy requirements
canon subject new subject-domain-loop \
  --domain dev --title "Subject domain loop" \
  --summary "The durable product unit + its per-domain loop" \
  --owner-role implementer
```

- `<id>` is a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`) — the durable
  `subject_id` join-spine key, never renumbered once assigned.
- `--domain <d>` is a kebab-case slug validated at write. The CLOSED
  base vocabulary (`planning`, `design`, `dev`, `data`, `test`) lives in
  the `canon/vocab` plugin (the typed-vocabulary mechanism); consumer
  repos extend it there, never in canon-model — run `canon context` to
  see the domains a repo has activated.
- `--summary` is optional; `--owner-role` defaults to `implementer` when
  omitted, and a hidden `--actor-id` defaults to `canon` (the envelope
  agent_id), matching `canon review add`. A freshly-authored Subject
  with no `change_ids`/`scenario_ids` yet is a valid, minimal record and
  starts at status `proposed`.
- Envelope + policy are validated at write, exactly like every other
  record kind. `--json` prints the written record.

### 2. Retrieve subject-scoped strategy memory BEFORE working

Pull role + subject-scoped guidance in, so lessons from prior work on
this subject (and its domain) inform the run:

```bash
canon retrieve --role dev --domain dev --subject subject-domain-loop --k 5
```

- `--regime <key>` XOR the derived pair `--domain <d> [--subject <id>]`:
  give the FULL `regime_key` directly, OR let retrieval derive it from
  structured inputs. Never both.
- With `--domain`/`--subject`, retrieval tries area candidates in a
  fixed FALLBACK order — `<domain>-<subject_id>` first, then `<domain>`
  — so a subject's own accumulated lessons win, falling back to the
  domain's shared memory when the subject is new. (The `regime_key`
  grammar is four fixed segments; the domain/subject hierarchy is
  encoded IN the area segment, and retrieval always re-derives the
  candidates from the structured inputs — the encoding is never parsed
  back.)
- Fail-soft: an empty or missing store degrades to an empty guidance
  list, never a nonzero exit. See the `canon-retrieve` skill for the
  full retrieval contract.

### 3. Work → pin every artifact to `subject_id`

Do the work. Every artifact produced joins back to the Subject:

- **Scenarios**: tag a `.feature` scenario `@subject:<subject-id>`;
  `canon inventory sync` maps the tag onto `Scenario.subject_id`. A
  malformed/absent tag simply leaves it unset (fail-soft), never an
  error.
- **Changes**: adopt imported plan changes under the Subject (see
  [Adopt flow](#adopt-flow-from-canon-ingest-plans)).
- **Reviews / evidence / divergences**: authored as usual (`canon
  review add`, `canon gate`, `canon divergence …`) against the
  scenarios the Subject links — the join spine carries them back to the
  Subject through `scenario_id`.

### 4. Verdicts / trajectories → learn (knowledge out)

As work is reviewed, verdicts and trajectories ingest under the
subject-scoped regime (`canon ingest artifacts`), so `canon learn`
distills strategies keyed to this subject and domain — the exact memory
step 2 retrieves next time. The loop closes: knowledge in at the start,
knowledge out at the end, both keyed to the Subject.

## Status lifecycle + the shipped evidence gate

`canon subject status <id> <state>` performs a policy-gated transition.
The legal chain is:

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

On success the status is updated in place; `--json` prints the UPDATED
Subject record (carrying the new state). On a gate block it prints
violations by failure class to stderr, exits 1, and the record is
unchanged.

**The `verifying → shipped` evidence gate:** shipping additionally
requires that EVERY `scenario_ids` entry the Subject links carries a
latest, non-Divergent verdict in the ledger (reusing the trust gate's
own `ledger::latest_verdicts` fold — last-wins-by-`at`, the same rule
`canon gate check` uses). If any linked scenario is uncovered or its
latest verdict is `Divergent`, the transition prints the violating
scenarios by failure class, exits 1, and the status stays `verifying`.
Fail closed: a subject never ships on unproven scenarios.

`retired` is not gated — a subject can always be retired.

## Adopt flow from `canon ingest plans`

Planning docs become `Change`/`Task` records via `canon ingest plans`
(openspec change dirs, superpowers writing-plans docs — see the
`canon-plan-import` skill), but that stops at import. `canon subject
adopt` lifts an imported change into a managed Subject:

```bash
canon ingest plans --repo .                          # import → Change/Task rows
canon subject adopt subject-domain-loop-plan \
  --subject subject-domain-loop                        # link change → subject
```

`adopt` stamps `Change.subject_id` (additive on the wire — a pre-s36
`Change` is byte-identical until adopted) and adds the change to the
Subject's `change_ids`, so `canon query --kind change --change-id …` and
`canon query --kind subject` agree on the link from both ends.

## Reading the per-domain management view

```bash
canon query --kind subject --domain dev --status building
canon report --repo .        # renders the "Subjects" panel (mart_subjects)
```

- `canon query --kind subject [--domain <d>] [--status <s>]` — the
  per-domain management view; `--domain`/`--status` are subject-only
  filters over the standard query fan-out.
- `canon report`'s **Subjects** panel (`mart_subjects`) is a per-domain
  rollup: one row per subject (`domain`, `subject_id`, `title`,
  `status`, `scenario_count`, `covered_scenarios`), where
  `covered_scenarios` counts linked scenarios carrying a latest
  non-Divergent verdict — the same coverage the shipped gate enforces,
  surfaced read-only. It is also exported by `canon report --snapshot`
  into the dashboard's Parquet manifest.

## What this skill does NOT cover

- The `regime_key`/retrieval fallback mechanics themselves — see the
  `canon-retrieve` skill.
- Importing plan dialects into `Change`/`Task` — see the
  `canon-plan-import` skill.
- Strategy distillation/promotion/demotion — see `canon-learn` /
  `canon-reward`.
- The `canon/vocab` domain-enum plugin authoring — see the
  `typed-authoring-vocabulary` skill; this skill only documents that a
  `--domain` value must be an activated vocabulary domain.
