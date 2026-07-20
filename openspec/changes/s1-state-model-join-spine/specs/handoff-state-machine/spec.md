## ADDED Requirements

### Requirement: Handoff's state-machine fields are wire-compatible with the donor monorepo's handoffs table
`canon-model`'s `Handoff` type SHALL carry the same state-machine fields as
the donor monorepo's `handoffs` Postgres table: `id`, `state`
(`pending|in-progress|done|abandoned`),
`chain_id`, `parent_handoff_id`, `seq`, `claimed_by` (CAS), and
`openspec_change_slug`, such that these seven fields are interchangeable
between a canon-written row and a row written by the donor CLI's handoff queue.
This is state-machine-core compatibility, not a full-row column match:
the donor monorepo's `trigger` column (`NOT NULL`, no default), its
`created_at`/`created_by_*`/`refs_extra` columns, and canon's own envelope
fields have no cross-analog (S1 design D4; S4 owns bridging that gap for
live-table reads/writes).

#### Scenario: A canon-written Handoff round-trips through the donor monorepo's table shape
- **WHEN** a `Handoff` value is serialized to the column set the donor monorepo's
  `handoffs` table expects
- **THEN** every one of `id`, `state`, `chain_id`, `parent_handoff_id`,
  `seq`, `claimed_by`, `openspec_change_slug` maps to a matching column
  with no lossy transformation, and `state` only ever takes one of the
  four values `pending`, `in-progress`, `done`, `abandoned`.

#### Scenario: An invalid state transition is rejected
- **WHEN** code attempts to move a `Handoff` from `done` or `abandoned`
  back to `pending` or `in-progress`
- **THEN** `canon-model` rejects the transition (these are terminal
  states in the state machine the donor monorepo's table already enforces via CAS
  `UPDATE … WHERE state='pending'` semantics).

### Requirement: Per-domain Handoff body templates
The `Handoff` body SHALL be a domain-scoped, template-validated payload
distinct from the fixed state-machine fields; a template registry,
referenced from `canon.yaml`, SHALL validate and render the body per
domain.

#### Scenario: A registered domain's body validates against its template
- **WHEN** a `Handoff` is constructed with `body.domain` set to a domain
  registered in `canon.yaml`'s `handoff_templates`
- **THEN** the registry's template for that domain validates
  `body.fields` and either accepts the value or returns a structured
  `EvidenceViolation` naming the missing/invalid field — never a silent
  accept of malformed content.

#### Scenario: An unregistered domain is rejected before write
- **WHEN** a `Handoff` is constructed with `body.domain` set to a domain
  absent from the active `canon.yaml`'s `handoff_templates`
- **THEN** construction fails with a stable failure-class violation rather
  than persisting a Handoff whose body no template can validate or render.

#### Scenario: State-machine fields are independent of body domain
- **WHEN** two `Handoff` records use different `body.domain` values
- **THEN** both still expose the identical fixed set of state-machine
  fields (`id`/`state`/`chain_id`/`parent_handoff_id`/`seq`/`claimed_by`/
  `openspec_change_slug`) with identical semantics, so a domain-agnostic
  consumer (the donor CLI's handoff queue view) can operate on either without
  knowing the body's domain.
