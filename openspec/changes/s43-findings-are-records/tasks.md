# s43 findings-are-records — tasks

## 1. The record kind

- [x] 1.1 `RecordKind::Finding` exists as the fourteenth kind, with a — ✅ RecordKind::Finding is the 14th kind with a fully typed body and no untyped escape hatch
      typed body carrying reviewed commit, round, seq, severity,
      disposition, reviewer, summary, and optional resolution/introducing
      commits and file ref. No untyped escape hatch field.
- [x] 1.2 Natural key is `{reviewed_sha}__{round}__{seq}`, resolved in — ✅ natural key is {change_id}__{round:04}__{seq:04}, resolved and validated in partition.rs; not area-scoped
      `partition.rs` with a `validate_body` arm and partition tests. Not
      area-scoped; routed in `canon.yaml`.
- [x] 1.3 Schema registered, a well-formed fixture exercises every field — ✅ schema registered and generated output regenerates drift-free at fourteen kinds
      including optionals, and generated output regenerates drift-free at
      fourteen kinds. Depends on 1.1.
- [x] 1.4 Doc comments state the three invariants: `resolution_sha` — ✅ the three invariants are in doc comments and the derivation test pins that fix-of-fix is computed, never stored
      coherence with `Fixed`, `introduced_by` is sourced or `None` and
      never inferred, and fix-of-fix is derived by join and never stored.

## 2. Authoring

- [x] 2.1 `canon finding add` writes a staged `Finding`, refusing every — ✅ canon finding add stages a Finding and refuses every row-line-break separator before staging anything
      line break that could forge a document row — the same set
      `canon evidence add` refuses — before staging anything.
- [x] 2.2 An incoherent finding is refused loudly and stages nothing: — ✅ an incoherent finding is refused loudly and stages nothing, each diagnostic naming what was wrong
      `Fixed` with no `resolution_sha`, a `resolution_sha` while `Open`,
      and a malformed SHA each name what was wrong. Depends on 2.1.
- [x] 2.3 The help states what the record does and does NOT establish, — ✅ the help states RECORDED OBSERVATION, NOT PROOF and enumerates what canon does not verify
      in the register `canon evidence add --help` set: a finding is a
      recorded observation, not proof the defect existed or was fixed.

## 3. Derivation

- [x] 3.1 `mart_review_rounds` reports, per round: reviewed sha, finding — ✅ mart_review_rounds reports per-round counts by severity and disposition, folded at the new kind's key
      count, counts by severity and by disposition. Folded at the new
      kind's key like every other multi-version view.
- [x] 3.2 The fix-of-fix count is computed by joining `introduced_by` — ✅ fix_of_fix is an EXISTS semi-join of introduced_by against earlier resolution_sha, with no stored boolean in the path
      against earlier findings' `resolution_sha` — no stored boolean
      anywhere in the path. Depends on 3.1.
- [x] 3.3 A finding with `introduced_by = None` is counted as UNKNOWN, — ✅ introduced_by_unsourced is its own UNKNOWN bucket, never folded into not-a-fix-of-fix
      never as not-a-fix-of-fix, and the panel states the count is a
      floor. Depends on 3.2.
- [x] 3.4 The panel reaches `.canon/REPORT.md`, the snapshot contract, — ✅ the panel reaches REPORT.md, the 8-table snapshot contract, and the dashboard twin
      and the dashboard twin, with the same claim on both surfaces.
      Depends on 3.1.

## 4. Dogfood

- [x] 4.1 v0.4.0's FINDING-BEARING rounds are backfilled from the review — ✅ rounds 8-11 transcribed; introduced_by left unset where no commit could be sourced, reviewed_sha absent for the round that reviewed an uncommitted worktree
      artifacts, with `introduced_by` set only where a commit can be
      sourced and left `None` otherwise. Originally worded "eleven
      rounds", which this change cannot deliver and does not: a round
      that finds nothing writes no record, so s42's rounds 1-7 and 12
      leave no trace and the rounds-RUN count is not derivable at all.
      The panel states plainly what it cannot count.
- [x] 4.2 The derived fix-of-fix count is whatever the records produce, — ✅ the fix-of-fix count is whatever the records produce, not a number carried in prose; my published `four` and my correction `two` were both wrong and the panel now states what the number means and in which directions it errs
      and the release note states what that number MEANS rather than
      asserting a total. This task originally predicted it would read
      `2` — it reads 19, and both my published `four` and my correction
      `two` were wrong: 13 findings across rounds 10 and 11 were
      introduced by pure fix commits, 6 more by a commit that mixed a
      fix with the feature and cannot be attributed either way. Writing
      the expected answer into the task was the same habit this change
      exists to break. Depends on 4.1.
- [x] 4.3 s43's own review rounds are recorded through `canon finding — ✅ s43's own review findings were recorded through canon finding add during the change, not backfilled at the end
      add` as they happen, not backfilled at the end. Depends on 2.1.

## 5. Verification

- [x] 5.1 `cargo test --workspace` green; live-pg suite green; — ✅ the workspace suite passes with zero failures at fourteen kinds
      generated-output drift clean at fourteen kinds.
- [x] 5.2 `canon gate check` clean, `report --check` no drift, `format` — ✅ gate clean, report --check no drift, format 0 violations, release-safety checker coherent
      0 violations, release-safety checker passes, website builds.
- [x] 5.3 Every task box above is closed through `canon evidence add` — ✅ every task box is closed through evidence add -> gate promote -> gate task; the gate refuses an unevidenced flip
      → `gate promote` → `gate task`, never by hand.
