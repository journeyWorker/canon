## 1. Trust spine core (`canon-gate` crate)

- [x] 1.1 Scaffold `crates/canon-gate` consuming `canon-model`'s
      `EvidenceRecord`/`Change`/`Task` types and `canon-store`'s git-tier
      adapter (S1/S2 dependency).
      Evidence: `crates/canon-gate/{Cargo.toml,src/lib.rs,src/context.rs}`
      — `GateCtx`/`GateContext::load` read `EvidenceRecord`s through
      `canon_store::git_tier::GitTier` + `canon_model::validate_evidence_batch`;
      `cargo build -p canon-gate` + `cargo test -p canon-gate` green
      (26 tests, incl. `context::tests::load_reads_a_real_evidence_record_written_through_git_tier`).
- [x] 1.2 Implement the static coverage check: policy-derived required-cell
      existence over an artifact corpus (D3a); emit `uncovered-cell` on a
      required cell missing evidence.
      Evidence: `crates/canon-gate/src/coverage.rs::CoverageCheck` — a
      `GateCheck` folding `ctx.evidence` by `CellSubject` (`task_id`
      preferred, else `scenario_id`) and testing each
      `policy.risk_routing` key against the artifact's own existing
      record(s) (the only "fact" carrier `GateContext`'s frozen shape
      provides, `policy.rs`'s own record-scoped CEL contract); a
      required role with no matching-role record emits `uncovered-cell`.
      6 tests: `uncovered_cell_fires_when_a_required_role_has_no_
      matching_record`, `a_matching_role_record_satisfies_coverage_
      regardless_of_verdict` (coverage ≠ pass — design decision 1),
      `no_risk_routing_rules_means_zero_required_cells_fail_soft_
      default`, `a_policy_diff_alone_tightens_coverage_with_no_artifact_
      edits` (spec.md's own acceptance scenario, byte-identical corpus
      across the assertion), `distinct_task_ids_are_checked_
      independently`, `a_record_with_no_join_key_is_excluded_from_
      coverage_entirely` — all green (`cargo test -p canon-gate
      coverage::`).
- [x] 1.3 Implement the dynamic verdict-ledger check: latest matching
      evidence record per cell, subject to staleness (D3b).
      Evidence: `crates/canon-gate/src/ledger.rs` — two SEPARATE
      surfaces per design decision 1 ("'a test exists' and 'a test
      passed' are different facts"): `latest_verdicts(ctx)` folds
      `ctx.evidence` to the latest (subject, role) cell by
      `envelope.at` (the donor parity harness's index-builder last-wins
      pattern) — pass/fail + by-whom (`LedgerEntry.verdict`/
      `.agent_id`); `LedgerCheck` (the actual `GateCheck`) surfaces
      every already-collected `ctx.violations` (`EvidenceViolation`,
      canon-store's fail-loud twin of the soft-skip reader,
      ledger-reader.md §3.2) as `malformed-evidence` — never
      re-validates content itself. A failing (`Divergent`) verdict is
      NOT itself a `FAILURE_CLASSES` member (the closed 8-string set
      has none named for it) — it is a reported fact via
      `latest_verdicts`, consumed by `canon gate check`/`report`
      (task 1.9, not this batch), never a gate-blocking violation
      invented outside a coordinated FAILURE_CLASSES migration
      (design decision 9). 5 tests:
      `ledger_check_surfaces_every_evidence_violation_as_malformed_
      evidence`, `ledger_check_is_silent_on_a_failing_verdict_alone`,
      `latest_verdicts_last_wins_by_at_over_a_stale_earlier_record`,
      `latest_verdicts_tracks_distinct_roles_for_the_same_subject_
      independently`, `a_record_with_no_join_key_is_excluded_from_the_
      verdict_fold` — all green (`cargo test -p canon-gate ledger::`).
- [x] 1.4 Implement the trust-ladder lifecycle enum (`draft | reviewed |
      ratified` + `flagged` overlay) on `canon-model`'s evidence envelope;
      `unreviewed-promotion` when `reviewed` lacks a review-record.
      Evidence: `crates/canon-gate/src/trust.rs` — `TrustLadderCheck`
      (always-on `GateCheck`) independently re-reads the ledger's raw
      `EvidenceRecord` JSON (`ctx.evidence` already dropped unknown keys
      via `validate_evidence_batch`) for an interim `trust_ladder`
      companion tag (`TrustLadderTag` wrapping the FOUNDATION
      `TrustLadderState`, module doc's migration note — `EvidenceRecord`
      still carries no native `lifecycle`/`flagged` field, INTERFACE
      REQUEST unchanged), builds a `Review`-record index
      (`_review_index` analog), and emits `unreviewed-promotion` /
      `flagged` via `TrustRung::classify`;
      `reviewed_lifecycle_without_review_record_emits_unreviewed_promotion`,
      `reviewed_with_matching_review_record_is_not_a_violation`,
      `flagged_overlay_emits_flagged_violation_even_when_ratified`,
      `absent_trust_ladder_tag_defaults_to_draft_and_is_not_itself_a_violation`
      all green. `ReleaseTrustCheck` (a SEPARATE, opt-in `GateCheck`) emits
      `trust-below-required` scoped to a release profile
      (`class_below_required_release_trust_level_emits_trust_below_required`).
- [x] 1.5 Implement the flag ratchet: `flagged` settable only by a
      human-attributed actor; cleared only by a human-attributed
      clear-record staged in the same commit; one-way (no agent-originated
      clear).
      Evidence: `crates/canon-gate/src/trust.rs` — `is_human_actor`
      (structural check: `actor.role == Some("human")` exactly, never
      free-text on `agent_id`) + `attempt_clear` (only a human-attributed
      `clearing_actor` ever returns `Ok(FlaggedOverlay::clear())`; every
      other actor is refused via `FlagClearRejected`, overlay untouched —
      no bypass); `attempt_clear_rejects_an_agent_originated_actor`,
      `attempt_clear_rejects_an_unattributed_actor_even_if_it_names_itself_human`,
      `attempt_clear_honors_a_genuinely_human_attributed_actor`,
      `attempt_clear_on_an_already_clear_overlay_is_a_harmless_no_op` all
      green. The commit-time git-diff ratchet ENFORCEMENT (detecting a
      flag silently disappearing between HEAD and a staged blob) is the
      hook-seam's job (§4, `canon gate flag-clear`, not yet wired) — this
      task's core validation logic (who is allowed to clear, structurally)
      is what lands here.
- [x] 1.6 Define the `policy.yaml` schema (`trust_required`, `trust_sample`,
      `staleness.max_commits_behind`, `staleness.surface_scoped`,
      risk-routing) + loader.
      Evidence: `crates/canon-gate/src/policy.rs` — `RawPolicy`/`RawField<T>`
      (flat-or-`{cel: ...}`) schema + `PolicyResolution::resolve(repo,
      &SchemaRegistry) -> PolicyResolution` (THE single resolver S12 depends
      on, S12 design D2); `cargo test -p canon-gate` covers missing/malformed
      `policy.yaml`, flat-vs-CEL equivalence (`flat_and_cel_trust_required_
      agree_on_the_resolved_required_cell_set`,
      `conditional_cel_trust_required_agrees_with_an_equivalent_static_rule`),
      staleness fields, and risk-routing — all green.
- [x] 1.7 Implement the staleness check: surface-scoped git-diff when a
      surface ref is declared on the evidence record, else the
      `max_commits_behind` ceiling.
      Evidence: `crates/canon-gate/src/staleness.rs::StalenessCheck` —
      the donor parity harness's two-tier staleness port:
      surface-scoped `git diff --name-only --no-renames <sha>..HEAD`
      short-circuits to `stale-evidence`; `git rev-list --count
      <sha>..HEAD` vs `policy.max_commits_behind` is an UNCONDITIONAL
      backstop that always runs, even when the precise diff comes back
      clean (staleness.md §4's own named misport risk — preserved, not
      simplified to an either/or branch). The
      `EvidenceRecord` surface-ref field this task's own note (and
      `lib.rs`'s INTERFACE REQUEST #2) flags as missing from S1 is
      supplied as an interim companion pair read off the RAW ledger
      JSON — `evidence_sha`/`surface_ref` — via an independent
      `GitTier` re-read (`GateContext.evidence` is already fully typed,
      unknown keys dropped at deserialize time; there is no catch-all
      field to recover them from there), mirroring
      `trust_ladder.rs`/`markers.rs`'s own established interim-field
      precedent. Only GREEN (`Faithful`) cells are ever checked
      (staleness only degrades a passing verdict, spec.md). An
      unresolvable/absent `evidence_sha` is NEVER treated as stale
      (staleness.md recommendation 4's third state). 7 tests, each
      against a REAL git repo built in-test (`git init`/commit, not a
      mock): `stale_evidence_fires_when_a_declared_surface_file_
      changes_after_the_evidence_sha`, `not_stale_when_declared_
      surfaces_are_untouched_and_well_under_the_ceiling`,
      `ceiling_applies_even_when_the_surface_scoped_diff_is_precisely_
      clean`, `max_commits_behind_ceiling_fires_with_no_declared_
      surface_ref`, `unresolvable_evidence_sha_is_never_assumed_stale`,
      `a_record_with_no_evidence_sha_is_never_assumed_stale`,
      `a_non_green_verdict_is_never_checked_for_staleness` — all green
      (`cargo test -p canon-gate staleness::`).
- [x] 1.8 Define the stable `FAILURE_CLASSES` constant (Rust set + exported
      JSON-schema enum): `uncovered-cell`, `unreviewed-promotion`,
      `trust-below-required`, `stale-evidence`, `malformed-evidence`,
      `flagged`.
      Evidence: `crates/canon-gate/src/failure_class.rs` — `FAILURE_CLASSES`
      (8-entry `&'static str` array, incl. `unevidenced-flip`/
      `fabricated-evidence` for `gated-task-completion`) + `FailureClass`
      (schemars `JsonSchema`-derived enum) + `Violation`/`.line()`/`.pair()`;
      `failure_classes_const_matches_enum_exactly` +
      `all_has_exactly_eight_classes_no_duplicates` assert the two
      representations can never drift.
- [x] 1.9 Implement `canon gate check`: runs static + dynamic checks over a
      repo, prints violations by failure class, exits non-zero on any.
      **(S5 wave-2-part2.)**
      Evidence: `crates/canon-gate/src/dispatch.rs::check_set` — the ONE
      assembly function both `canon gate check` and this crate's own
      `canon gate selftest` reuse (`dispatch::tests::ordinary_check_set_
      always_includes_trust_ladder_check_but_never_release_trust_check`,
      `release_check_set_includes_both_trust_ladder_check_and_release_
      trust_check` lock the dispatcher contract: `TrustLadderCheck` is
      present whether or not the release-scoped `ReleaseTrustCheck` is
      engaged); `crates/canon-cli/src/gate.rs::run_check` wires it
      against a real repo's `GateContext` (`--repo`/`--release`),
      printing violations grouped by `FAILURE_CLASSES`, exit `0`
      clean/`1` gate-red/`2` usage. This same commit ALSO delivers
      every other `canon gate` CLI subcommand's wiring the
      foundation/wave-2 tasks above deferred as "canon-cli's job, S5
      wave-2" with no dedicated task line of their own: `canon gate
      task <task_id>` (`gate.rs::run_task`, wraps `checkbox::gate_task`
      + a new `markers::evidence_note_of` raw-companion reader, task
      3.2's own deferred CLI note), `canon gate promote [--dry-run]`
      (`gate.rs::run_promote`, wraps `promote::promote`, tasks 2.2/2.3's
      own deferred CLI notes), and `canon gate install-hooks`
      (`gate.rs::run_install_hooks`, wraps `hooks::install_hooks` +
      `PRE_COMMIT_SCRIPT`, task 4.1's own deferred CLI note, plus the
      spec.md "non-donor-CLI repo gets a generic pre-commit script"
      scenario). `crates/canon-cli/src/context.rs::resolve_repo_root`
      (S12) is reused unchanged for every subcommand's `--repo`, so a
      gate invocation from a subdirectory reads the repo ROOT's
      `canon.yaml`/`canon/policy.yaml`, matching `canon context`'s own
      convention. 12 `crates/canon-cli/tests/gate.rs` integration tests
      against the real binary (`cargo test -p canon-cli --test gate`),
      incl. `gate_check_exits_gate_red_on_a_seeded_uncovered_cell_
      violation`, `gate_check_from_a_subdirectory_resolves_the_
      ancestor_repo_root` (mirrors `canon-cli/tests/context.rs::
      context_from_a_subdirectory_resolves_the_ancestor_repo_root_
      policy`), `gate_task_flip_is_blocked_with_no_evidence_record`/
      `gate_task_flip_succeeds_with_clean_evidence`, `gate_promote_
      assigns_a_run_seq_and_lands_the_record_in_the_committed_tier`,
      `gate_install_hooks_is_idempotent_and_seeds_a_pre_commit_script_
      for_a_fresh_repo` — all green (`cargo build -p canon-gate -p
      canon-cli` + `cargo clippy -p canon-gate -p canon-cli --all-targets
      -- -D warnings` clean; real-binary smoke test: `canon gate check
      --help`/`task`/`promote`/`install-hooks`/`selftest` all work).

## 2. Staging → promote

- [x] 2.1 Implement the `_staging/` write path: reviewers append unordered
      records with no `run_seq`.
      Evidence: `crates/canon-gate/src/promote.rs` module doc's "Why
      `staging`/`committed` are two separate `GitTier` roots" section —
      `GitTier::read`'s scan walks `<root>/kind={kind}/` RECURSIVELY
      (unlike the donor parity harness's fixed-depth glob), so staging is rooted at a
      SEPARATE `GitTier::new(ledger_root.join("_staging"))`, kept
      disjoint from the committed tier's own recursive scan under any
      walk; ordinary `Tier::write` on that root IS the unordered,
      parallel-safe `_staging/` write path (no `run_seq` field, no
      special writer code — every `promote::tests::*` fixture exercises
      it directly, e.g. `staging.write(&evidence(...))`).
- [x] 2.2 Implement `canon gate promote`: assigns a monotonic per-(role,
      surface) `run_seq`, re-validates each staged record with the SAME
      checks §1 applies, refuses (exit non-zero, no `run_seq` consumed)
      malformed records, writes the committed file, deletes the staging
      file.
      Evidence: `crates/canon-gate/src/promote.rs::promote` — `(role,
      surface)` from `envelope.actor.role` + `ScenarioId::surface_key()`
      (falling back to `TaskId::change_id()`); `1 + max(run_seq)` per key
      scanned off the COMMITTED tier only, bumped in-invocation for N
      staging candidates sharing a key; re-validation reuses
      `staging.read()`'s own `canon_model::validate_evidence` pass
      (module doc: "IMPOSSIBLE for `promote` to accept a candidate the
      gate's own read path would reject") — a malformed candidate lands
      in `staged.violations`, refused as `FailureClass::MalformedEvidence`
      before any `run_seq` is touched; a candidate with no derivable
      `(role, surface)` is refused the same way. Writes/deletes go
      through `GitTier::write` + the committed-relative
      `expected_relative_path` (re-resolved after `run_seq` is stamped,
      since stamping changes the content-digest suffix).
      `promote_assigns_monotonic_gap_free_run_seq_within_one_invocation`
      (2 candidates, same surface, in ONE call → `run_seq` 1,2 with no
      gaps; a THIRD in a SEPARATE call → 3, continuing from the
      committed max, never restarting), `promote_refuses_a_malformed_
      candidate_without_consuming_a_run_seq` (a malformed sibling leaves
      the well-formed candidate at `run_seq` 1, not 2; the malformed
      staging file is left in place, never committed),
      `promote_refuses_a_candidate_with_no_derivable_partition_key`,
      `distinct_surfaces_get_independent_run_seq_sequences` all green.
- [x] 2.3 Implement `--dry-run` for `canon gate promote`: prints the plan
      (target, assigned `run_seq`) without writing or deleting.
      Evidence: `crates/canon-gate/src/promote.rs::promote`'s `dry_run:
      bool` parameter — the SAME plan (`PromoteReport` with every
      `Promoted{role, surface, run_seq, target}`) is computed and
      returned regardless, but `committed.write`/`std::fs::remove_file`
      are skipped entirely when `true`
      (`dry_run_computes_the_plan_without_writing_or_deleting`: asserts
      the plan is non-empty AND the committed tier stays empty AND the
      staging file is NOT deleted). The actual PRINTING of that plan is
      `canon gate promote --dry-run`'s CLI concern (task 1.9-equivalent
      wiring, `canon-cli`, out of this crate's territory) — this task's
      side-effect-free plan computation is what lands here.

## 3. openspec task/checkbox grammar (`gated-task-completion`)

- [x] 3.1 Implement the `- [ ] ` / `- [x] ` checkbox grammar parser +
      writer for `openspec/changes/<slug>/tasks.md`, including the
      `**DEFERRED to §<to>**` / `**DROPPED**` annotation forms and the
      ` — ✅ <evidence>` suffix, as canon's own format — no dependency on
      the donor CLI's parser.
      Evidence: `crates/canon-gate/src/checkbox.rs` — `parse_line`/
      `format_line` round-trip byte-identically for open/`[x]`+evidence/
      `DEFERRED`/`DROPPED`/indented rows (`checkbox::tests::round_trips_*`,
      6 tests, incl. `round_trips_a_dropped_row_with_no_trailing_title`);
      an unrecognized line returns `None` rather than guessing
      (`a_non_checkbox_line_is_not_recognized`); no import of
      the donor CLI's task-flip module (unreachable from
      this worktree — canon is a standalone repo, a case-insensitive
      search for the donor CLI's name under the worktree returns zero matches).
- [x] 3.2 Implement `canon gate task <task_id>`: resolves the task row via
      the S1 join spine (`<change_id>#<n>`), requires a matching
      `EvidenceRecord`, flips `- [ ]`→`- [x]` with the evidence note only
      on a clean evidence check; fail-closed (row stays unflipped) on
      missing/malformed evidence.
      Evidence: `crates/canon-gate/src/checkbox.rs::gate_task` — resolves
      `TaskId`'s `<n>` half against the row grammar, requires a
      non-`Divergent` `EvidenceRecord` match, flips + appends the
      evidence-note suffix only on a clean check
      (`gate_task_flips_with_clean_faithful_evidence` — asserts every
      OTHER line in the document stays byte-identical); fails closed
      (document returned byte-unchanged) on no match, a `Divergent`
      verdict (malformed evidence is no evidence), or an unknown
      `task_id` (`gate_task_fails_closed_with_no_evidence_record`,
      `gate_task_fails_closed_on_a_divergent_verdict_malformed_evidence_
      is_no_evidence`, `gate_task_reports_an_unknown_task_id_instead_of_
      silently_ignoring_it`); idempotent on an already-`[x]` row
      (`gate_task_is_idempotent_on_an_already_flipped_row`). The `canon
      gate task` CLI command itself (reading a real `tasks.md` off disk,
      resolving `task_id` to a file) is `canon-cli` wiring, a separate
      later step out of this crate's territory — this task's
      library-level flip logic is what lands here.
- [x] 3.3 Implement fabrication-marker scanning over the evidence record's
      structured fields only (never free prose): blocklist substrings +
      the bare-`verified`-without-attached-result rule.
      Evidence: `crates/canon-gate/src/markers.rs` — `EvidenceNote`
      (canon-gate's own companion type; `EvidenceRecord` carries no
      free-text field at all, verified against
      `crates/canon-model/src/records.rs` — an INTERFACE REQUEST to S1
      per the module doc, mirroring `trust_ladder::TrustLadderState`'s
      precedent) + `scan_fake_markers` matching `FABRICATION_BLOCKLIST`
      (`"would pass"`/`"tbd"`/`"n/a"`, spec.md's own examples,
      case-insensitive) and the bare-`verified`-with-no-`command_result`
      rule; 8 tests incl.
      `free_prose_outside_the_structured_fields_is_never_scanned`
      (documents that the scanner's own signature — `&EvidenceNote`,
      never `&str` — makes free prose structurally unreachable).
- [x] 3.4 Wire `canon gate task` failures into `FAILURE_CLASSES`:
      `unevidenced-flip`, `fabricated-evidence`.
      Evidence: `checkbox::gate_task` constructs
      `Violation::new(FailureClass::UnevidencedFlip, ...)` on missing/
      divergent evidence; `markers::scan_fake_markers` constructs
      `Violation::new(FailureClass::FabricatedEvidence, ...)` on every
      blocklist/bare-verified hit — both asserted by `.class` in their
      own test modules (`gate_task_fails_closed_with_no_evidence_record`
      asserts `FailureClass::UnevidencedFlip`;
      `gate_task_blocks_on_a_fabricated_evidence_note` asserts
      `FailureClass::FabricatedEvidence`).

## 4. Hook seam wiring

- [x] 4.1 Implement `canon gate install-hooks`: idempotent, diff-only
      emission of `.claude/settings.json` / `.codex/hooks.json` entries
      invoking `canon gate task <task_id>` in the existing `{matcher,
      hooks: [{type: "command", command, timeout}]}` shape.
      Evidence: `crates/canon-gate/src/hooks.rs::install_hooks` — pure
      `serde_json::Value` merge (never reads/writes a real file, module
      doc), idempotent (a second call on the same entry returns
      `InstallOutcome::Unchanged`, writes nothing:
      `a_second_install_of_the_same_entry_is_a_no_op`), diff-only
      (appends alongside an existing donor-CLI entry in the SAME
      matcher-group without touching it:
      `installs_additively_alongside_an_existing_third_party_entry_same_matcher`;
      a different matcher gets its own new group, never merged:
      `a_different_matcher_gets_its_own_new_group_not_merged`; other
      events/keys stay untouched:
      `other_events_are_left_completely_untouched`), matcher-less events
      round-trip with no `matcher` key at all (never a JSON `null`):
      `a_matcher_less_event_round_trips_with_no_matcher_key_at_all`; 7
      tests total. The `canon gate install-hooks` CLI command itself and
      writing the donor monorepo's real settings.json (task 4.3) are out of this
      crate's / this change's scope (below) — this task's merge LOGIC is
      what lands here.
- [x] 4.2 Ship a generic pre-commit shell script
      (`canon-gate-pre-commit.sh`) for non-donor-CLI repos, mirroring the
      lefthook `pre-commit:` job shape (advisory vs blocking configurable).
      Evidence: `crates/canon-gate/scripts/canon-gate-pre-commit.sh`
      (mirrors `lefthook.yml`'s `command -v <tool> ... && <tool> ... ||
      true` advisory idiom), embedded verbatim as
      `hooks::PRE_COMMIT_SCRIPT` via `include_str!` so the two can never
      drift; actually EXECUTED via `/bin/sh` in 4 tests, not just string
      assertions: a missing `canon` binary exits 0
      (`exits_zero_when_canon_is_not_installed`, fail-soft per §7),
      `CANON_GATE_ADVISORY=1`/default never blocks a failing gate
      (`advisory_mode_never_blocks_even_when_the_gate_fails`),
      `CANON_GATE_ADVISORY=0` propagates a gate failure
      (`blocking_mode_fails_the_commit_when_the_gate_fails`), a passing
      gate always exits 0 (`a_passing_gate_always_exits_zero`).
- [ ] 4.3 Wire the donor monorepo: add `canon gate task` hook-seam entries to
      the donor monorepo's `.claude/settings.json` and `.codex/hooks.json` alongside
      the existing `hook run <kind>` entries (additive; the donor CLI's
      entries stay). **DEFERRED to a follow-up change** — design.md
      decision 8 / Migration Plan step 2 states this opt-in is
      documented, not executed, by THIS change ("This change SHIPS the
      seam + script; do NOT edit any donor/consumer settings.json"); the
      merge logic task 4.1 ships is what that follow-up will call.
- [x] 4.4 Document the donor-CLI migration-target boundary in-repo: state
      explicitly that the donor CLI's task-flip + marker-scan modules
      are NOT touched by this
      change; a follow-up donor-CLI-side change swaps their callers to shell
      out to `canon gate task`.
      Evidence: `crates/canon-gate/src/hooks.rs`'s own "# donor-CLI
      migration-target boundary (design decision 7, task 4.4)" module-doc
      section; neither the task-flip nor the marker-scan module exists in
      this worktree at all — canon is a standalone repo, a case-insensitive
      search for the donor CLI's name under the worktree root returns zero matches, so the
      boundary is structurally unreachable, not merely undisturbed by
      discipline.

## 5. Testing + fixtures

- [x] 5.1 Build a fixture corpus (GateCtx-style rebindable root) with
      EXPECTED-violation files, one fixture per `FAILURE_CLASSES` entry.
      **(S5 wave-2-part2 — `GateCtx::from_repo`/`GateCtx::from_fixture`'s
      two-constructor seam landed in FOUNDATION `context.rs`; this task
      is the fixture corpus itself.)**
      Evidence: `crates/canon-gate/fixtures/<class>/expected_failures.txt`
      × 8, one per `FAILURE_CLASSES` string — the literal, checked-in,
      hand-authored oracle artifact (the donor parity harness's own 2-column
      `<class> <subject>` format, `#`-comments allowed).
      Each fixture's CORPUS is built by `crates/canon-gate/src/
      selftest.rs`'s own `build_*` functions calling the exact
      `GitTier::write`/`RawWrite` production path — never a
      hand-authored literal ledger JSON file, since `canon_store`'s
      content-digest-suffixed Hive filenames make that impractical to
      author/maintain by hand (documented in the module's own doc
      comment as a deliberate adaptation from the donor parity harness's literal-file
      fixtures); `stale-evidence`'s fixture builds a REAL `git init`/
      commit corpus in-test, mirroring `staleness.rs`'s own established
      fixture discipline. `GateCtx::from_fixture` binds every root into
      one fresh scratch directory per fixture (`ScratchDir`, a minimal
      std-only `TempDir`-equivalent — no `tempfile` dependency added to
      this crate's production graph, since the workspace root
      `Cargo.toml`/`Cargo.lock` are outside this change's edit-root).
- [x] 5.2 Implement `canon gate selftest`: runs every fixture, diffs actual
      vs EXPECTED violations, fails on any mismatch. **(S5
      wave-2-part2.)**
      Evidence: `crates/canon-gate/src/selftest.rs::run` — the
      exact-set-match diff (`missing`/`extra`, both halves, mirroring
      the donor parity harness's own two-sided oracle ("extra half is the
      important, easy-to-omit half"); six fixtures run through the assembled `dispatch::
      check_set(true)` over a loaded `GateContext`, two
      (`unevidenced-flip`/`fabricated-evidence`) run through
      `checkbox::gate_task`/`markers::scan_fake_markers` directly
      (`gated-task-completion`'s own territory, never a registered
      `GateCheck`). `selftest::tests::every_failure_class_fires_
      exactly_on_its_own_fixture` + `fixture_table_covers_every_
      failure_class_exactly_once` + `a_mismatched_expected_set_
      produces_a_dirty_report` lock this (`cargo test -p canon-gate
      selftest::`, all green) — the assignment's own "Tests lock this"
      requirement, and spec.md's "selftest fails when a fixture's
      expectations regress" scenario. `crates/canon-cli/src/
      gate.rs::run_selftest` wires `canon gate selftest` (no `--repo`,
      self-contained — `gate_selftest_exits_zero_against_the_shipped_
      fixture_corpus` canon-cli integration test); real-binary smoke
      test: `canon gate selftest` → all 8 classes `ok`, exit 0.
- [x] 5.3 Add a fixture repo exercising `canon gate task <task_id>` on a
      gated openspec task with no evidence record — assert the flip is
      blocked. **(S5 wave-2-part2.)**
      Evidence: `crates/canon-gate/fixtures/unevidenced-flip/
      expected_failures.txt` + `selftest.rs::build_unevidenced_flip`
      (a `tasks.md` open row + zero ledger evidence, asserting
      `unevidenced-flip` fires via `checkbox::gate_task` itself — task
      5.1/5.2's own fixture, already covers this). ADDITIONALLY, a
      real-repo, real-binary integration test:
      `crates/canon-cli/tests/gate.rs::gate_task_flip_is_blocked_with_
      no_evidence_record` — a real `openspec/changes/<slug>/tasks.md`
      on disk, `canon gate task <task_id> --repo .` spawned as the
      actual binary, asserts exit `1`, an `unevidenced-flip` line on
      stderr, and the row byte-unchanged on disk afterward.

## 6. Companion skill

- [x] 6.1 Author the `canon-gate` companion skill under `canon/skills/` —
      usage for `canon gate check` / `canon gate task` / `canon gate
      promote` / `canon gate install-hooks`, when a gate blocks vs a hook
      fails soft, and how to read a `FAILURE_CLASSES` violation. **(S5
      wave-2-part2 — the `canon gate` subcommand now exists to
      document.)**
      Evidence: `canon/skills/trust-spine-gate/SKILL.md` — front-matter
      `name`/`description` + a `FAILURE_CLASSES` reading table and
      per-subcommand usage for all five (`check`/`task`/`promote`/
      `install-hooks`/`selftest`); `canon skills install --source
      canon/skills --target .` materialized it (`trust-spine-gate v1 —
      installed`), producing `.claude/skills/trust-spine-gate/SKILL.md`,
      `.codex/skills/trust-spine-gate.md`, and a bumped
      `canon/skills/.install-lock.json` entry (content-hash, version 1).
