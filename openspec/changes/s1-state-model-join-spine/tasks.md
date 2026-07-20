## 1. Envelope and record-kind scaffolding

- [x] 1.1 Define `Envelope { schema: u32, kind: RecordKind, at:
      DateTime<Utc>, actor: Actor }` and `Actor { agent_id: String, role:
      RoleId, session_id: Option<SessionId>, model: Option<String> }` in
      `crates/canon-model`, replacing S0's marker-constant stub.
      Evidence: `crates/canon-model/src/envelope.rs` (S0's `CRATE`
      marker const removed from `lib.rs`; `envelope::tests::actor_never_has_a_bare_by_field`
      green).
- [x] 1.2 Define the twelve closed record-kind types (`Change`, `Task`,
      `Scenario`, `Session`, `Run`, `Event`, `Handoff`, `Review`,
      `Divergence`, `Trajectory`, `StrategyItem`, `EvidenceRecord`), each
      composing `Envelope` via `#[serde(flatten)]` plus its own fields — no
      type defines an ad hoc actor/`by` field outside `Envelope.actor`.
      Evidence: `crates/canon-model/src/records.rs` (11 kinds) +
      `src/handoff.rs` (`Handoff`); `RecordKind::ALL.len() == 12` asserted
      by `envelope::tests::all_twelve_kinds_present_exactly_once`.
- [x] 1.3 Add serde round-trip tests for all twelve kinds (construct →
      serialize → deserialize → assert equality), replacing S0's
      marker-only test per crate.
      Evidence: 11 `round_trip_test!`-generated tests in `records.rs` +
      `handoff::tests::handoff_round_trips_and_carries_full_envelope`;
      `cargo test -p canon-model` — 48/48 green.

## 2. Join-spine keys

- [x] 2.1 Define the eight join-key newtypes (`ChangeId`, `TaskId`,
      `ScenarioId`, `SessionId`, `RunId`, `HandoffId`, `Sha`/`PrNumber`,
      `RegimeKey`), each with a doc comment stating its grammar and its
      "joins" relationships (source for the generated doc, task 2.3).
      Evidence: `crates/canon-model/src/ids.rs`, `join_key_newtype!`
      macro (grammar/joins doc comment + `GRAMMAR`/`JOINS` consts from
      one literal source).
- [x] 2.2 Enforce `ScenarioId`'s `<area>.<surface>.<nn>` grammar and
      `TaskId`'s `<change_id>#<n>` grammar at construction (reject
      malformed input; no renumbering mechanism for an existing
      `scenario_id`).
      Evidence: `ids::is_scenario_id`/`is_task_id` (ported from
      `tools/parity.py::SCENARIO_ID_RE`); `ids::tests::scenario_id_rejects_malformed`,
      `task_id_parses_hierarchical_numbers` green; no mutation API exists
      on `ScenarioId` (fields are `parse`-only, no setter).
- [x] 2.3 Add a join-spine doc generator (an `xtask`/binary target reading
      the newtypes' doc comments) that emits the eight-key table into a
      committed doc file; wire it into a `cargo xtask check-generated` (or
      equivalent) step that diffs generator output against the committed
      file and fails non-zero on drift.
      Evidence: `crates/canon-model/src/bin/xtask.rs` + `.cargo/config.toml`
      alias; `crates/canon-model/JOIN_SPINE.md` committed. Verified live:
      tampering `JOIN_SPINE.md` then running `cargo xtask check-generated`
      exits 1 with a DRIFT line; restoring it exits 0. Also asserted by
      `gen::tests::committed_generated_output_matches_current_source`
      (part of `cargo test --workspace`).

## 3. JSON-schema export

- [x] 3.1 Add `schemars` (or equivalent) derive to every record kind and
      every join-key newtype.
      Evidence: `#[derive(..., JsonSchema)]` on all 12 record kinds +
      hand-written `JsonSchema` impls for every join-key newtype
      (`ids.rs`'s macro; `RunId`/`PrNumber` hand-specialized since
      `ulid`/no external crate offers a derive for a validated wrapper).
- [x] 3.2 Add a schema-export step emitting one `.schema.json` per record
      kind into a `schemas/` output directory; wire it into the same
      `cargo xtask check-generated` step as task 2.3 so schema drift also
      fails CI.
      Evidence: `crates/canon-model/src/schema_export.rs` +
      `crates/canon-model/schemas/*.schema.json` (12 files, committed);
      same `cargo xtask check-generated`/`gen::tests` drift check as 2.3
      covers both artifacts in one pass.
- [x] 3.3 Verify a field addition to any record kind changes only that
      kind's `.schema.json` (no other file needs a manual edit).
      Evidence: live-verified — added a temporary field to `Task`, ran
      `cargo xtask write`, `git status --short crates/canon-model/schemas/`
      showed only `task.schema.json` as modified (`AM`), all 11 others
      unchanged (`A`, i.e. still byte-identical to the staged baseline);
      reverted, re-ran `cargo xtask write` + `cargo test --workspace`
      (green) to restore.

## 4. Evidence integrity

- [x] 4.1 Define `FailureClass` as a fixed enum with a stable `as_str()`
      mapping (seed the initial set from `tools/parity.py`'s
      `FAILURE_CLASSES` where the same failure category applies — e.g.
      `malformed`).
      Evidence: `crates/canon-model/src/evidence.rs`, `FailureClass`
      (`Malformed` seeded from `parity.py`'s `"malformed"`; four
      canon-specific classes for concerns `parity.py` has none of).
      `evidence::tests::failure_class_as_str_matches_serde_kebab_case`
      asserts `as_str()` ≡ serde wire encoding.
- [x] 4.2 Implement `canon_model::validate_evidence(candidate: &RawRecord)
      -> Result<(), EvidenceViolation>` — skip + report on malformed
      input, never panic; accept and return `Ok(())` on well-formed input.
      Evidence: `evidence::validate_evidence` (+ `validate_evidence_batch`
      for the skip-not-crash loop, `_load_ledger`-style).
- [x] 4.3 Add tests: a record missing `actor` is skipped and reported with
      the correct `FailureClass`; a complete record validates with no
      violation; a batch of five records with one malformed entry
      processes the other four without aborting.
      Evidence: `evidence::tests::record_missing_actor_is_skipped_and_reported`,
      `well_formed_record_validates_with_no_violation`,
      `batch_of_five_with_one_malformed_processes_the_other_four` — all
      green.

## 5. Handoff wire-compat and template registry

- [x] 5.1 Define `Handoff`'s fixed state-machine fields (`id`, `state`,
      `chain_id`, `parent_handoff_id`, `seq`, `claimed_by`, `claimed_at`,
      `completed_at`, `abandoned_at`, `openspec_change_slug`,
      `research_vendor_slug`, `tags`, `title`) matching
      the donor monorepo's `handoffs` table's columns; `state` as a
      closed 4-variant enum (`Pending|InProgress|Done|Abandoned`).
      Evidence: `crates/canon-model/src/handoff.rs`, `Handoff` struct +
      `HandoffState`; field list read directly from
      the donor monorepo's `handoffs` table lines 17-54.
      Deliberately excludes `trigger`/`created_*`/`refs_extra` columns —
      not in this task's own literal field list (see S1 report).
