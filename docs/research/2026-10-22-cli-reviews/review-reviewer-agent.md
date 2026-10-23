# Canon 0.28 CLI review

## Verdict

**Ship with changes.** The generated-spec, typed-report, clean-cutover, and preview/commit direction is strong, but the plan is not yet implementable from the skill alone because the final grammar, schema-version contract, batch atomicity, output variants, and several operand/required-option shapes are still underspecified.

## Findings

### Blocker: publish one complete, internally consistent grammar

**What:** D1 says `move`, `update`, `get`, and `delete` are not used, but D6 and the rename table introduce `artifacts move` (`plan.md` D1, D6, Renames). `context` vs `authoring`, `records` grouping, and `run` vs `dispatch` remain unresolved. The rename table does not specify every final operand and required option shape.

**Why:** An agent following only the generated skill cannot choose between a rejected verb and an exception, or construct several commands without probing help.

**Fix:** Make a complete final table normative before implementation. Remove `move` from the denylist or rename that leaf; resolve open groups; record every leaf's operand kind/arity, required options, mutual exclusions, effect, output variants, and example; generate skill and appendix from it.

### Blocker: define one schema-version grammar

**What:** D3 says `meta.schemaVersion` is `major.minor`, while D2/D8 require report-specific schema versions and major bumps. The checklist requires a stable report name, but the plan never states whether values such as `0.28.gate-check` are valid (`plan.md` D2/D3/D8; `cli-interface-design-checklist.md` JSON contract; `2026-10-22-cli-audit-output.md` §3).

**Why:** Consumers cannot implement compatibility checks or select a schema when the field has two grammars.

**Fix:** Specify one format, e.g. `<major>.<minor>.<report-name>`, define envelope-error versions and bump rules, put it in the generated spec, and test it in T7/T8.

### Blocker: make batch all-or-nothing enforceable

**What:** D7 promises JSON-array/NDJSON batches are all-or-nothing but only validates before the first write. It does not define rollback/transaction behavior when a later filesystem, ledger, git, or artifact operation fails, or define plan-digest/prompt/per-item report semantics.

**Why:** A batch can partially mutate state and then fail, making retries duplicate records.

**Fix:** Implement a transaction/journal and test a runtime write failure after a valid first item, or change the contract to best-effort and report committed/failed items explicitly. Define ordering, duplicate items, limits, and preview/commit scope.

### Blocker: specify output variants

**What:** D3 excludes artifacts/streams from the envelope; D2 requires outputs for every leaf; D7 requires a combined batch report. The plan does not say which form applies to `export`, `report --snapshot`, context packs, prompt operations, approval payloads, or artifact-plus-report commands (`plan.md` D2/D3/D7; `2026-10-22-cli-audit-output.md` §1–3).

**Why:** An agent cannot know whether stdout is an envelope, artifact path, raw bytes, or stream. The retrieve hook currently parses a bare array (`2026-10-22-cli-audit-callers.md` §5–7).

**Fix:** Add an output matrix per leaf and mode: media type, envelope status, schema id/version, stderr, and artifact location. Migrate and test retrieve and all artifact consumers before changing shape.

### Should-fix: decide operands and requiredness per leaf

**What:** D1 gives the rule “primary object is the operand” but does not decide it for `divergence accept/add/defer/resolve`, ordinary evidence, finding add, review change, or purge. `--project-id` requiredness changes with repository configuration (`plan.md` D1, Renames; current schema).

**Why:** Agents will put an id in an option and get a usage error, or target the wrong object. Configuration-dependent syntax makes generated examples invalid in some repositories.

**Fix:** Add a per-leaf operand table; choose one object kind/arity, mark all other ids as `--<noun>-id`, define inference/refusal for project roots, and specify purge's target. Add parser tests for every identifier-bearing leaf and batch form.

### Should-fix: make `--input` lossless and bounded

**What:** D7 does not define paths relative to repo/cwd, null vs omitted, empty arrays, repeated values, negated flags, comma-list options, positional operands, stdin collisions, document/line/batch limits, or whether commit controls may be input keys.

**Why:** Document-to-argv conversion can change meaning, split paths, consume the wrong stdin, or accept an unbounded batch.

**Fix:** Publish schemas for omitted/null/empty/repeatable/negated/operand values; reject ambiguous stdin; set limits; preserve each value as one argv element without shell parsing; test every arity and hostile path.

### Should-fix: close safety and refusal gaps

