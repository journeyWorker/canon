# Verdict

**Not ready.** The per-leaf table closes most round-1 gaps and is substantially more usable, but it still violates its own one-name/one-meaning rule and contains non-runnable generated next steps. Resolve the blockers below, then re-run the table and generated-skill walk before implementation.

## Round-1 status

| Round-1 finding | Status | Evidence in the revision |
|---|---|---|
| Reviewer: publish one complete, internally consistent grammar | partly | `cli-commands-0.28.md` is now normative and resolves most operands, but D1/O4/O5 still leave `--kind` and `--outcome` with multiple meanings, and the shared `--role` has distinct scope semantics. |
| Reviewer: define one schema-version grammar | resolved | D3 and Q4 define per-report `"1.0"`; `meta.command` names the report and `canonVersion` names the release. |
| Reviewer: make batch all-or-nothing enforceable | partly | D7 now explicitly permits a later write failure with `written`/`failed`/`skipped`, so it no longer promises rollback, but the batch output schema and all side-effect boundaries are not present in the leaf table. |
| Reviewer: specify output variants | partly | The table labels artifacts and streams, and D3/D7 describe JSON mode, but batch variants and several watch/artifact stdout contracts are not independently specified as schemas. |
| Reviewer: decide operands and requiredness per leaf | resolved | The table gives operands and required options for the previously unresolved divergence, evidence, review, finding, and purge leaves; the resolved-questions appendix confirms the decisions. |
| Reviewer: make `--input` lossless and bounded | resolved | D7 now specifies camelCase keys, `operands`, null refusal, path base, `--key=value` argv construction, stdin conflicts, 8 MiB/10,000 limits, and batch validation. |
| Reviewer: close safety and refusal gaps | partly | D6/table now name all five preview commands and the purge kind set, but migration recovery still emits a digest placeholder and the complete preview/output contract is not generated per leaf. |
| Reviewer: make errors and exits mechanical | partly | D3 defines field paths, redaction, suggestions, hints, and retryability; D4 defines the main mapping, but T5 does not test the claimed per-command/runtime accuracy of those fields. |
| Reviewer: strengthen clean-cutover recovery | partly | D8/D9 add hook normalization, migration backups, version detection, and a fixture, but the prescribed migration next step is not runnable until a dynamic digest is obtained. |
| Reviewer: expand T1–T16 | partly | T17/T18 and T16 add commit/reverification/input coverage, but no test proves the three remaining option-meaning collisions, dynamic migration next steps, or complete batch/output schemas. |
| Reviewer: standard CLI conventions omitted | not resolved | `--no-input` and `--explain` remain intentionally omitted, and large-output bounds are only present on selected leaves; D6 explains the first omission but does not close the checklist gap. |
| Gemini B1: `--commit` boolean versus valued SHA collision | resolved | D7b explicitly reserves `--commit` for destructive execution and uses `--rev`, `--resolution-rev`, and `--introduced-rev` in the table. |
| Gemini B2: `CANON_ROLE` breaks self-description | resolved | D3 explicitly makes `schema`, `context`, and `skills get` independent of `CANON_ROLE`; `--as` is the explicit filter. |
| Gemini B3: installed retrieve/hooks break silently | resolved | D9 specifies `(.data // .)`, renamed hook commands, migration detection, backups, and a 0.27 fixture; `gate install-hooks` has the new default. |
| Gemini B4: `--input` translation/path/batch hazards | resolved | D7 specifies lossless argv, cwd-relative paths, null/boolean rules, stdin refusal, limits, and indexed batch errors. |
| Gemini S1: grammar/rename violations | partly | `report` and `subject move` are split and `fmt` is removed, but the table still has the `--kind`, `--outcome`, and `--role` collisions described below. |
| Gemini S2: ambiguous primary operands | resolved | D1 r1/r2/r3 and the table explicitly settle the formerly ambiguous leaves, including `subject adopt`, divergence, review, finding close, and purge. |
| Gemini S3: envelope/exit/meta gaps | partly | The schema-version and relative-context issues are fixed, and partial data is named for checks; the registry-to-runtime accuracy and dynamic recovery details remain unproven. |
| Gemini S4: destructive effect/purge values | resolved | Q1 says all five are `destructive`, and the table restricts purge to `session`, `run`, `event`, and `trajectory`. |
| Gemini S5: spec/test gaps | partly | T1–T18 improve coverage substantially, but T3 still cannot pass while the same options have distinct meanings, and T14 cannot validate an ellipsis placeholder as a runnable command. |
| Gemini N1: harness session-id fallback | not resolved | The plan continues to require/accept `--session-id` but does not adopt the proposed harness-environment capture. This is not independently blocking, but remains an intentional gap. |
| Gemini N2: `trace_check.py` hard-coded verb/hint coupling | partly | D9 says the read/write split comes from spec effect and T14 parses docs/hints, but it does not explicitly replace the existing regex/read-action implementation or test its exact migration wording. |
| Gemini N3: `format` no-op/check and alias | partly | The `fmt` alias is removed, but the table retains required `<DIR>` and the no-op `--check` (O6), contrary to the recommended complete cleanup. |

