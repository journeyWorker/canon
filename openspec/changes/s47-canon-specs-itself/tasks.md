# s47 canon-specs-itself — tasks

## 0. Baseline

- [x] 0.1 Record the committed before-measurement, each number produced by a — ✅ baseline: 1 root (platformer), 16 scenarios all platformer.*, 6 .feature files under examples/, spec_coverage absent — ✅ Faithful evidence recorded 2026-08-06T06:33:38.238818+00:00 by canon
      pasted command's output, never typed: `specs.roots[]` has ONE entry
      (`platformer`); `canon query --kind scenario` reads 16, all
      `platformer.*`; `find . -name '*.feature'` outside fixtures reads 6
      files, all under `examples/platformer/specs`; `spec_coverage` absent
      from `.canon/policy.yaml`, so `canon gate check` is clean by omission.
- [x] 0.2 Record what enabling it on the pre-s47 corpus would have said: 12 — ✅ 12 uncovered-cell, every one platformer.* — captured by appending the section, running the gate, restoring — ✅ Faithful evidence recorded 2026-08-06T06:33:38.278301+00:00 by canon
      `uncovered-cell` violations, every one a `platformer.*`. Captured by
      appending the section, running the gate, and restoring the file.
- [x] 0.3 Record the four pre-existing platformer evidence records verbatim, — ✅ recorded verbatim in proposal.md — ref `cargo test -p canon-gate spec_coverage` on platformer.movement.01 — ✅ Faithful evidence recorded 2026-08-06T06:33:38.311082+00:00 by canon
      including the `ref` that names a canon unit test against a game
      scenario. This is the claim s47 does not inherit.

## 1. The corpus

- [x] 1.1 `canon.yaml` gains a second `specs.roots[]` entry: id `canon`, root — ✅ canon.yaml `- id: canon / root: specs`, with the why-a-second-root comment — ✅ Faithful evidence recorded 2026-08-06T06:33:38.342729+00:00 by canon
      `specs`. The comment states why a second root exists at all and that
      `scope:` is what keeps a half-written area from blocking.
- [x] 1.2 `specs/features/kind=feature/area=finding/close.feature` — eight — ✅ specs/features/kind=feature/area=finding/close.feature — 8 scenarios — ✅ Faithful evidence recorded 2026-08-06T06:33:38.373845+00:00 by canon
      scenarios describing `canon finding close` and the promote-time
      transition exemption, in the tag grammar `<area>.<surface>.<nn>`.
- [x] 1.3 Each scenario carries `@subject:finding-lifecycle`, so the subject — ✅ `@subject:finding-lifecycle` on all 8 — ✅ Faithful evidence recorded 2026-08-06T06:33:38.404956+00:00 by canon
      join that `scope` filters on is populated at index time rather than
      backfilled.
- [x] 1.4 `canon format <root>` reports the new corpus clean, and `canon — ✅ `canon format specs` 1 file 0 violations; `canon inventory sync` root canon 8 scanned 8 written, 0 diagnostics — ✅ Faithful evidence recorded 2026-08-06T06:33:38.445374+00:00 by canon
      inventory sync` materializes exactly 8 `Scenario` records under project
      `canon` with zero diagnostics.
- [x] 1.5 Scenarios describe OBSERVABLE behavior, not test names: a reader who — ✅ scenarios name behavior (`A fix that did not hold reopens the finding`), never a test fn — ✅ Faithful evidence recorded 2026-08-06T06:33:38.489037+00:00 by canon
      has never seen the code can tell what canon does from the corpus alone.

## 2. The subject

- [x] 2.1 `canon subject new finding-lifecycle --domain dev`, advanced — ✅ subject new + status proposed→specced→building→verifying, all through the CLI — ✅ Faithful evidence recorded 2026-08-06T06:33:38.521950+00:00 by canon
      `proposed → specced → building → verifying` through the CLI, never by
      editing a record.
- [x] 2.2 All 8 canon scenarios resolve to it; verified by reading the folded — ✅ read back from folded Scenario records: 8/8 resolve to finding-lifecycle — ✅ Faithful evidence recorded 2026-08-06T06:33:38.552704+00:00 by canon
      `Scenario` records' `subject_id`, not by trusting the tag.