**What:** D6 names five preview/commit leaves but gives no complete safety matrix, target definition for purge, digest canonicalization/expiry/replay rules, or guarantee that preview itself does not write. It also adds dry-run inconsistently to other writers (`plan.md` D6; audit surface X4).

**Why:** A stale digest can apply a plan to different data, and a supposedly dry run may still refresh indexes.

**Fix:** Add a per-command safety table with effect class, preview schema, digest inputs/canonicalization, commit requirements, and refusal guarantees; test changes between preview and commit.

### Should-fix: make errors and exits mechanical

**What:** D3 requires `received`, `retryable`, and `hint` without defining field paths, redaction, or retryability; D4 has a special `brief check` exit convention (`plan.md` D3/D4; `2026-10-22-cli-audit-output.md` §4–6; coverage-and-original §A3).

**Why:** Agents cannot safely associate an error with input or decide whether retrying is useful.

**Fix:** Define JSON paths/argv names, redaction, expected/received rules, and a registry rule for retryability. Make exits uniform or encode exceptions in metadata and test every error code/exit.

### Should-fix: strengthen clean-cutover recovery

**What:** D8/D9 list skills, AGENTS, docs, dashboard, trace checker, conformance, hooks, and examples but do not define version detection, backups/rollback, or how migration works when an old hook/binary cannot parse the new command (`plan.md` D8/D9; `2026-10-22-cli-audit-callers.md` §1–7).

**Why:** Installed 0.25-era hooks are a major breakage surface; the first new invocation may fail before it repairs callers.

**Fix:** Ship a versioned migration manifest and old-to-new map; make migrate idempotent, back up files, report changes, and provide a standalone recovery invocation. Add an installed-old-hook fixture to D9/T14.

### Should-fix: expand T1–T16

**What:** The tests do not explicitly cover schema-version grammar, complete error/exit mapping, received/retryability, argv/path safety, runtime batch atomicity, plan-digest binding, output media types, or installed-caller migration (`plan.md` T1–T16; checklist Enforcement).

**Why:** Generated metadata proves existence, not that runtime semantics are true.

**Fix:** Add tests for output matrices, error invariants, hostile argv round trips, digest invalidation, rollback, old-hook migration, and literal command scans over generated/installed artifacts.

### Note: standard CLI conventions omitted

The checklist recommends `--no-input`, bounded output, and `--explain CODE`; the plan rejects `--no-input` and does not define universal bounds or diagnostic lookup (`plan.md` D6/D7; checklist Safety, Output, Errors). If the single prompt rule remains, expose `interactive`, `promptedInputs`, and non-TTY behavior in generated metadata, add bounds, and decide whether task-scoped context is intentionally out of scope.

## Answers to the eight questions

1. **Grammar:** Most renames are guessable, especially `subject show/status`, `divergence list`, `task done`, and `strategy`. `artifacts move` contradicts the verb denylist; `brief show` is not listed as a generated leaf; `gate promote` now spans gate and divergence concepts; `evidence add`/`evidence audit` and `review scenario`/`review change` need complete option tables. The final list below is the walkable surface.
2. **Operands:** The rule is good but not decidable for divergence mutations, ordinary evidence, finding add, review change, purge, and several authoring commands. Primary object and repeatability must be explicit per leaf, not inferred from old options. `--project-id` must have a stable inference rule.
3. **Envelope/errors:** `ok` and command/schema metadata are improvements. Traps are the version grammar conflict, artifact/stream exceptions, partial data on failure, `meta.next` versus old top-level `next`, and unspecified `received`/retryability/redaction.
4. **Input:** Unknown-field and conflict refusal are right. Missing rules cover stdin, path base, null/empty/repeatable values, negated flags, size limits, batch rollback, output shape, and commit/plan digest. “Document to argv” must be lossless and non-shell-parsed.
5. **Safety:** The five named preview/commit operations are plausible; `purge` target is undefined and any rewrite/delete operation needs classification. Digest binding and preview side effects are unspecified. `export`/`report` file writes need explicit treatment.
6. **Cutover:** No aliases is acceptable only with the promised regenerated skill/docs/hooks, removed-spelling hints, migration manifest, backups, version detection, and old-hook recovery. D8/D9 currently enumerate consumers but not recovery mechanics.
7. **Spec/tests:** T1–T16 are an excellent skeleton for parser/spec drift and typed reports, but miss runtime atomicity, output variants, digest binding, hostile argv, error semantics, and installed caller migration.
8. **Big CLIs/checklist:** Standard `--no-input`, `--yes`, bounds, explainable diagnostics, task-scoped context, and explicit output media types are stronger conventions in `cli-interface-design-checklist.md`; the plan intentionally omits or leaves them partial.

