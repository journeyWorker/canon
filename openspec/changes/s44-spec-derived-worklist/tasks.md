# s44 spec-derived-worklist — tasks

## 0. Make the join real (prerequisite)

Round 1 R1: 0/35 evidence records carry `scenario_id`, 0/16 scenarios
carry `subject_id`, 0 `@subject:` tags exist. Nothing below phase 3 is
acceptable until this phase produces a corpus where the check can be
observed to be both non-empty and non-total.

- [ ] 0.1 Record the baseline as a committed measurement: counts of
      `Scenario` rows, `Scenario` rows with `subject_id`, `EvidenceRecord`
      rows, and `EvidenceRecord` rows with `scenario_id`, derived by a
      command whose output is pasted into the evidence note. This is the
      number phase 3's acceptance is compared against.
- [ ] 0.2 Tag `examples/platformer/specs`'s `.feature` files with
      `@subject:<id>` and re-run `canon inventory sync`, so a non-zero
      subset of `Scenario` rows carries `subject_id`
      (`records.rs:335-337` maps the tag; absent/malformed is fail-soft
      to `None`). A subset, deliberately not all 16 — the check must be
      observed distinguishing linked from unlinked. Depends on 0.1.
- [ ] 0.3 Author at least one scenario-keyed `EvidenceRecord`
      (`canon evidence add --scenario-id …`) so at least one scenario is
      genuinely covered, and leave others uncovered. Verify
      `CellSubject::of` (`coverage.rs:85-91`) resolves it as
      `CellSubject::Scenario` — it prefers `task_id`, so a record
      carrying both is NOT a scenario-keyed record for coverage purposes.
      Depends on 0.2.
- [ ] 0.4 Re-run 0.1's measurement and record the delta. Acceptance:
      strictly between 0 and all scenarios are covered, so phase 3 has a
      corpus that can distinguish a correct check from an all-red one and
      from an inert one. Depends on 0.3.

## 1. GateContext learns the spec corpus

- [ ] 1.1 `GateContext` carries `scenarios: Vec<Scenario>`,
      `divergences: Vec<Divergence>`, and `subjects: Vec<Subject>`
      alongside `evidence`, read in `GateContext::load`. Loaded HERE
      rather than by the check, because `GateContext` is the documented
      "loaded once per gate run" seam (`context.rs:108-109`) and a check
      opening its own tier forks that contract.
- [ ] 1.2 The three reads' malformed rows land in a SEPARATE field, not
      `violations`. `LedgerCheck` maps every `ctx.violations` entry to
      `MalformedEvidence` (`ledger.rs:133-138`) and is unconditionally in
      `check_set` (`dispatch.rs:49-53`), so reusing that field would turn
      a green repo red with no policy change. Pinned by a test: a corpus
      with one malformed `kind=scenario` row and no `spec_coverage`
      section produces the same violations as one without it.
      Depends on 1.1.
- [ ] 1.3 The three reads are UNCONDITIONAL, so adding `spec_coverage` to
      `policy.yaml` alone tightens the gate with zero corpus edits — the
      property pinned for `risk_routing` at `coverage.rs:237-250`. Pinned
      the same way: a test that adds only the policy section and asserts
      the violation set changes. Depends on 1.1.
- [ ] 1.4 Refuse rather than degrade when routing sends `scenario`,
      `divergence`, or `subject` off the rung `GateContext` reads.
      `load` hardcodes `GitTier` (`context.rs:144`) while `routing` is
      per-repo configurable (`canon.yaml:31-63`); a silently empty corpus
      makes the check fail OPEN. Either route the reads through
      `TierRegistry` or emit a diagnostic and refuse to enable
      `spec_coverage`. Test both branches. Depends on 1.1.
