# The contract repository's examples, as `atip check` tests them

Copied from https://github.com/journeyWorker/atip at commit `365244cb15ce55ea737dec6ca3d8b714441d899d`
(`examples/report-*.json`, `examples/schema-errors*.json` and `examples/invalid/{envelope,report}-*.json`), unmodified.
`tests/atip_examples.rs` reads this table, so a file without a row, or a row whose file is missing, fails the test.

- `accept`: the envelope validates against the envelope schema and breaks no L0 rule that reads one envelope: the failure rules of `L0.ok-exit` against the registry rows, and, for an unknown-command failure, the rules of `L0.unknown-command`.
- `reject`: some layer named in `By` rejects it: `schema` (the envelope schema), `L0.ok-exit`, or `L0.unknown-command`.
- `skip`: the file breaks only a rule of the L1 or L2 check named in `By`, which `atip check` does not run yet (0.34, 0.35). The test checks that no L0 layer rejects it, so a skip stays honest, and the reason says what the file breaks.
- `registry`: not an envelope; its `errors` are the registry rows the failure codes are looked up in.

| File | Expect | By | Why |
|---|---|---|---|
| `report-acme-deploy-start-accepted.json` | accept | - | a success envelope of the contract examples |
| `report-acme-filter-test.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-export-fields.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-export-null.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-list-fields-nested.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-list-fields-note-not-profile.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-list-fields-null-profile.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-list-fields-parent-child.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-list-fields-structural-enum.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-roster-fields.json` | accept | - | a success envelope of the contract examples |
| `report-acme-member-tag-batch.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-batch-first.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-batch-preflight.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-batch.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-input.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-stale-plan.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-unknown-command.json` | accept | - | a failure envelope of the contract examples |
| `report-failure-usage.json` | accept | - | a failure envelope of the contract examples |
| `report-failure.json` | accept | - | a failure envelope of the contract examples |
| `report-finding-list-fields.json` | accept | - | a success envelope of the contract examples |
| `report-preview.json` | accept | - | a success envelope of the contract examples |
| `report-skills-show.json` | accept | - | a success envelope of the contract examples |
| `report-success.json` | accept | - | a success envelope of the contract examples |
| `schema-errors.json` | registry | - | the registry of the canon translation: the rows that failure codes are looked up in |
| `schema-errors-acme.json` | registry | - | the registry of the synthetic tool acme |
| `invalid/envelope-flat-report.json` | reject | schema | a flat document: the payload beside ok, no data/meta nesting (6.1) |
| `invalid/envelope-lowercase-code.json` | reject | schema | a code that is not UPPER_SNAKE_CASE (7.1) |
| `invalid/envelope-missing-retryable.json` | reject | schema | an error without retryable (7.1.1) |
| `invalid/envelope-null-context.json` | reject | schema | null in meta.context (6.5, 6.7) |
| `invalid/envelope-null-hint.json` | reject | schema | hint: null (6.5) |
| `invalid/envelope-release-schema-version.json` | reject | schema | a schemaVersion that encodes the release (6.6, 15.3) |
| `invalid/envelope-unknown-meta-key.json` | reject | schema | a meta member the contract does not define (6.6) |
| `invalid/report-acme-member-list-fields-leak.json` | skip | L1.fields | a --fields projection that leaves an unselected member (6.10) |
| `invalid/report-acme-member-list-fields-missing-child.json` | skip | L1.fields | a --fields projection that drops a selected child (6.10) |
| `invalid/report-acme-member-list-fields-missing-nested.json` | skip | L1.fields | a --fields projection that drops a selected nested member (6.10) |
| `invalid/report-acme-member-list-fields-parent-child-leak.json` | skip | L1.fields | a --fields projection that leaks a sibling of a selected child (6.10) |
| `invalid/report-acme-member-list-next-page-missing.json` | skip | L1.paging | a page with nextCursor but no next entry for the next page (6.9) |
| `invalid/report-acme-member-tag-batch-failed-without-error.json` | skip | L2.batch-incomplete | a failed batch item without its own error (11.11) |
| `invalid/report-batch-failed-without-error.json` | skip | L2.batch-incomplete | a failed batch item without its own error (11.11) |
| `invalid/report-batch-incomplete-no-failed-item.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE with no failed item (11.11) |
| `invalid/report-batch-incomplete-without-data.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE without the batch report in data (11.11) |
| `invalid/report-batch-incomplete-wrong-field.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE whose field is not the failed index (11.11) |
| `invalid/report-batch-nested-retryable.json` | skip | L2.batch-incomplete | a nested item error whose retryable differs from its row (11.11) |
| `invalid/report-batch-nested-unregistered.json` | skip | L2.batch-incomplete | a nested item error with an unregistered code (11.11) |
| `invalid/report-batch-skipped-with-data.json` | skip | L2.batch-incomplete | a skipped batch item that carries data (11.11) |
| `invalid/report-batch-status-order.json` | skip | L2.batch-incomplete | batch item statuses out of order (11.11) |
| `invalid/report-batch-wrong-version.json` | skip | L2.batch-incomplete | a batch report with the wrong schemaVersion (6.11, 11.11) |
| `invalid/report-failure-retryable-mismatch.json` | reject | L0.ok-exit | a failure whose retryable differs from its registry row (7.3.3) |
| `invalid/report-failure-unresolved-version.json` | reject | L0.unknown-command | an unknown-command failure whose schemaVersion is not 1.0 (6.11) |
| `invalid/report-failure-version-mismatch.json` | skip | L1.output-data | a failure whose schemaVersion no output of the command has (6.11) |
| `invalid/report-fields-missing-selected.json` | skip | L1.fields | a projection missing a selected member (6.10) |
| `invalid/report-fields-unselected.json` | skip | L1.fields | a projection with an unselected member (6.10) |
| `invalid/report-next-command-mismatch.json` | skip | L1.next | a next command string that does not split into its argv (6.8) |
| `invalid/report-next-placeholder.json` | skip | L1.next | a next argv holding a placeholder (6.8) |
| `invalid/report-next-without-json.json` | skip | L1.next | a next argv without --json in JSON mode (6.8) |
| `invalid/report-output-missing.json` | skip | L1.output-data | meta.output missing where the command has several outputs (6.6) |
| `invalid/report-page-cursor-without-next.json` | skip | L1.paging | nextCursor without a next entry (6.9) |
| `invalid/report-refusal-with-data.json` | reject | L0.ok-exit | a refusal (FINDING_NOT_FOUND, registry exit 2) that carries data (6.4) |
| `invalid/report-skills-show-markdown-key.json` | skip | L2.skills | a skills show answer that does not follow the guide schema (14) |
