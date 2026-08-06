# s48 the-whole-surface-specced — tasks

## 0. Baseline

- [x] 0.1 Record the committed before-measurement, each number produced by a — ✅ baseline: 1 area, 8 scenarios, 1 subject; surface = 25 commands, 14 kinds, 1673 #[test] — ✅ Faithful evidence recorded 2026-08-06T11:22:02.692034+00:00 by canon
      pasted command's output, never typed: 1 canon spec area (`finding`),
      8 canon scenarios, 1 Subject at `verifying`, and the counted surface
      this is measured against — 25 commands from `canon --help`, 14 record
      kinds, 1673 `#[test]` across ten crates.
- [x] 0.2 Record that `canon gate check` was clean at that baseline, and WHY — ✅ clean, but 8 of 24 scenarios in scope — the clean measured almost nothing — ✅ Faithful evidence recorded 2026-08-06T11:22:02.880377+00:00 by canon
      that clean was weak: almost nothing was in scope.

## 1. Subjects

- [x] 1.1 One Subject per area, twenty joining `finding-lifecycle`, each — ✅ 21 subjects at `verifying` (20 new + finding-lifecycle), all advanced through the CLI — ✅ Faithful evidence recorded 2026-08-06T11:22:02.973509+00:00 by canon
      advanced `proposed → specced → building → verifying` through the CLI so
      every area is inside `scope: [building, verifying]`.
- [x] 1.2 `examples/platformer` gets NO Subject at `building`/`verifying`. It — ✅ platformer-traversal stays `proposed`; 10 platformer scenarios carry no subject at all — ✅ Faithful evidence recorded 2026-08-06T11:22:03.083539+00:00 by canon
      is a demo, not a product unit, and pinning it to one to make a number
      move is the fabrication this gate exists to refuse.

## 2. The corpus

- [x] 2.1 Twenty-six `.feature` files under — ✅ 26 .feature files across 21 areas — ✅ Faithful evidence recorded 2026-08-06T11:22:03.233305+00:00 by canon
      `specs/features/kind=feature/area=<area>/`, covering context, format,
      authoring, inventory, gate, evidence, finding, review, divergence,
      subject, ingest, query, report, retrieve, learn, dispatch, tier,
      plugin, policy, vocab and skills.
- [x] 2.2 `canon format specs` reports every file checked with zero — ✅ `canon format specs` — 26 file(s) checked, 0 violation(s); non-zero, unlike s47's finding — ✅ Faithful evidence recorded 2026-08-06T11:22:03.320118+00:00 by canon
      violations, and the count is non-zero — s47's finding was that a
      zero-file "clean" proves nothing.
- [x] 2.3 `canon inventory sync` materializes one `Scenario` per tagged — ✅ `inventory sync` root canon — 217 scanned, 209 written (8 unchanged), 0 diagnostics — ✅ Faithful evidence recorded 2026-08-06T11:22:03.502030+00:00 by canon
      scenario under project `canon`, with zero diagnostics.
- [x] 2.4 Every scenario carries `@subject:<id>` resolving to a real Subject; — ✅ read back from folded records: 217/217 canon scenarios carry a subject_id, 0 dangling — ✅ Faithful evidence recorded 2026-08-06T11:22:03.670378+00:00 by canon
      verified by reading the folded records' `subject_id`, not the tags.
- [x] 2.5 Scenarios describe OBSERVABLE behavior — a refusal, a transition, a — ✅ titles are claims (`A subagent transcript becomes a child run under the main agent's root run`), never fn names — ✅ Faithful evidence recorded 2026-08-06T11:22:03.797801+00:00 by canon
      derived number, an idempotence property, a boundary — never a function
      name. A reader who has not seen the code can tell what canon does.

## 3. Evidence, and the audit that earns it

- [x] 3.1 Every scenario gets an `EvidenceRecord` keyed `(canon, <id>)` whose — ✅ 209 EvidenceRecords keyed (canon, <id>) + s47's 8 = 217 — ✅ Faithful evidence recorded 2026-08-06T11:22:03.969133+00:00 by canon
      `ref` is the exact `cargo test` invocation exercising it.