## 3. Evidence

- [x] 3.1 Every one of the 8 scenarios gets an `EvidenceRecord` keyed — ✅ 8 EvidenceRecords keyed (canon, finding.close.NN) — ✅ Faithful evidence recorded 2026-08-06T06:33:38.586515+00:00 by canon
      `(canon, <scenario_id>)` whose `ref` is the exact `cargo test`
      invocation exercising it.
- [x] 3.2 Each cited test is RUN and observed passing BEFORE its record is — ✅ all 8 cited tests run first — each `test result: ok. 1 passed` before its record was written — ✅ Faithful evidence recorded 2026-08-06T06:33:38.616578+00:00 by canon
      written. The list of 8 test names and their `1 passed` results is the
      task's evidence. An attestation authored ahead of the run is the thing
      this repo refuses.
- [x] 3.3 Promoted through `canon gate promote`, never written to the ledger — ✅ `canon gate promote: promoted evidence_record=8` — ✅ Faithful evidence recorded 2026-08-06T06:33:38.649062+00:00 by canon
      directly.

## 4. The gate

- [x] 4.1 `.canon/policy.yaml` gains `spec_coverage: require_evidence: true` — ✅ scope [building, verifying]; `canon gate check: clean (0 violations)` with the section ENABLED — ✅ Faithful evidence recorded 2026-08-06T06:33:38.680186+00:00 by canon
      with `scope: [building, verifying]`, and `canon gate check` is clean.
- [x] 4.2 Prove the check is NOT inert on canon's real corpus: widening — ✅ scope+proposed → 3 violations (platformer.movement.03/04, moving.02); narrowed → 0. Not inert — ✅ Faithful evidence recorded 2026-08-06T06:33:38.712987+00:00 by canon
      `scope` to include `proposed` must surface real violations, and
      narrowing it must remove them. Record both counts. A check that passes
      by seeing nothing is the failure mode this task exists to exclude.
- [x] 4.3 `scope` is used as documented — a subject-status filter, not a — ✅ 6 platformer scenarios excluded via subject platformer-traversal at `proposed`; 10 via no subject at all — ✅ Faithful evidence recorded 2026-08-06T06:33:38.743747+00:00 by canon
      project filter. Record which platformer scenarios are excluded and by
      which of the two mechanisms (subject at `proposed`, or no subject at
      all), so the exclusion is a stated consequence rather than a
      coincidence.

## 5. The authoring flag

- [x] 5.1 `canon feature new` and `canon scenario new` take `--project <id>`, — ✅ `--project <id>` on both, verified against the real two-root repo — ✅ Faithful evidence recorded 2026-08-06T06:33:38.779054+00:00 by canon
      selecting a configured `specs.roots[]` entry by the id that becomes the
      record's `project_id`.
- [x] 5.2 One shared resolver owns the rule for both commands; the four — ✅ shared `resolve_spec_root`; 4 resolution cases unit-tested + 4 command-level tests — ✅ Faithful evidence recorded 2026-08-06T06:33:38.811934+00:00 by canon
      resolution cases (lone root, named project, unknown id, ambiguous
      multi-root) are each unit-tested.
- [x] 5.3 The `///` sentences claiming these commands "have no `--spec-root` — ✅ both `has no --spec-root override` sentences removed — ✅ Faithful evidence recorded 2026-08-06T06:33:38.841063+00:00 by canon
      override" are now false and do not survive.
- [x] 5.4 A single-root repo's behavior and output are byte-unchanged. — ✅ single-root path asserted byte-unchanged by `feature_new_still_writes_under_the_lone_configured_root_with_no_project_flag` — ✅ Faithful evidence recorded 2026-08-06T06:33:38.870026+00:00 by canon

## 6. The three defects this surfaced

- [x] 6.1 Record as a finding: `canon format spec` names a POSITIONAL corpus — ✅ finding s47#1.1 — `canon format spec` checked a nonexistent `spec/`; real roots read 1 and 6 files — ✅ Faithful evidence recorded 2026-08-06T06:33:38.901658+00:00 by canon
      root, this repo has no `spec/`, so every release verification reporting
      `0 file(s) checked, 0 violation(s)` was reporting on a directory that
      does not exist.