- [ ] 1.5 Update the three `GateContext` STRUCT-LITERAL sites that adding
      fields breaks: `coverage.rs:198`, `ledger.rs:167`,
      `staleness.rs:320`. The 22 `GateContext::load` call sites need no
      change because `load`'s signature is untouched:
      `canon-cli/src/gate.rs:121,267`; `canon-cli/src/subject.rs:429`;
      `canon-gate/src/context.rs:251,277`; `dispatch.rs:108`;
      `selftest.rs:239,248`; `staleness.rs:550,555,567,577,584`;
      `trust.rs:356,373,385,407,422,438,452,474,516`.
      `canon evidence add` constructs only `GateCtx`, never
      `GateContext` (`evidence.rs:368,445,450`), and is unaffected.
      Depends on 1.1.

## 2. The policy section

- [ ] 2.1 `PolicyResolution` resolves an OPTIONAL `spec_coverage`
      section: `require_evidence: bool` and `scope: Vec<SubjectStatus>`.
      Absent resolves to `None` and derives zero violations, mirroring
      the empty-`risk_routing` case pinned at `coverage.rs:228-235`.
      Update the three `PolicyResolution` struct literals the new field
      breaks: `coverage.rs:188`, `ledger.rs:157`, `staleness.rs:293`.
- [ ] 2.2 A PRESENT-but-malformed `spec_coverage` emits a
      `PolicyDiagnostic` AND resolves to a poisoned value that makes
      `SpecCoverageCheck` emit a violation, so loudness lands at CHECK
      time. `PolicyResolution::resolve`'s signature is frozen infallible
      (`policy.rs:42-43`, `:327-328`, S12 design D2) and is NOT touched.
      Test that a malformed section is never silently equivalent to an
      absent one. Depends on 2.1.
- [ ] 2.3 `scope` values validate against the real `SubjectStatus` domain
      (`records.rs:86-92`); an unknown status names the offender and the
      legal set. Absent or empty `scope` means every scenario is in
      scope. Depends on 2.1.
- [ ] 2.4 `canon context` reports the new section through the existing
      `PolicySurface`/`summarize_policy` seam
      (`canon-cli/src/context.rs:137-141`, `:398-409`), and
      `CURRENT_CAPABILITY_VERSION` (`context.rs:70`) is bumped to `2` —
      the field exists so consumers can detect surface growth, and adding
      a section without bumping it defeats that. Depends on 2.1.

## 3. SpecCoverageCheck

- [ ] 3.1 A new `GateCheck` left-joins `ctx.scenarios` against
      `ctx.evidence` on the composite `(project_id, scenario_id)`
      identity the model requires (`records.rs:302-304`), NOT on
      `scenario_id` alone — `canon.yaml` permits multiple `specs.roots[]`,
      so two roots may carry the same scenario id. A scenario with no
      matching evidence emits `uncovered-cell <scenario_id> — spec
      scenario has no evidence record`. Reuses
      `FailureClass::UncoveredCell`; the closed eight-member set
      (`failure_class.rs:88-97`) is NOT extended, per `subject.rs:437`.
- [ ] 3.2 Promote `live_bindings_of` out of `canon-report`
      (`divergence.rs:65-89`, currently private) into `canon-model`, and
      have both `canon divergence status` and this check use the one
      derivation. canon-gate may not depend on canon-report
      (`gate_independence.rs:59-64`), so reimplementing it locally would
      be the second fold R4 forbids. Depends on 1.1.
- [ ] 3.3 A scenario whose folded state is `Open`, `StillDivergent`, or
      `ResolvedInvalid` emits `uncovered-cell` with a state-naming
      detail. `ResolvedInvalid` (`fold.rs:60-72`) is included
      deliberately: it means the resolution has gone stale against the
      live sha, which is the most mismatched state in the enum. Depends
      on 3.2.
- [ ] 3.4 A scenario whose latest ledger verdict is `Divergent` emits a
      distinguishing detail, reusing `latest_verdicts` (`ledger.rs:98`,
      already `pub` and already consumed cross-crate at
      `subject.rs:430`) rather than a third derivation. Depends on 3.1.