## Things done right

- One generated checked-in spec with exhaustive metadata and drift testing (`plan.md` D2).
- Typed reports and schema validation instead of untyped JSON (`plan.md` D2, T7/T10).
- Stable envelope, context, next steps, and registered error codes (`plan.md` D3).
- Preview-by-default plus digest-bound commit and refusal-without-side-effects (`plan.md` D6, T12).
- Unknown-field/conflict refusal and generic JSON/NDJSON input (`plan.md` D7).
- Clean cutover paired with generated skills/docs/hooks and removed-spelling hints (`plan.md` D8/D9).

## Common agent flows and every guess

### Write a subject

1. `canon init --repo . --providers omp --json` — **[GUESS]** provider values, root normalization, and whether init's new dry-run default applies.
2. `canon subject new feature-login --domain web --title Login --json` — **[GUESS]** requiredness is inherited from current schema; final row is absent.
3. `canon change new login --subject-id feature-login --kind feature --title Login --json` — **[GUESS]** final operand/options and valid kind.
4. `canon brief show --change-id login --json` — **[GUESS]** whether show takes `--change-id` or `<CHANGE_ID>`.
5. `canon brief check --change-id login --json` — syntax unchanged in plan.
6. `canon brief approve login --approver alice --note approved --json` — **[GUESS]** note required and operand shape; rename only names approver.
7. `canon scenario new smoke --title logs-in --subject-id feature-login --change-id login --case '...' --flow '...' --json` — **[GUESS]** repeatability and input replacement for `--from`.

### Record evidence for several scenarios

1. `canon dispatch begin --role implementer --regime default --actor-id agent-1 --provider omp --model gpt --task login#1 --json` — **[GUESS]** valid regime and task grammar.
2. `canon dispatch end --run-id RUN --outcome succeeded --json` — **[GUESS]** closed outcome values.
3. `canon evidence add --scenario-id auth.login.01 --class test-run --ref 'cargo test' --actor-id agent-1 --role implementer --json` — **[GUESS]** ordinary evidence retains this leaf and its required fields.
4. Repeat step 3 for `.02` — **[GUESS]** whether ordinary evidence accepts repeated operands or only input batch.
5. `canon evidence audit auth.login.01 --evaluator-id eval --evaluator-version 1 --evaluator-digest HASH --audit-result confirmed --ref report.json --json` — **[GUESS]** whether audit retains `--ref` and exact requiredness.
6. `canon gate promote --kind evidence --json` — **[GUESS]** whether kind is required and its values after merging divergence promotion.

### Review scenarios

1. `canon retrieve --role reviewer --limit 20 --json` — **[GUESS]** envelope versus bare array; current hook expects bare array.
2. `canon review scenario auth.login.01 auth.login.02 --actor-id reviewer-1 --role reviewer --pin --json` — **[GUESS]** pin arity, actor/reviewer replacement, project-id inference.
3. `canon finding add --change-id login --round 1 --seq 1 --severity blocker --reviewer reviewer-1 --summary '...' --json` — **[GUESS]** whether reviewer remains or becomes actor/role.
4. `canon finding close login#1#1 --disposition fixed --json` — **[GUESS]** finding-key syntax.
5. `canon divergence accept --project-id root --scenario-id auth.login.01 --reason accepted --tracking issue-1 --reviewer reviewer-1 --json` — **[GUESS]** operand migration and tracking requiredness.

### Ship a subject

1. `canon subject show feature-login --json` — operand shape is implied but not fully specified.
2. `canon gate check --json` — exit 1 means failed check under D4.
3. `canon subject status feature-login --to shipped --reason ready --actor-id agent-1 --json` — **[GUESS]** final requiredness and actor option.
4. Run `canon gate check --json` again — **[GUESS]** whether required and whether `meta.next` is authoritative; current `status` uses top-level `next` while skill expects `meta.next` (`2026-10-22-cli-audit-callers.md` §6).
5. If refused, `canon divergence accept ... --override-reason '...' --json` — **[GUESS]** whether override is present on this leaf.

## Appendix A — full provisional final leaf list

