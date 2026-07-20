# s36 subject-domain-loop — tasks

## 1. canon-model foundation (D1 13th kind + D2 domain + D3 additive Change)

- [x] 1.1 D1: `RecordKind::Subject` added to the enum, `ALL`
      (`[RecordKind; 13]`, appended after `EvidenceRecord`), `as_str`
      (`"subject"`), and the by-id flat `partition_template` shape
      (`is_area_scoped` false). Module doc + `CanonRecord` trait doc +
      the `all_thirteen_kinds_present_exactly_once` test all moved from
      twelve/`== 12` to thirteen/`== 13`. Evidence: `cargo test
      -p canon-model` — `envelope::tests` green, `RecordKind::ALL.len()
      == 13`.
- [x] 1.2 D1: `SubjectId` join-spine-key newtype in `ids.rs` via the
      existing `join_key_newtype!` macro (kebab-slug grammar, reusing
      `is_kebab_slug`), `impl SubjectId::parse` mirroring `ChangeId`.
      Added to `join_spine_doc::rows()` (ninth row, joins "subject ↔
      change ↔ scenario"); module doc + render text + row-count test
      moved eight → nine. Evidence: `ids::tests::
      subject_id_accepts_and_rejects_like_a_kebab_slug` green;
      `join_spine_doc::tests::exactly_nine_rows` green.
- [x] 1.3 D2: `Subject` record struct (`envelope` flatten,
      `subject_id`, `title`, `summary`, `domain: String`, `status:
      SubjectStatus`, `owner_role`, `change_ids`, `scenario_ids`);
      `SubjectStatus` enum (`proposed|specced|building|verifying|
      shipped|retired`, snake_case). `domain` validated shape-only at
      parse via `deserialize_domain_slug` (kebab), vocabulary left to
      `canon/vocab` — mirrors `HandoffBody.domain`. `impl CanonRecord`.
      Evidence: `records::tests::subject_round_trips`,
      `subject_without_links_round_trips`,
      `subject_with_malformed_domain_fails_to_deserialize` green.
- [x] 1.4 D3: `Change.subject_id: Option<SubjectId>` (additive,
      `#[serde(default, skip_serializing_if = "Option::is_none")]`);
      `Change::new` defaults it `None`. Evidence: `records::tests::
      change_with_subject_id_round_trips` and
      `change_without_subject_id_key_deserializes_none_and_reserializes_without_the_key`
      green.
- [x] 1.5 Regenerate `schemas/subject.schema.json` + `JOIN_SPINE.md`
      via `cargo xtask write`; `subject.schema.json` is exported
      automatically (keyed by `kind.as_str()`, no explicit per-kind
      list to extend). Evidence: `git status` shows new
      `crates/canon-model/schemas/subject.schema.json`, modified
      `schemas/change.schema.json` (+ `subject_id`) + `JOIN_SPINE.md`;
      `gen::tests::committed_generated_output_matches_current_source`
      green (drift clean).
- [x] 1.6 Round-trip fixture `fixtures/well-formed/subject.json`;
      `fixtures.rs` dispatch arm + corpus count moved 12 → 13.
      Evidence: `fixtures::round_trip_all` green (13 well-formed
      fixtures, one per kind).
- [x] 1.7 Downstream exhaustiveness: minimal `RecordKind::Subject` arm
      in `canon-store::partition::{resolve_partition, validate_body}`
      (by-id flat, natural key = `subject_id`, mirrors `Change`).
      Evidence: `cargo check -p canon-model -p canon-store` clean; no
      other crate's `match RecordKind` broke.

## 2. canon-store routing/aging

- [x] 2.1 `subject: local` routing added to the repo's `canon.yaml` and
      auto-scaffolded by `canon init` (routing loop iterates
      `RecordKind::ALL`, so the 13th kind lands for free); persistence
      exercised through `TierRegistry` by the `canon subject` CLI
      integration tests. Evidence: `canon-store` `real_canon_yaml`
      routing test green; `canon-cli/tests/subject.rs` new→query
      round-trip green.
- [x] 2.2 `mart_subjects` view appended to `canon-store/sql/views.sql`
      (reads `stg_records WHERE kind='subject'` like `mart_trust_matrix`
      — no per-kind `stg_` view needed). Evidence:
      `canon-report/tests/marts.rs::subjects_matches_the_fixture_corpus_exactly`
      green.

## 3. canon/vocab domain enum

- [x] 3.1 Base domain enum (`planning|design|dev|data|test`) declared in
      `canon/vocab/canon.core/enums.yaml` with consumer-extension doc.
      Evidence: `canon context` enums section lists `domain: planning,
      design, dev, data, test`.
- [ ] 3.2 Write-time domain-membership validation wired through the
      vocab index (shape stays canon-model's; membership is the vocab
      layer's).

## 4. canon-cli surface

- [x] 4.1 `canon subject new <id> --domain <d> --title <t> [--summary]
      [--owner-role]` — envelope-stamped, shape-validated, duplicate id
      refused. Evidence: `canon-cli/tests/subject.rs` (new→query
      round-trip, duplicate refused) green.
- [x] 4.2 `canon subject adopt <change_id> --subject <id>` — stamps
      `Change.subject_id`, appends deduped `Subject.change_ids`, both
      re-persisted so fold-latest reads one row. `--derive` (Subject
      stub from a Change's `## Why`) **DEFERRED** to a follow-up change
      — adopt-after-ingest covers the fixed contract. Evidence:
      `tests/subject.rs::adopt` cases green.
- [x] 4.3 `canon subject status <id> <state>` — transition chain
      enforced (off-chain refused exit 2); `verifying → shipped`
      evidence-gated via `canon_gate::latest_verdicts` (every linked
      scenario non-Divergent), violations by failure class, fail closed
      exit 1. Evidence: `tests/subject.rs` shipped-blocked/-allowed +
      off-chain cases green.
- [x] 4.4 `canon query --kind subject [--domain <d>] [--status <s>]`
      (per-flag kind-gating, subject fold-latest) + `canon report`
      Subjects panel (`mart_subjects` per-domain rollup, snapshot table
      7/7, dashboard fixture regenerated). Evidence: query/report/
      snapshot/fresh-repo suites green.

## 5. canon-learn hierarchical regime fallback

- [x] 5.1 Hierarchical retrieval ladder shipped as in-segment encoding:
      area candidates `[<domain>-<subject_id>, <domain>]` (RegimeKey
      stays 4 fixed segments — supersedes this row's earlier
      `<domain>/<subject_id>` slash phrasing; candidates are always
      derived from structured inputs, never parsed back).
      `canon_learn::retrieve_first_nonempty` + `canon retrieve
      (--regime | --domain [--subject])` XOR surface, fail-soft,
      serving-regime reported on fallback. Evidence: `canon-learn`
      guidance unit tests + `canon-cli/tests/retrieve.rs` fallback
      integration tests green.
- [x] 5.2 Subject→domain consolidation contract documented (guidance
      module doc + `canon-subject` skill): on `shipped`/`retired` an
      agent re-distills still-valid subject-scoped strategies to the
      `<domain>` area via the EXISTING `canon learn promote` flow — by
      design no new code path, never an LLM call inside canon.

## 6. canon-subject skill + inventory tag

- [x] 6.1 `canon/skills/canon-subject/SKILL.md` — subject-write-first
      loop, vocabulary rule ("feature docs" = Gherkin `.feature` files;
      "subject" = the management record), lifecycle + shipped gate,
      adopt flow; materialized via `canon skills install`.
- [x] 6.2 Gherkin `@subject:<id>` tag mapped by `canon inventory sync`
      onto `Scenario.subject_id` (additive model field; malformed tag =
      named fail-soft diagnostic, scenario still indexed; multi-tag =
      violation, first wins). Evidence: inventory sync tests (28) green.
- [x] 6.3 Website docs EN+KO: `canon subject *` command rows, retrieve
      `--domain/--subject`, concepts Subjects section + vocabulary
      rule, examples §5. Evidence: `bun run build` green, 21 pages.

## 7. Verification

- [x] 7.1 `cargo test --workspace` green offline; `canon selftest` (11
      suites) + `canon gate selftest` green.
- [x] 7.2 Live smoke: authored `checkout-flow` (dev), ingested a plan
      change, adopted it, walked proposed→specced→building→verifying→
      shipped, queried `--kind subject --domain dev` (1 row), off-chain
      `shipped → building` refused with the chain named.
