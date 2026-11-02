# The contract repository's examples, as `atip check` tests them

Copied from https://github.com/journeyWorker/atip at the tag v1.2.0 (commit `65c89a9b45fec06163223ab1d5969f807c20fc26`), unmodified:
`examples/report-*.json`, `examples/schema-error*.json`, `examples/schema-warning*.json`, `examples/stream-*.ndjson` and, of `examples/invalid/`,
`envelope-*`, `report-*`, `schema-error*`, `schema-warning*`, `schema-ledger-*`, `schema-version-not-contract-version.json` and `stream-*`.
`tests/atip_examples.rs` reads this table, so a file without a row, or a row whose file is missing, fails the test.

A document is judged by the layers a run of `atip check` applies to one envelope, the functions of `checks::rules`: `L0.envelope`, `L0.no-null`, `L0.contract-version`, `L0.hint-form`, `L0.registry`, `L0.next`, `L0.page`, and, for a failure, `L0.ok-exit` and `L0.unknown-command`. The tool `acme` claims 1.2 and the tool `tally` 1.1; another tool claims what its document declares. A stream is judged by the envelope in the `result` of its `end` line.

- `accept`: no layer reports anything.
- `reject`: the layer named in `By` reports something and no other layer does (one owner per pass condition).
- `skip`: the file breaks only a rule of the L1 or L2 check named in `By`, which `atip check` does not run yet (L1 in 0.34 by `canon-0-34-atip-check-l1`, L2 in 0.35). The test checks that no L0 layer rejects it, so a skip stays honest, and the reason says what the file breaks.

