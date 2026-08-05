# s46 a-finding-can-be-closed — tasks

## 0. Baseline

- [x] 0.1 Record the committed before-measurement, each number produced by a — ✅ baseline: s44 row rounds_recorded=1 findings=14 severity_blocker=8 disposition_open=14 disposition_fixed=0; 14 files on disk all `open`; `canon finding --help` listed `add` only — ✅ Faithful evidence recorded 2026-08-05T14:21:24.097762+00:00 by canon
      pasted command's output, never typed: `mart_review_totals`'s s44 row
      (`rounds_recorded=1 findings=14 severity_blocker=8 disposition_open=14
      disposition_fixed=0`), the s44 `Finding` file count and disposition
      tally on disk (`14 {'open': 14}`), and `canon finding --help`'s verb
      list (`add` only). Phase 5 compares against these exact numbers.
- [x] 0.2 Record that `canon finding add` at an occupied key refuses, and the — ✅ captured verbatim in proposal.md — `refused — s44-spec-derived-worklist__0001__0001 is already occupied by a committed finding` — ✅ Faithful evidence recorded 2026-08-05T14:21:24.249831+00:00 by canon
      verbatim refusal, so the gap this change closes is a captured command
      output rather than a claim.

## 1. The predicate

- [x] 1.1 `Finding::is_disposition_transition_of` in `canon-model`: every — ✅ canon-model/src/records.rs `Finding::is_disposition_transition_of` — ✅ Faithful evidence recorded 2026-08-05T14:21:24.450622+00:00 by canon
      identity/content field byte-equal, disposition actually different,
      envelope excluded.
- [x] 1.2 It lives on the record, not in `canon-gate`, because only the kind — ✅ predicate is a `Finding` method; canon-gate calls it, never re-implements the field list — ✅ Faithful evidence recorded 2026-08-05T14:21:24.602037+00:00 by canon
      knows which of its fields carry identity and which carry state.
- [x] 1.3 A test asserts the accept case in both directions — `open → fixed` — ✅ `a_disposition_only_change_is_a_transition` asserts both directions — ✅ Faithful evidence recorded 2026-08-05T14:21:24.704497+00:00 by canon
      AND `fixed → open`. A ledger that could only close would record half a
      finding's life.
- [x] 1.4 A test asserts an unchanged disposition is NOT a transition. Without — ✅ `an_unchanged_disposition_is_not_a_transition` — ✅ Faithful evidence recorded 2026-08-05T14:21:24.785519+00:00 by canon
      it an identical body appends forever at one key, each copy a new
      `content_digest12` from its fresh envelope alone.
- [x] 1.5 A test moves each of the eight identity/content fields in turn, — ✅ `every_identity_field_defeats_the_transition` — 8 cases, each moving the disposition too — ✅ Faithful evidence recorded 2026-08-05T14:21:24.859317+00:00 by canon
      ALONGSIDE the disposition, and asserts each defeats the predicate. Every
      case moves the disposition too, so a predicate that checked only the
      disposition would pass all eight.
- [x] 1.6 A test asserts a later author at a later instant still transitions. — ✅ `a_later_author_at_a_later_instant_still_transitions` — ✅ Faithful evidence recorded 2026-08-05T14:21:24.938647+00:00 by canon

## 2. The exemption, at the authority

- [x] 2.1 `promote_verbatim` admits a second body at a `Unique` key iff the — ✅ promote.rs `is_disposition_transition` gates the `NaturalKeyRule::Unique` arm — ✅ Faithful evidence recorded 2026-08-05T14:21:25.059515+00:00 by canon
      predicate holds against the committed occupant. Not in the CLI verb: a
      hand-written body must clear the same bar.
- [x] 2.2 `committed_by_key` folds to the LATEST version per key through — ✅ `committed_by_key` built through `fold_latest_by_key`, first-path-wins removed — ✅ Faithful evidence recorded 2026-08-05T14:21:25.218679+00:00 by canon
      `canon_store::fold::fold_latest_by_key` — the same supersession rule
      every reader uses — replacing the pre-s46 first-path-wins pick. Which
      version is "the occupant" became a real question the moment a key could
      legitimately carry two.