- [x] 6.2 Record as a finding, and leave it OPEN: four `EvidenceRecord`s — ✅ finding s47#1.2, left OPEN — no verb retracts an EvidenceRecord — ✅ Faithful evidence recorded 2026-08-06T06:33:38.941797+00:00 by canon
      claim a canon unit test verifies a platformer game scenario.
      Append-only, so they stay, and canon has no verb that retracts an
      attestation — `canon finding close` moves a finding's disposition,
      nothing moves an `EvidenceRecord`'s. `open` is the true answer; closing
      it would be the fabrication this change is about.
- [x] 6.4 Record as a finding and FIX: `Scenario` is re-written by `canon — ✅ finding s47#1.3, fixed — Scenario added to fold_rewritten_kind; test fails 2-vs-1 without it — ✅ Faithful evidence recorded 2026-08-06T06:33:38.973073+00:00 by canon
      inventory sync` but was absent from `canon query`'s re-written-kind
      fold, so the gate folded 24 while `canon query` reported 38 for one
      corpus. A regression test must FAIL without the fix — assert the two
      agree, not that a count is 1.
- [x] 6.5 `Review` is deliberately NOT added to that fold list, and the reason — ✅ `review` absence documented on fold_rewritten_kind with the pinned-sha reason — ✅ Faithful evidence recorded 2026-08-06T06:33:39.004294+00:00 by canon
      is written where the list is: its natural key carries the pinned sha, so
      two attestations at two commits are two keys, never two versions.
- [x] 6.3 Every verification invocation in this change names a real root, and — ✅ every verification here names a real root; counts are 1 and 6, never 0 — ✅ Faithful evidence recorded 2026-08-06T06:33:39.035722+00:00 by canon
      the recorded counts are non-zero where a corpus exists.

## 7. Acceptance

- [x] 7.1 `canon gate check` clean with `spec_coverage` ENABLED — the first — ✅ `canon gate check: clean (0 violations)` with spec_coverage ENABLED against canon's own corpus — ✅ Faithful evidence recorded 2026-08-06T06:33:39.066412+00:00 by canon
      time canon's own corpus has been the thing measured.
- [x] 7.2 `canon query --kind scenario` reads 24: 16 platformer + 8 canon. — ✅ `canon query --kind scenario` reads 24 (16 platformer + 8 canon), was 38 before the fold fix — ✅ Faithful evidence recorded 2026-08-06T06:33:39.095948+00:00 by canon
- [x] 7.3 `canon report --check` drift-free and `.canon/REPORT.md`'s scope — ✅ `canon report --check: no drift`; scope panel carries 8 canon rows, evidence_covered ✓ green ✓ faithful — ✅ Faithful evidence recorded 2026-08-06T06:33:39.125130+00:00 by canon
      panel carries canon's own rows.
- [x] 7.4 Full workspace suite green, and the dashboard tests green. — ✅ 90 suites green — ✅ Faithful evidence recorded 2026-08-06T06:33:39.155482+00:00 by canon
- [x] 7.5 The two FIXED findings from phase 6 are closed through `canon — ✅ s47#1.1 and #1.3 closed at this change's commit; #1.2 stays open and the report shows it — ✅ Faithful evidence recorded 2026-08-06T06:33:39.186156+00:00 by canon
      finding close` at this change's commit — s46's machinery used for the
      first time on findings it did not author. The third stays `open`, and
      the report shows it open.
- [x] 7.6 s44's `canons_own_policy_does_not_enable_spec_coverage` guard fired — ✅ the s44 guard FAILED on the new section, then was replaced by canons_own_policy_enables_spec_coverage_against_its_own_corpus — ✅ Faithful evidence recorded 2026-08-06T06:33:39.216856+00:00 by canon
      when the section appeared, exactly as it was written to. Its successor
      asserts the ENABLED shape including the scope, so removing the scope
      fails here rather than silently making a demo corpus blocking.