- [x] 3.2 EVERY cited invocation is executed by the parent and observed — ✅ parent re-ran all 209 cited invocations: PASS 209 / 209, FAIL 0 — not trusted from the authors' reports — ✅ Faithful evidence recorded 2026-08-06T11:22:04.094675+00:00 by canon
      passing before any record is written — not trusted from the author's
      report. Record the pass count against the scenario count.
- [x] 3.3 Audit every citation for MEANING, not just for passing: a test that — ✅ semantic audit of all 209 citations flagged 15; each test body was read and judged individually — ✅ Faithful evidence recorded 2026-08-06T11:22:04.173419+00:00 by canon
      passes while testing something else is a false attestation that no
      green gate would catch. Record how many were flagged and what happened
      to each.
- [x] 3.4 A claim that outran its test is corrected by rewriting the SCENARIO — ✅ 11 scenarios rewritten to what their test proves; 4 flags were false positives and the claims stood. Zero tests loosened — ✅ Faithful evidence recorded 2026-08-06T11:22:04.265584+00:00 by canon
      to what the test proves — never by loosening the test, and never by
      leaving the claim standing. Record the count.
- [x] 3.5 A behavior with no test gets NO scenario. Record how many were — ✅ 29 behaviors reported unbacked and deliberately left out — the honest inventory of what canon does not test — ✅ Faithful evidence recorded 2026-08-06T11:22:04.334930+00:00 by canon
      reported unbacked and left out; that list is a real inventory of what
      canon does not test, and is worth more than a scenario nobody can
      attest.

## 4. The gate

- [x] 4.1 `canon gate check` clean with every canon area in scope. — ✅ `canon gate check: clean (0 violations)` with all 21 areas in scope — ✅ Faithful evidence recorded 2026-08-06T11:22:04.454373+00:00 by canon
- [x] 4.2 Prove NOT INERT at this scale: widening `scope` to include — ✅ scope+proposed → 3 violations (platformer.movement.03/04, moving.02); narrowed → 0. Not inert at 217 — ✅ Faithful evidence recorded 2026-08-06T11:22:04.517970+00:00 by canon
      `proposed` must still surface real violations, and narrowing it must
      remove them. A check over 217 scenarios that passes by seeing nothing
      is the failure this task exists to exclude.
- [x] 4.3 `spec_coverage`'s configuration is UNCHANGED from s47 — same — ✅ policy.yaml's spec_coverage block is byte-identical to s47's — ✅ Faithful evidence recorded 2026-08-06T11:22:04.606482+00:00 by canon
      `require_evidence`, same `scope`. What changed is the corpus, not the
      rule.

## 5. Acceptance

- [x] 5.1 `canon query --kind scenario` reads 233: 217 canon + 16 platformer. — ✅ `canon query --kind scenario` reads 233 = 217 canon + 16 platformer — ✅ Faithful evidence recorded 2026-08-06T11:22:04.720360+00:00 by canon
- [x] 5.2 Every canon scenario resolves to a Subject; zero dangling. — ✅ 217/217 resolve to a Subject, 0 dangling — ✅ Faithful evidence recorded 2026-08-06T11:22:04.787202+00:00 by canon
- [x] 5.3 `canon report --check` drift-free, and the scope panel carries — ✅ `canon report --check: no drift`; scope panel carries canon rows across every area — ✅ Faithful evidence recorded 2026-08-06T11:22:04.853881+00:00 by canon
      canon's own rows across every area.
- [x] 5.4 Full workspace suite green and the dashboard tests green. — ✅ 90 suites green; dashboard 112 tests, 0 fail — ✅ Faithful evidence recorded 2026-08-06T11:22:04.910548+00:00 by canon
- [x] 5.5 `canon gate check --release`, `canon gate selftest`, and — ✅ gate check --release clean; selftest ok; format 26 files and 6 files, 0 violations both roots — ✅ Faithful evidence recorded 2026-08-06T11:22:05.074722+00:00 by canon
      `canon format` over BOTH configured roots all clean.