- [x] 2.3 A candidate promoted earlier in the SAME drain never counts as the — ✅ `!claimed_here.contains(&natural_key) && ...` guards the exemption — ✅ Faithful evidence recorded 2026-08-05T14:21:25.393597+00:00 by canon
      occupant to supersede: two transitions of one key in one call have no
      defined order between them.
- [x] 2.4 An unreadable or unparseable occupant answers `false`. Refusing on — ✅ unreadable/unparseable occupant returns false; documented as the safe direction — ✅ Faithful evidence recorded 2026-08-05T14:21:25.540679+00:00 by canon
      "I could not read what is already there" is the safe direction.
- [x] 2.5 `NaturalKeyRule::Unique`'s doc states the exemption and why it is — ✅ `NaturalKeyRule::Unique` doc rewritten; the false 'At most ONE' sentence is gone — ✅ Faithful evidence recorded 2026-08-05T14:21:25.639074+00:00 by canon
      NARROWER than `Versioned`, not a softer spelling of it. The doc as
      written before this change ("At most ONE committed record per natural
      key") is now false and must not survive.
- [x] 2.6 Tests: transition admitted alongside its predecessor (both records — ✅ 4 tests in promote.rs; canon-gate 151 passed — ✅ Faithful evidence recorded 2026-08-05T14:21:25.727913+00:00 by canon
      committed); a resubmitted CURRENT disposition refused against the latest
      version (no third body, forever); two transitions in one drain commit
      one and refuse one; a body that moves the summary as well as the
      disposition still refused with the two-findings-one-identity message.

## 3. The verb

- [x] 3.1 `canon finding close --change-id --round --seq --disposition — ✅ `canon finding close`, FindingCloseArgs + run_close — ✅ Faithful evidence recorded 2026-08-05T14:21:25.793132+00:00 by canon
      [--resolution-sha]`, staging the transition of an already-committed
      finding.
- [x] 3.2 A separate verb, not a flag on `add`: it READS the committed record — ✅ every content field READ from the committed record; FindingCloseArgs carries no severity/reviewer/summary — ✅ Faithful evidence recorded 2026-08-05T14:21:25.949950+00:00 by canon
      and copies every content field, so a close structurally cannot alter
      severity, reviewer, summary, or either sourced sha.
- [x] 3.3 The lookup folds to the CURRENT version, not the first record read. — ✅ `find_finding` folds via `fold_latest_by_key`; `the_current_disposition_is_the_latest_version_not_the_first_read` — ✅ Faithful evidence recorded 2026-08-05T14:21:26.057935+00:00 by canon
      A key legitimately carries two after one close; comparing against the
      original would miss the no-op and stage a third record saying nothing.
- [x] 3.4 Refuses a key no COMMITTED finding holds, and refuses a — ✅ `close_refuses_a_key_no_committed_finding_holds`, `close_refuses_a_finding_that_was_never_committed` — ✅ Faithful evidence recorded 2026-08-05T14:21:26.224796+00:00 by canon
      staged-but-unpromoted one by name — the latter with the honest repair
      (promote it, or edit the staged copy).
- [x] 3.5 Enforces the `disposition ⇔ resolution_sha` biconditional BEFORE the — ✅ `close_enforces_the_disposition_resolution_sha_pair`, `close_refuses_a_resolution_sha_this_repo_does_not_hold` — ✅ Faithful evidence recorded 2026-08-05T14:21:26.491185+00:00 by canon
      lookup, so a caller who named the wrong flags is told that rather than
      told their finding is missing. `--resolution-sha` must name a commit
      this repo holds.
- [x] 3.6 Closing to the CURRENT disposition exits 0 and stages nothing — — ✅ `closing_to_the_current_disposition_is_a_no_op_that_stages_nothing` — ✅ Faithful evidence recorded 2026-08-05T14:21:26.733974+00:00 by canon
      re-running a close is how a caller recovers from a half-finished batch.
- [x] 3.7 Reopening is permitted: a fix that did not hold is a fact the ledger — ✅ `a_finding_can_be_reopened` — ✅ Faithful evidence recorded 2026-08-05T14:21:26.915813+00:00 by canon
      must carry.
- [x] 3.8 `canon finding add`'s occupied-key refusal names `close` for the — ✅ add's refusal now ends `To move THAT finding's disposition instead, use \`canon finding close\`` — ✅ Faithful evidence recorded 2026-08-05T14:21:27.032412+00:00 by canon
      case the author actually meant.
- [x] 3.9 Tests for 3.1–3.7, including one asserting the committed corpus is — ✅ 9 tests incl. `close_never_edits_the_record_it_supersedes`; canon-cli 294 passed — ✅ Faithful evidence recorded 2026-08-05T14:21:27.194931+00:00 by canon
      byte-unchanged by a close.

## 4. The reader

- [x] 4.1 `canon query --kind finding` folds to one current row per natural — ✅ `fold_subject_kind` renamed to `fold_rewritten_kind`, matching `Subject | Finding` — ✅ Faithful evidence recorded 2026-08-05T14:21:27.359722+00:00 by canon
      key, via the existing `fold_subject_kind` generalized to
      `fold_rewritten_kind` over `Subject | Finding` — not a third
      near-duplicate function.
- [x] 4.2 `mart_review_rounds` and `views.sql` are UNCHANGED. The SQL fold was — ✅ `git status --short crates/canon-store/sql/views.sql crates/canon-report/src/marts.rs packages/dashboard` is empty — ✅ Faithful evidence recorded 2026-08-05T14:21:27.476356+00:00 by canon
      already correct; verify rather than edit, and record that the diff is
      empty.

## 5. Acceptance

- [x] 5.1 Close all 14 s44 findings against their real resolution commits, — ✅ `canon gate promote: promoted evidence_record=0, finding=14` — ✅ Faithful evidence recorded 2026-08-05T14:21:27.653653+00:00 by canon
      through the gate, and record the promoted count.
- [x] 5.2 `mart_review_totals`'s s44 row reads `disposition_open=0 — ✅ s44 row now `findings=14 severity_blocker=8 disposition_open=0 disposition_fixed=14` — ✅ Faithful evidence recorded 2026-08-05T14:21:27.875050+00:00 by canon
      disposition_fixed=14`, `findings` still 14, and `severity_blocker`
      unchanged at 8 — the fold keeps one version per key, so closing must
      move the disposition split and NOTHING else.
- [x] 5.3 The s44 `Finding` file count on disk is 28 and `canon query --kind — ✅ 28 files on disk, `canon query --kind finding` reports 14 s44 rows — ✅ Faithful evidence recorded 2026-08-05T14:21:28.016860+00:00 by canon
      finding` reports 14 for s44: the pair is the history, folded to one
      current row.
- [x] 5.4 `canon report --check` is drift-free, `canon gate check` is clean, — ✅ report --check no drift; gate check clean (0 violations); format spec 0 violations; 90 suites green — ✅ Faithful evidence recorded 2026-08-05T14:21:28.198467+00:00 by canon
      `canon format spec` is clean, and the full workspace suite passes.
- [x] 5.5 `.canon/REPORT.md`'s regenerated review totals row is committed, and — ✅ .canon/REPORT.md regenerated and committed; the release sentence is copied from the totals row — ✅ Faithful evidence recorded 2026-08-05T14:21:28.315957+00:00 by canon
      the release narrative is copied from it rather than composed.

## 6. Docs

- [x] 6.1 `crates/canon-cli/src/finding.rs`'s module doc gains the section — ✅ finding.rs module doc — 'A finding is raised open and CLOSED later' — ✅ Faithful evidence recorded 2026-08-05T14:21:28.409266+00:00 by canon
      explaining what a second body at an occupied key legitimately IS, and
      why `close` is a verb rather than a flag.
- [x] 6.2 `canon finding`'s top-level `after_help` shows the full loop — ✅ `canon finding --help` shows the four-step loop and that the committed record stays — ✅ Faithful evidence recorded 2026-08-05T14:21:28.517226+00:00 by canon
      including `close`, and states that the committed record stays.
- [x] 6.3 `canon-report-dashboard`'s skill panel prose names `canon finding — ✅ canon-report-dashboard v18 installed into .claude/skills and .codex/skills — ✅ Faithful evidence recorded 2026-08-05T14:21:28.575246+00:00 by canon
      close` as how a disposition moves, and the skills are re-materialized
      into `.claude/skills` and `.codex/skills`.