| File | Expect | By | Why |
|---|---|---|---|
| `report-acme-deploy-start-accepted.json` | accept | - | an envelope of the contract examples |
| `report-acme-filter-test.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-export-fields.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-export-null.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-map-keys.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-nested.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-note-not-profile.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-null-profile.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-parent-child.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-fields-structural-enum.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-list-warning.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-roster-fields.json` | accept | - | an envelope of the contract examples |
| `report-acme-member-tag-batch.json` | accept | - | an envelope of the contract examples |
| `report-failure-acme-deploy-start-stopped-midway.json` | accept | - | an envelope of the contract examples: a 1.2 failure with `STOPPED_MIDWAY` and the refusal's code as its reason (8.3.2) |
| `report-failure-acme-input-details-reason.json` | accept | - | an envelope of the contract examples |
| `report-failure-acme-input-reason.json` | accept | - | an envelope of the contract examples |
| `report-failure-acme-schema-error-warning-only.json` | accept | - | an envelope of the contract examples |
| `report-failure-batch-first.json` | accept | - | an envelope of the contract examples |
| `report-failure-batch-preflight.json` | accept | - | an envelope of the contract examples |
| `report-failure-batch.json` | accept | - | an envelope of the contract examples |
| `report-failure-input.json` | accept | - | an envelope of the contract examples |
| `report-failure-ledger-prose-hint.json` | accept | - | an envelope of the contract examples |
| `report-failure-stale-plan.json` | accept | - | an envelope of the contract examples |
| `report-failure-unknown-command.json` | accept | - | an envelope of the contract examples |
| `report-failure-usage.json` | accept | - | an envelope of the contract examples |
| `report-failure.json` | accept | - | an envelope of the contract examples |
| `report-finding-list-fields.json` | accept | - | an envelope of the contract examples |
| `report-ledger-entry-list-fields.json` | accept | - | an envelope of the contract examples |
| `report-preview.json` | accept | - | an envelope of the contract examples |
| `report-skills-show.json` | accept | - | an envelope of the contract examples |
| `report-success.json` | accept | - | an envelope of the contract examples |
| `schema-error-acme-input-invalid.json` | accept | - | a schema answer of the contract examples |
| `schema-error.json` | accept | - | a schema answer of the contract examples |
| `schema-errors-acme.json` | accept | - | a schema answer of the contract examples |
| `schema-errors-ledger.json` | accept | - | a schema answer of the contract examples |
| `schema-errors-tally.json` | accept | - | a schema answer of the contract examples: a 1.1 registry whose own `STOPPED_MIDWAY` is retryable (7.2.1) |
| `schema-errors.json` | accept | - | a schema answer of the contract examples |
| `schema-warning-acme-member-archived.json` | accept | - | a schema answer of the contract examples |
| `schema-warnings-acme.json` | accept | - | a schema answer of the contract examples |
| `stream-acme-build-failed.ndjson` | accept | - | a stream of the contract examples: its end result is judged |
| `stream-acme-build-quiet.ndjson` | accept | - | a stream of the contract examples: its end result is judged |
| `stream-acme-build-refused.ndjson` | accept | - | a stream of the contract examples: its end result is judged |
| `stream-acme-build.ndjson` | accept | - | a stream of the contract examples: its end result is judged |
| `invalid/envelope-contract-version-without-contract.json` | reject | L0.contract-version | contractVersion without contract: a 1.1 tool sets both markers (6.6, 15.7) |
| `invalid/envelope-flat-report.json` | reject | L0.envelope | a flat document: the payload beside ok, no data/meta nesting (6.1) |
| `invalid/envelope-lowercase-code.json` | reject | L0.envelope | a code that is not UPPER_SNAKE_CASE (7.1) |
| `invalid/envelope-missing-retryable.json` | reject | L0.envelope | an error without retryable (7.1.1) |
| `invalid/envelope-null-context.json` | reject | L0.no-null | null in meta.context (6.5, 6.7) |
| `invalid/envelope-null-hint.json` | reject | L0.no-null | hint: null (6.5) |
| `invalid/envelope-release-schema-version.json` | reject | L0.envelope | a schemaVersion that encodes the release (6.6, 15.3) |
| `invalid/envelope-unknown-meta-key.json` | reject | L0.envelope | a meta member the contract does not define (6.6) |
| `invalid/report-acme-contract-version-unknown.json` | reject | L0.contract-version | a contractVersion that is not the claimed minor (15.7) |
| `invalid/report-acme-details-reason-unregistered.json` | reject | L0.registry | a reason in details that the code does not register (7.6.2) |
| `invalid/report-acme-hint-not-a-command-line.json` | reject | L0.hint-form | a hint that starts with the tool name and does not split: an unclosed quote (7.1.2) |
| `invalid/report-acme-hint-prose-starting-with-tool.json` | reject | L0.hint-form | a prose hint that starts with the tool name in a document that declares 1.1 (7.1.2) |
| `invalid/report-acme-markers-missing.json` | reject | L0.contract-version | an ordinary answer of a 1.1 tool without contract and contractVersion (15.7) |
| `invalid/report-acme-member-list-fields-empty-key-leak.json` | skip | L1.fields | a --fields projection of a map that leaks an unselected member (6.10) |
| `invalid/report-acme-member-list-fields-leak.json` | skip | L1.fields | a --fields projection that leaves an unselected member (6.10) |
| `invalid/report-acme-member-list-fields-map-key-in-1-0.json` | reject | L0.contract-version | a bracketed key in meta.fields in a document without the 1.1 markers (6.10, 10.13 rule 16) |
| `invalid/report-acme-member-list-fields-map-key-leak.json` | skip | L1.fields | a map-key selection that leaves an unselected key (6.10) |
| `invalid/report-acme-member-list-fields-map-key-nul.json` | reject | L0.envelope | a bracketed key that holds U+0000: no argv can carry it (6.10) |
| `invalid/report-acme-member-list-fields-map-key-without-descent.json` | skip | L1.fields | a map-key selection that descends below the key (6.10) |
| `invalid/report-acme-member-list-fields-missing-child.json` | skip | L1.fields | a --fields projection that drops a selected child (6.10) |
| `invalid/report-acme-member-list-fields-missing-nested.json` | skip | L1.fields | a --fields projection that drops a selected nested member (6.10) |
| `invalid/report-acme-member-list-fields-not-a-map-key.json` | skip | L1.fields | a path that selects a key of a member that is not a declared map (6.10) |
| `invalid/report-acme-member-list-fields-parent-child-leak.json` | skip | L1.fields | a --fields projection that leaks a sibling of a selected child (6.10) |
| `invalid/report-acme-member-list-next-page-missing.json` | reject | L0.page | a page with nextCursor but no next entry for the next page (6.9) |
| `invalid/report-acme-member-tag-batch-failed-without-error.json` | skip | L2.batch-incomplete | a failed batch item without its own error (11.11) |
| `invalid/report-acme-member-tag-batch-item-details-reason-unregistered.json` | skip | L2.batch-incomplete | a reason of a batch item detail that its code does not register (7.6.2, 11.11) |
| `invalid/report-acme-member-tag-batch-item-reason-unregistered.json` | skip | L2.batch-incomplete | a reason of a batch item error that its code does not register (7.6.2, 11.11) |
| `invalid/report-acme-reason-in-1-0.json` | reject | L0.contract-version | error.reason in a document without the 1.1 markers (7.1.1, 10.13 rule 16) |
| `invalid/report-acme-reason-unregistered.json` | reject | L0.registry | a reason the registry does not list under the code (7.6.2) |
| `invalid/report-acme-schema-error-warning-only-no-hint.json` | reject | L0.registry | schema --error given a warning-only code, refused without the --warning hint (7.3.4) |
| `invalid/report-acme-warning-unregistered.json` | reject | L0.registry | a meta.warnings code that neither registry has (7.5.1) |
| `invalid/report-batch-failed-without-error.json` | skip | L2.batch-incomplete | a failed batch item without its own error (11.11) |
| `invalid/report-batch-incomplete-no-failed-item.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE with no failed item (11.11) |
| `invalid/report-batch-incomplete-without-data.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE without the batch report in data (11.11) |
| `invalid/report-batch-incomplete-wrong-field.json` | skip | L2.batch-incomplete | BATCH_INCOMPLETE whose field is not the failed index (11.11) |
| `invalid/report-batch-item-details-reason-in-1-0.json` | skip | L2.batch-incomplete | a reason in a batch item detail of a 1.0 document (10.13 rule 16, 11.11) |
| `invalid/report-batch-nested-retryable.json` | skip | L2.batch-incomplete | a nested item error whose retryable differs from its row (11.11) |
| `invalid/report-batch-nested-unregistered.json` | skip | L2.batch-incomplete | a nested item error with an unregistered code (11.11) |
| `invalid/report-batch-skipped-with-data.json` | skip | L2.batch-incomplete | a skipped batch item that carries data (11.11) |
| `invalid/report-batch-status-order.json` | skip | L2.batch-incomplete | batch item statuses out of order (11.11) |
| `invalid/report-batch-wrong-version.json` | skip | L2.batch-incomplete | a batch report with the wrong schemaVersion (6.11, 11.11) |
| `invalid/report-contract-version-explicit-1-0.json` | reject | L0.contract-version | contractVersion written as "1.0": it is never written out (15.7) |
| `invalid/report-failure-retryable-mismatch.json` | reject | L0.ok-exit | a failure whose retryable differs from its registry row (7.3.3) |
| `invalid/report-failure-unresolved-version.json` | reject | L0.unknown-command | an UNKNOWN_COMMAND failure whose schemaVersion is not "1.0" (6.11) |
| `invalid/report-failure-version-mismatch.json` | skip | L1.output-data | a failure whose schemaVersion is not the output the invocation would have produced (6.11) |
| `invalid/report-fields-missing-selected.json` | skip | L1.fields | a projection that lacks a selected member (6.10) |
| `invalid/report-fields-unselected.json` | skip | L1.fields | a projection that keeps an unselected member (6.10) |
| `invalid/report-ledger-entry-list-map-key.json` | skip | L1.fields | a map-key selection on a 1.0 tool (6.10) |
| `invalid/report-next-command-mismatch.json` | reject | L0.next | a next command that does not split into the tool and its argv (6.8) |
| `invalid/report-next-placeholder.json` | reject | L0.next | a placeholder in a next argv (6.8) |
| `invalid/report-next-without-json.json` | reject | L0.next | a next step of a JSON-mode answer without --json (6.8) |
| `invalid/report-output-missing.json` | skip | L1.output-data | a report whose meta.output names no output of the command (6.6, 10.9) |
| `invalid/report-page-cursor-without-next.json` | reject | L0.page | a page with nextCursor but no next entry that carries it (6.9) |
| `invalid/report-refusal-with-data.json` | reject | L0.ok-exit | a refusal that carries data (6.4) |
| `invalid/report-skills-show-markdown-key.json` | skip | L2.skills | a guide with a key the guide schema does not define (14) |
| `invalid/schema-error-acme-reason-lowercase.json` | reject | L0.registry | a registry reason that is not UPPER_SNAKE_CASE (7.6.1) |
| `invalid/schema-errors-acme-duplicate-code.json` | skip | L1.registry | a code listed twice in the registry (7.3.4) |
| `invalid/schema-errors-acme-stopped-midway-exit-2.json` | skip | L1.registry | `STOPPED_MIDWAY` at exit 2 in a 1.2 registry (7.2.1, 8.3.2) |
| `invalid/schema-errors-acme-duplicate-reason.json` | skip | L1.reasons | a reason listed twice under one code (7.6.1) |
| `invalid/schema-errors-acme-reasons-in-1-0.json` | reject | L0.contract-version | registry reasons in a schema answer without the 1.1 markers (7.6, 10.13 rule 16) |
| `invalid/schema-ledger-meta-bracket-field.json` | reject | L0.contract-version | a bracketed key in meta.fields of a 1.0 tool (6.10, 10.13 rule 16) |
| `invalid/schema-ledger-meta-warning-unregistered.json` | reject | L0.registry | a meta.warnings code that neither the error registry nor a warning registry has (7.5.1) |
| `invalid/schema-version-not-contract-version.json` | reject | L0.contract-version | a schema answer whose schemaVersion is not the contract version it declares (10.1.4) |
| `invalid/schema-warnings-acme-duplicate.json` | skip | L1.warnings | a warning code listed twice (7.5.2) |
| `invalid/schema-warnings-acme-in-1-0.json` | reject | L0.contract-version | the warnings document kind in a schema answer without the 1.1 markers (7.5.2, 10.13 rule 16) |
| `invalid/schema-warnings-acme-with-exit.json` | reject | L0.registry | a warning registry entry with an exit: it has none (7.5.2) |
| `invalid/stream-acme-build-bad-event.ndjson` | skip | L2.stream | an event that fails its eventRef schema (13.5) |
| `invalid/stream-acme-build-cut-off.ndjson` | skip | L2.stream | a stream without its end line (13.5) |
| `invalid/stream-acme-build-end-bad-data.ndjson` | skip | L2.stream | an end result whose data fails the output schema (13.5, 10.9) |
| `invalid/stream-acme-build-end-not-envelope.ndjson` | reject | L0.envelope | the result of a stream end line is not an envelope (13.5) |
| `invalid/stream-acme-build-end-other-command.ndjson` | skip | L2.stream | start and end that name different commands (13.5) |
| `invalid/stream-acme-build-end-refusal-with-data.ndjson` | reject | L0.ok-exit | the end result is a refusal that carries data (13.5, 6.4) |
| `invalid/stream-acme-build-end-retryable-mismatch.ndjson` | reject | L0.ok-exit | the end result carries a retryable that differs from its registry row (13.5, 7.3.3) |
| `invalid/stream-acme-build-end-version-mismatch.ndjson` | skip | L2.stream | start and end that disagree on the schemaVersion (13.5) |
| `invalid/stream-acme-build-event-without-type.ndjson` | skip | L2.stream | an event line without its type (13.5) |
| `invalid/stream-acme-build-markers-missing.ndjson` | reject | L0.contract-version | the end result of a 1.1 tool without the markers (15.7, 13.5) |
| `invalid/stream-acme-build-missing-start.ndjson` | skip | L2.stream | a stream whose first line is not the start line (13.5) |
| `invalid/stream-acme-build-one-line-not-refusal.ndjson` | skip | L2.stream | a usage error that prints a stream that is not one refusal envelope (13.5) |
| `invalid/stream-acme-build-quiet-with-event.ndjson` | skip | L2.stream | an event of a stream that declares none (13.5) |
| `invalid/stream-acme-build-start-contract-version-mismatch.ndjson` | skip | L2.stream | start and end that declare different minors (13.5, 15.7) |
| `invalid/stream-acme-build-start-contract-version-1-0.ndjson` | reject | L0.contract-version | a stream that writes `contractVersion` as "1.0" (6.6, 13.5) |
| `invalid/stream-acme-build-start-contract-version-unknown.ndjson` | reject | L0.contract-version | the end result declares a minor that is not the claimed one (15.7, 13.5) |
| `invalid/stream-acme-build-start-output-mismatch.ndjson` | skip | L2.stream | a start line that names another output than the end result (13.5) |
| `invalid/stream-acme-build-start-without-output.ndjson` | skip | L2.stream | a start line without the output a multi-output command must name (13.5) |
