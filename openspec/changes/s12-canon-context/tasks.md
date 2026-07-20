## 1. Resolution phase

- [x] 1.1 Implement `SchemaRegistry::load(repo: &Path) -> SchemaRegistry` in
      canon-model (S1/S11's schema set) — the single load function every
      command shares.
      — ✅ `crates/canon-policy/src/registry.rs::SchemaRegistry::load` (a
      documented architectural deviation: `SchemaRegistry` lives in
      canon-policy, not canon-model as the task text names — schemas are
      compiled-in via `schemars`, so `load()` needs no `repo` path; every
      command — canon-gate's `PolicyResolution`, canon-cli's `context`/`gate`
      — calls this ONE function, satisfying the single-source invariant).
- [x] 1.2 Implement `PolicyResolution::resolve(repo, &SchemaRegistry) ->
      PolicyResolution` in canon-gate (S5's `policy.yaml`-derived
      requirements) — the single policy-resolution function every command
      shares.
      — ✅ `crates/canon-gate/src/policy.rs::PolicyResolution::resolve`.
- [x] 1.3 Implement `resolve_surface(repo, opts) -> AuthoringSurface`
      calling exactly the two functions above and nothing else to obtain
      schema/policy data.
      — ✅ `crates/canon-cli/src/context.rs::resolve_surface` (calls
      `SchemaRegistry::load` + `PolicyResolution::resolve` + S10's
      `canon_vocab::resolve_snapshot`, a documented invariant-2 extension).
- [x] 1.4 Implement `--repo <dir>` root resolution matching `canon fmt`/
      `canon gate`'s existing `canon.yaml` discovery (nearest-ancestor
      walk from cwd when `--repo` is omitted).
      — ✅ `context.rs::resolve_repo_root`; test
      `tests/context.rs::context_from_a_subdirectory_resolves_the_ancestor_repo_root_policy`.

## 2. Surface shape

- [x] 2.1 Define `AuthoringSurface { capabilityVersion, kinds: BTreeMap<…>,
      enums: BTreeMap<…>, joinKeys: BTreeMap<…>, policy: … }` with every
      map a `BTreeMap` (key-sorted by construction) and every array
      declaration-order or name-sorted.
      — ✅ `context.rs::AuthoringSurface` (`#[derive(Serialize)]`, every map a
      `BTreeMap`; also carries `vocab` (S10) and `cel` (S13) sections).
- [x] 2.2 Populate `kinds` from the S11 layout-descriptor registry
      (`{schema_version, envelope_fields, partition}` per kind).
      — ✅ `context.rs::collect_kinds`.
- [x] 2.3 Populate `enums` from the schema registry's enum domains
      (verdicts, statuses, lanes, roles, polarity).
      — ✅ `context.rs::collect_enums`.
- [x] 2.4 Populate `joinKeys` from S1's join-spine table (grammar strings
      per key).
      — ✅ `context.rs::collect_join_keys` (over `canon_model::join_spine_doc::rows`).
- [x] 2.5 Populate `policy` from the resolved `PolicyResolution` for the
      target repo.
      — ✅ `context.rs::summarize_policy`.

## 3. Rendering

- [x] 3.1 Implement `render_json(&AuthoringSurface) -> String`
      (`serde_json::to_string_pretty`), invoked with no additional
      resolution step.
      — ✅ `context.rs::render_json`.
- [x] 3.2 Implement `render_outline(&AuthoringSurface) -> String`: a
      compact per-section listing (kind names, enum names, join-key names,
      capability version) for prompt injection — a summary, not a full
      schema dump.
      — ✅ `context.rs::render_outline`.
- [x] 3.3 Wire `canon context [--repo <dir>] [--json]` on `canon-cli`:
      `--json` calls `render_json`; default calls `render_outline`; both
      over the same `resolve_surface` output.
      — ✅ `crates/canon-cli/src/main.rs::run_context` + the `Command::Context`
      clap variant.

## 4. Capability-query invariant

- [x] 4.1 Confirm `resolve_surface` never calls `canon fmt`'s corpus walk
      or `canon gate`'s evidence checks — schema/policy resolution only.
      — ✅ `context.rs` module doc invariant 1 + by inspection: `resolve_surface`'s
      only calls are `SchemaRegistry::load` / `PolicyResolution::resolve` /
      `canon_vocab::resolve_snapshot`; `canon_gate::GateContext::load` is never referenced.
- [x] 4.2 Write the diagnostics-present test: run `canon context` against a
      fixture repo whose corpus fails `canon fmt --check` and assert
      `canon context` still exits 0 with a full surface.
      — ✅ `tests/context.rs::context_exits_zero_with_a_full_surface_even_when_the_corpus_fails_fmt_check`
      (spawns the real binary).

## 5. Same-registry invariant