Required options below mean command-specific required values; shared `--json`, `--as`, and conditional `--repo` are omitted. `?` is an explicit unresolved plan mismatch, not an implementation recommendation. The list contains 71 leaves after the stated merges/splits.

| Leaf | Operand | Required options |
|---|---|---|
| adapter authorize | — | --response, --repo |
| adapter validate | — | --response |
| artifacts move | — | --tier |
| artifacts verify | — | — |
| artifacts store | — | — |
| brief show | ? | ? --change-id or <CHANGE_ID> |
| brief approve | <CHANGE_ID> | --approver, --note |
| brief check | — | — |
| change new | <SLUG> | --subject-id, --title |
| context | — | — |
| context-pack add | — | --manifest |
| context-pack show | <ID> | — |
| context-pack verify | <ID> | — |
| dashboard | — | — |
| demo review | — | — |
| demo init | — | — |
| dispatch begin | — | --role, --regime |
| dispatch diff | — | — |
| dispatch end | — | --run-id, --outcome |
| divergence accept | ? likely <SCENARIO_ID> | --project-id, --reason, --tracking, --reviewer |
| divergence add | ? likely <SCENARIO_ID> | --project-id, --commit, --reviewer, --role |
| divergence defer | ? likely <SCENARIO_ID> | --project-id, --commit, --reviewer, --reason, --expiry, --role |
| divergence list | — | — |
| divergence resolve | ? likely <SCENARIO_ID> | --project-id, --commit, --reviewer, --role |
| evidence add | ? | --class, --ref |
| evidence approval-payload | — | --commit, --approver, --approval-at |
| evidence audit | <SCENARIO_ID> | --evaluator-id, --evaluator-version, --evaluator-digest, --audit-result |
| export | — | --kind |
| feature new | <SURFACE> | --title |
| finding add | ? | --change-id, --round, --seq, --severity, --reviewer, --summary |
| finding close | <FINDING_KEY> | --disposition |
| finding list | — | --change-id |
| format | <ROOT> | — |
| gate check | — | — |
| gate install-hooks | — | — |
| gate promote | — | ? --kind |
| selftest | — | — |
| task done | <TASK_ID> | — |
| ingest records | — | — |
| ingest plans | — | — |
| ingest sessions | — | — |
| init | — | — |
| inventory sync | — | — |
| strategy approval-payload | <STRATEGY_ID> | ? --evaluation, --approver |
| strategy promote | <STRATEGY_ID> | — |
| strategy rollback | <STRATEGY_ID> | --reason, --actor-id, --signature-file, --approval-at |
| ledger compact | — | — |
| migrate | — | — |
| plugin sync | <PLUGIN_ID> | — |
| policy approve | — | --approver |
| policy show | — | — |
| prompt add | — | --name, --bundle-version, --manifest |
| prompt show | — | --name, --bundle-version |
| purge | ? target unspecified | --kind, --before |
| query | — | --kind |
| regime show | — | --role-segment, --repo, --area, --hash |
| report | — | — |
| retrieve | — | --role |
| review scenario | <SCENARIO_ID>... | ? --reviewer, --pin |
| review change | <CHANGE_ID> or ? | ? --reviewer, --pin |
| scenario new | [<TAG>] | — |
| schema | [<PATH>...] | — |
| skills check | — | — |
| skills doctor | — | — |
| skills install | — | — |
| status | — | — |
| subject adopt | <CHANGE_ID> | --subject-id |
| subject new | <ID> | --domain, --title |
| subject show | <ID> | — |
| subject status | <ID> | --to |
| subject unadopt | <CHANGE_ID> | --subject-id |
| tier age | — | — |

**Count/mismatch note:** Current schema has 70 leaves (`canon-0.27-schema-all.json`). The provisional count assumes one fewer from merging `gate selftest` into `selftest` and `divergence promote` into `gate promote`, plus `brief show`, review split, evidence audit split, and subject show split. The plan must confirm whether `brief show` is represented in the generated spec and whether evidence ordinary/audit is exactly two leaves. Requiredness and operands marked `?` are precisely the gaps the final normative table must close.

## Sources

`plan.md` D1–D9, Renames, T1–T16, owner decisions; `canon-0.27-schema-all.json`; `2026-10-22-cli-audit-surface.md`; `2026-10-22-cli-audit-output.md`; `2026-10-22-cli-audit-callers.md`; `2026-10-22-cli-bigcli-research.md`; `2026-10-22-cli-coverage-and-original.md`; `cli-interface-design-checklist.md`.