- [ ] 3.5 `scope` narrows blocking to scenarios whose `subject_id` names
      a Subject in one of the configured statuses, joined against
      `ctx.subjects`. A scenario with no `subject_id` is in scope only
      when `scope` is absent; a `subject_id` naming no Subject record
      emits its own detail rather than being skipped, because a dangling
      link is a corpus defect the operator should see. Depends on 2.3,
      3.1, and 0.2 — 0.2 is what makes this observable at all.
- [ ] 3.6 Registered in `check_set` (`dispatch.rs:49-53`);
      `canon gate check --release` is a strict superset and stays
      unaffected. The new check's own cases live in its module's
      `#[test]`s, NOT the selftest fixture corpus, because
      `selftest.rs:425` asserts one fixture per `FAILURE_CLASSES` entry
      and `UncoveredCell` already owns its fixture. Depends on 3.1.
- [ ] 3.7 Module doc justifies a SEPARATE check with the three arguments
      that hold, not the one that merely explains `risk_routing`:
      (a) `CellSubject::of` (`coverage.rs:85-91`) is record→subject,
      the inverse of the corpus→subject direction a left join needs;
      (b) the two checks answer different questions under different
      policy sections with different opt-in semantics, and folding an
      opt-in-silent behavior into `CoverageCheck` makes its currently
      unconditional contract conditional; (c) `check_set` membership is
      itself the per-check axis. Also record that `risk_routing` cannot
      decide requiredness here — CEL binds to `RecordKind::EvidenceRecord`
      (`coverage.rs:16-19`) and evaluates only against records that
      exist (`coverage.rs:24-27`) — as supporting, not primary,
      reasoning.

## 4. Acceptance

- [ ] 4.1 Within-run A/B at ONE commit, the shape `plugin_sync.rs:174-189`
      already uses: run `canon gate check` over a corpus with no
      `spec_coverage` section, add the section, run again, and assert the
      first output is byte-identical to a golden captured from the
      current release binary. "Before and after this change" is not
      expressible in-tree and is not what is asserted.
- [ ] 4.2 A test asserts canon's own `.canon/policy.yaml` resolves to
      `spec_coverage: None`, so this repo's verdicts provably do not
      move. Turning it on for canon itself is a separate, deliberate act.
      Depends on 2.1.
- [ ] 4.3 `canon gate selftest` passes unchanged, with
      `FAILURE_CLASSES.len()` still 8 and the fixture bijection
      (`selftest.rs:425`) intact. Depends on 3.6.
- [ ] 4.4 On phase 0's corpus, the check emits violations for strictly
      more than zero and strictly fewer than all scenarios, with each
      class of detail (no-evidence, divergence-state, divergent-verdict,
      dangling-subject) exercised at least once. Depends on 0.4 and 3.5.
- [ ] 4.5 Record `canon gate check`'s wall time on canon's own corpus
      before and after the three added reads (R5). No threshold is
      asserted; the number is recorded so a later regression has a
      baseline. Depends on 1.1.

## 5. Docs

- [ ] 5.1 `canon-gate` skill documents the new check, the `spec_coverage`
      section, that it is opt-in, and that malformed rows from the new
      kinds do not surface as `malformed-evidence`. Regenerated into
      `.claude/`/`.codex/` by `canon skills install`, with the
      regenerated files as the task's evidence.
- [ ] 5.2 `canon-subject` skill notes that the corpus-wide check
      generalizes the `verifying → shipped` gate it already documents at
      `SKILL.md:84-113`, and that `@subject:` tagging is what brings a
      scenario into `scope`.

## Deferred to `s45-report-scenario-driver`

The report-side inventory move and the Task demotion. Round 1 established
the split: no shared source file, its own pinned Parquet/dashboard
contract (`snapshot.rs:22`, `marts.rs:53`), and a corrected premise —
`_No rows._` is caused by `task: hot` routing to a rung the report cannot
read (`views.sql:494-496`), not by plan-dialect ownership. Recorded here
so it is not re-derived wrongly.

## Deferred: website copy

`packages/website/.../trust-spine.mdx` (en + ko). No build-time coupling
to these crates and a separate review audience including a translation;
it should not gate the Rust change.