**Count:** 9 resolved / 12 partly / 2 not resolved (23 round-1 findings, including notes).

## New findings — blocker

### 1. Enforce one meaning for `--kind`, `--outcome`, and `--role`

**Where:** `plan.md` D1 option rule and O4/O5; the shared-options header and the `change new`, `query`/`export`/`purge`, `dispatch end`, `review scenario`, `review change`, `retrieve`, and `dispatch begin` rows in `cli-commands-0.28.md`.

**Why:** D1/T3 require one option spelling to have one value name, arity, and meaning, but the final table leaves three direct violations. `--kind` selects record kinds on `query`, `export`, `purge`, and `gate promote`, while `change new --kind {feature,chore}` selects a brief template. `--outcome` means a dispatch result (`succeeded|failed`) but a review result (`signed|indeterminate|unavailable`). Most seriously, the shared option is declared as attribution `--role`, while the table explicitly says `retrieve --role` and `dispatch begin --role` are scopes/guidance rather than attribution. A generated skill cannot safely reuse a shared vocabulary row or derive one JSON input field for these cases, and T3's stated one-meaning assertion cannot pass.

**Fix:** Rename at least `change new --kind` (for example `--change-kind`), use distinct names for dispatch and review outcomes (for example `--run-outcome` and `--review-outcome`), and separate scope from attribution (`--scope-role` or an equivalent command-specific name). Remove O4/O5's exception instead of weakening T3.

### 2. Make D11's generated batch recovery command runnable

**Where:** `plan.md` D11, especially the command `evidence add --all-scenarios --report junit:a.xml --report junit:b.xml`; `cli-commands-0.28.md` row `canon evidence add`.

**Why:** The table makes `--class <CLASS>` and `--ref <TEXT>` unconditionally required for `evidence add`. D11's prescribed command omits both, so the exact command that `status`, `meta.next`, and refusal hints are required to lead with is rejected by the parser. This directly violates D2's rule that generated `meta.next`/skill commands are runnable and makes the newly promised one-command re-verification flow unusable.

**Fix:** Either include valid `--class` and `--ref` (and any other required attribution fields) in the D11 batch template, or make a separately specified report-only evidence mode with its own leaf and schema. Add the exact generated command to T14/T18 and parse it against the final spec.

## New findings — should-fix

### 3. Publish the batch output as a first-class per-leaf schema variant

**Where:** D2 outputs/inputs, D3/D7, and the output column for every writing leaf in `cli-commands-0.28.md`.

**Why:** D7 promises that a JSON-array/NDJSON invocation returns `data.items[]` with `written`, `failed`, `skipped`, and `unchanged`, while each affected table row declares only its ordinary `Report <Name>`. No `Batch<Name>Report`, discriminator, schema version, or `outputs` variant tells an agent whether `data` is the ordinary record or the batch result. T7/T8 can validate an implementation against an ad-hoc schema while the generated skill still cannot determine the response shape.

**Fix:** Add a standard batch output variant to each writing leaf's `outputs` entry, with a stable schema name/version and exit-2 validation-error shape. Put the batch example and whether `data` is present on exit 1/2 in the per-leaf row.

### 4. Do not make `init --check --json` syntactically impossible

**Where:** `cli-commands-0.28.md`, `canon init` row: “`--check` conflicts with every other option”; shared options: `--json`, `--as`, and conditional `--repo`.

**Why:** The literal “every other option” includes the universal `--json` output mode. Thus the documented `InitCheckReport` cannot be requested as JSON, despite D3 saying every leaf has `--json` and the table presenting `--json` as the machine-readable contract. It also makes the check's repo selection and explicit role impossible if they are treated as options in the same conflict set.

**Fix:** Define the conflict only against init's mutating command options (`--no-agents-md`, `--policy`, `--no-policy`, `--providers`, `--language`, `--dry-run`), and explicitly allow shared output/context options. Add a parser witness for `canon init --check --json`.

### 5. Define JSON/NDJSON framing for `--input` files and stdin

**Where:** D7 and T16.