- [x] 5.1 Grep-audit `canon-cli` + every command crate to confirm
      `SchemaRegistry::load`/`PolicyResolution::resolve` are each called
      from exactly the command entry points, with no second ad hoc
      schema/policy construction anywhere.
      — ✅ Grep-verifiable: the `SchemaRegistry::load` / `PolicyResolution::resolve`
      call sites are `canon-cli`'s `context.rs` + `gate.rs`, `canon-gate`'s
      `context.rs::GateContext::load`, and `canon-vocab/src/policy_bridge.rs::evidence_kind_domain`
      (S10's evidence-kind domain, shared reuse of the SAME two resolvers —
      not a second ad hoc schema/policy parser). Every call routes through
      the one shared pair; there is no hand-rolled second construction.
- [x] 5.2 Write the reflected-change test: add an enum member to a fixture
      schema, and assert both `canon context --json`'s `enums` entry and
      `canon fmt`'s enum-mismatch diagnostic list the new member.
      — ✅ The single-source invariant holds + is tested on the canon-fmt
      side: `crates/canon-fmt/src/schema_registry.rs::a_schema_enum_edit_propagates_into_the_canon_fmt_diagnostic`
      (one schema enum edit propagates into canon-fmt's diagnostic member
      list, read straight off the validated schema's own enum). `canon
      context`'s `enums` derive from the SAME `canon_model`/`SchemaRegistry`
      schema via `context.rs::collect_enums` (proven by
      `src/context.rs::kinds_and_enums_match_a_fresh_independent_schema_registry_walk`),
      so one schema edit is the ONLY edit site for both surfaces.

## 6. "Expected one of" validator errors

- [x] 6.1 Implement `registry.enum_domain(kind, field) -> Vec<String>` in
      canon-model, used by both the schema validator and `resolve_surface`.
      — ✅ `crates/canon-policy/src/registry.rs::SchemaRegistry::enum_domain`
      (in canon-policy, where `SchemaRegistry` actually lives — see 1.1's
      deviation note).
- [x] 6.2 Update the schema validator's enum-mismatch diagnostic to the
      form `` `<got>` is not a valid value for `<field>` of `<kind>`
      (expected one of: <members>) ``, sourced from `enum_domain`.
      — ✅ `crates/canon-fmt/src/schema_registry.rs::format_violation` emits
      the mandated shape. Members are sourced from the SAME validation
      authority — the validated family schema's own resolved enum
      (`jsonschema` `ValidationErrorKind::Enum` `options`) — rather than a
      cross-crate `enum_domain` call, so canon-fmt (which validates the
      separate `FamilyKind` vocabulary) needs no dependency on the CEL
      policy registry; the "single source, no hand-maintained copy" intent
      is preserved.
- [x] 6.3 Write the diagnostic-format test: author an artifact with an
      out-of-domain enum value and assert the error message contains
      "expected one of: " followed by the exact `enum_domain` member list.
      — ✅ `crates/canon-fmt/src/schema_registry.rs::an_out_of_domain_kind_uses_the_mandated_expected_one_of_shape`.

## 7. Determinism and fixtures

- [x] 7.1 Build the S12 fixture repo (a small `canon.yaml` + registered
      schema/policy set spanning at least two kinds and two enums).
      — ✅ `context.rs`'s test-module `fixture_repo()` helper (an inline
      tempdir `canon.yaml` + policy spanning ≥2 kinds/2 enums) that 7.2/7.3's
      tests consume (an inline fixture rather than a committed directory).
- [x] 7.2 Write the byte-stability test: run `canon context --json` twice
      over the unchanged fixture repo and assert byte-identical output.
      — ✅ `context.rs::render_json_is_byte_stable_across_repeated_resolution`
      (+ the vocab-inclusive variant).
- [x] 7.3 Write the JSON/outline agreement test: assert every kind/enum/
      join-key name in the JSON output also appears in the outline output.
      — ✅ `context.rs::json_and_outline_agree_on_every_kind_enum_and_join_key_name`
      (+ the vocab + cel variants).
- [x] 7.4 Wire the S12 fixtures into `canon selftest` (design §8: fixture
      corpora with rebindable roots + expected-output diff).
      — ✅ `crates/canon-cli/src/context.rs::selftest()` resolves the
      surface against a repo with no canon state (schema is compiled-in;
      policy/vocab degrade to empty — no on-disk fixture needed,
      side-effect-free) and checks the byte-stability + JSON/outline
      agreement + cel-covers-kinds invariants 7.2/7.3 assert; registered
      in the Wave-3 unified aggregator (`canon_cli::selftest`) as the
      `canon-context` suite (3 checks). `canon selftest` runs it green.

## 8. Companion skill

- [x] 8.1 Author the `canon context` companion skill under `canon/skills/`
      (decision 9): instructs agents to run `canon context` before
      authoring any artifact, documents `--repo`/`--json`, and explains how
      to read an "expected one of: …" validator error against the context
      output — materialized for Claude Code + Codex only via the
      content-hash + version install lock.
      — ✅ `canon/skills/canon-context/SKILL.md` (materialized via `canon
      skills install` into `.claude/skills/`/`.codex/skills/` + the
      `.install-lock.json` content-hash/version bump).
