# s49 authoring-the-corpus-agents-write — tasks

## 1. Provenance position

- [x] 1.1 `canon-fmt::gherkin::scan` accepts `# canon:` in the contiguous — ✅ leading block accepted, both positions count once; 43 canon-fmt tests green
      tag/comment block directly above a header, as well as the first
      non-blank line after it. Both positions on one header count once.
- [x] 1.2 `canon format`'s `missing-provenance` message names both — ✅ message: 'either directly above it (with its tags) or directly below it'
      accepted positions.
- [x] 1.3 Both scaffolds emit the leading form (comment, tag, header). — ✅ 20 scaffold tests green; canon format specs: 26 files, 0 violations over the pre-s49 trailing-form corpus
      Existing trailing-form fixtures stay clean under `canon format`.

## 2. Scaffold attribution

- [x] 2.1 `canon scenario new` / `canon feature new` take `--actor <id>`, — ✅ flag, then CANON_ACTOR, then canon-scaffold; smoke: agent_id Main / reviewer-bot on disk
      falling back to `CANON_ACTOR`, then `canon-scaffold`.
- [x] 2.2 The `at` stamp is truncated to whole seconds. — ✅ at stamped 2026-09-16T11:04:12Z, no fractional part

## 3. The lane axis

- [x] 3.1 `gherkin::scan` collects `@lane:<value>` per scenario exactly as — ✅ @lane collected, @foo:bar namespace collected, @p2 ignored
      `@subject:`.
- [x] 3.2 `Scenario.lane: Option<String>` — additive, `default` + — ✅ lane additive, absent key when unset, malformed slug rejected on deserialize; scenario.schema.json updated
      `skip_serializing_if`, kebab-slug shape on deserialize (the
      `domain` rule). Schema version unchanged. JSON schema export updated.
- [x] 3.3 `canon inventory sync` maps the tag onto `lane`; two tags → — ✅ lane on record; first wins; malformed dropped with diagnostic; smoke: bogus lane refused against canon.core enum
      first wins + counted diagnostic; a non-slug value → dropped +
      counted diagnostic; membership against a vocabulary `lane` enum
      when one is declared (fail-soft when none is).
- [x] 3.4 `canon.core` declares `lane: [behavior, design, architecture, — ✅ vocab.enums.lane: behavior, design, architecture, process
      process]`.
- [x] 3.5 `canon scenario new --lane <v>` emits `@lane:<v>` (membership — ✅ refused exit 2: not a valid value for lane (expected one of: behavior, design, architecture, process)
      checked at write, the `subject --domain` rule).
- [x] 3.6 `canon scenario new --subject <id>` emits `@subject:<id>`. — ✅ @subject:shop-core emitted above @lane and the id tag
- [x] 3.7 `canon inventory sync` counts `@name:value` tags whose namespace — ✅ smoke: [tag] shop.cart.04 carries an unrecognized @foo: namespace; sync exit 0
      is neither `subject` nor `lane` as a diagnostic; plain tags stay
      silent.

## 4. The gate honors lanes

- [x] 4.1 `spec_coverage.exclude_lanes: [<lane>, …]` in `policy.yaml`; — ✅ smoke: 3 uncovered to 2 with exclude_lanes [process]; Bad Lane poisons the section
      a listed lane is out of scope AFTER `scope` is applied. Absent →
      unchanged. Unknown key still fails loud (`deny_unknown_fields`).

## 5. Guidance

- [x] 5.1 `canon/skills/canon-authoring/SKILL.md`: area/surface/scenario — ✅ vocabulary, lanes + exclude_lanes yaml, subject pinning, commands, never hand-type # canon, 6 named anti-patterns, worked example
      definitions, one surface per file, lanes, subject pinning, never
      hand-type `# canon:`, the grab-bag anti-pattern.
- [x] 5.2 `canon init` prints `canon skills install` as its next step. — ✅ canon init: next: canon skills install to install authoring guidance
- [x] 5.3 `canon-fmt` (`missing-provenance` row), `canon-subject` (scenario — ✅ canon-authoring v1 installed; fmt/inventory/subject bumped by one; others unchanged
      pinning), `canon-inventory` (lane/subject/namespace diagnostics)
      updated. Skills re-materialized; lock bumps only changed skills.

## 6. Dogfood and acceptance

- [x] 6.1 Scenarios under `area=authoring` and `area=format` for every — ✅ 9 scenarios added across authoring/format/inventory/gate, each with a cited test the parent ran: 9/9 pass
      behavior above that a test exercises; each cites its test.
- [x] 6.2 `canon format specs` clean; `canon inventory sync` clean; — ✅ 26 files 0 violations; 226 scanned; gate check clean, --release clean, selftest ok
      `canon gate check` clean.
- [x] 6.3 `cargo test --workspace` green. — ✅ 1907 passed, 0 failed
