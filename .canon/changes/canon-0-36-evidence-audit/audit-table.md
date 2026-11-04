# Audit of the scenarios whose latest evidence binds fewer cases than an earlier record

Chore `canon-0-36-evidence-audit` (issue #136). Actor `impl-evidence-audit`, session `s-0-36-evidence-audit`. Worktree at main 1419794e5, workspace nextest run 3793 passed (`cargo nextest run --workspace --locked --no-fail-fast`, report `target/nextest/default/junit.xml`).

## The list and how it was drawn

Every evidence record of a scenario of project `canon`, the latest in `.canon/ledger/kind=evidence` and the earlier ones in `.canon/archive/kind=evidence`, was read; its bound cases are the `attachments[].case` values. A scenario is on the list when the latest record binds fewer cases than the record with the most cases before it. The cases lost are the earlier records' cases the latest one lacks.

- Before: 45 scenarios (the 54 of the 0.35 close, less the ones `canon-0-36-claim-1-4` re-bound).
- After the re-record (same extraction): 7 scenarios, each one a "removed" or "moved" row below, not "lost".

Checks of each dropped case: the current test list (the 3793 cases of the report), `git log -S<case name>` for the commit that removed or moved it, and the scenario text.

## Counts

| disposition | scenarios |
|---|---|
| lost, re-recorded | 38 |
| renamed, moved or removed, no case to bind | 7 (4 moved out with the atip crate, 3 removed) |
| left canon | 0 |
| total | 45 |

Re-recorded: 38 lost scenarios with their full case lists (`--report-case`), plus the 63 evidenced scenarios of `gate-trust-spine` that had no evidence after the approval of this chore (their latest cases re-bound), plus 7 audited scenarios with their remaining cases: 108 per-scenario records; before them one `--all-scenarios` run of 773 scenarios. The scenarios of the checker (atip.check.*, atip.checkl1.*, atip.checkl2safety.10, atip.l0.07, atip.crate*, atip.cratecursor.*, atip.crateplan.03) left canon with `canon-0-36-atip-dep`: they are not indexed and are out of the audit.

## Commands

```
CANON_ROLE=implementer canon evidence add --all-scenarios --project canon --report junit:target/nextest/default/junit.xml --artifact atip.claim14.14=examples/platformer/reports/atip-deep-1.4.txt --actor impl-evidence-audit --session s-0-36-evidence-audit --summary ...
CANON_ROLE=implementer canon gate promote
CANON_ROLE=implementer canon evidence add --promote --input batch.json     # 108 per-scenario records: scenario, project, report, reportCase[], role, actor, session, summary
```

`atip.claim14.14` re-binds the report of `just atip-deep` of the round 2 record (same bytes, restored from the artifact store, same code tree) so the all-scenarios run drops no attachment.

## Audit table

| scenario | earlier record (max cases) | latest before | dropped case | class | action |
|---|---|---|---|---|---|
| `atip.codenames.04` | 3 | 1 | `atip_codenames_04_a_listed_code_that_follows_the_rule_fails_the_test`<br>`atip_codenames_04_the_list_of_codes_still_to_reclassify_holds_only_registered_codes` | removed: the two cases tested the list of codes still to reclassify, which the approved change canon-0-36-codes-table removed (ff534b234, 030701e96); the rule case is bound | none |
| `atip.contract11.03` | 2 | 1 | `schema::tests::atip_contract11_03_a_reason_and_a_warning_row_validate` | moved: the crate tests (schema::/project:: modules of atip-crate) left the workspace with the crate (06e398e81, atip-dep); the canon-side case is bound | none |
| `atip.l0.06` | 2 | 1 | `schema::tests::atip_l0_06_a_conforming_success_and_failure_pass` | moved: the crate tests (schema::/project:: modules of atip-crate) left the workspace with the crate (06e398e81, atip-dep); the canon-side case is bound | none |
| `atip.l0.19` | 2 | 1 | `schema::tests::atip_l0_19_a_registry_row_has_an_exit_of_one_or_two` | moved: the crate tests (schema::/project:: modules of atip-crate) left the workspace with the crate (06e398e81, atip-dep); the canon-side case is bound | none |
| `atip.planjcs.02` | 2 | 1 | `atip_planjcs_10_a_refused_commit_without_the_lock_file_leaves_the_tree_byte_identical` | removed: atip_planjcs_10_... tests a purge and belongs to atip.planjcs.10, which binds it; impl-lockfix bound it to .02 once (1a7c512d) and the next record dropped it | none |
| `atip.schemadoc.06` | 3 | 1 | `project::tests::atip_schemadoc_06_a_custom_parser_without_a_table_entry_fails_and_with_one_passes`<br>`project::tests::atip_schemadoc_06_a_default_is_parsed_with_its_type_and_one_that_does_not_parse_fails` | moved: the crate tests (schema::/project:: modules of atip-crate) left the workspace with the crate (06e398e81, atip-dep); the canon-side case is bound | none |
| `context.capability.01` | 1 | 0 | `context_exits_zero_with_a_full_surface_even_when_the_corpus_fails_fmt_check` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.02` | 1 | 0 | `context::tests::resolve_surface_never_fails_on_a_repo_with_no_canon_state_at_all` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.03` | 1 | 0 | `context::tests::render_json_is_byte_stable_across_repeated_resolution` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.04` | 1 | 0 | `context::tests::json_and_outline_agree_on_every_kind_enum_and_join_key_name` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.05` | 1 | 0 | `context::tests::kinds_and_enums_match_a_fresh_independent_schema_registry_walk` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.06` | 1 | 0 | `context::tests::cel_surface_matches_a_fresh_independent_bindings_for_walk` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.07` | 1 | 0 | `context_from_a_subdirectory_resolves_the_ancestor_repo_root_policy` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.08` | 1 | 0 | `context::tests::vocab_surface_matches_a_fresh_independent_resolve_snapshot_call` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.09` | 1 | 0 | `context::tests::resolve_surface_never_fails_with_no_vocab_directory_at_all` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.10` | 1 | 0 | `context::tests::risk_tiers_are_projected_to_json_and_outline` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.11` | 1 | 0 | `context::tests::a_malformed_spec_coverage_surfaces_as_invalid_and_a_valid_one_names_its_cases` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.12` | 1 | 0 | `context_outline_opens_with_a_status_pointer_and_leaves_json_unchanged` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.13` | 1 | 0 | `context_header_warns_when_no_policy_enforces_coverage` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.24` | 2 | 0 | `commands_that_run_without_a_canon_yaml_still_do`<br>`every_command_that_needs_canon_state_refuses_without_a_canon_yaml` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 2 case(s) |
| `context.capability.25` | 1 | 0 | `enum_options_list_their_values_and_defaults` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.26` | 1 | 0 | `every_read_leaf_leaves_the_repo_tree_unchanged_or_declares_its_write` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.38` | 1 | 0 | `commands_are_the_spec_leaves_each_role_uses` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.39` | 1 | 0 | `kinds_are_every_record_kind_under_its_user_facing_name` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.41` | 3 | 0 | `completion_json_is_the_script_in_an_envelope`<br>`completion_refuses_a_shell_it_does_not_know_and_needs_no_repo`<br>`completion_writes_a_script_for_each_shell` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 3 case(s) |
| `context.capability.42` | 1 | 0 | `completion_refuses_a_shell_it_does_not_know_and_needs_no_repo` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.capability.43` | 3 | 0 | `a_known_role_and_a_policy_named_role_do_not_warn`<br>`a_misspelt_role_runs_and_warns_with_the_closest_known_role`<br>`an_unknown_role_with_nothing_close_still_runs_and_warns_without_a_suggestion` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 3 case(s) |
| `context.capability.44` | 1 | 0 | `an_unknown_canon_role_is_refused_with_unknown_role` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.01` | 1 | 0 | `status_counts_each_subject_through_the_gates_joins` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.02` | 1 | 0 | `status_next_names_the_command_for_each_gap` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.03` | 1 | 0 | `context_status_62_ship_is_the_agents_step_once_the_brief_is_approved` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.05` | 1 | 0 | `status_warns_when_no_policy_enforces_coverage` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.06` | 1 | 0 | `status_reports_an_unreadable_canon_yaml_and_is_unverifiable` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.07` | 1 | 0 | `status_is_a_read_that_writes_nothing` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.08` | 1 | 0 | `status_caps_next_at_five` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.10` | 1 | 0 | `status_fails_closed_when_scenarios_route_away` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.11` | 1 | 0 | `status_treats_an_invalid_coverage_policy_as_a_blocking_gap` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.12` | 2 | 0 | `status_names_the_spec_root_when_several_are_configured`<br>`status_omits_the_spec_root_flag_with_one_root` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 2 case(s) |
| `context.status.13` | 1 | 0 | `status_header_prints_the_effective_settings` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.14` | 1 | 0 | `status_next_names_the_command_for_each_gap` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.15` | 1 | 0 | `status_with_no_feature_scenarios_points_at_writing_them` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 1 case(s) |
| `context.status.40` | 6 | 0 | `a_repo_without_a_policy_gets_no_approval_advisory`<br>`approve_records_the_policy_digest_the_approver_the_time_and_the_note`<br>`approve_refuses_a_repo_with_no_policy`<br>`approve_refuses_an_empty_approver`<br>`gate_check_and_status_advise_until_the_current_policy_is_approved`<br>`the_approval_file_is_never_ignored` | lost: the latest record (impl-gateclean, 2026-11-03) bound 0 case(s): a command-only record of the 0.35 gate-clean re-run, no cases | re-recorded with 6 case(s) |
| `ingest.sessions.16` | 2 | 1 | `ingest_sessions_16b_subject_and_change_writes_to_the_hot_store_print_its_location_too` | lost: the latest record (impl-claim-1-4, 2026-11-04) bound 1 case(s): the case of the scenario id only, and the case whose id carries a letter is not owned by the scenario | re-recorded with 3 case(s) |
| `ledger.migrate.10` | 4 | 2 | `ledger_migrate_10b_a_row_holding_a_planned_key_with_other_json_refuses_the_run`<br>`ledger_migrate_10c_an_object_holding_a_planned_key_with_other_json_refuses_the_run` | lost: the latest record (impl-claim-1-4, 2026-11-04) bound 2 case(s): the case of the scenario id only, and the case whose id carries a letter is not owned by the scenario | re-recorded with 4 case(s) |
| `report.dashboard.05` | 2 | 1 | `report_dashboard_05_reads_korean_headings_by_their_markers` | removed: the Korean-heading case went with the Korean headings when the briefs took English headings (ab6806214, 0.31 brief-format); the scenario text no longer asks for it | none |