- [x] 5.2 Add a mapping test asserting every `Handoff` field serializes to
      a column name and type matching the donor monorepo's `handoffs` table's Drizzle schema
      (read that file's column list as the fixture source of truth; no
      live hosted-Postgres connection required for this check).
      Evidence: `handoff::tests::every_field_maps_to_a_handoffs_ts_column`
      (checks all 13 task-5.1 columns' presence + required-ness against
      the JSON-schema output) — green, no DB connection.
- [x] 5.3 Reject invalid state transitions (`done`/`abandoned` are
      terminal — no transition back to `pending`/`in-progress`).
      Evidence: `HandoffState::can_transition_to` (closed 3-pair
      allow-list) + `Handoff::transition_to`;
      `handoff::tests::terminal_states_reject_transitions_back_to_pending_or_in_progress`
      green.
- [x] 5.4 Define `HandoffBody { domain: DomainId, template_version: u32,
      fields: serde_json::Value }` and the `HandoffTemplate` trait
      (`domain()`, `validate()`, `render()`).
      Evidence: `handoff.rs`, `HandoffBody` + `HandoffTemplate` trait.
- [x] 5.5 Implement the template registry: `canon.yaml`'s
      `handoff_templates:` key lists registered domains; registry lookup
      resolves `HandoffBody.domain` to its `HandoffTemplate` impl or
      returns an unregistered-domain violation.
      Evidence: `handoff::TemplateRegistry::from_manifest` (parses
      `canon.yaml`, gates compiled templates by that list);
      `handoff::tests::registers_gihoek_from_the_repos_own_canon_yaml`
      reads THIS repo's root `canon.yaml` (new file, `handoff_templates: [기획]`).
- [x] 5.6 Author one concrete template (기획: title + summary +
      acceptance-criteria fields) as the registry's fixture proof.
      Evidence: `handoff::GihoekTemplate` (title/summary/acceptance-criteria,
      all required).
- [x] 5.7 Add tests: a 기획-domain body with valid fields validates and
      renders; a 기획-domain body missing `acceptance-criteria` returns a
      structured violation naming the missing field; a body with an
      unregistered domain is rejected before write; two Handoffs with
      different domains still expose an identical state-machine field set.
      Evidence: `handoff::tests::gihoek_domain_with_valid_fields_validates_and_renders`,
      `gihoek_domain_missing_acceptance_criteria_names_the_field`,
      `unregistered_domain_is_rejected_before_write`,
      `two_domains_expose_identical_state_machine_fields` — all green.

## 6. Companion skill and fixtures

- [x] 6.1 Author this spec's companion skill under
      `canon/skills/state-model/SKILL.md`, covering: how to add a new
      record kind (the closed-set review process, D1), how to bump a
      kind's `schema` version, and how to register a new Handoff domain
      template.
      Evidence: `canon/skills/state-model/SKILL.md`; materialized via
      `canon skills install --source canon/skills --target .` (first run:
      "state-model v1 — installed"; second run: "state-model v1 —
      unchanged", byte-identical lock).
- [x] 6.2 Add a fixture corpus (`crates/canon-model/fixtures/`) covering at
      least one instance of each of the twelve record kinds (well-formed)
      plus deliberately malformed variants (missing `actor`, invalid
      `scenario_id` grammar, invalid `state` transition, unregistered
      Handoff domain) with an EXPECTED-violations file; add a
      `canon_model::fixtures::round_trip_all` (or equivalent) test that
      round-trips every well-formed fixture and asserts every malformed
      fixture produces exactly its EXPECTED `FailureClass` — this is the
      "schema crate round-trips all fixture corpora" acceptance bar from
      the design doc's S1 section.
      Evidence: `crates/canon-model/fixtures/well-formed/*.json` (12
      files) + `fixtures/malformed/*.json` (4 files) +
      `fixtures/EXPECTED-violations.json`; `fixtures::round_trip_all`
      (`crates/canon-model/src/fixtures.rs`) green.