**Why:** D7 says an input beginning with `{` or `[` is inline JSON, otherwise it is a path; a file or stdin may be a JSON array or NDJSON. It never says how a pretty-printed single JSON object is distinguished from NDJSON, how blank lines/comments are handled, or whether a one-object file returns an ordinary report or a batch report. A valid multi-line JSON object can therefore be split into invalid NDJSON records, and the response shape remains ambiguous.

**Fix:** Specify framing (for example: parse the complete source as JSON first; only if that fails and the source is file/stdin, parse non-empty lines as NDJSON), define blank-line/comment rules, and state that one object is scalar invocation while an array/NDJSON sequence is batch. Add these cases to T16.

### 6. Replace the migration digest ellipsis with a two-step generated recovery

**Where:** D9: `meta.next` puts `canon migrate --commit --plan-digest …` first; D6's preview/commit contract and D8's requirement that hints be working invocations.

**Why:** `--plan-digest` is required to commit a migration, but `…` is not a digest and cannot be parsed. An old hook/skill consumer therefore receives a recovery command it cannot execute, precisely at the cutover path that must recover without probing help. The plan already defines the safe preview, so the next step must first run the preview and then carry its returned digest.

**Fix:** Make the generated next step `canon migrate --json` (preview), document extraction of `data.planDigest`, then construct `canon migrate --commit --plan-digest <returned digest> --json`; or provide a dedicated safe `migrate --apply-preview <DIGEST>` recovery field whose generated argv contains a concrete value. Test the exact `meta.next[].argv` shape in the migration fixture.

## Agent flow walk

The following uses only the normative table and D1–D12. `<...>` denotes a value that must be discovered from the repo or a prior report; those are the remaining guesses/unknowns, not alternate syntax.

### 1. Write a subject with a brief and scenarios

```text
canon init --repo . --providers omp --json
canon subject new payments --domain dev --title Payments --json
canon change new add-search --subject-id payments --title "Add search" --json
canon brief show add-search --json
canon brief check add-search --json
# human-only step, so ask the human:
canon brief approve add-search --approver alice --note "approved as written" --json
canon feature new payments.search --title "Search" --project-id <PROJECT_ID_IF_MULTIPLE_ROOTS> --json
canon scenario new payments.search.01 --title "Find an existing payment" --subject-id payments --change-id add-search --flow F1 --feature <FEATURE_FILE> --json
canon scenario new payments.search.02 --title "No matching payment" --subject-id payments --change-id add-search --flow F2 --feature <FEATURE_FILE> --json
```

**Guesses/unknowns:** `<PROJECT_ID_IF_MULTIPLE_ROOTS>` is configuration-dependent (the table does state the rule, but the value must be discovered); `<FEATURE_FILE>` and the repo's activated `domain`/flow vocabulary are not inferable from the command table. The `--role`/`--kind` collision also makes any generated input document for `change new` unsafe until blocker 1 is fixed. `brief approve` is intentionally human-only, not an agent guess.

For several scenarios, the table's replacement for the removed `--from` is an input batch, not repeated CLI operands:

```text
canon scenario new --input '[{"operands":["payments.search.03"],"title":"Timeout"},{"operands":["payments.search.04"],"title":"Retry"}]' --json
```

This syntax is determined by D7, but the exact JSON batch response is still a guess until finding 3 is fixed.

### 2. Record evidence for several scenarios

```text
canon regime show --role-segment implementer --repo-name <REPO_NAME> --area <AREA> --hash <6_TO_64_LOWERCASE_HEX> --json
canon dispatch begin --role implementer --regime <REGIME_KEY> --actor-id agent-1 --provider omp --model <MODEL> --task-id add-search#1 --json
# run the tests, then:
canon dispatch end <RUN_ID> --outcome succeeded --json
canon evidence add --all-scenarios --class test-run --ref "<COMMAND_OR_REPORT_REF>" --summary "<SUMMARY>" --run-id <RUN_ID> --report junit:<JUNIT_FILE> --actor-id agent-1 --role implementer --json
canon gate promote --kind evidence --json
```

**Guesses/unknowns:** `<REGIME_KEY>` must be the exact key built from the repo's role/name/area/hash and `<RUN_ID>` must be returned by dispatch; the table gives the syntax but not those values. The table states `--report` accepts `junit`, `cucumber`, or `swift-testing`, but does not define whether the path must exist before evidence is recorded or how a failed/skipped report is surfaced outside D11. `--role` currently has the scope/attribution collision from blocker 1. `gate promote --kind evidence` is optional in the table, so the explicit form is valid; the report's staged/promoted item shape is not shown in this flow.

For one specific scenario, the table gives the fully determined shape:

```text
canon evidence audit payments.search.01 --ref <REPORT_REF> --evaluator-id eval --evaluator-version 1 --evaluator-digest <64_LOWERCASE_HEX> --audit-result confirmed --actor-id auditor --role reviewer --json
```

**Guess:** The evaluator version is free text, but the valid evaluator identity/digest and the project id (when multiple roots exist) are repo-specific.

### 3. Review the scenarios and findings

```text
canon retrieve --role reviewer --domain dev --limit 20 --json
canon review scenario payments.search.01 payments.search.02 --actor-id reviewer-1 --original-spec-ref docs/prd.md --rev HEAD --role reviewer --json
canon finding add --change-id add-search --round 1 --seq 1 --severity blocker --actor-id reviewer-1 --summary "<FINDING>" --rev HEAD --role reviewer --json
canon finding list --change-id add-search --open --json
canon finding close add-search --round 1 --seq 1 --disposition fixed --resolution-rev HEAD --actor-id implementer --role implementer --json
canon gate promote --kind finding --json
```

**Guesses/unknowns:** The command/operand/required-option syntax is now specified. The review's `--original-spec-ref` must be a valid repo-relative provenance reference, and the scenario ids must already be indexed; the table says those values can refuse but does not provide discovery commands. The finding close row says `--role` defaults to reviewer, while this example uses implementer; whether the implementer may close a reviewer finding is a policy/runtime rule not exposed in the row. The review report's exact pinned evidence fields are not shown, so an agent must use the schema rather than infer them.

### 4. Ship the subject

```text
canon subject show payments --json
canon gate check --json
canon subject move payments --to shipped --actor-id implementer --json
canon gate check --json
```

**Guesses/unknowns:** The transition syntax and done criterion are specified. Whether the gate permits shipping depends on evidence, approvals, policy, and chore review; the table describes refusal classes but not the concrete prerequisite records in a single discoverable command. If `gate check` fails, `meta.next[].argv` is authoritative, but the actual next command is data-dependent.

A divergence acceptance cannot be guessed as an agent workaround: `canon divergence accept <SCENARIO_ID>...` is human-only and must be handed to the human, per D10.

### 5. Chore re-verification

```text
canon change new docs-cleanup --subject-id payments --kind chore --title "Update docs" --json
canon review change docs-cleanup --actor-id reviewer-1 --original-spec-ref docs/prd.md --rev HEAD --role reviewer --json
# if D11 says the codeTree changed:
canon evidence add --all-scenarios --class test-run --ref "<REPORT_REF>" --report junit:a.xml --report junit:b.xml --actor-id agent-1 --role implementer --rev HEAD --json
canon gate promote --kind evidence --json
canon subject move payments --to shipped --actor-id implementer --json
```

**Guess:** D11 explicitly names `evidence add --all-scenarios --report junit:a.xml --report junit:b.xml`, but that command is invalid because the table requires `--class` and `--ref`; I supplied them here as placeholders. The report paths, class, ref, actor, and whether `--project-id` is needed are not specified by D11. This is the concrete blocker in finding 2. For a docs-only chore, D11 says no scenario evidence is needed and only the change review is required, but the exact status/ship command that proves the codeTree comparison is not shown.

### 6. Destructive purge

```text
canon purge --kind session --before 2026-01-01 --json
# read data.planDigest from the preview, then:
canon purge --kind session --before 2026-01-01 --commit --plan-digest <PLAN_DIGEST_FROM_PREVIEW> --json
```

**Guesses/unknowns:** The syntax, closed kind set, preview default, and commit guard are specified. `<PLAN_DIGEST_FROM_PREVIEW>` must be copied from the first report; the table does not state the digest encoding/canonicalization, expiry/replay behavior, or exact `effects[]` schema. A changed dataset between the two calls should produce `PLAN_CHANGED`, but the plan does not show how the agent discovers the new preview or whether the old digest may ever be reused.

## 10-line summary

Verdict: not ready; fix the remaining grammar and recovery blockers before implementation.
Round-1 findings: 9 resolved, 12 partly resolved, 2 not resolved.
The final table successfully settles most previously ambiguous operands and required options.
D3/D7b/D9 address schema versioning, role self-description, commits, and installed caller migration.
`--kind` still means record kind and change template kind, violating D1/T3.
`--outcome` still means dispatch outcome and review outcome, with incompatible value sets.
`--role` is attribution in shared options but scope in retrieve and dispatch begin.
D11 emits a non-runnable evidence batch because required `--class` and `--ref` are omitted.
Batch response variants and framing still need explicit generated schemas.
The flows are mostly constructible, but repo-derived ids, reports, and dynamic plan digests remain explicit discovery points.
