# Changelog

The full release notes before 0.32 are readable with `git show e304d13b:README.md`.

## Unreleased

### Breaking

**The stateless checks of every command answer in the order of atip/1 1.2** (`canon-0-35-canon-check-order`; spec 11.7 step 6, 10.8.1, 10.8.3). A call that breaks two things at once is answered by the earlier one, with its code and `field`, the same for every tool that declares 1.2: a failure to parse (an unknown option or command word, a missing option value, a value that is no integer or not one of the `possibleValues`; the earliest on the command line) → a missing `required` argument (the operands in their order, then the options in their declaration order) → the domains (the same order) → the constraints (in the order `canon schema` lists them) → the validators (in the order it lists them). The order is the same with `--input` and without it. Codes, exits, the `constraints`, `domain` and `validators` of every document in `canon schema` and what a command does with a call that breaks one thing are unchanged; only the answer to a call that breaks several differs.

| Call (everything else valid) | 0.34 answers | 0.35 answers |
|---|---|---|
| `canon finding add … --round 0 --promote --no-promote` | `USAGE`, `--promote` (clap met the conflict first) | `FINDING_INVALID_ARGUMENT`, `--round` (a domain answers before a constraint) |
| `canon finding add --change Bad` (other required arguments missing) | `USAGE`, `--change` (clap read the value as it met it) | `USAGE`, `--round` (a missing required argument answers before a domain; the field is the first missing one) |
| `canon finding close add-login --seq x --bogus` | `USAGE`, `--bogus` (clap drops the verdict on a value when an unknown option follows it) | `USAGE`, `--seq` (the earliest failure to parse) |
| `canon finding add … --session " x" --round 0` | `USAGE`, `--session` (a value clap's parser refused) | `FINDING_INVALID_ARGUMENT`, `--round` (domains answer in the document's order: `--round` is declared before `--session`, whatever the line's order) |
| `CANON_ROLE=bogus canon finding add … --round 0` | `UNKNOWN_ROLE` | `FINDING_INVALID_ARGUMENT`, `--round` (a check on an environment value is a validator, and answers after the others; alone it is still `UNKNOWN_ROLE`) |
| `canon --as zzz nosuch` (and `canon --as zzz`, `canon finding --as zzz`) | `UNKNOWN_COMMAND` / `COMMAND_REQUIRED` (the option was moved after the command words before it was read) | `USAGE`, `--as` (a failure to parse at the place it was written) |

`--as` is a closed set the parser reads: a value that is no role is a failure to parse (`USAGE`, `--as`), as before, answered at its position on the line; `--as` still overrides an unknown `CANON_ROLE`. `CANON_ROLE` is the one variable that fills an option's value of a command; `CANON_ACTOR` fills `--actor` of `feature new` and `scenario new` (no domain) and is read by the validators of a relayed approval. A domain and a validator now judge the argument object (the value given, else the variable's, else the default); a constraint still counts only what was given (spec 10.8.1).

The refusals the stateless stage makes now say what they say in one voice, so a few messages and hints changed (the code and the `field` did not):

| Refusal | 0.34 message | 0.35 message |
|---|---|---|
| `canon evidence add` with no `--class` and no `--report` | `the following required arguments were not provided: --class, --ref` (clap) | ``one of `--class`, `--report` is required, and none was given`` |
| `canon ingest plans --dialect openspec` | `the following required arguments were not provided: --plan-source` | `` `--dialect` is given, which needs `--plan-source` `` |
| `canon init --policy light --no-policy` | `the argument '--policy' cannot be used with '--no-policy'` | `` `--check`, `--policy`, `--no-policy` are exclusive: at most one may be given `` |
| a `--change`, `--subject`, `--scenario`, `--session`, … outside its grammar | `invalid value 'Bad' for '--change <CHANGE_ID>': invalid ChangeId: …` | `--change must be a kebab-case slug: …` (the value stays in `received`) |
| an integer outside its bound (`--limit 0`) | ``invalid value '0' for '--limit <N>': `0` is below 1: an integer from 1 to 9007199254740991`` | `--limit must be an integer from 1 to 9007199254740991` |
| `canon init --policy light --no-policy` | `expected`: `light`, `standard`, `strict` | the same (the refusal of an excluding pair names the closed set of the argument at fault) |
| any domain or constraint refusal | `hint` named `canon schema <command> --json` | `hint` holds a working call, then `canon schema <command> --json` |

For contributors: no relation is declared to clap any more (`conflicts_with`, `requires`, `required_unless_present`, `ArgGroup` are gone from `cli/`): clap enforces them while it parses, ahead of the domains. They are rows of `arg_rules` (`rules_of(command, path)` lists a command's rules in document order); `arg_rules::relaxed::tree` is the tree the first parse of an invocation runs on (an argument that has a domain, and every integer, is read as text there); `validators::parse` runs that parse, the domain and constraint stage and then the real parse; `role::declare` leaves the unknown `CANON_ROLE` to `main`, which raises it after the other validators. `tests/atip_check_order.rs` builds the pairs of stages from the command documents the way `L1.check-order` does (scenarios `atip.checkorder.01` to `16`).

**`--input` follows the order of spec 11.7** (`canon-0-35-input-order`, scenarios `atip.inputorder.01-10`): read and frame, fragment validation (every problem), the command line's own parse failure, the conflict check on normalised values, then the merged arguments. Nothing changes for a document that is right and does not conflict.

| 0.34 | 0.35 |
|---|---|
| a `#` line of an NDJSON file was skipped as a comment | every non-empty line is a document; a `#` line is `INPUT_INVALID`, `reason` `NOT_JSON`, `field` `--input`. **Remove comment lines from NDJSON files** (blank lines are still skipped) |
| only the first problem of a document was reported, the keys taken in alphabetical order | every problem is reported, in the order the document wrote its keys: the first is the error (`field` its pointer, `/roundd`) and, with two or more, `error.details` holds each as `{code, reason?, message, field, expected?, received?, suggestions?, hint?}`; one problem has no `details` |
| a value outside a closed set in a document was `USAGE`, `field` `/disposition` | `INPUT_INVALID`, `reason` `INVALID_VALUE`, `field` `/disposition`, `expected` the set. The same value on argv is still `USAGE`, `field` `--disposition`: the code follows where the value came from |
| `--round abc --input '{"round":1}'` was `INPUT_CONFLICT`; `--bogus --input '{"seq":"x"}'` and `--round --input '{"seq":"x"}'` (an option with no value) were `USAGE` before the document was read | `USAGE`, `field` `--round`: the command line's own parse failure (an unknown option, a missing or ill-typed value, a value outside its set) answers before the conflict check, as it does without `--input`, **but after the document's own problems** (step 2 comes before step 3): the last two are `INPUT_INVALID`, `field` `/seq`, whatever is wrong on the command line. A missing required argument is not a parse failure: the document may give it |
| a value given on argv and in a document was compared as text, so `--round 1` and `{"round":"1"}` conflicted | compared as the number they denote: `1`, `"1"`, `-0`, `"-0"` and `0`, and `--round +1` against `1`, do not conflict; `--round 1` against `{"round":"2"}` is `INPUT_CONFLICT` with the document pointer. argv still accepts `+1` (the contract does not fix argv's spelling) |
| `{"round":"+1"}` and `{"round":1.0}` reached clap and were accepted or `USAGE` | an integer of a document is a JSON integer or a string of `-?(0|[1-9][0-9]*)` within ±(2^53-1) (`-0` is `0`); `"+1"`, `"0x1"`, `"1e3"`, `" 1"`, `1.0` and `9007199254740992` are `INPUT_INVALID`, `INVALID_VALUE` at the key |
| a number was accepted for a string option (`{"title":5}` was `"5"`) | `INPUT_INVALID`, `INVALID_VALUE`: no coercion happens (spec 11.5). Write `"5"` |
| `{"operands":[]}` and more operands than the command takes were `USAGE` | `INPUT_INVALID`, `INVALID_VALUE`, `field` `/operands` (the fragment schema: `minItems` 1, and `maxItems` when no operand is variadic) |

**Batches follow spec 11.8 and 11.11** (`canon-0-35-batch-pointers`, scenarios `atip.batchpointers.01-07`): preflight reads no state, and the three outcomes of a batch are the contract's table.

| 0.34 | 0.35 |
|---|---|
| a refusal of an element of a batch pointed at `/items/1/round` | `/1/round`: the index, then the key (`/0/operands/0`); no pointer holds `/items/`. A single `--input` is still `/round` |
| `BATCH_INCOMPLETE` had the `field` of the failed element's own error (`/items/1/operands`) | `field` is exactly `/<index>` (`/1`); the element's own pointer is in its `data.items[1].error.field` (`/1/operands`) |
| the 14 commands that have `--dry-run` (`artifacts store`, `export`, `gate install-hooks`, `gate promote`, `ingest plans`, `ingest records`, `ingest sessions`, `init`, `inventory sync`, `plugin sync`, `skills install`, `strategy promote`, `subject adopt`, `subject unadopt`) ran every element's `--dry-run` in preflight, which reads state: an element naming an object that does not exist (or a missing repository) refused the whole batch, exit 2, with the element's code, no `data`, and nothing run | preflight is steps 1 to 6 of spec 11.7 and reads no state, so those elements run: the element fails when it runs, exit 1, `BATCH_INCOMPLETE`, `field` `/<index>`, the elements before it done, it `failed` with its own `error`, the rest `skipped`. **Upgrade note: a batch of one of these commands that preflight used to refuse whole can now stop part-way, with the earlier elements written. A batch is not a transaction: read `data.items` to see how far it went, fix the failed element and run it again (the written ones report `unchanged`).** A fault that reads no state (a type, an enumeration, an unknown key, a domain, a constraint, a validator) still refuses the whole batch in preflight, exit 2 |
| an element that fails a state precondition at index 0 of one of those commands was exit 2 | exit 1, `BATCH_INCOMPLETE`, `field` `/0`, for every command, whether or not it has `--dry-run` |
| a `failed` element could carry the `data` its run printed with its failure | a `failed` element has `error` and no `data`; a `skipped` one has neither |

**`canon skills show` is the contract's guide command** (`canon-0-35-skills-show`; atip/1 1.2 spec 14, 4.8). `NAME` is optional: without it the command prints the default guide, `canon`, whose text names every guide. `--json` answers `data` in the shape of the contract's `guide.schema.json`, which `crates/atip` now vendors (`atip::GUIDE_SCHEMA`, `atip::guide_problems`; digest-pinned): `{name, text, guides, full, x-canon: {version}}`. An unknown name, or one outside the grammar of a name (an upper-case letter, a space), is the same `USAGE` refusal as before with `field` `NAME` and `expected` the guide names. The first line of every `--help` now names both entrances. The output's `meta.schemaVersion` is `2.0` (a member was removed).

| Before (0.34) | Now (0.35) |
|---|---|
| `canon skills show` (no name) → `USAGE` (`NAME` was required) | prints the default guide, `canon` (`name: "canon"` in `--json`) |
| `data.markdown` | `data.text` |
| `data.version` | `data["x-canon"].version` |
| (no member) | `data.guides`: every guide name, in order; `data.full`: whether `--full` material is in `text` |
| `meta.schemaVersion` `1.0`; `canon schema skills show` `schemas.SkillsShowReport` an inline schema | `2.0`; `{"$ref": "https://atip.dev/1/guide.schema.json"}` |
| first line of `--help`: `Agents: "canon schema --json" describes every command as JSON.` | `Agents: "canon schema --json" describes every command; "canon skills show" prints the guide.` |
| `canon schema skills show`: `NAME` required | `NAME` optional, `defaultNote` ``the default guide, `canon` `` |

Every command line in a guide's fenced code blocks now parses (spec 14.5); `tests/atip_skills_show.rs` (`guide_commands_parse`) checks it for every guide by the command documents and by canon's parser, with an `--input` document read element by element, and finds a misspelled option, a dropped required argument, a value outside its domain and an unknown command. Lines with a pipe, a redirection or a `<` that is no `<UPPER_CASE>` placeholder are outside the rule (86 lines of the guides, all unchanged). The lines that were changed:

- the syntax line of every leaf in the generated **Commands** block of `role-implementer`, `role-orchestrator`, `role-planner` and `role-reviewer` (`canon finding add --change <CHANGE_ID> [--round <N>] …`) is now a `usage:` line (`usage: canon finding add …`): it shows optional, repeated and exclusive parts and was never a command to run; the runnable example under it is unchanged (`tests/doc_gen`, the legend above the block says so);
- a trailing shell comment (`canon status   # subjects by status`) is a `# …` line above the command, in `SKILL.src.md` 332-337 and in `canon-artifact-ingest` 12-14, `canon-authoring` 95-99, `canon-context` 16-18, `canon-fmt` 12, `canon-gate` 45, 46, 740, 741, `canon-inventory` 95-97, `canon-plan-import` 67-69, `canon-plugins` 52-54, 73-75, `canon-report-dashboard` 43, 70, 129, `canon-session-ingest` 21-25, 178, `canon-storage` 125-127, `canon-subject` 43, 133, 356, 358-359, 496, 497, 570, 571, 821 (50 lines);
- a sample of canon's own output in a ```` ```stderr ```` or ```` ```output ```` block that began with `canon ` now begins `stderr: canon …` / `output: canon …` (16 lines: `canon-gate` 317, 318, 556, `canon-inventory` 17, `canon-review` 471, `canon-storage` 319, 335, 353, `canon-subject` 185, 186, 262, 319, 320, 380, 437, 438);
- the stage diagram of `canon-inventory` ("Where the index sits in the loop") names each stage before its command (`review:   canon review scenario`).

For contributors: the Commands block is generated by `tests/doc_gen`; `canon skills install` is unchanged (the installed file is still the stub that points at `skills show`).

**canon registers `STOPPED_MIDWAY` and says it where a command stopped after its earlier steps wrote (`canon-0-35-canon-stopped-midway`, `atip.midway.01-12`).** `STOPPED_MIDWAY` is the contract's reserved code (spec 7.2.1, 8.3.2) for a command that works in steps and stopped after an earlier step made its effects, because a later step met what the command refuses before its work. It is exit 1 and not retryable, `reason` is the code of that refusal (never a compound name), `meta.next` leads with a command that reads what was done and the repeat comes after it, and `canon schema --errors --json` and `canon schema --error STOPPED_MIDWAY --json` list the reasons with the fix that says what to do after reading. A batch element that stops this way is `failed` with `STOPPED_MIDWAY` and its reason as its own error and the batch is `BATCH_INCOMPLETE`. The move is a subset, as the contract allows (spec 9.10 applies to each rename): one code moves in 0.35, the others stay as the audit below decides. The rule that decides: a stop is `STOPPED_MIDWAY` only for a refusal that has no code for the work of its own; an I/O failure keeps its `*_FAILED` code (`WRITE_UNAVAILABLE` before the work is `WRITE_FAILED` during it, spec 8.3), and a stop with no earlier effect is still exit 2 with the refusal's code.

| 0.34 | 0.35 | exit |
|---|---|---|
| `INIT_SKILL_INSTALL_FAILED`, the skill source or the `skills:` section refused by `canon skills install` | `STOPPED_MIDWAY`, `reason` `SKILLS_INVALID_ARGUMENT`; `meta.next` is `canon status --json`, then `canon skills install --json` | 1 |
| `INIT_SKILL_INSTALL_FAILED`, a skill directory that cannot be written | `WRITE_FAILED` (it is the I/O category's code for the work) | 1 |
| `INIT_SKILL_INSTALL_FAILED`, any other install failure | `SKILLS_FAILED` (the code `canon skills install` gives it) | 1 |
| `canon init --dry-run` with the same causes (nothing was written) | the refusal's own code, `SKILLS_INVALID_ARGUMENT` or `WRITE_UNAVAILABLE` (2), or `SKILLS_FAILED` (1) | 2 / 1 |

A script that matched `INIT_SKILL_INSTALL_FAILED` matches `STOPPED_MIDWAY` with `reason` `SKILLS_INVALID_ARGUMENT` now (and `WRITE_FAILED`, `SKILLS_FAILED` for the other two); run `meta.next[0]` before anything is repeated, because the repeat of `canon init` would find canon.yaml already there.

**The audit of the exit-1 codes (`atip.midway.09-10`).** Every exit-1 code of the 0.34 registry was judged. *Moved*: `INIT_SKILL_INSTALL_FAILED`. *Kept*, each for one of these reasons:

| Why it stays | Codes |
|---|---|
| a real I/O failure of the work, which keeps its own name (spec 8.3) | `ARTIFACTS_STORE_FAILED`, `BRIEF_WRITE_FAILED`, `CHANGE_ADOPTION_INCOMPLETE`, `CONTEXT_OBJECT_CONFLICT`, `DASHBOARD_FAILED`, `EVIDENCE_STORE_FAILED`, `HOOKS_INSTALL_FAILED`, `INGEST_FAILED`, `INVENTORY_FAILED`, `LEARN_DEMOTION_FAILED`, `LEARN_PROMOTION_FAILED`, `LEARN_ROLLBACK_FAILED`, `LEDGER_COMPACT_FAILED`, `MIGRATE_INDEX_FAILED`, `MIGRATE_PINS_FAILED`, `MIGRATE_STOPPED`, `PLUGIN_SYNC_ERRORS`, `POLICY_APPROVAL_FAILED`, `PROMOTE_FAILED`, `QUERY_FAILED`, `REPORT_FAILED`, `RETENTION_EXPORT_FAILED`, `RETENTION_PURGE_FAILED`, `SEARCH_INDEX_FAILED`, `SKILLS_FAILED`, `TIER_FAILED`, `WRITE_FAILED` |
| the command ran and its answer is negative or incomplete (a finding, a verdict, a failed check) | `ADAPTER_EXECUTION_NOT_AUTHORIZED`, `ADAPTER_RESPONSE_INVALID`, `ARTIFACTS_VERIFY_FAILED`, `BRIEF_CHANGED_SINCE_APPROVAL`, `BRIEF_HAS_BLOCKERS`, `CONFIG_CHECK_FAILED`, `CONTEXT_PACK_TAMPERED`, `DIVERGENCE_REFUSED`, `EVIDENCE_CONTRACT_MISMATCH`, `EVIDENCE_FABRICATED`, `EVIDENCE_REPORT_CONTRADICTION`, `FORMAT_VIOLATIONS`, `GATE_TASK_BLOCKED`, `GATE_TASK_EVIDENCE_INVALID`, `GATE_VIOLATIONS`, `INGEST_SOURCE_UNUSABLE`, `INVENTORY_ABORTED`, `LEARN_EVALUATION_REJECTED`, `LEARN_PROMOTION_BLOCKED`, `POLICY_APPROVAL_UNREADABLE`, `POLICY_CHANGED_SINCE_APPROVAL`, `PROMOTE_REFUSED`, `REPORT_STALE`, `SELFTEST_FAILED`, `SKILLS_DRIFT`, `SUBJECT_GATE_VIOLATION`, `UNVERIFIABLE` |
| no earlier effect stays: the refusal is the first thing that happens, or what was written was rolled back | `BRIEF_UNAPPROVED`, `CONTEXT_GIT_UNAVAILABLE`, `DASHBOARD_NOT_BUILT`, `DASHBOARD_PORT_IN_USE`, `DISPATCH_END_IN_PROGRESS`, `EVIDENCE_BATCH_REFUSED` |
| a bug of canon | `INTERNAL`, `SERIALIZE_FAILED` |
| the batch's own code | `BATCH_INCOMPLETE` |

The candidates the contract author named, judged one by one: `MIGRATE_STOPPED` stays (every site is a write, move, remove or re-import that failed after `migrate` began; the one re-check of a destination at apply time repeats a plan-time refusal that only a concurrent writer can make true), `CHANGE_ADOPTION_INCOMPLETE` stays (the only cause is the second record's store write failing, the I/O category), `EVIDENCE_BATCH_REFUSED` stays (the refusal rolls back every record and blob the run wrote, so no earlier effect stays), `DISPATCH_END_IN_PROGRESS` stays (the end lock is taken before any effect), `PROMOTE_FAILED` stays (the staged record was written, then the ledger write failed). The rest of the move is canon's to make in a later release (spec 9.10).

### Added

- **`error.details`** (spec 7.1.1): `atip::CliError::details(errors)` lists every error of one input and `ErrorBody.details` carries them, as `ErrorDetailBody` items (`code`, `reason?`, `message`, `field?`, `expected?`, `received?`, `suggestions?`, `hint?`; no `retryable`, no nested `details`), when there are two or more; the first is also the error itself. In text mode each is a `problem: <field>: <message>` line. canon uses it for the problems of an `--input` document (`canon-0-35-input-order`).
- **`atip check --json` lists every finding of a check (`data.checks[].findings[]`).** A check that is `fail` or `unprovided` has `findings`: one entry for each thing it found, never cut and never capped. An entry is `{kind, message, argv?, code?}`: `kind` is the name of the condition the check judged (`version-is-claim`, `paged-next-cursor`, `returned-at-most-limit`, ...: the public spelling of the report, one per condition, the same names the checker's tests plant; a rename is listed here with the old and the new name), `message` is the sentence `reason` is made of, `argv` is the call that decided it (the same argv `observed` shows; absent for a finding that rests on a whole document, such as the registry rows), and `code` is an error code a refused fixture call carries. A check that lacks a fixture entry has one finding of kind `missing-fixture-entry` for each, and the `message` is the entry (`commands`, `probe`, `registry`, `warnings`, `globalOptions`, `commands.<id>.paged`, `commands.<id>.domainRejects`, ...). Two kinds are not conditions: `ran-over-time-limit` (an invocation the check made for itself ran over the time limit) and `missing-fixture-entry`. `pass`, `unsupported` and `entailed` checks have no `findings`; the SHOULD checks' `advisories` stay where they were. `reason` is still the first three messages, each cut at 240 characters, and `and N more`; the text report is the text it was, and a line whose reason is cut ends with `(all in --json findings)`. A condition that failed for several things at once (`reserved-codes`, `entries-valid`, `error-answers`, `required-codes`, `commands-registered`, the `schema-doc` validation of `L1.tree` and `L1.walk`) now has one finding for each thing, so its `reason` joins them with `; ` where `reserved-codes` joined them with `, ` and a list inside one message was cut at three. Results, levels and exit codes do not change.
- The `atip` crate knows the minor 1.2: `atip::Minor` (`V1_0`, `V1_1`, `V1_2`; `parse`, `as_str`, `at_least`) and `atip::CONTRACT_VERSION`, now `"1.2"`, the newest minor the crate implements. **It is no tool's declaration.** canon declares `json_out::DECLARED_MINOR` (`"1.1"`) and `atip check` `output::DECLARED_MINOR` (`"1.1"`, `"1.2"` since `canon-0-35-check-1-2`), so `meta.contractVersion` of canon is the same bytes as in 0.34 (`canon-0-35-crate-1-2`, `atip.crate12.01-03`).
- `atip::RESERVED_CODES`: the thirteen reserved codes (spec 7.2.1) with exit, retryability and the minor that introduced them, `STOPPED_MIDWAY` (exit 1, not retryable, 1.2) last; `atip::reserved_code(name, minor)` and `atip::registration_problem(minor, code, exit, retryable)`, the check that a registry row keeps the reserved exit and retryability, which `atip check` (`L1.registry`) and a tool's own tests share. In a 1.1 or 1.0 document `STOPPED_MIDWAY` is the tool's own code, so it is not judged there (spec 8.3.2). The checker's copy of the table is gone (`atip.crate12.04-05`).
- `atip::flag_only_options(minor)`: `json`, `input`, `commit` and `plan-digest`, and since 1.2 `limit`, `cursor`, `all` and `fields`; a tool adds its own (canon's `repo` and `as`) before it passes the set to `project::option_doc`. `atip::flag_only_problems(options, minor)` names an option of the set that a command document does not mark `flagOnly` (`atip.crate12.06-07`).
- `atip::ReasonInfo::from_code`, `atip::reasons_with_exit` and `atip::with_row`, `const fn`s that build a row's `reasons` from the other rows of the table, for the `STOPPED_MIDWAY` row whose reasons are the tool's exit-2 codes (spec 8.3.2): the order of the table does not matter, and a code the table does not hold stops the evaluation (`atip::reason_of_code`), so it does not compile (`atip.crate12.08-09`).
- **`atip check` takes a claim of atip/1 1.2, and `L1` runs the 1.2 rows for it.** `--contract` and the fixture's `contractVersion` take `1.0`, `1.1` and `1.2` (`USAGE` and `FIXTURE_INVALID` list the three; two claims that differ are still `USAGE`); everything that was true of a 1.1 claim (`L0.hint-form`, `L1.hint-command`, the `contract` and `contractVersion` markers) is true of a 1.2 one, and `data.contractVersion` is the claim. The checker's own envelope and `atip.fixture.json` say `1.2` and it passes its own L0 checks as a 1.2 claim. A 1.2 claim runs **`L1.check-order`** (MUST; new, after `L1.validators`; spec 11.7 step 6): for each runnable command the checker changes one passing invocation into a pair of faults (two unknown options, an unknown option and a value of the wrong type, an unknown option and a missing required argument, a missing required argument and a broken domain or constraint, two broken domains, a broken domain and a broken constraint, two constraints with different codes, a constraint and a validator with different codes), runs the pair and the earlier fault alone, and finds that the answer of the pair must have the code and the `field` of the earlier stage (kind `earlier-stage-answers`; a pair that needs a third fault is not used; `unsupported` when no command has a pair, `unprovided` with `commands.<id>.ok` when none has an invocation to change). It adds the 1.2 half of four rows: `L1.grammar` (`limit`, `cursor`, `all` and `fields` have `flagOnly: true`; kind `reserved-shape`), `L1.registry` (`STOPPED_MIDWAY` has exit 1 and is not retryable; kind `reserved-exit`; below 1.2 the name is the tool's own code and any exit is allowed), `L1.paging` (the `next` entry of a `paged` call: `--limit 1` replaced by `--all` is not refused with `CURSOR_INVALID` (`cursor-survives-all`); an option written out at its declared default that is not `null`, and a variable of an option the call did not give set to another valid value, are (`cursor-bound-to-given-arguments`, `cursor-bound-to-environment`); a command with `input` is paged through `--input -` (`paged-through-input`) and its `next` carries no `--input`, no default and no variable it did not give, and goes on from that page (`next-after-input`); an `--all`, default or `env` the command does not have is `unsupported` for that part and is told in `notes`) and `L1.fields` (the first selection with no `--json` is not refused (`fields-without-json`), and an unknown path ends exit 2 with the error on stderr with no `--json` either (`unknown-path-refused-in-text`); the text is not compared). **A 1.1 or 1.0 claim has none of these**: the report has no `L1.check-order` and the same results as before. `exec::Call::with_stdin` gives a call a document on stdin.
- The contract examples and schema copies the checker's tests read are the tag `v1.2.0`: `schema-doc.schema.json` of `crates/atip` (only the description of `flagOnly` changed; its digest pin moves), the new examples `tally` (1.1) and `acme` (1.2) (`schema-errors-tally.json`, `schema-command-tally-entry-list.json`, `report-failure-acme-deploy-start-stopped-midway.json`, and the invalid `schema-command-acme-member-list-limit-not-flag-only.json`, `schema-errors-acme-stopped-midway-exit-2.json`, `stream-acme-build-start-contract-version-1-0.ndjson`) with `TABLE.md`. A case of `L1.output-data` that marks an optional field required in the output schema is pinned: it is caught (`data-validates`), and `L1.fields` fails with it (`atip.check.99-113`).
- **`atip check` fixture key `notRun` leaves a command out of the run, by name.** `notRun` is an array of command ids (`"notRun": ["dashboard"]`), beside `commands`, for a command that does not end when it is run (a server) or that must not run here. The checker runs it by no invocation: not as `ok`, `paged`, `fields` or an example, not as a `meta.next` it follows, and not as a constraint, domain, number or validator probe; `--help` and the schema of it are still read, and the example syntax check (`L1.examples`) still judges its examples. The report names it: each check that runs commands has `notRun` (`["dashboard"]`) in `--json` and `(not run: dashboard)` on its text line, and it never changes a result. A `notRun` id that `commands` lists too is `FIXTURE_INVALID` (exit 2, `--fixture`), and so is, from L1, an id that is no command of the tool's `schema --json` tree; at L0 there is no list of commands, so an unknown id is not an error there. canon's own fixture lists `dashboard`, and canon passes L1 with it.
- `atip::cursor` (`canon-0-35-crate-cursor`): the paging cursor's binding, encoding, decoding and next-page argv, one implementation for every tool. `Cursors::new(minor)` (or `for_declared("1.2")`, which refuses a minor the crate does not know) binds an invocation's arguments by the minor the tool declares: 1.1 (and 1.0) binds every explicit argument except `--limit`, `--cursor` and `--fields`, `--all` and `--json` included, and has no environment source; 1.2 leaves out `--json`, `--limit`, `--all`, `--cursor` and `--fields`, counts an argument given with its default value as given, and binds a value an option's `env` variable supplied, marked as environment. `--input` is in neither, as its arguments are already in the list. A `false` switch, a `0` count and an empty repeated option are not given (spec 10.7.2). The binding is the SHA-256, 64 lower-case hex digits, of the RFC 8785 text of the arguments (in the minor's own document, so a cursor of 1.1 is never accepted by 1.2), so the same arguments bind the same in every tool whatever their order or spelling. `encode` writes a cursor as one ASCII string with no whitespace or quote (scope, binding, position, a 16-digit check) and `decode` returns the position or a `CursorInvalid`: `FOREIGN` for any text that is not a cursor of the crate (one changed character included) or was issued by another command, `ARGUMENTS_CHANGED` for other arguments, and `CursorInvalid::expired()` for the tool to say when it cannot resume the position. `CliError::from(CursorInvalid)` is `CURSOR_INVALID` (exit 2, not retryable) with `field` `--cursor`, the `reason` and a hint; `CURSOR_INVALID_REASONS` are the three rows a tool registers. `next_page_argv(path, given, cursor, json_mode)` writes the call that continues a page: the explicit arguments, `--cursor=<cursor>` in place of any earlier one, `--json` in JSON mode, and the operands after one `--`; a default, an environment value and `--input` are not written, so the same call in the same environment binds the same (`atip.cratecursor.01-11`). The text of a cursor is the crate's to change between releases; canon's own cursors move to it in `canon-0-35-canon-cursor-1-2`, so canon prints the same cursors as in 0.34 until then.
- `atip::given::{Given, write_argv}`: an invocation's normalised arguments (name, kind, value, source, the tool's own flag-only mark) and their argv in the forms of spec 9.12.3 and 11.9: `--name=value` for every valued option (so no value is read as an option, a stdin selector or an inline-JSON or file selector), a `true` switch as `--name`, a count's name repeated, a `json` value as its compact text in one word and the operands after one `--`. The plan digest of `canon-0-35-crate-plan` writes its argv with it too.
- `atip::jcs::to_string`: RFC 8785 canonical JSON for the values a tool digests: members sorted by UTF-16 code units, the escapes of the RFC, no whitespace, `null` and empty members kept. It writes a number only when it is a whole number within ±(2^53-1) (`1` and `1.0` are `1`); any other number is `jcs::Error::NumberNotSafe` instead of a digest another implementation would compute differently (`atip.cratecursor.12`). `sha2` is now a dependency of `atip`.
- **`atip check` L1.paging (a claim of 1.2) writes out the default of a repeatable option one occurrence for each element, and says when it cannot read a `next` entry.** A repeatable option declares an array default (`["a", "b"]`); the check gave it as one value, the JSON text of the array, which an enum or integer option refuses with `USAGE`, so a conforming tool failed `cursor-bound-to-given-arguments`. It now writes `--kind=a --kind=b`, and an empty array writes nothing out. A `next` entry that does not parse in its own command's document leaves the 1.2 half of the row unsupported for that command and `notes` say so, where it was skipped without a word (review of `canon-0-35-check-1-2`; `atip.check.103`).
- `L1.check-order` (a claim of 1.2) says in `notes` each pair it judged, `<command>: <pair> (`<argv of the pair>` against `<argv of the earlier fault alone>`)`, and observes the run of the earlier fault alone, so a passing report shows the answers its verdict rests on (`atip.check.110`). The fake tool of the tests has a seeded violation for every order the row judges (`unknownOptionDeferred`, `rulesReversed` besides the five of 0.35).
- `atip::plan` (`canon-0-35-crate-plan`): the plan of a destructive command, its digest and the commit gate, one implementation for every tool (spec 12.2-12.8, 8.3.2). `Plan { effects, inputs }` holds the `Effect`s (`action`, `path?`, `record?`, `detail?`, and members of the tool's own) in the order the commit makes them and the `inputs` that determine them: the normalised arguments and a digest of each state and file content the plan read. The crate does not look inside `inputs`. `Plan::digest()` is `sha256:` and the lower-case hex SHA-256 of the RFC 8785 text of `{"effects": [...], "inputs": {...}}` (so the same plan has the same digest in every tool, keys in any order are one plan, any other change is another digest, and a number outside ±(2^53-1) or a fraction is an error), as spec 12.6 advises; **it is not the digest canon prints in 0.34**, which hashes the effects alone, with serde's field order and no prefix: canon moves to this one in `canon-0-35-plan-jcs`, and until then canon's output is unchanged. `Preview` (`planDigest`, `effects`, `committed`) is the preview's `data` and `preview_schema()` its JSON Schema, for a tool's report schema to reference. `commit_next` writes the step a preview's `meta.next` leads with: the normalised arguments as `--name=value` options (no `--input -`, no default, no environment value), then `--commit`, `--plan-digest=<digest>` and `--json`, the operands after one `--`; its `command` splits back to the tool name and `argv`. `preview_next` is the same call without the commit options. `mode(commit, plan_digest)` refuses one option without the other (`Unpaired`, whose `error` is `USAGE` or the tool's finer code, with `field` the missing option; `COMMIT_REQUIRES` are the two `requires` rules a command document lists for them) and `gate(mode, current, preview)` is `Ok(true)` for a commit of the plan whose digest is `current`, `Ok(false)` for a preview, and `Stale` (`PLAN_STALE`, exit 2, `field` `--plan-digest`, `next` the preview) for any other digest, one character off or written without the prefix included. A digest another system computed goes through `gate` as it is. `stopped_by_stale` is the `STOPPED_MIDWAY` (exit 1, not retryable) a commit gives when it meets a changed plan after some of its effects, with the reason `PLAN_STALE` and a `next` that leads with the command that reads what was done. The crate has no code that makes an effect (`atip.crateplan.01-11`).

### Changed

- **`effectNote` of the writers that refresh the index, of `subject move` and of `dashboard`** (`canon-0-35-read-writes`). `divergence accept`, `evidence add`, `evidence audit`, `artifacts store`, `review change` and `review scenario`: "refreshes stale scenario-index records first" is "refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing". `subject move`: "reads without --to: `subject move <id>` shows the subject and writes nothing; with --to it refreshes stale scenario-index records first" (a form with no `--to` has never existed: `--to` is required) is "refreshes stale scenario-index records once the transition is allowed; a refused call writes nothing". `subject show`: "refreshes the stale scenario-index cache once the subject is found; a refused call writes nothing". `dashboard`: "prints one report, then serves until signalled; regenerates .canon/dashboard-snapshot; POST /api/briefs/approve rewrites a brief" (spec 3.6: a server, not a stream; the fixture's `notRun: ["dashboard"]` keeps `atip check` from running it).
- `atip::fields::Rules` is built by `Rules::at(Minor)` or `Rules::for_minor("1.2")` (which refuses a minor the crate does not know), not by its field `v11`, which is private. 1.1 and 1.2 have the same rules (`atip.crate12.10-11`).
- **`atip check` reads a fixture invocation the tool refuses by what refused it.** An `ok`, `paged` or `fields` invocation the fixture lists to succeed and the tool refuses with exit 2 is one of two things. A refusal before any state is read (spec 11.7 steps 1 to 6) is the fixture's or the tool's fault and fails `L0.one-document`, the check that judges every `ok` invocation, as a finding of kind `ok-is-not-refused`: `USAGE` and the finer codes the tool registered for it, `UNKNOWN_COMMAND`, `COMMAND_REQUIRED`, every `INPUT_*` code, and, from L1, a code the command's document declares on a constraint or a validator (whatever the fixture's `env`) and a code `L1.domains` saw refuse an argument. Any other exit 2 code is a precondition (a credential, a backend that is not there): every check that runs the invocation is `unprovided` with a finding of kind `refused-precondition` that carries the `argv` and the `code`, and `reason` names the code, so the level is not met and the report says what to fix, where it said nothing about the checks that never got an answer (`L0.one-document`, `L0.page`, and from L1 `L1.paging`, `L1.output-data`, `L1.fields`, `L1.constraints`, `L1.domains`, `L1.numbers`, `L1.validators`). A violation the check already saw before the refusal stays a `fail`; the `CURSOR_INVALID` that `L1.paging` provokes and the `USAGE` that `L1.fields` provokes for a path nothing declares are those checks' own and are judged by them. At L0, with no command documents, only the reserved codes above count as before-state; a code the registry lacks, or the fixture has no registry for, is a precondition.
- **`L1.defaults` reads the help line an option opens, not any line that names it.** The block of `--limit` is the line that opens, behind blanks, with `--limit` or with short options before it (`-l, --limit <N>`) and goes on until the next option line or a blank line. A name in a description (`--limit` in the text of `--json`), in a usage line, in a bullet or in a sentence opens no block, so the default of `--limit` is never compared on the `--json` line and a default shown beside a mention is not another option's. A failure of `L1.defaults` reads the same default as before for every help text that did not name one option in another's description.
- **`atip` links `jsonschema` only with the new feature `validate` (off by default; `canon-0-35-crate-validate`).** `cargo tree -p atip -e normal` lists 35 crates instead of 103: 68 fewer, `jsonschema` and what only it needs; the probe tool below lists 36, `atip-check` 119. The JSON Schema validators (`atip::envelope_problems`, `error_problems`, `registry_entry_problems`, `warning_entry_problems`, `schema_doc_problems`, `schema::contract_registry`, `fields::projected_problems`, `domain::compile` and `domain::Compiled`) need it and are unchanged with it on; the vendored schemas and every constant stay without it. `atip-check`, `canon-cli` and the crate's own tests turn it on, so canon and `atip check` print the same bytes as in 0.34. **canon itself still links `jsonschema`**: `canon-fmt` (the validation of the records, used by `canon-gate`, `canon-learn` and `canon-vocab`) depends on it, so `cargo tree -p canon-cli -e normal` does not shrink (339 crates, `jsonschema` among them). `crates/atip-probe` (not published) is the smallest tool built on `atip` with its default features, and its test reads `cargo tree` to hold the claim: no `jsonschema` for it, and one for `atip-check` and `canon-cli`.
- `atip::domain::admits` judges a domain without a validator: every keyword of the profile (spec 10.7.4: `type`, `enum`, `const`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`, `minLength`, `maxLength`, `pattern` with the `regex` crate, `properties`, `required`, `additionalProperties`, `items`; the annotations are ignored) is read by the crate, and the answer is the one `domain::compile` gives, which a differential test holds for every domain canon declares (about 40, with values inside and outside their bounds) and for a battery of schemas of the profile. A keyword of JSON Schema outside the profile (`anyOf`, `$ref`, `minItems`, ...) is judged by the validator when `validate` is on, and is an error that names the keyword when it is off; no canon domain uses one. A `multipleOf` of a decimal reads the numbers as the decimals they were written as, as the validator does.
- A batch element's error keeps its `reason` in `data.items[].error` when the registry holds that reason under the element's code (it was dropped before): an element that stops midway reads `STOPPED_MIDWAY` with the refusal's code as its reason, and the batch is `BATCH_INCOMPLETE` as before (`atip.midway.08`).

### Fixed

- **A refused call writes nothing, whatever the scenario index** (`canon-0-35-read-writes`; spec 8.2, 12.2). `subject show`, `subject move`, `evidence add`, `evidence audit`, `review scenario`, `review change`, `divergence accept` and `artifacts store` rebuilt a stale scenario index before they looked at the call, so a refusal that came after it (`SUBJECT_NOT_FOUND`, `SUBJECT_INVALID_TRANSITION`, `EVIDENCE_BIND_REFUSED`, `REVIEW_SCENARIO_NOT_INDEXED`, `DIVERGENCE_INVALID_ARGUMENT`, `WRITE_UNAVAILABLE`, …) left a rewritten `.canon/ledger/kind=scenario/` behind. Now they read the index the refresh would leave **in memory** and write it once the call is past its last refusal, with the record the call writes; a refused call leaves the repository, `HOME` and the temporary directory byte for byte as they were. `artifacts store --dry-run` and a repeat of an identical `evidence add` or `review` (`unchanged`) still write no index. Nothing changes for an accepted call, except that its refresh line comes after the checks and not before them.
- **`subject show` declares what it writes.** It stays a `read` command and refreshes the stale scenario-index cache once the subject is found (spec 12.1); its `effectNote` says so. In JSON mode nothing is written to stderr (the refresh is a `SCENARIO_INDEX_REFRESHED` entry of `meta.warnings`); the text mode keeps its progress line.

- `atip::split_argv` reads a backslash outside quotes the way a POSIX shell does, so it reads back what `atip::quote` writes: `split_argv(join(words.map(quote)))` is `words` for every list of words. Before, `quote("it's")` (`'it'\''s'`) split back to `it\s`, and the checker's `L0.next` reported a correct tool whose next step held a word with a single quote. A backslash makes the next character literal, a backslash and a newline join the line, and a backslash at the end of the input stays one backslash; inside single quotes a backslash is a character, and inside double quotes only `\"` and `\\` are escapes, as before. `quote`'s output does not change, so canon's `command` strings are the same bytes (`canon-0-35-crate-shell`, `atip.crateshell.01-06`).

## 0.34.0 — 2026-11-02

canon adopts atip/1 1.1.2 and meets its L1: `canon schema` is the contract's self-description, every argument declares its constraints, domain and validators, paging cursors and `--fields` follow the contract, and `atip check` runs the 28 L0 and 31 L1 checks.

### Breaking

canon adopts atip/1 **1.1.2** (https://github.com/journeyWorker/atip: spec 6.6, 7.1 to 7.6, 9.13, 10.1 to 10.13, 15.6, 15.7; `meta.contractVersion` stays `1.1`). The vendored contract schemas in `crates/atip/schema/` are the v1.1.0 ones, except `envelope.schema.json` (v1.1.1, which makes the schema itself refuse a written-out `"1.0"`), pinned by digest; `atip::envelope_problems` no longer checks `"1.0"` beside the schema. The 0.33 names below are deleted, with no aliases; every consumer in this repository (the guides the binary carries, the website in English and Korean, the dashboard, the npm launcher, the dogfood trace check, the conformance corpus, the command spec and the scenario sentences) moved to the new names in the same change. A caller written for 1.0 reads `meta.contractVersion` first; a caller that ignores unknown members (the contract requires it, 15.6) keeps working.

**The envelope and `schema`.**

| 0.33 | 0.34 |
|---|---|
| `meta.contract` alone | `meta.contract` (`atip/1`) and `meta.contractVersion` (`1.1`) on every envelope, success and failure alike; the value is never `1.0` |
| `canon schema --errors` and `--error <CODE>` answers without `data.kind` | `data.kind` is `errors` or `error`; the answer's `meta.schemaVersion` is `1.1` (so is every `canon schema` answer's: the tree, group and command shapes are in the next table) |
| no warning registry | `canon schema --warnings --json` (`data.kind` `warnings`) and `canon schema --warning <CODE> --json` (`data.kind` `warning`): rows are `{code, meaning, fix}`, with no exit and no retryability. `canon schema <path>`, `--errors`, `--error`, `--warnings` and `--warning` exclude one another (`USAGE`) |
| a warning-only code is an error registry row with `exit: 1` | it left the error registry: `canon schema --errors` no longer lists it and `canon schema --error <CODE>` on it is `USAGE` (`field` `--error`, `hint` `canon schema --warning <CODE> --json`). **A 1.0 caller that looked a warning code up with `--error` no longer finds it there: read `--warnings` first.** The codes that moved are listed below |
| `UNKNOWN_ERROR_CODE` (`canon schema --error NOPE`) | `USAGE`, `field` `--error`, `received` the code, `hint` `canon schema --errors --json` (the same for an unknown `--warning` code: `field` `--warning`) |

The warning-only codes that moved to the warning registry (45): `BRIEF_ADOPTION_UNAPPROVED`, `BRIEF_DRAFT_PENDING`, `BRIEF_OVERRIDE_ON_RECORD`, `BRIEF_UNAPPROVED_NOT_ENFORCED`, `DASHBOARD_SNAPSHOT_UNAVAILABLE`, `DISPATCH_DIFF_RECORDS_UNAVAILABLE`, `DISPATCH_RUN_COPIES_DIVERGE`, `DISPATCH_TIER_NOT_PERSISTED`, `LEDGER_KIND_ROUTED_AWAY`, `LEDGER_NO_SCENARIOS`, `LEDGER_RECORDS_UNREADABLE`, `LEDGER_ROLLBACK_NOT_RESTORED`, `LEDGER_UNREADABLE`, `LEGACY_GUIDE_STALE`, `LEGACY_STATE_FOUND`, `OLD_BRIEF_FORMAT_FOUND`, `OLD_LAYOUT_FOUND`, `OVERRIDE_REASON_UNUSED`, `OVERRIDE_RECORDED`, `OVERRIDE_REFUSAL_WAIVED`, `POLICY_MISSING`, `POLICY_NOT_APPROVED`, `POLICY_SPEC_COVERAGE_ABSENT`, `POLICY_SPEC_COVERAGE_INVALID`, `POLICY_UNREADABLE`, `QUERY_PLUGIN_DIAGNOSTIC`, `REFERENCES_UNCHECKED`, `REPORT_TIER_NOT_READ_DIRECTLY`, `RETRIEVE_REGIME_FELL_BACK`, `REVIEW_RECORDS_UNMIGRATED`, `ROLE_UNKNOWN_RECORDED`, `SCENARIO_INDEX_CHECK_FAILED`, `SCENARIO_INDEX_NOT_REFRESHED`, `SCENARIO_INDEX_REFRESHED`, `SCENARIO_SUBJECT_UNKNOWN`, `SEARCH_GITIGNORE_MISSING`, `SEARCH_INDEX_IN_MEMORY`, `SEARCH_INDEX_REBUILT`, `STRATEGY_DEMOTED`, `SUBJECT_OVERRIDE_ON_RECORD`, `SUBJECT_REOPENED`, `TASK_PROOF_IS_CANON_COMMAND`, `TASK_REIMPORT_INCOMPLETE`, `TASK_STATUS_NOT_REFRESHED`, `UNKNOWN_ROLE_IGNORED`.

These codes are in both registries, with the same meaning, because the same situation is an error in some invocations and a warning in others (spec 7.5.3): `CONFIG_MISSING`, `CONFIG_UNREADABLE`, `DISPATCH_MANIFEST_MISMATCH`, `DISPATCH_MANIFEST_UNREADABLE`, `LAYOUT_MIGRATION_REQUIRED`, `POLICY_APPROVAL_UNREADABLE`, `POLICY_CHANGED_SINCE_APPROVAL`.

**Reasons.** `error.reason` is a member of the error (spec 7.1, 7.6) and the registry lists the reasons of a code in `reasons: [{reason, meaning, fix}]` (`canon schema --errors`, `--error INPUT_INVALID`). A reason never changes the code's exit or `retryable`; an error without one is still complete.

| 0.33 | 0.34 |
|---|---|
| `error.x-canon.reason` `unreadable` (`INPUT_INVALID`, `field` `--input`) | `error.reason` `UNREADABLE` |
| `x-canon.reason` `tty` | `STDIN_TTY` |
| `x-canon.reason` `empty` | `EMPTY` |
| `x-canon.reason` `syntax` | `NOT_JSON` (the source is not JSON), or `NOT_OBJECT` (it is JSON, but the document or an item of a batch is not an object) |
| `x-canon.reason` `batch-unsupported` | `BATCH_UNSUPPORTED` |
| a fault in the document (pointer `field`) had no reason | `UNKNOWN_KEY` (a key that is no option of the command), `FLAG_ONLY_KEY` (a key that names a flag-only option), `INVALID_VALUE` (a value of the wrong type, outside its closed set, or `null`) |
| `INPUT_TOO_LARGE` had none | `TOO_MANY_BYTES` (the source is over 8 MiB), `TOO_MANY_ITEMS` (a batch is over 10000 items) |

**Wrong calls.**

| Call | 0.33 | 0.34 |
|---|---|---|
| an empty value (`canon finding add --change ""`) | `USAGE` without `received` | `USAGE` with `received: ""`; a value that was never sent still has no `received` |
| `canon finding --repo . --json`, `canon finding --as planner --json` (a tool-wide option after a group word) | `USAGE` | `COMMAND_REQUIRED`, `expected` the commands of the group; `--repo` and `--as` do nothing on a group and are not an error there (`canon finding --repo . list …` runs `list` with `--repo .`). `canon finding --bogus` and `canon finding --repo --json` (no value) are still `USAGE` |
| a `USAGE` that cannot name its argument | any hint | no `field`, and `hint` is `canon schema <command> --json` (`canon schema --json` when no command resolved) |
| `canon retrieve` with no scope | hint prose that started with `canon retrieve …` and went on in a sentence | hint `canon retrieve --for-role <ROLE> --domain <DOMAIN>` |
| `canon subject move <S> --to verifying` without `--reason` | hint with a lowercase placeholder | hint `canon subject move <S> --to verifying --reason <REASON> --as <ROLE>` |

A hint is now exactly one of a command line, a template with `<UPPER_CASE>` placeholders, or one sentence of prose; a hint that starts with `canon ` holds that one command line and nothing else (spec 7.1.2). A test reads every hint in the sources and in the conformance corpus and holds it to that.

**Consumers.** The dashboard's briefs page, the npm launcher and `evals/dogfood/trace_check.py` read only the members they know, skip an enum string they do not know (an approval `outcome`, a finding `severity`) instead of rejecting the document, and have tests that feed them a document with members and values from a later minor (contract 15.6). The guides teach the same.

**`canon schema` is the L1 self-description.** The answers are the contract's documents (spec 10): a `tree`, a `group` and one `command` document per command, with the names of atip/1 and nothing else. The names below are deleted with no alias; `docs/cli/canon.cli.json` (the spec file), the skills, the website in English and Korean, `canon context`, the dogfood trace check, the design notes and the conformance corpus all read the new names, and a test fails when one of them reads an old name.

| 0.33 | 0.34 |
|---|---|
| `canon schema --json`: `data` is `{commands, role?}`, a flat list of the 78 commands, each `{command, effect, about, roles, ...}` | `data.kind` is `tree`, `data.level` is `L1`, and `data.commands` lists the 27 groups and the 78 commands once each, a group before its commands. An entry is `{command, path, kind, summary}`, and a command also has `effect` and `x-canon.roles`. The top-level `role` is gone: `--as <ROLE>` still keeps only that role's commands and the groups that hold one, and says so in `data.x-canon.as` (`{"as": "reviewer"}`; an unfiltered tree has no `x-canon`), and `CANON_ROLE` still never filters |
| `about` | `summary` (a group has one too) |
| `roles` (a member of a command) | `x-canon.roles` (an extension member, spec 10.12; empty for the three human-only acts) |
| `canon schema finding` (a group word) was an unknown path or an empty answer | a `group` document: `{kind: "group", command, path, summary, commands}` with the direct children |
| `canon schema <COMMAND> --json`: `{command, effect, nodeKind, supportsJson, about, aliases, arguments, options, requiredOneOf, exits, errorCodes, outputs, inputs, examples, roles}` | `{kind: "command", command, path, summary, effect, idempotency?, effectNote?, operands, options, errors, outputs, input?, examples, schemas, x-canon}` |
| `nodeKind` (`leaf` or `group`) | `kind` (`command` or `group`) |
| `supportsJson` | none: every command takes `--json`, and its option says `flagOnly: true` |
| `aliases` of a command | none (canon has no command alias) |
| `arguments` | `operands`, each `{name, valueName, valueType, required, variadic, default, help}`; the name is the bare noun (`change`, not `change_ids`) |
| an option's `kind` (`value` or `flag`) | `valueType`: `switch`, `count`, `string`, `integer`, `number`, `path`, `enum` or `json`, read from the clap action and value parser. A parser canon wrote itself is listed with its type in `CUSTOM_PARSERS`; an argument with one that is not listed fails the generation |
| an option's `default` was an array of strings, `[]` for none, `["."]` for `init --repo`; `--limit` printed `[]` although its help said 100 | the typed default: `100`, `25`, `"."`, `false` for a switch, `0` for a count, an array for a repeatable option, `null` for none or for a required option. A default computed at run time is `null` with `defaultNote` (`--repo`: the nearest ancestor directory holding a canon.yaml). Every `[default: X]` of `--help` is the document's default, and the other way round |
| an option's `aliases`, `conflictsWith`, `requires`, and an operand's `arity` | none: the relations are `constraints` of `canon-0-34-atip-constraints` |
| `requiredOneOf` of a command | none in `canon schema` (the guides read clap's declaration); `argumentsRef` schemas carry it as `oneOf` |
| `--actor`'s environment variable was in its help text | `env: "CANON_ACTOR"` |
| options that cannot be given in an `--input` document were found by trying | `flagOnly: true` on `json`, `input`, `commit`, `plan-digest`, `repo` and `as` |
| `exits` (each row a code and a meaning) | none: a command's exits are 0 and the exit of each code in its `errors`; the registry (`schema --errors`) says what a code exits |
| `errorCodes` | `errors`: every code the command can emit, with the reserved ones of the features it has (`INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE` with `--input`, `BATCH_INCOMPLETE` with a batch, `PLAN_STALE` with `--commit`, `CURSOR_INVALID` with `--cursor`) |
| `outputs[].report` (a name) | `outputs[].schemaRef` (`#/schemas/<Name>`); `kind`, `schemaVersion`, `mediaType` (an artifact) and `batch` stay; an output that is one of several says `when`; no two outputs share a `schemaRef` (`brief approve` has three, `init` two) |
| `dashboard` declared a `stream` output | one `report` output: its `--json` is one envelope. A `stream` is for NDJSON event lines only |
| every report schema in one top-level `schemas` map of the spec file, each with its own `$id` and `$defs` | each command document holds the schemas its refs name in `schemas`: the `$defs` of a report are siblings, every reference is `#/schemas/<Name>`, and no member has an `$id` |
| `inputs[]` (a list of one: `schemaRef`, `batch`, `maxBytes`, `example`, `batchExample`) | `input` `{option, schemaRef, argumentsRef, batch, maxBytes, maxItems?, example}` on the 47 commands that take `--input`. `schemaRef` is the fragment schema of one document (no root `required`, no flag-only key, `additionalProperties: false`, an integer is a number or a numeric string); the 30 schemas that had `required` are now `argumentsRef`, the schema of the arguments after the document is merged with the command line. `example` is one complete valid document; `batchExample` is gone (an array of the example is a batch) |
| one example per command, without `--json`, or with it before the path | `examples`: complete calls that end with `--json` after the path, no placeholder, no `--input`; a test parses each with the real parser |
| the first line of `canon --help` was the introduction | `Agents: "canon schema --json" describes every command as JSON.`, on the root, every group and every command; the introduction is below it |
| `specVersion` `"1"` and a spec file of `commands` (nodes), `globalOptions`, `schemas` | `specVersion` `"2"`: `tree` and `commands` (one command document each, byte-equal to `canon schema <COMMAND> --json`'s `data`); the global options are in each command's `options` |
| `canon schema`'s own `data` had no `kind` for a listing or a command | `data.kind` is `tree`, `group` or `command`; `meta.schemaVersion` is `1.1` |
| an unknown path: exit 2, `UNKNOWN_COMMAND`, message with the nearest matches | exit 2, `UNKNOWN_COMMAND`, `expected` the valid words where the path went wrong, `suggestions` the nearest of them; a path under a command (`schema finding close now`) is the same refusal without `expected` (no word is valid there); `field` is `command` in both |
| `canon schema <GROUP> --as <ROLE>` listed every child of the group, including commands the role's tree omits | the group document lists the children of the role's tree and names the role in `x-canon.as`; a group that holds no command of the role is `UNKNOWN_COMMAND` (field `command`, `expected` the groups the role's tree lists there) |
| `--as` and `--skills-source` read `CANON_ROLE` and `CANON_SKILLS_SOURCE` without saying so | each command's document declares the variables it reads: `env: "CANON_ROLE"` on `--as` and on attribution `--role`, `env: "CANON_SKILLS_SOURCE"` on `--skills-source` of `skills install`, `skills check` and `skills doctor`, `env: "CANON_ACTOR"` on `--actor` of `brief approve`, `migrate` and `policy approve` (spec 16.3) |
| `canon schema` warned `UNKNOWN_ROLE_IGNORED` about a bad `CANON_ROLE` | `canon schema` does not read `CANON_ROLE` at all (spec 10.2.2, 16.3): its `--as` declares no `env`, and the answer is byte-equal with and without the variable. `canon context` and `canon skills show` still warn |
| `skills install|check|doctor --providers` had no `defaultNote` although the help describes the default | `defaultNote` says how it is found |

**`atip check`.** The `atip-check` crate (19 L0 checks in 0.33) now runs the 28 L0 checks of atip/1 1.1 (https://github.com/journeyWorker/atip, tag v1.1.0, `approaches/checker.md`). A fixture written for 0.33 needs the changes below; nothing is read under the old name.

| 0.33 | 0.34 |
|---|---|
| 19 L0 checks (15 MUST, 4 SHOULD) | 28 L0 checks: the 19 plus `L0.envelope`, `L0.contract-version`, `L0.stderr-result`, `L0.stderr-quiet`, `L0.no-prompt`, `L0.registry`, `L0.usage-hint`, `L0.hint-form`, `L0.global-on-group`; a claim of 1.1 leaves 27 applicable (21 MUST, 6 SHOULD: `L0.contract` is a 1.0 rule), a claim of 1.0 leaves 27 (`L0.hint-form` does not apply) |
| the envelope schema was judged inside `L0.no-null`, `L0.ok-exit` and `L0.contract` | `L0.envelope` judges the envelope schema; `L0.no-null` only `null` in `meta` and `error`; `L0.ok-exit` only the exit, `ok` and `retryable` against the registry; `L0.contract-version` the members of a later minor |
| `L0.no-env-switch` failed a probe that waited for input | `L0.no-prompt` runs the fixture's `missingValue` invocation on a terminal; `L0.no-env-switch` no longer waits for input |
| stderr judged by `L0.ok-exit`, `L0.unknown-command`, `L0.version` and others | `L0.stderr-result` (MUST: the error or the result on stderr) and `L0.stderr-quiet` (SHOULD: anything else on stderr in JSON mode) own it; `L0.version` still needs an empty stderr |
| registry rows `{code, exit, retryable, meaning, fix}` | `{code, exit, retryable, reasons?}`: names only, no prose; a row with `meaning` or `fix` is `FIXTURE_INVALID` |
| `atip check --bin <b> --level L0 --fixture <f>` | the same, plus `--contract <MAJOR.MINOR>` (`1.0` or `1.1`); the claim is `--contract`, else the fixture's `contractVersion`, else `1.0`; two that differ are `USAGE` and no binary runs |
| `FIXTURE_INVALID` without a reason | `FIXTURE_INVALID` has the reasons `NOT_JSON`, `SETUP_FAILED`, `UNREADABLE`, `WRONG_SHAPE` (`error.reason`) |
| the checker's own envelopes: `meta.contract` | also `meta.contractVersion: "1.1"`, `error.reason` where a code has reasons, and the warning `SHOULD_FAILED` in `meta.warnings` when a SHOULD check failed and the level is met |
| `data` of a run: `level`, `passed`, `checks` | also `data.contractVersion`, the minor the checks judged |
| a fixture of a check that lacks an entry it needs: `unprovided` | unchanged, and a violation the run saw is `fail` whatever the fixture lacks |

**`atip check --level L1`.** `atip check` opens L1 (the self-description, the arguments and the data) of atip/1 1.1.2 (https://github.com/journeyWorker/atip, tag v1.1.2, `approaches/checker.md`); `L2` is still `LEVEL_UNSUPPORTED` and its hint says 0.35. A run at L1 is the 28 L0 checks and 31 L1 checks (25 MUST and 6 SHOULD for a 1.1 claim; the report's `data.level` is `L1`). The checks that judged what L1 now owns change as follows.

| 0.33 and the L0 of 0.34 | 0.34 |
|---|---|
| `--level L1` is `LEVEL_UNSUPPORTED` (hint: 0.34) | `--level L1` runs; `--level L2` is `LEVEL_UNSUPPORTED` with `expected` `["L0", "L1"]` and the hint that L2 arrives in 0.35 |
| `schema` was not read; a fixture needed no `schema` entry | an L1 run reads `schema --json`, `schema <path> --json` for every tree entry, `--errors`, `--error`, `--warnings` and `--warning`; a tool that claims L1 without `schema` fails `L1.tree` |
| `L0.page` and the `L1.paging` it points to: whether a tool pages was learned from `--cursor x` on the fixture's commands, and a paging tool with no paging call was `unprovided` | `commands.<id>.paged` lists invocations that return `meta.page.nextCursor`; from L1 the command documents decide which commands page (a command whose document declares `--cursor` does), at L0 the answer to `--cursor x` does (the code of `L0.unknown-option` with `field` `--cursor` is a command that does not page, `CURSOR_INVALID` is one that does). A paging command with neither a `paged` entry nor an observed `nextCursor` is `unprovided`; `unsupported` only when no command pages; a command the fixture does not list never makes either `unsupported` |
| the fixture's `registry` was compared with `schema --errors` and `warnings` with `schema --warnings` | `"registry": "from-schema"` takes both from the tool and compares nothing (`L0.registry` then checks only that every emitted code, reason and warning is one the tool prints); a fixture with it has no `warnings` member; a tool without `schema --errors` makes `L0.ok-exit` and `L0.registry` `unprovided` |
| `L0.global-on-group` failed a group that ran | from L1 a group that is not refused with `COMMAND_REQUIRED` is `L1.groups-not-runnable`'s failure and `L0.global-on-group` is `unsupported` for it |
| `L0.registry` judged the reasons of an entry against the registry-entry schema and compared the entry `--error <CODE>` prints with the list's, reasons included | from L1 the reasons of an entry (their form, no `exit` or `retryable` of their own, the same in the list and the entry) are `L1.reasons`'; `L0.registry` judges the rest |
| `USAGE` in an expected result | `USAGE` or its registered finer replacement (`UNKNOWN_OPTION`, `OPTION_REQUIRED` with exit 2) in the L1 checks (spec 7.2.2) |
| `FIXTURE_INVALID` reasons `NOT_JSON`, `SETUP_FAILED`, `UNREADABLE`, `WRONG_SHAPE` | also `PREPARE_FAILED` (a `prepare` program of the fixture did not exit 0, did not start, ran over the time limit or is the binary under test: the run stops and reports no check) |
| `data` of a run had no record of how the state was made | `data.prepare` lists every `prepare` step with its argv, so whoever reviews a level claim sees what was made outside the tool |
| `L0.page`: a listed command that answered `--cursor x` with exit 0, or refused it with another code or field than a non-paging tool's and `CURSOR_INVALID`, was inconclusive | it is read as paging (the cautious reading, contract 1.1.2): without a `paged` entry and an observed `nextCursor` the check is `unprovided` |
| `L0.registry` with `"registry": "from-schema"` and no `schema --errors` was `unprovided` | `unsupported`; only `L0.ok-exit`, the one check that then needs the registry, is `unprovided` |

The 31 L1 checks (`L1.tree`, `L1.no-state`, `L1.walk`, `L1.quoted-path`, `L1.schema-errors`, `L1.registry`, `L1.reasons`, `L1.reasons-reserved`, `L1.warnings`, `L1.reserved-applicable`, `L1.codes-known`, `L1.groups-not-runnable`, `L1.grammar`, `L1.depth`, `L1.one-meaning`, `L1.one-meaning-values`, `L1.examples`, `L1.example-runs`, `L1.constraints`, `L1.domains`, `L1.patterns`, `L1.numbers`, `L1.validators`, `L1.output-data`, `L1.next`, `L1.hint-command`, `L1.help-line`, `L1.defaults`, `L1.deterministic`, `L1.paging`, `L1.fields`) judge, for every command the tree names, the documents against `schema-doc.schema.json` and the rules of spec 10.13 that JSON Schema cannot say (the ones `scripts/check.ts` of the contract implements, ported to Rust), and probe what the documents declare with real invocations: one invocation per rule group of each `constraints` list, values at each bound of each `domain`, the two numbers beyond the safe integers, every validator's `rejects`, `--limit 1` and the next page, a reused cursor with a changed argument, `--fields` selections. A probe is made only for `read` commands and writes the fixture lists (contract 1.1.2: a write no `commands` entry lists is outside what `L1.constraints`, `L1.domains` and `L1.validators` probe, so it never makes one of them `unprovided`); each of those three checks carries `unlisted`, the ids of the write and destructive commands no entry covers, as information that changes no result, so a fixture cannot hide a write silently. A constraint that every violating invocation also breaks another declared rule for is `entailed`, with the rules named; equivalent rules are one test and fail when their codes differ. `L1.patterns` compares the ECMAScript (`u`, the `regress` crate) and Rust `regex` readings of every published pattern and passes a difference only as a verified Unicode-version difference (one code point, a pattern with a category, each engine's own tables giving it different categories), naming both Unicode versions and the code point; the contract's `scripts/regex-cases.json` (v1.1.2) is the evidence that `regress` answers as Bun does.

**Arguments declare what they accept: `constraints`, `domain` and `validators`.** Every command document lists the relations between its arguments, the restrictions of each argument's own value and the other checks that read no state (spec 10.7.4, 10.7.5, 10.8), and a test drives every one of them through the real parser, so a declaration cannot be false: a call that breaks exactly one rule is refused with the rule's code, exit 2, a `field` that is one of its arguments, and writes nothing; a call that meets it is not refused for it; a value just inside a domain is accepted and one just outside is refused with the code the document names; every validator's `rejects` call is refused with its code in an empty directory, with no repository and no `setup`; every `example` meets the constraints and domains of its command. A survey of the sources fails when a handler refuses an argument that no document declares and no table classifies (a check that reads state is classified as a precondition and is never declared). The tables are `crates/canon-cli/src/arg_rules/`; what clap says through its public getters (`conflicts_with`, `ArgGroup`) is read by `atip::constraint::clap_rules`, not restated.

| 0.33 | 0.34 |
|---|---|
| clap's `requires` and `conflicts_with` were read from the `Debug` text of a clap argument (`atip::project::relations`) and listed as `conflictsWith` and `requires` on the option and `requiredOneOf` on the command | deleted: a command document's `constraints` is the one list of relations, and the guides and the syntax lines are drawn from it. `atMostOne` for a set whose members all conflict pairwise (`init`: `--check` with each of the others, `--policy` with `--no-policy`), `exactlyOne` for a required group (`review scenario`: `--upstream-ref`, `--original-spec-ref`), `requires` for `--commit`/`--plan-digest` in both directions |
| `evidence add`'s `--task`, `--scenario`, `--all-scenarios`, `finding close`'s `--reason`, `retrieve`'s `--regime` or `--domain`, a calendar date, a lower bound on `--round` were known only to the handler | declared with the code the handler gives (`EVIDENCE_INVALID_ARGUMENT`, `FINDING_REASON_REQUIRED`, `USAGE`): `atLeastOne`, `exactlyOne`, `requires`, `forbids` with `if.in` on a value, domains, validators |
| no `domain` | `domain` on every string, path and integer argument with a restriction: `minimum`/`maximum` for a number (the Rust integer type's bounds are in it: a `u32` is `{minimum: 0, maximum: 4294967295}`; `--round` and `--seq` `minimum: 1`, `--limit` and `--max-artifact-mib` `minimum: 1`), `minLength`, `maxLength` and `pattern` for text: `pattern` is `^…$` of the restricted I-Regexp of the contract, compiles with the Rust `regex` crate, and is a necessary condition (a value outside it is always refused; a value inside may still be refused by a validator or by state). Every id kind (change, subject, project, scenario, task, role, session, run, review, strategy), the time shape of the 12 time options, non-blank and one-line texts, `NAME@VERSION`, `FORMAT:PATH`, `--digest`, `--regime`, `trace`'s operand, `--language`, `--providers` have one |
| no `validators` | `validators` for what neither can say, each with `rejects` calls: the calendar (`2026-02-30`) of each of the 12 time options (`--since`, `--before`, `--after`, `--expiry`, `--as-of`, `--approval-at`), `--role` against the role of `--regime` (`retrieve`, `dispatch begin`), `--status` against the kind (`query`), a blocker or should-fix `finding add` with `--introduced-rev` or `--introduced-unknown`, the digests of a relayed `brief approve`, `--artifact` scopes, the window of `export` |

New refusals. These are the only calls that were accepted in 0.33 and are refused now:

| Call | 0.33 | 0.34 |
|---|---|---|
| `canon export --kind change --before 2026-01-02 --after 2026-02-01` (`--after` later than `--before`) | exit 0, an empty export | exit 2, `USAGE`, `field` `--after`, before any read or write (also with `--dry-run` and `--output`); an equal pair is still an empty window |
| `--max-artifact-mib`, `--interval-secs`, a paging `--limit` or `retrieve --limit` above 9007199254740991 (2^53-1), for example `--max-artifact-mib 9007199254740993` | read as a 64-bit integer | exit 2, `USAGE`, `field` the option; nothing is rounded into range. `9007199254740991` is still accepted; a negative value was and is refused (`--limit -1` now names `--limit` instead of the `-1` clap read as a flag: every integer option takes a negative number as its value, so the refusal names it), and `-0` is the integer 0 (`--interval-secs -0`, `retrieve --limit -0`), as in the contract's integer grammar |

Step 6 of 11.7 (spec 10.8) runs in one order, for every invocation, before any precondition or state read: required, then each `domain`, then each constraint in the order the document lists them (clap's relations, then the tables'), then each validator; the first failure ends the call. The declarations are the enforcement: the domains and constraints of a command (`arg_rules::enforce`, `validators::check`) are evaluated from the tables themselves, so a rule or a domain that carries the command's own code (`FINDING_INVALID_ARGUMENT`, `EVIDENCE_INVALID_ARGUMENT`, `DIVERGENCE_INVALID_ARGUMENT`, `SUBJECT_INVALID_ARGUMENT`, `FINDING_REASON_REQUIRED`, `TRACE_OPERAND_INVALID`, ...) answers the same in an empty directory and in a configured repository (it used to answer `CONFIG_MISSING`, or the missing file, or `PLAN_SOURCES_MISSING`). The same stage enforces the relations clap skips when the argument a `requires` names conflicts with one present: `migrate --backup F --relayed-via ask` and `migrate --revert --commit --plan-digest D --actor x` are `USAGE` naming `--revert` and `--relayed-via`. The message of such a refusal is generated from the declaration (`` `--disposition` is rejected or deferred, which needs `--reason` ``) where the handler's own sentence used to be; the handlers' checks are still there for the arguments the declarations do not say. A violation's `field` is the first argument of the rule at fault in the rule's own order, an operand by its value name: for `atMostOne` the first given of `names` (`finding add --introduced-rev R --introduced-unknown` names `--introduced-rev`), for `requires` the first missing of `then`, for `forbids` the first given of `then`. A validator runs after them: `evidence add --all-scenarios --scenario S --artifact A=f` is the `forbids` of `--scenario`, not the `--artifact` scope validator. The validators that were in a handler after the repository was found run before it (`validators.rs`): `brief approve`'s digests and the relay checks of `brief approve` and `policy approve`, `evidence add` and `evidence audit`'s `--class audit` and `--artifact` scopes, `finding add`'s checks, `export`'s window, `query`'s `--status` and `--kind`. The relay role check is the first relay validator, so `brief approve --relayed-via ask --actor 'a b'` with no role declared answers `USAGE` about the role (field `--relayed-via`), and the `--actor` word check comes after it. A call that broke an earlier check of the same command and a later moved one keeps its answer, except `evidence add` reports a broken `--artifact` scope before a blank `--ref`, and the `--project` default.

The environment is a stateless input (spec 11.7), so what it fills is declared and checked before any state is read. `--as` and, on `brief approve`, `policy approve` and `migrate`, `--actor` carry `env` (`CANON_ROLE`, `CANON_ACTOR`); a constraint does not count an environment value as "given" (`--actor` still `requires` `--relayed-via`). A relayed approval (`brief approve`, `policy approve`, `divergence accept`, `migrate`) has three validators over `--relayed-via`, all `USAGE`: a role is declared (`--as` or `CANON_ROLE`), the actor is named (`--actor` or `CANON_ACTOR`; not on `divergence accept`, whose `--actor` is required) and is one word, and `--approver` and `--note` are not blank. `--approver` and `--note` of `brief approve` and `policy approve` are no longer a `domain`: the blank human answer is `APPROVAL_INCOMPLETE` (`POLICY_APPROVAL_REFUSED` for the policy's approver) and a blank relayed answer is `USAGE`, which one domain cannot say; each has a validator with its own code. The refusals of `divergence accept` and `migrate` for these now come before `CONFIG_MISSING` too. `--input` twice is `USAGE` (`field` `--input`); an array on a command that takes no batch is `INPUT_INVALID`, reason `BATCH_UNSUPPORTED` (step 1, declared by `input.batch`, not a validator); `--input -` beside another option whose value ends in `-` (`evidence add --report junit:-`, `--artifact -`) is `USAGE`, `field` the one given later on the command line, and no document declares it. `--fields` with a batch is refused by the batch output's own declaration, which has no `fields` (spec 6.10).

`EVIDENCE_APPROVAL_INVALID` is one code (exit 2) with two reasons in the registry: `MALFORMED` (the four approval options do not go together, the approver is not one trimmed line: raised from the arguments alone, before any state is read, so it now also answers in a directory with no repository) and `SIGNATURE_MISMATCH` (the signature file is missing or unreadable, is not armored text, no `allowed_signers` verifier is pinned, the approval policy is malformed, or the signature does not verify). A caller that read the code alone reads the same; `error.reason` says which.

A refusal names the argument at fault in `field` (spec 7.1.1): an option as written, an operand by its value name with no brackets.

| Call | 0.33 `field` | 0.34 `field` |
|---|---|---|
| a bad operand value, for example `canon subject show Bad_Id` | `<SUBJECT_ID>` | `SUBJECT_ID` |
| `canon format` with a bad directory, `canon search ""`, `canon trace nonsense`, `canon context-pack show <bad id>` | `dir`, `text`, `id`, `<PACK_ID>` | `DIR`, `TEXT`, `ID`, `PACK_ID` |
| a missing required argument or group member (for example `evidence add --report-case x` without `--report`) | the first argument clap listed | the argument the rule at fault names (`--report`) |
| `canon retrieve --regime R --subject S` (no `--domain`) | `--regime` | `--subject` |
| the handler refusals of a declared relation, domain or validator (`--round 0`, a blank `--summary`, `--reason` missing, an `--artifact` scope, a blank `--approver`, ...) | none | the option |

A batch runs the whole stateless stage on every element (domains, constraints, validators, spec 11.11) before any element's own dry run or write: a stateless fault in any element is exit 2 with `field` `/items/<i>/<key>` and nothing written, also in a directory with no repository (`export`, `artifacts store`, `ingest records`, `ingest sessions` used to answer the first element's `CONFIG_MISSING`). The calendar of every time option (including `--expiry` and `--as-of` of `divergence`), the form of `--surface-ref` and `--introduced-rev self` are validators of that stage, after the domains and the constraints, not clap's value parsers. An integer refusal names the declared bound (`--round must be an integer of at least 1`).

The 64-bit integer options are `--max-artifact-mib` (`artifacts store`, `evidence add`, `evidence audit`, `review scenario`, `review change`), `--interval-secs` (`ingest sessions`, `ingest records`), the paging `--limit` (`query`, `export`, `finding list`, `divergence list`) and `retrieve --limit`. The 32-bit and 16-bit options (`--round`, `--seq`, `--port`, `--expand`, `--timeout`, `search --limit`) are bounded by their Rust type, which their `domain` states; nothing new is refused for them.

The relations declared in 0.34 (every command with a rule or a validator; a domain is listed in `canon schema <command> --json` under the option):

| Command | Constraints and validators |
|---|---|
| `artifacts move` | requires: `--commit` → `--plan-digest`; requires: `--plan-digest` → `--commit` |
| `brief approve` | requires: `--actor` → `--relayed-via`; requires: `--digest` → `--relayed-via`; validator `--relayed-via`, `--as` (`USAGE`); validator `--relayed-via`, `--actor` (`USAGE`); validator `--relayed-via`, `--approver`, `--note` (`USAGE`); validator `--approver`, `--note` (`APPROVAL_INCOMPLETE`); validator `--relayed-via`, `--digest`, `--change` (`USAGE`); validator `--digest` (`USAGE`) |
| `dispatch begin` | requires: `--skill-digest` → `--skill` (`DISPATCH_INVALID_ARGUMENT`); validator `--role`, `--regime` (`DISPATCH_ROLE_REGIME_MISMATCH`) |
| `divergence accept` | requires: `--approver` → `--relayed-via`; requires: `--note` → `--relayed-via`; validator `--expiry` (`USAGE`); validator `--relayed-via`, `--as` (`USAGE`); validator `--relayed-via`, `--approver`, `--note` (`USAGE`) |
| `divergence add` | atMostOne `--promote`, `--no-promote`; requires: `--state` = `deferred` → `--reason`, `--expiry` (`DIVERGENCE_INVALID_ARGUMENT`); validator `--expiry` (`USAGE`) |
| `divergence defer` | validator `--expiry` (`USAGE`) |
| `divergence list` | atMostOne `--limit`, `--all`; validator `--as-of` (`USAGE`) |
| `evidence add` | atMostOne `--promote`, `--no-promote`; atLeastOne `--class`, `--report`; atLeastOne `--ref`, `--report`; atLeastOne `--task`, `--scenario`, `--all-scenarios` (`EVIDENCE_INVALID_ARGUMENT`); forbids: `--all-scenarios` → `--scenario`, `--task`, `--report-case`, `--approver`, `--approval-role`, `--signature-file`, `--approval-at` (`EVIDENCE_INVALID_ARGUMENT`); requires: `--all-scenarios` → `--report` (`EVIDENCE_INVALID_ARGUMENT`); requires: `--report-case` → `--report` (`EVIDENCE_INVALID_ARGUMENT`); requires: `--command-result` → `--summary` (`EVIDENCE_INVALID_ARGUMENT`); requires: `--approver` → `--approval-role`, `--signature-file`, `--approval-at` (`EVIDENCE_APPROVAL_INVALID`); requires: `--approval-role` → `--approver`, `--signature-file`, `--approval-at` (`EVIDENCE_APPROVAL_INVALID`); requires: `--signature-file` → `--approver`, `--approval-role`, `--approval-at` (`EVIDENCE_APPROVAL_INVALID`); requires: `--approval-at` → `--approver`, `--approval-role`, `--signature-file` (`EVIDENCE_APPROVAL_INVALID`); validator `--approval-at` (`USAGE`); validator `--surface-ref` (`USAGE`); validator `--class` (`EVIDENCE_INVALID_ARGUMENT`); validator `--artifact`, `--scenario`, `--all-scenarios` (`EVIDENCE_INVALID_ARGUMENT`) |
| `evidence approval-payload` | atLeastOne `--task`, `--scenario` (`EVIDENCE_REFUSED`); validator `--approval-at` (`USAGE`); validator `--surface-ref` (`USAGE`) |
| `evidence audit` | atMostOne `--promote`, `--no-promote`; requires: `--report-case` → `--report` (`EVIDENCE_INVALID_ARGUMENT`); requires: `--command-result` → `--summary` (`EVIDENCE_INVALID_ARGUMENT`); validator `--artifact`, `--scenario` (`EVIDENCE_INVALID_ARGUMENT`) |
| `export` | atMostOne `--limit`, `--all`; validator `--before`, `--after` (`USAGE`); validator `--after`, `--before` (`USAGE`) |
| `finding add` | atMostOne `--promote`, `--no-promote`; atMostOne `--introduced-rev`, `--introduced-unknown` (`FINDING_INVALID_ARGUMENT`); requires: `--resolution-rev` → `--disposition` (`FINDING_RESOLUTION_REV_UNEXPECTED`); forbids: `--disposition` = `open`/`rejected`/`deferred` → `--resolution-rev` (`FINDING_RESOLUTION_REV_UNEXPECTED`); validator `--severity`, `--introduced-rev`, `--introduced-unknown` (`FINDING_INTRODUCED_REQUIRED`); validator `--introduced-rev` (`USAGE`) |
| `finding close` | atMostOne `--promote`, `--no-promote`; requires: `--disposition` = `rejected`/`deferred` → `--reason` (`FINDING_REASON_REQUIRED`); forbids: `--disposition` = `open`/`rejected`/`deferred` → `--resolution-rev` (`FINDING_RESOLUTION_REV_UNEXPECTED`) |
| `finding list` | atMostOne `--limit`, `--all` |
| `ingest plans` | requires: `--dialect` → `--plan-source`; requires: `--plan-source` → `--dialect` |
| `init` | atMostOne `--check`, `--no-agents-md`; atMostOne `--check`, `--policy`, `--no-policy`; atMostOne `--check`, `--providers`; atMostOne `--check`, `--language`; atMostOne `--check`, `--dry-run` |
| `ledger compact` | requires: `--commit` → `--plan-digest`; requires: `--plan-digest` → `--commit` |
| `migrate` | atMostOne `--revert`, `--relayed-via`; requires: `--commit` → `--plan-digest`; requires: `--plan-digest` → `--commit`; requires: `--backup` → `--revert`; requires: `--relayed-via` → `--commit`; requires: `--actor` → `--relayed-via`; requires: `--approver` → `--relayed-via`; requires: `--note` → `--relayed-via`; validator `--relayed-via`, `--as` (`USAGE`); validator `--relayed-via`, `--actor` (`USAGE`); validator `--relayed-via`, `--approver`, `--note` (`USAGE`) |
| `policy approve` | requires: `--actor` → `--relayed-via`; requires: `--relayed-via` → `--note`; validator `--relayed-via`, `--as` (`USAGE`); validator `--relayed-via`, `--actor` (`USAGE`); validator `--relayed-via`, `--approver`, `--note` (`USAGE`); validator `--approver` (`POLICY_APPROVAL_REFUSED`) |
| `purge` | requires: `--commit` → `--plan-digest`; requires: `--plan-digest` → `--commit`; validator `--before` (`USAGE`) |
| `query` | atMostOne `--plugin`, `--history`; atMostOne `--limit`, `--all`; forbids: `--kind` = `scenario`/`session`/`run`/`event`/`handoff`/`review`/`divergence`/`trajectory`/`strategy_item`/`evidence`/`subject`/`finding` → `--change` (`QUERY_INVALID_ARGUMENT`); forbids: `--kind` = `scenario`/`session`/`run`/`event`/`handoff`/`review`/`divergence`/`trajectory`/`strategy_item`/`evidence`/`finding` → `--status` (`QUERY_INVALID_ARGUMENT`); forbids: `--kind` = `change`/`task`/`scenario`/`session`/`run`/`event`/`handoff`/`review`/`divergence`/`trajectory`/`strategy_item`/`evidence`/`finding` → `--domain` (`QUERY_INVALID_ARGUMENT`); validator `--kind`, `--status` (`QUERY_INVALID_ARGUMENT`); validator `--since` (`USAGE`) |
| `retrieve` | exactlyOne `--regime`, `--domain`; requires: `--subject` → `--domain`; validator `--for-role`, `--regime` (`USAGE`) |
| `review change` | exactlyOne `--upstream-ref`, `--original-spec-ref` |
| `review scenario` | exactlyOne `--upstream-ref`, `--original-spec-ref` |
| `scenario new` | requires: `--change` → `--flow` |
| `schema` | atMostOne `--path`, `--errors`, `--error`, `--warnings`, `--warning` |
| `strategy approval-payload` | validator `--approval-at` (`USAGE`) |
| `strategy promote` | requires: `--evaluation` → `--approval`; requires: `--approval` → `--evaluation`; requires: `--signature-file` → `--approval` |
| `strategy rollback` | validator `--approval-at` (`USAGE`) |
| `tier age` | requires: `--commit` → `--plan-digest`; requires: `--plan-digest` → `--commit` |

**Paging and `--fields`** (spec 6.9, 6.10, 7.2.3, 10.9, 10.13 rules 11 and 15). A cursor is bound to the call that issued it, and `--fields` is the contract's field selection, declared in the command document and reported in `meta.fields`. The names below are deleted with no alias.

| 0.33 | 0.34 |
|---|---|
| a cursor named the command and `--kind` (`query:<kind>`; `finding list:<change>`): a changed `--since`, `--before`, `--open`, `--as-of` or `--domain` went on at the same record, silently | a cursor is bound to the command and every argument of the call that issued it except `--limit`, `--all`, `--cursor`, `--fields` and `--json` (`--all` is the unbounded `--limit` and `--json` only changes how the answer is shown, so neither binds; a cursor issued in text mode works with `--json` and the reverse). atip/1 1.1.x names only `--limit`, `--cursor` and `--fields` as exceptions; exempting `--json` and `--all` is the behaviour planned for atip/1 1.2, and no result set changes because of it. An argument equal to its default still binds: `--repo .` is not an omitted `--repo`. Another argument: exit 2, `CURSOR_INVALID`, `field` `--cursor`, `error.reason` `ARGUMENTS_CHANGED`; nothing is written. Spelling and order do not matter (`--kind=x` is `--kind x`) |
| `CURSOR_INVALID` had no reason; its message told three situations apart | `error.reason`: `FOREIGN` (not issued by this command, or not a cursor), `ARGUMENTS_CHANGED`, `EXPIRED` (issued for these arguments, but the record it points after is gone); registered under the code (`canon schema --error CURSOR_INVALID --json`) |
| a cursor of 0.33 (and of 0.29, which had its own message) continued a listing | `FOREIGN`: a cursor now carries its binding, so an older one is not one this canon issued. Start the listing again without `--cursor` |
| `--fields` was split at every comma and matched ignoring case, `_` and `-`: `changeIds`, `Reward`, `resolution_sha` | a path is the member's own key, written as it is, and the one spelling is the only one: a record's keys are snake_case, a report's own members camelCase (spec 6.3). `Reward`, `REWARD` and every other case are `USAGE` |
| `canon query --kind subject --fields changeIds` | `--fields change_ids` |
| `canon finding list --fields resolution_sha,file_ref` | `--fields resolutionSha,fileRef` |
| `canon divergence list --fields project_id,scenario_id` | drop them: `projectId` and `scenarioId` are kept in every row, so they are not selectable (`USAGE`) and need not be named |
| `canon status --fields status_version,canon_version,dangling_references` | `--fields statusVersion,canonVersion,danglingReferences` |
| `canon context --fields capability_version,join_keys` | `--fields capabilityVersion,joinKeys` |
| `--fields` took only top-level member names | the contract's path grammar: `a.b`, `a[]`, `a[].b`, and `["any.key"]` for a member whose name is not an identifier (an identifier is never bracketed). A comma inside a bracketed key does not separate paths. The paths canon declares are all top-level members, so only the dot-free spelling occurs in its answers; the grammar, the projection and the checks are `atip::fields`, held by unit tests against the contract's examples |
| `--fields` is a list option | one string option (`valueType` `string`); the list is split by the grammar. A path listed twice is kept once; an empty value, a malformed path and a path that is not declared are `USAGE`, `field` `--fields`, `expected` the selectable paths, in text mode as with `--json` (a text listing is not cut) |
| `--fields` could be given in a document (`{"fields": "id"}`), and a batch of `export` could carry it per element | `--fields` is `flagOnly` on all six commands: a `fields` key of a document is `INPUT_INVALID`, reason `FLAG_ONLY_KEY`, `field` a pointer to the key (`/fields`, `/items/1/fields`), at plan time, before any element is run. The output of a batch declares no `fields`, so `--fields` on the command line of a batch is `USAGE`, `field` `--fields`, also before any write; neither is a `BATCH_INCOMPLETE`. The fragment schemas have no `fields` property. Remove `fields` from input documents and pass `--fields` on a single call |
| `query --fields` accepted the members of the `--kind`'s schema and refused another kind's | the declared paths are the members of every kind's record (`canon schema query --json`); a member the `--kind` lacks selects nothing (it is absent from the records, as the contract says). A name no kind has is `USAGE`; its hint lists the members of this `--kind` |
| `finding list --fields summary` returned rows of `summary` alone | `round` and `seq` are always kept (`fields.keep`), as are `projectId` and `scenarioId` of `divergence list`, and `unchanged`, `waitingOn` and `nextOmitted` of `status`; a kept path cannot be selected (`USAGE`) |
| nothing in `meta` said what a projection kept | `meta.fields`: the kept paths (always-kept first, then the selected), each in its one spelling |
| a command document did not say what `--fields` takes | `outputs[].fields` `{records, paths, keep?}` on `finding list` (`/findings`), `divergence list` (`/scenarios`), `query` (`/records`), `status` and `context` (`""`: the report is the record) and on each of the three outputs of `export` (`/records`, `""`, `""`). No output has a `default`: without `--fields` the answer is the whole record and has no `meta.fields`. The paths are read off the record schema of the report, so a declaration cannot name a member the report lacks |
| a projected record had to pass a schema whose `required` was dropped by hand (`ProjectedFindingRow`, `ProjectedScenarioRow`, `ProjectedExportRecord`; the unpublished `…FieldsReport` types) | the published schema is the whole record's; the projected `data` passes it with `required` reduced to the members present in every object at or below the record (spec 6.10), which debug builds assert on every projected answer. The schemas of `finding list`, `divergence list`, `query`, `status` and `context` keep their version; the cut nodes of all six hold the contract's projectable conditions |
| `canon export --json` was one output, `ExportReport` (1.0, 1.1 on the wire): the manifest, the file written, or the preview (an `anyOf`), and its records were one of two shapes | **Decision** (contract 1.1.1 allows two ways; canon splits by shape rather than keeping a union without `fields`): three outputs, each with a `when`, its own `fields` and named by `meta.output`: `ExportReport` (the manifest; **2.0**, 2.1 on the wire: it is the manifest alone, its `records` are one shape that `--fields` cuts at `/records`, and it has no `meta` member), `ExportWrittenReport` (`--output` with `--json`; `fields.records` `""`, paths `kind`, `output`, `recordCount`) and `ExportPreviewReport` (`--dry-run`; paths `dryRun`, `kind`, `output`, `recordCount`). `--fields` is read against the output the call selects: with `--output --json` its paths are those of the written answer while the file still holds the whole manifest; in text mode (printing, or writing the file) the manifest is the output, so `--fields` cuts its records. The file `--output` writes, and the manifest of text mode, still hold the page they cover as `meta` beside the manifest |
| the report of `query` held `LedgerRecord` (`{type: object}`) | `QueryRecord`: the same object, with every member any kind declares listed as a property, so a path can be declared and checked |
| `meta.schemaVersion` of a failure without `data` was the version of the alphabetically first output of the command | the version of the first output its command document lists (`export`: `ExportReport`; `init`: `InitReport`) |
| `export`'s `when` said the manifest was the answer when neither `--output` nor `--dry-run` was given | `ExportReport` is the answer when `--dry-run` is not given and `--output` is not given or `--json` is not (in text mode the manifest goes to stdout, or to the `--output` file); `ExportWrittenReport` when `--json` and `--output` are given and `--dry-run` is not; `ExportPreviewReport` with `--dry-run`. `export --fields` help says which output it is read against |
| `query`'s `data.count` and `export`'s `data.recordCount` look like `meta.page.returned` | unchanged, and deliberate: they are report content (the records of the answer), not paging state; removing them would break consumers for no gain. The reports hold no `page`, `nextCursor`, `returned`, `hasMore` or `meta` member |


### Added
- `atip check --level L1` (31 checks over `canon schema`, arguments and data) and 28 L0 checks for a 1.1 claim; `--contract <MAJOR.MINOR>`; `data.contractVersion` and `data.prepare` in a run; the reasons `PREPARE_FAILED` and `SHOULD_FAILED` (warning).
- Fixture keys (all optional): `contractVersion`, `globalOptions`, `warnings`, `registry: "from-schema"`, `prepare`, and in `commands.<id>`: `ok`, `refused`, `missingValue`, `paged`, `fields`, `domainRejects`. A missing entry makes the checks that need it `unprovided`; a violation the run observed is still `fail`. `atip check --help` names every key.
- `L0.no-prompt` runs each `missingValue` on a pseudo-terminal; `L0.registry` compares the registry, the warning registry and the reasons with the fixture and requires every emitted code, reason and warning to be registered; `L0.refusal-effects` compares the fixture's `probe` before and after a refusal.
- `L1.validators` runs every `rejects` call without `setup`; `L1.next` follows at most 300 `meta.next` entries of read commands and says how many it left.
- `canon schema --warnings`, `--warning <CODE>`, `reasons` in registry rows, `constraints`, `domain` and `validators` in every command document, `outputs[].fields`, `meta.fields`.
- CI job `atip-l1` and `just atip` run L1 on the canon binary. canon meets L1 with one SHOULD advisory: `L1.one-meaning-values` reports that `--kind` takes disjoint value sets on `gate promote` and `purge`; both are subsets of the record-kind enum of `query` and `export`, which spec 9.6 allows.
- The contract repository's examples (tag v1.1.0, `crates/atip-check/tests/fixtures/atip-examples/`) and about 180 planted violations of the fake tool `atip-fake-tool` (now also a conforming L1 tool) are test input; each violation must fail exactly its owner.

### Changed
- canon's fixture (`crates/canon-cli/tests/atip/canon.fixture.json`) claims 1.1 and serves L0 and L1: `prepare` makes a git repository with one empty commit, `setup` creates two subjects, a change with two findings and two scenarios with a divergence each, `commands` lists `paged` and `fields`; `registry` and `warnings` are generated from `canon schema` (`CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check` rewrites it). `atip`'s own fixture (`crates/atip-check/atip.fixture.json`) claims 1.1.
- The shared part of the schema projection is in the `atip` crate (`atip::doc`, `atip::project`, `atip::fields`); canon's tables (`effect`, `idempotency`, error codes, outputs, roles, examples) stay in `canon-cli`.
- The conformance corpus was regenerated; the new expectations are `atip-1-1-*`, the `schema` command documents and five `evidence add` refusals (they carry `field`).

### Fixed
- A refusal changes nothing (spec 8.2), `.git` included: the code tree that `gate check`, `evidence add` and the commands that bind a commit compute is built in a scratch object directory, so none of them leaves a blob or a tree in `.git/objects`; `canon trace` refuses before it writes `.canon/index`.
- Handler refusals of a declared relation, domain or validator now name the option in `field`, and `EVIDENCE_APPROVAL_INVALID` from the arguments alone answers in a directory with no repository.

### Upgrading
- Read `meta.contractVersion` (`1.1`) before the 1.1 names; ignore members and enum values you do not know.
- Look a warning code up with `canon schema --warning <CODE> --json`, not `--error`; an unknown code is `USAGE`, not `UNKNOWN_ERROR_CODE`.
- Read `error.reason` (`UNREADABLE`, `STDIN_TTY`, `EMPTY`, `NOT_JSON`, `NOT_OBJECT`, `BATCH_UNSUPPORTED`, ...) instead of `error.x-canon.reason`.
- Read `canon schema` with the new names (the first table of the `canon schema` section); `docs/cli/canon.cli.json` is `specVersion` `"2"`.
- Spell `--fields` paths as the member's own key (`change_ids` for a record, `resolutionSha` for a report), drop `--fields` from `--input` documents and from batches, and start a listing again without `--cursor` if the cursor came from 0.33 or earlier.
- Fix the calls now refused: `export --after` later than `--before`, integer options above 2^53-1.
- An `atip check` fixture written for 0.33: replace `registry` and `warnings` prose rows by names (or `"registry": "from-schema"`), add `contractVersion`, and expect `FIXTURE_INVALID` for a row with `meaning` or `fix`.

## 0.33.0 — 2026-11-01

### Breaking

canon's `--json` output and error contract now follow atip/1 L0 (https://github.com/journeyWorker/atip, spec sections 4 to 8). The old names below are deleted: there are no aliases, and every consumer in this repository (the guides the binary carries, the website in both languages, the dashboard, the dogfood evals, the conformance corpus, the command spec file) moved in the same release.

**The envelope.** `meta` holds only the members the contract defines and canon's own `x-canon`; a member with no value is absent (never `null`, never `[]`).

| 0.32 | 0.33 |
|---|---|
| `meta.canonVersion` | `meta.toolVersion`; new `meta.tool` (`canon`) and `meta.contract` (`atip/1`) on every envelope. `data.canonVersion` of `canon status` is a member of that report and stays |
| `meta.command` `"canon"` when nothing resolved | `meta.command` is absent when not even the first word resolved; the path resolved so far otherwise (`canon finding nosuch` → `finding`) |
| `meta.context.role`, `meta.context.actor` (and `null`s) | gone; `meta.context` is `{repo}` (a path relative to the working directory), present for a command that takes `--repo` |
| `meta.returned`, `meta.total`, `meta.nextCursor`, `meta.hasMore` | `meta.page` = `{returned, total, nextCursor?}`; `nextCursor` present means more follow and `meta.next` holds the same call plus `--cursor <nextCursor>` |
| `meta.unchanged: true` | `data.unchanged: true` of a write's report (a report that is a stored record, `canon subject new/adopt/move/unadopt`, or not an object: `meta.x-canon.unchanged`) |
| `meta.waitingOn`, `meta.nextOmitted` | `data.waitingOn`, `data.nextOmitted` (same rule: `meta.x-canon` for a record); `canon status` has them in `data` too |
| `meta.hotStore` of `canon subject …` | `meta.x-canon.hotStore` (the data is the stored record) |
| `meta.note` of `canon scenario new` | `data.note` |
| `meta.warnings: ["text"]` | `meta.warnings: [{"code", "message", "field"?}]`; every warning situation has a registered code (listed below) |
| `meta.next[].why` | `meta.next[].reason` |
| `meta.next[].actor`, `.role` | `meta.next[].x-canon.actor`, `.x-canon.role` |
| `meta.next[]` with `argv: []` for a command holding a `<placeholder>` | not an entry: the guidance is in the error `hint` or the report text (text mode still shows it) |
| `meta.next[].argv` without `--json` | in JSON mode every entry's `argv` carries `--json` where the command takes it |
| `meta.next: []` on a write | absent |
| `error.hint: null`, `error.suggestions: []` | absent |
| `meta.output` | new: the report's name (`BriefApproveReport`, `InitCheckReport`, `BatchFindingCloseReport`) when the command declares several outputs |
| the canon-specific envelope schemas (`envelopes` of `docs/cli/canon.cli.json`) | removed; the contract's `envelope.schema.json`, `error.schema.json` and `registry-entry.schema.json` are vendored unchanged in `crates/atip/schema/` and pinned by digest; `docs/cli/canon.cli.json` has `"contract": "atip/1"` |

Every report of a command that writes, and `canon status`, gained the optional members `unchanged`, `waitingOn` and `nextOmitted`, so their `schemaVersion` is `1.1` (`BatchReport` too, which also reports `unchanged` when every element did). `canon scenario new` also gained `data.note`.

**Wrong calls.** `--json` is accepted anywhere before `--`, at the root and on groups too (`canon --json`, `canon finding --json`).

| Call | 0.32 | 0.33 |
|---|---|---|
| an unknown command | `USAGE` (clap's text in text mode) | `UNKNOWN_COMMAND`, `field` `command`, `expected` the valid words, `received`, `suggestions` |
| no command (`canon`, `canon finding`, `canon brief`, `canon report`) | `USAGE` | `COMMAND_REQUIRED`, `expected` the commands of that level (for `brief` and `report` the 0.28 removed-spelling message is the hint) |
| an unknown option, a missing value, a bad `--fields` name | `USAGE`, or `UNKNOWN_FIELD` for `--fields` | `USAGE`, `field` the argument; a bad `--fields` name lists the selectable fields in `expected` |
| a bug (panic) | exit 101 | `INTERNAL`, exit 1, the message says how to report it; a closed stdout still ends the process by SIGPIPE (a signal exit, outside the registry) |

In text mode every refusal leaves stdout empty and opens stderr with `canon: <CODE>: <message>`, then indented `field:`, `expected:`, `suggestions:` and `hint:` lines (it was `canon <command path>: <CODE> — <message>` and `hint: …`). A failure that carries a human report (exit 1: `canon gate check` red, `canon brief check` with blockers) still prints that report on stdout and no error line, since the report is the answer; the contract's 7.4.1 form is RECOMMENDED, not required.

**Error codes.** The old names are deleted.

| 0.32 | 0.33 | exit |
|---|---|---|
| `PLAN_CHANGED` | `PLAN_STALE` | 2 |
| `INPUT_ELEMENT_FAILED` | `BATCH_INCOMPLETE` (data shape and pointers unchanged; a crashed element's own error is `INTERNAL`) | 1 |
| `INPUT_BATCH_UNSUPPORTED`, `INPUT_INVALID_TYPE`, `INPUT_UNKNOWN_FIELD`, `INPUT_UNREADABLE`, `INPUT_STDIN_TTY` | `INPUT_INVALID`. A fault in the value of `--input` itself has `field` `--input` and `error.x-canon.reason`: `unreadable`, `tty`, `empty`, `syntax` or `batch-unsupported` (the contract's one code cannot say which; splitting the code is an atip/2 candidate). A fault in the document against its fragment schema has a JSON pointer `field` and no reason | 2 |
| `INPUT_STDIN_CONFLICT` | `USAGE`, `field` `--input` | 2 |
| `--input` given twice or without a value (was `INPUT_INVALID`) | `USAGE`, `field` `--input` | 2 |
| `INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE` | unchanged | 2 |
| `INVALID_CURSOR` | `CURSOR_INVALID` (`field` `--cursor`) | 2 |
| `UNKNOWN_FIELD` | `USAGE`, `field` `--fields` | 2 |
| `USAGE` for an unknown command / no command | `UNKNOWN_COMMAND` / `COMMAND_REQUIRED` | 2 |
| (none) | `INTERNAL` | 1 |
| `ADAPTER_REFUSED` used with exit 1 and 2 | `ADAPTER_REFUSED` (2); `ADAPTER_RESPONSE_INVALID` (1) for `canon adapter validate` on an envelope that breaks a rule | 2 / 1 |
| `EVIDENCE_BATCH_REFUSED` used with exit 2 and 1 | `EVIDENCE_BATCH_REFUSED` only for a refused promotion (1); an unwritable directory is `WRITE_UNAVAILABLE` (2), a store problem `EVIDENCE_STORE_UNAVAILABLE` (2) or `EVIDENCE_STORE_FAILED` (1), a write failing after the first record `WRITE_FAILED` (1) | 1 / 2 |

**Exit codes** now come from the code's registry row alone (`canon schema --errors --json`); nothing sets an exit at a call site, and only 0, 1 and 2 exist (signal exits 128+n are outside the registry). Exit 1: the command ran and its answer is negative or incomplete; exit 2: it refused to run. `canon subject move` blocked by a gate violation, `canon brief approve` and `canon evidence add` batches, `canon adapter` and the batch runner no longer compute an exit. `POLICY_APPROVAL_UNREADABLE` is exit 1 (it was registered 2 but `subject move` returned 1). `BRIEF_UNREADABLE` read during `canon brief approve`'s check is exit 2 (it was 1).

`WRITE_FAILED` is exit 1 (a write that began and failed). The same failure known before the first write (a target that cannot be written) is the new `WRITE_UNAVAILABLE`, exit 2, with nothing changed, and the message names the path. Every command that writes checks it up front (the helper is `write_mode.rs`): `subject new/move/adopt/unadopt`, `change new`, `feature new`, `scenario new`, `inventory sync`, `ingest plans/sessions/records`, `report build/snapshot`, `dispatch begin/end`, `context-pack add`, `prompt add`, `plugin sync`, `gate promote`, `gate install-hooks`, `divergence add/accept/defer/resolve`, `ledger compact`, `tier age`, `purge`, `export`, `strategy promote/rollback`, `artifacts store/move`, `skills install`, `evidence add`/`audit`, `finding add/close`, `review`, `task done`, `init`, `demo init/review`, `migrate`, `brief approve` and `policy approve`. A store with its own precondition code keeps it (`ARTIFACTS_STORE_UNREACHABLE`, `EVIDENCE_STORE_UNAVAILABLE`). `ingest plans` now refuses on a read-only repo even when it would have written nothing, because opening its store creates `hot.db`. `canon retrieve` scoped by neither or both of `--regime` and `--domain` (and a `--for-role` that disagrees with `--regime`) is `USAGE` with `field` now; `RETRIEVE_FAILED` stays for a regime key that cannot be derived.

The thirteen exit-2 codes named for a failure during the work (and `WRITE_FAILED`, the fourteenth row), decided one by one (a: the same failure can happen before and during the work, so the code is split; b: only before, stays exit 2; c: only during, becomes exit 1):

| Code | Decision | Split into | Exit |
|---|---|---|---|
| `ARTIFACTS_STORE_FAILED` | a | `ARTIFACTS_STORE_UNREACHABLE` (2) before, `ARTIFACTS_STORE_FAILED` (1, retryable) during | 2 → 1 |
| `ARTIFACTS_STORE_UNREACHABLE` | a | stays for the store that cannot be attached or reached before the work; mid-upload is `ARTIFACTS_STORE_FAILED` | 2 |
| `EVIDENCE_STORE_FAILED` | a | `EVIDENCE_STORE_UNAVAILABLE` (2, retryable) before, `EVIDENCE_STORE_FAILED` (1) during | 2 → 1 |
| `HOOKS_INSTALL_FAILED` | a | `HOOKS_INSTALL_REFUSED` (2) before, `HOOKS_INSTALL_FAILED` (1) during | 2 → 1 |
| `INIT_SKILL_INSTALL_FAILED` | c | raised only after `canon.yaml` was written | 2 → 1 |
| `INVENTORY_FAILED` | a | `INVENTORY_REFUSED` (2) for configuration and lock problems, `INVENTORY_FAILED` (1) for store errors while syncing | 2 → 1 |
| `LEDGER_ARCHIVE_UNAVAILABLE` | b | an up-front check that the ledger root is named `ledger` | 2 |
| `MIGRATE_PINS_FAILED` | c | raised only after files moved and the index was rebuilt | 2 → 1 |
| `PLUGIN_SYNC_FAILED` | b | every error of the run is a resolution error; write errors are `PLUGIN_SYNC_ERRORS` (1) | 2 |
| `POLICY_APPROVAL_FAILED` | a | `POLICY_APPROVAL_REFUSED` (2) before, `POLICY_APPROVAL_FAILED` (1) when writing the approval file | 2 → 1 |
| `RETENTION_EXPORT_FAILED` | a | `RETENTION_EXPORT_REFUSED` (2) before, `RETENTION_EXPORT_FAILED` (1) during | 2 → 1 |
| `RETENTION_PURGE_FAILED` | a | `RETENTION_PURGE_REFUSED` (2) before, `RETENTION_PURGE_FAILED` (1) once `--commit` is deleting | 2 → 1 |
| `RETRIEVE_FAILED` | b | only usage and derivation errors; a store outage still degrades to empty guidance and exit 0 | 2 |
| `WRITE_FAILED` | a | `WRITE_UNAVAILABLE` (2) before, `WRITE_FAILED` (1) during | 2 → 1 |

`MIGRATE_STOPPED` and `CHANGE_ADOPTION_INCOMPLETE` describe work that began, so they are exit 1 now (they were 2): a caller that sees exit 2 may assume nothing changed. A failure between the two records of `canon subject adopt` / `unadopt` is `CHANGE_ADOPTION_INCOMPLETE` (it was `SUBJECT_ADOPT_REFUSED`, exit 2).

**Warning codes.** New, registered with `exit` 1 or 2 and `retryable` false like error codes: `BRIEF_ADOPTION_UNAPPROVED`, `BRIEF_DRAFT_PENDING`, `BRIEF_OVERRIDE_ON_RECORD`, `BRIEF_UNAPPROVED_NOT_ENFORCED`, `DASHBOARD_SNAPSHOT_UNAVAILABLE`, `DISPATCH_DIFF_RECORDS_UNAVAILABLE`, `DISPATCH_RUN_COPIES_DIVERGE`, `DISPATCH_TIER_NOT_PERSISTED`, `LEDGER_KIND_ROUTED_AWAY`, `LEDGER_NO_SCENARIOS`, `LEDGER_RECORDS_UNREADABLE`, `LEDGER_ROLLBACK_NOT_RESTORED`, `LEDGER_UNREADABLE`, `LEGACY_GUIDE_STALE`, `LEGACY_STATE_FOUND`, `OLD_BRIEF_FORMAT_FOUND`, `OLD_LAYOUT_FOUND`, `OVERRIDE_REASON_UNUSED`, `OVERRIDE_RECORDED`, `OVERRIDE_REFUSAL_WAIVED`, `POLICY_MISSING`, `POLICY_NOT_APPROVED`, `POLICY_SPEC_COVERAGE_ABSENT`, `POLICY_SPEC_COVERAGE_INVALID`, `POLICY_UNREADABLE`, `QUERY_PLUGIN_DIAGNOSTIC`, `REFERENCES_UNCHECKED`, `REPORT_TIER_NOT_READ_DIRECTLY`, `RETRIEVE_REGIME_FELL_BACK`, `REVIEW_RECORDS_UNMIGRATED`, `ROLE_UNKNOWN_RECORDED`, `SCENARIO_INDEX_CHECK_FAILED`, `SCENARIO_INDEX_NOT_REFRESHED`, `SCENARIO_INDEX_REFRESHED`, `SCENARIO_SUBJECT_UNKNOWN`, `SEARCH_GITIGNORE_MISSING`, `SEARCH_INDEX_IN_MEMORY`, `SEARCH_INDEX_REBUILT`, `STRATEGY_DEMOTED`, `SUBJECT_OVERRIDE_ON_RECORD`, `SUBJECT_REOPENED`, `TASK_PROOF_IS_CANON_COMMAND`, `TASK_REIMPORT_INCOMPLETE`, `TASK_STATUS_NOT_REFRESHED`, `UNKNOWN_ROLE_IGNORED`, plus the existing `CONFIG_MISSING`, `CONFIG_UNREADABLE`, `DISPATCH_MANIFEST_MISMATCH`, `DISPATCH_MANIFEST_UNREADABLE`, `LAYOUT_MIGRATION_REQUIRED`, `POLICY_APPROVAL_UNREADABLE` and `POLICY_CHANGED_SINCE_APPROVAL` where the same situation was a warning.

**Consumers.** The npm launcher exits 128+n when the binary dies by a signal (it was 1, which canon now uses for a ran-and-found answer). The dashboard reads an absent `error.hint`. The embedded guides, the website (English and Korean), the dogfood evals and `crates/canon-cli/tests/old_names.rs` carry the new names; the conformance corpus was regenerated and differs only by the contract changes above.

### Added
- `atip check --bin <binary> --level L0 --fixture <file> [--json]`, a new workspace crate `atip-check` with the binary `atip`: it runs a built binary and checks the atip/1 L0 contract from outside (argv in; stdout, stderr, exit status and the files it changes out). It runs 19 L0 checks (15 MUST, 4 SHOULD): the 16 of the contract repository's `approaches/checker.md` and `L0.refusal-effects`, `L0.next` and `L0.page`. Each pass condition belongs to exactly one check, so breaking one rule fails one check. Each invocation runs in its own fresh copy of the fixture state with an environment of only `PATH`, `HOME`, `LANG=C.UTF-8` and the fixture's variables, and every check reports `pass`, `fail`, `unprovided`, `unsupported` (does not apply) or `entailed`. A violation seen is a `fail` even when the fixture lacks the entry; `unprovided` is only for a check that saw nothing wrong and lacked `probe` or `registry`. Every MUST check passing exits 0; a failed level exits 1 with `LEVEL_NOT_MET` and the check list in `data`; a refusal exits 2 (`USAGE`, `BINARY_UNUSABLE`, `FIXTURE_INVALID`, `LEVEL_UNSUPPORTED`) and writes nothing. `--level L1` and `L2` are refused with `LEVEL_UNSUPPORTED` until 0.34 and 0.35. A refusal (exit 2) that changes the working directory or `HOME` fails `L0.refusal-effects` and names the paths. A registry the tool prints with `schema --errors --json` is compared with the fixture's. A probe that waits for input on a terminal fails `L0.no-env-switch`. Output on stderr that carries no error or result is reported as a recommendation, not a failure; a signal exit counts in both forms (wait status, 128+n). `atip` follows the contract at L0 itself, ends by SIGPIPE on a closed stdout, and passes `atip check` on itself.
- The checker's tests plant one violation per pass condition (about fifty) in a fake tool (`atip-fake-tool`, behaviour set by a JSON configuration; a test-only binary behind the `test-support` feature, not built by `cargo build -p atip-check` or installed by `cargo install`) and require that the condition's owner, and no other check, fails; the same run with the condition switched off must not fail. The examples of the contract repository, pinned at commit `365244c`, are test input (`crates/atip-check/tests/fixtures/atip-examples/TABLE.md`).
- CI job `atip-l0` and `just atip` run `atip check` at L0 on the canon binary and on `atip`. canon's fixture is `crates/canon-cli/tests/atip/canon.fixture.json`; its `registry` is generated from `canon schema --errors --json` and a test fails when the committed file differs (`CANON_BLESS_ATIP_FIXTURE=1 cargo test -p canon-cli --test atip_check` rewrites it).

### Changed
- The error value (`CliError` and its builder), `redact`, the error-code row type, JSON-mode detection, the one-line envelope output and the shell quoting and splitting of next-step commands moved into a new workspace crate, `atip`, that depends on no canon crate. Output, exit codes and the command spec file are byte-identical to 0.32.0.

## 0.32.0 — 2026-10-31

An agent can record the approval a human gave in the conversation, and release notes move from the README to this file.

### Added
- `canon brief show <change> --body` prints the brief as written, then a `digest: sha256:…` line equal to the digest `canon brief approve` records. `--json` gives `body` and `digest`.
- `--relayed-via <tool>` on `canon brief approve`, `canon policy approve`, `canon divergence accept` and `canon migrate --commit`: an agent role records the human's answer from the conversation (`--approver`, `--note` with the human's words, `--actor` or `CANON_ACTOR`). `brief approve` with `--relayed-via` also needs `--digest <change>=<sha256:…>` per brief, the digest the human was shown; a brief that changed since is refused with `RELAY_DIGEST_MISMATCH` (exit 2, nothing written).
- Relayed approvals are marked: the `Approved:` line reads `(relayed by <role> <actor> via <tool>)`, and `policy.approval.json`, divergence acceptance records and the migrate journal gain a `relay` field. `canon brief show`, `canon status` (`relay` in each brief entry) and the dashboard briefs page show it.
- `approval: { relay: allow | deny }` in `canon.yaml`, default `allow`. With `deny`, a relayed call is refused with `HUMAN_ONLY` naming the key. `canon init` writes the key as a comment.

### Changed
- `canon brief approve`, `canon policy approve` and `canon migrate` accept `--actor`; `canon divergence accept` and `canon migrate` accept `--approver` and `--note`.
- An agent role calling one of the four commands without `--relayed-via` still gets `HUMAN_ONLY`; the hint now describes showing the brief, asking, and recording with `--relayed-via`. The skill guides and `canon status` next steps describe the same procedure instead of "report the command and wait".
- Release notes live in `CHANGELOG.md`; the README keeps the introduction and quick start. Contributors run `just` to list the build, test, lint and gate recipes.

## 0.31.0 — 2026-10-28

Id options lose their `-id` suffix, briefs become plain frontmatter documents, and `canon search` / `canon trace` are new.

### Breaking
- The 14 id options lose `-id` on every command (`--actor-id` → `--actor`, `--change-id` → `--change`; `--repo` stays). Old spellings have no alias and fail with `USAGE` (exit 2).
- `--input` document keys follow the option names (`scenarioId` → `scenario`); old keys fail with `INPUT_UNKNOWN_FIELD` (exit 2). Record, report, error and `--fields` keys keep `*_id` / `*Id`.
- `canon status --json` `statusVersion` is `4`: a subject's `brief` object is replaced by `briefs`, a list with one entry per change the subject answers for.
- A brief is frontmatter, a title, `##` sections (exact English headings) and the `Approved:` line, with no HTML comments; waivers are the frontmatter `waive` list and a chore has `kind: chore`.
- A 0.30 brief gets blocker `brief.old-format`; `evidence add`, `subject move` to ship or reopen, `subject adopt` and `review change` refuse it with `OLD_BRIEF_FORMAT` until `canon migrate`.
- Not-found and invalid input now exit 2; exit 1 stays for check results and refused verdicts. `canon adapter authorize` exits 2 (`ADAPTER_REFUSED`) for a broken envelope or policy file (was 1).

### Added
- `canon search "<text>"` finds records, brief sections, flow steps and scenario text, with neighbors; options `--kind`, `--expand <N>`, `--limit <N>`. No match is exit 0; empty text is exit 2.
- `canon trace <finding key|review id|sha>` prints one review loop in time order, including archived versions. An unknown or invalid operand is exit 2.
- `canon brief approve` takes several change ids; if any is refused the call fails with `BRIEF_APPROVE_REFUSED` (exit 2) and writes nothing. The dashboard briefs page can approve several too.
- `canon migrate` also rewrites 0.30 briefs (`--commit --plan-digest`); a commit that re-binds approvals is human-only (`HUMAN_ONLY` for agent roles).
- A brief with invalid frontmatter is a blocker (`brief.frontmatter-invalid`); `canon context-pack show` / `verify` accept only `sha256:` plus 64 lower-case hex digits.

### Changed
- A subject answers for every adopted change that is tagged, approved or a chore, whatever the adoption order; an unapproved chore blocks reopen and ship. An untagged, never-approved change is only a draft.
- `canon demo init` refuses a directory holding `canon.yaml` (`INIT_CONFIG_EXISTS`); `canon gate install-hooks` refuses a `--repo` that is not a directory (`HOOKS_INSTALL_FAILED`).
- `canon search` / `canon trace` keep a derived index in `.canon/index/`, which `canon init` adds to `.gitignore`; deleting it loses nothing.

### Fixed
- Commands without `--repo` (`canon schema`, `canon skills show`) no longer fail with `CONFIG_MISSING` outside a repo in release builds.
- `canon migrate --revert` refuses when a file the migrate left was deleted since.

### Upgrading
1. Run `canon migrate` on a repo with 0.30 briefs, then have the human run `canon migrate --commit --plan-digest <digest from the preview>`; `canon migrate --revert` undoes it.
2. Rewrite old option spellings in scripts and CI using the table below, and rename `--input` document keys the same way.

<!-- canon:generated removed-031 begin -->
| 0.30 spelling | 0.31 spelling | What changed |
|---|---|---|
| `--actor-id` on `canon dispatch begin`, `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence audit`, `canon feature new`, `canon finding add`, `canon finding close`, `canon review change`, `canon review scenario`, `canon scenario new`, `canon strategy rollback`, `canon subject move`, `canon subject new` | `--actor` |  |
| `--session-id` on `canon dispatch begin`, `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence audit`, `canon feature new`, `canon finding add`, `canon finding close`, `canon review change`, `canon review scenario`, `canon scenario new`, `canon subject move`, `canon subject new` | `--session` |  |
| `--project-id` on `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence approval-payload`, `canon evidence audit`, `canon feature new`, `canon review change`, `canon review scenario`, `canon scenario new` | `--project` |  |
| `--change-id` on `canon finding add`, `canon finding list`, `canon query`, `canon scenario new`, `canon subject adopt`, `canon subject unadopt` | `--change` |  |
| `--run-id` on `canon evidence add`, `canon evidence approval-payload`, `canon evidence audit`, `canon ingest records` | `--run` |  |
| `--task-id` on `canon dispatch begin`, `canon evidence add`, `canon evidence approval-payload` | `--task` |  |
| `--subject-id` on `canon change new`, `canon retrieve`, `canon scenario new` | `--subject` |  |
| `--scenario-id` on `canon evidence add`, `canon evidence approval-payload`, `canon finding add` | `--scenario` |  |
| `--skill-id` on `canon dispatch begin` | `--skill` |  |
| `--parent-run-id` on `canon dispatch begin` | `--parent-run` |  |
| `--evaluator-id` on `canon evidence audit` | `--evaluator` |  |
| `--review-id` on `canon finding add` | `--review` |  |
| `--plugin-id` on `canon query` | `--plugin` |  |
| `--contradicting-trajectory-id` on `canon strategy rollback` | `--contradicting-trajectory` |  |
<!-- canon:generated removed-031 end -->

3. Tools that read `canon status --json` must read `briefs` instead of `brief` (`statusVersion` 4).

## 0.30.0 — 2026-10-27

Stored records get one naming rule and `evidence_record` becomes `evidence`; `canon migrate` moves a 0.29 repo over.

### Breaking
- Every store names a record `kind=<kind>/<key>_<digest8>.json`, and the `area=` level is gone from ledger paths; the same key with different content is refused.
- The kind `evidence_record` is now `evidence` on disk, in the `kind` field, the schema, `canon.yaml` `routing:` and aging keys, and report views.
- A 0.29 repo warns on read commands and refuses writes until `canon migrate`; 0.29 query cursors are rejected.
- A new blocker or should-fix finding needs `--introduced-rev <rev>` or `--introduced-unknown`.

### Added
- `canon migrate` previews the plan and digest; `--commit --plan-digest <digest>` applies it with a backup under `.canon/migrate-backup/`; `--revert` undoes the last migrate.
- `canon finding add --review-id <id>` links a finding to its review (`--review` from 0.31); each review run gets one review id.
- `canon query` marks dangling references, and `canon status --json` counts them in `danglingReferences`.

### Upgrading
1. Run `canon migrate`, review the plan, then `canon migrate --commit --plan-digest <digest from the preview>`.
2. Restart any paged `canon query` from the first page.

## 0.29.0 — 2026-10-25

`canon gate check`, `canon status` and batch `canon evidence add` run much faster on large repos, with identical output.

## 0.28.0 — 2026-10-24

One grammar for the whole CLI, generated from one command spec. Old spellings fail with exit `2` and an `error.hint` naming the replacement; no deprecated aliases.

### Breaking
- Every group needs a subcommand; the primary object is the operand: `canon brief show`, `canon task done add-login#1`, `canon prompt add reviewer@v1`; mode flags became commands.
- One meaning per option: `--kind` record kind, `--role` attributed role, `--actor-id` who acts, `--approver` approving human, `--limit` pages, `--out` → `--output`.
- `--rev <REV>` replaces `--pin`, `--sha`, `--artifact-sha`, `--reviewed-sha`; `--resolution-rev` and `--introduced-rev` replace `--resolution-sha` and `--introduced-by`.
- `--json` prints one envelope and nothing on stderr: `{"ok", "data", "meta"}` or `{"ok": false, "error", "meta"}`; warnings in `meta.warnings`; every report, error and `--input` key is camelCase (`foo_bar` → `fooBar`), except `canon context`'s `policy.*` keys (snake_case, as in `policy.yaml`); the `export` result key `out` is `output`.
- `purge`, `tier age`, `ledger compact`, `artifacts move` and `migrate` preview by default; `--commit --plan-digest <DIGEST>` acts (`PLAN_CHANGED` if the plan moved); their `--dry-run` is removed.
- `brief approve`, `policy approve`, `divergence accept` are human-only (`HUMAN_ONLY`); `brief approve` with blockers fails `BRIEF_APPROVE_REFUSED` (exit `2`); `brief check` with blockers exits `1`.
- Error codes renamed: `ARTIFACTS_MIGRATE_REFUSED` → `ARTIFACTS_MOVE_REFUSED`, `EVIDENCE_VAULT_FAILED` → `ARTIFACTS_STORE_FAILED`, `FINDING_RESOLUTION_SHA_UNEXPECTED` → `FINDING_RESOLUTION_REV_UNEXPECTED`.
- Error codes removed: `PURGE_PLAN_REQUIRED`, `FINDING_RESOLUTION_SHA_REQUIRED`, `FINDING_UNRESOLVABLE_COMMIT` (an unknown revision is `NO_COMMIT`).
- Exit changes: every `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` exits `2`, as do `init --check` of a malformed config and `task done` on an unknown task.
- `canon context` keys record kinds by the name `--kind` takes (`evidence`, not `evidence_record`); `finding close` reports a repeat with `meta.unchanged: true` instead of a report `unchanged` key.
- `--project-id` only with more than one spec root; `schema --all` is gone (use `--as <role>`); `skills install` writes a short `SKILL.md` stub; an empty check reports `UNVERIFIABLE` (exit `1`).

### Added
- `--input` on write commands takes arguments as JSON: `--input '{"operands": ["auth.login.01"]}'`, `--input file.json` or `--input -`; keys are camelCase option names plus `operands`.
- `canon skills show <NAME> [--full]` prints the embedded guide; `canon schema <command> --json` shows effect, roles, exits, errors; `schema --errors --json` lists every error code.
- `canon context` gains a `commands` index (filtered by `--as`); `--fields` takes camelCase or snake_case, an unknown name is `UNKNOWN_FIELD` (exit `2`).
- `--repo <DIR>` is declared once for all commands (default: nearest `canon.yaml`); `--session-id` joins `--actor-id`; `--override-reason` is the only "proceed although refused" flag.
- `canon subject move --to` changes a subject's state (`subject show` replaces `subject status`); `evidence add --all-scenarios --report junit:a.xml` is the batch form.
- Uncommitted code is recorded, not refused: records carry a `code_tree` and `worktree_dirty`, and `canon.yaml` `code.exclude` (default `docs/**`, `*.md`) says what is not code.
- Destructive commands report `planDigest`, `effects[]`, `committed`; commands needing a commit refuse with `NO_COMMIT`; `canon migrate` rewrites hooks, old skill and `AGENTS.md`, reports `backupDir`.

### Changed
- Repeating an identical write is a no-op with `meta.unchanged` (exit `0`); `divergence accept` defaults `--role` to `reviewer`; `init --check` with a failing section exits `1`.

### Removed
- `canon gate task` (use `canon task done`), the `canon fmt` alias, `scenario new --from`, `review add --scenario-ids-from`, and the other spellings in the table below.

### Upgrading
1. Run `canon migrate`, then `canon migrate --commit --plan-digest <DIGEST>` (`meta.next[0].argv` has it with `--json`); backups go to `.canon/migrate-backup/<timestamp>/`.
2. Refresh the skill stub: `canon skills install`.
3. Rewrite old spellings in your scripts using the table below.

<!-- canon:generated removed begin -->
| 0.27 spelling | 0.28 spelling | What changed |
|---|---|---|
| `canon artifacts migrate` | `canon artifacts move --tier` | the bare run previews; `--commit --plan-digest` acts |
| `canon brief` (bare) | `canon brief show` | a group is never runnable; the change is an optional operand |
| `canon context-pack new` | `canon context-pack add --manifest` |  |
| `canon demo add` | `canon demo review` |  |
| `canon divergence promote` | `canon gate promote --kind divergence` |  |
| `canon divergence status` | `canon divergence list` |  |
| `canon evidence vault` | `canon artifacts store` |  |
| `canon fmt` | `canon format` | the `fmt` alias is gone; the directory is an optional operand |
| `canon gate selftest` | `canon selftest --suite trust-spine-gate` |  |
| `canon gate task` | `canon task done` | the task id is the operand; installed hooks are rewritten by `canon migrate` |
| `canon ingest artifacts` | `canon ingest records` | "artifact" now means the blob store only |
| `canon learn promote` | `canon strategy promote` | the strategy id is the operand |
| `canon learn approve` | `canon strategy approval-payload --evaluation --approver --approval-at` | the strategy id is the operand; `approve` is reserved for commands that record a human approval |
| `canon learn rollback` | `canon strategy rollback` | the strategy id is the operand |
| `canon regime-key` | `canon regime show --role-segment --repo-name --area --hash` |  |
| `canon report` (bare) | `canon report build` | `report check` and `report snapshot` are the other two modes |
| `canon review add` | `canon review scenario` | split by what it reviews: `review change` reviews a chore change |
| `canon subject status` | `canon subject show` | `subject move --to` changes the state |
| `--check` on `canon report` | `canon report check` |  |
| `--snapshot` on `canon report` | `canon report snapshot` | the output directory is the operand |
| `--check` on `canon format` | `canon format` | `format` always validates; drop the flag |
| `--check-config` on `canon init` | `--check` |  |
| `--change-id` on `canon brief show`, `canon brief check`, `canon brief approve` | `canon brief show` | the change is the operand |
| `--change` on `canon brief show`, `canon brief check`, `canon brief approve` | `canon brief show` | the change is the operand |
| `--change-id` on `canon finding close` | `canon finding close` | the change is the operand |
| `--run-id` on `canon dispatch end` | `canon dispatch end` | the run is the operand |
| `--subject-id` on `canon subject adopt`, `canon subject unadopt` | `canon subject adopt` | the subject is the operand and the change is `--change` |
| `--name` on `canon prompt add`, `canon prompt show` | `canon prompt add` | name and version are one operand, name@version |
| `--version` on `canon prompt add`, `canon prompt show` | `canon prompt add` | name and version are one operand, name@version |
| `--scenario-id` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon divergence accept` | `canon divergence add` | the scenario is the operand |
| `--scenario-id` on `canon review scenario` | `canon review scenario` | the scenarios are operands |
| `--scenario-ids-from` on `canon review scenario` | `--input` | a batch is an `--input` array |
| `--change-id` on `canon review scenario` | `canon review change` | a chore change is reviewed with `review change` |
| `--scenario-id` on `canon review change` | `canon review scenario` | scenarios are reviewed with `review scenario` |
| `--scenario-ids-from` on `canon review change` | `canon review scenario` | scenarios are reviewed with `review scenario` |
| `--change-id` on `canon review change` | `canon review change` | the change is the operand |
| `--from` on `canon scenario new` | `--input` | a batch is an `--input` array with `operands` per element |
| `--all` on `canon schema` | `canon schema` | the listing is unfiltered; `--as` filters it by role |
| `--agent-id` on `canon dispatch begin` | `--actor` |  |
| `--reviewer` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon divergence accept`, `canon finding add`, `canon review scenario`, `canon review change` | `--actor` | it is the reviewer, and required where `--reviewer` was |
| `--by` on `canon brief approve`, `canon policy approve` | `--approver` |  |
| `--principal` on `canon strategy approval-payload` | `--approver` |  |
| `--approval-by` on `canon evidence add`, `canon evidence approval-payload` | `--approver` |  |
| `--approved-at` on `canon strategy rollback` | `--approval-at` |  |
| `--signature` on `canon strategy promote` | `--signature-file` |  |
| `--approval-signature-file` on `canon evidence add` | `--signature-file` |  |
| `--context-manifest` on `canon dispatch begin` | `--manifest` |  |
| `--sha` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve` | `--rev` | the commit the record is about; default HEAD |
| `--pin` on `canon review scenario`, `canon review change` | `--rev` | default HEAD |
| `--artifact-sha` on `canon evidence add`, `canon evidence audit`, `canon evidence approval-payload` | `--rev` | default HEAD |
| `--reviewed-sha` on `canon finding add` | `--rev` | default HEAD |
| `--resolution-sha` on `canon finding add`, `canon finding close` | `--resolution-rev` | default HEAD when fixed |
| `--introduced-by` on `canon finding add` | `--introduced-rev` | no default |
| `--out` on `canon export` | `--output` | `-o` is the short form |
| `-k` on `canon retrieve` | `--limit` |  |
| `--k` on `canon retrieve` | `--limit` |  |
| `--role` on `canon retrieve` | `--for-role` | `--role` means only attribution |
| `--kind` on `canon evidence add` | `--class` | an external re-audit is `evidence audit`, not `--class audit` |
| `--kind` on `canon change new` | `--change-kind` | `--kind` is reserved for record kinds |
| `--status` on `canon dispatch end` | `--run-outcome` |  |
| `--status` on `canon divergence add` | `--state` |  |
| `--outcome` on `canon review scenario`, `canon review change` | `--review-outcome` |  |
| `--source` on `canon skills install`, `canon skills check`, `canon skills doctor` | `--skills-source` |  |
| `--source` on `canon ingest plans` | `--plan-source` |  |
| `--role` on `canon regime show` | `--role-segment` |  |
| `--repo` on `canon regime show` | `--repo-name` | the repo segment of the key; `--repo` names a directory on other commands |
| `--to` on `canon artifacts move` | `--tier` |  |
| `--target` on `canon skills install`, `canon skills check`, `canon skills doctor` | `--repo` | the repo is `--repo` on every command; omit it to use the nearest directory holding canon.yaml |
| `--canon-yaml` on `canon ingest sessions`, `canon query`, `canon tier age` | `--repo` | the repo's canon.yaml is found from `--repo`, or from the nearest directory holding one when it is omitted |
| `--dry-run` on `canon artifacts move`, `canon ledger compact`, `canon migrate`, `canon purge`, `canon tier age` | `--commit --plan-digest` | the bare run previews; `--commit --plan-digest` acts |
| `--plan` on `canon purge` | `canon purge` | the bare run previews; `--commit --plan-digest` acts |
| `--confirm` on `canon purge` | `--commit --plan-digest` | the bare run previews; `--commit --plan-digest` acts |
| `--evaluator-id` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--evaluator-version` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--evaluator-digest` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--audit-result` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
<!-- canon:generated removed end -->

4. Scripts reading `--json`: read `data`, use camelCase keys, treat exit `1` (finding) and `2` (refused) differently.
5. Use `--rev` (default `HEAD`) instead of `--pin`/`--sha`; use the bare preview then `--commit --plan-digest` instead of `purge --confirm`/`--dry-run`.

## 0.27.0 — 2026-10-22

Two safety fixes: a gate can no longer pass without canon state, and read commands no longer write.

### Breaking
- A command that needs canon state and finds none refuses with `CONFIG_MISSING` (exit `2`); before, `canon gate check` without `canon.yaml` printed `clean` and exited `0`.

### Fixed
- Commands that `canon schema` marks `effect: read` leave the tree unchanged (`status`, `gate check`, `report --check` and `query` reads no longer write).
- `query` fails with an unavailable-tier error when the hot path is a directory or not a SQLite file; `format` refuses a corpus root that is not a directory.

## 0.26.0 — 2026-10-22

`brief check` now reads a brief in the language it is written in, judges Intent and Questions, and adds six advisory prose rules.

### Added
- `brief check` detects the brief's language from its prose (`ko`, `en`; `ja` and `zh` get structure rules only); JSON gains `languageSource` and `languageNote`.
- Six advisory prose rules (`prose.code-citation`, `.parenthetical`, `.decision-history`, `.ai-vocabulary`, `.ko-ending`, `.length`), waivable in frontmatter; they never block `brief approve`.
- A brief with advisories but no blockers gets a polish step (`reference/brief-polish.md`, then `canon brief check`) in `canon status` and `meta.next` before the human step.
- The briefs page shows advisory totals, a per-brief badge, language, and findings grouped by rule.
- A `--project-id` naming no spec root in `canon.yaml` is refused with `UNKNOWN_SPEC_ROOT` (exit `2`) before anything is written.

### Changed
- `language:` in `canon.yaml` now overrides detection instead of being the only source.
- `canon init` and `canon migrate` ignore lines also cover `.canon/dashboard-snapshot/`, `.canon/learn/`, `.canon/r2/` and `.canon/archive/kind=scenario/`.

### Fixed
- The dashboard drains the request before closing, so a client always receives an early refusal.

### Upgrading
1. In an existing repo, run `canon migrate` once to add the new ignore lines.
2. Polish briefs that now report advisories, or waive a finding with a reason.
3. Fix scripts that pass a `--project-id` naming no configured spec root; they now exit `2`.

## 0.25.0 — 2026-10-21

Brief approval holds back only the change it approves, `canon dashboard` works in any repo and can approve briefs, and 0.24.0's deprecated spellings are removed.

### Breaking
- Removed 0.24.0 spellings: `--change`, `--project`, `--subject`, `--actor`, `--run` gain `-id`; `--kind evidence_record` becomes `--kind evidence`.
- `canon finding close --disposition rejected|deferred` now requires `--reason` (`FINDING_REASON_REQUIRED`).

### Added
- `canon dashboard` runs in any repo (binary embeds the app; `--port` if taken); `/briefs.html` approves briefs like `canon brief approve --change-id <id> --by <name>`, refused if changed or blocked.
- `canon subject unadopt <change> --subject-id <id>` moves a draft change off a subject; `change new --subject-id` and `subject adopt` warn when the subject is building, verifying or shipped.
- `finding close --reason` is stored as an optional `reason` on the closing version and shown in `finding list`.

### Changed
- Evidence needs approval only from the changes a scenario's `@flow:<change>.<F-id>` tags name (untagged: the subject's newest approved change); an untagged pending draft is outside scope.
- `canon skills install` deletes unedited files the bundle no longer ships and reports edited ones; `skills check` (`SKILLS_DRIFT`) and `skills doctor` name each orphan.

### Upgrading
1. Replace the removed spellings in scripts and add `--reason` to `finding close --disposition rejected|deferred`.
2. Run `canon skills install` once to prune files 0.24 shipped that 0.25 no longer does.

## 0.24.1 — 2026-10-21

A fixed blocker now re-verifies only the scenarios it names.

### Added
- `canon finding add` takes a repeatable `--scenario-id` (`FINDING_UNKNOWN_SCENARIO` for an unknown id), stored as an optional `scenario_ids` list on the Finding record; older records read unchanged.

### Changed
- After a `fixed` close of a finding with `scenario_ids`, only the named scenarios with evidence need newer evidence; `gate check`, ship and `status` agree.

## 0.24.0 — 2026-10-19

Each concept gets one flag name across every command, and evidence batches bind each attachment to the scenario it belongs to.

### Breaking
- `evidence add --all-scenarios` refuses a bare `--artifact <path>`; use `--artifact <scenario-id>=<path>` or let the report carry attachments per test.
- A report attachment that is missing or over the size limit is refused, naming the case and path; nothing is written.

### Added
- Reports can name which file belongs to which test: JUnit `[[ATTACHMENT|<path>]]` lines in `<system-out>` and Cucumber JSON `embeddings`.
- `canon schema <command>` lists deprecated spellings as `deprecatedAliases`.

### Changed
- Flags whose value is a record or entity id become `--<noun>-id`: `--change`, `--project`, `--subject`, `--actor`, `--run` → `--change-id`, `--project-id`, `--subject-id`, `--actor-id`, `--run-id`.
- `--kind` values of `query`, `export` and `purge` drop the `_record` suffix (`evidence_record` → `evidence`); the record's own `kind` field, ledger paths and `canon.yaml` routing keys keep it.
- The old spellings still run until 0.25.0, warning on stderr and adding `meta.deprecated` to JSON; `<kind>_record` is accepted for every `--kind`.

### Upgrading
1. Switch scripts to the new flag names and `--kind` values; the old ones warn until 0.25.0.
2. If a batch passed screenshots as bare `--artifact`, let the report carry them per test or use `--artifact <scenario-id>=<path>`.

## 0.23.0 — 2026-10-18

The working set keeps only current records: superseded versions move to `.canon/archive`, evidence stops carrying attachments, reviews pin verdicts and checks, and artifact bytes can live off git.

### Breaking
- `evidence add --drop-attachment` and `--drop-reason` are removed (exit 2), with the evidence `attachment_drop` field and the `EVIDENCE_CARRY_BLOB_MISSING` code; evidence binds only the files its own write names.
- Review JSON `pins.evidence: [{key, digest}]` becomes `pins.checks: [{role, verdict, checks: [...]}]`; `Review.scenario_id` is optional, `change_id` is new.
- `canon query` reads the ledger only; add `--history` to include superseded versions; `divergence resolve|defer|accept` stage and promote in one run (`--no-promote` stages only).
- Reviews predating 0.23 pins do not count until `canon migrate` pins them; a transition needing the review is refused as `unreviewed-promotion`.

### Added
- `.canon/archive/kind=<k>/…` holds superseded versions (one ledger file per key); `canon query --kind <kind> --history` returns all; `canon ledger compact [--dry-run]` moves them once.
- `canon.yaml` `artifact_store: { backend, prefix }` (`local` default, or an `s3` tier); `canon artifacts verify [--full]` checks bound files; exit 1 missing or altered, 2 store unreachable.
- `canon artifacts migrate --to <tier> [--dry-run]` uploads blobs, writes `artifact_store:`, removes local copies and git-ignores `.canon/artifacts/`; no way back.
- `canon review add --change <slug>` reviews a chore as a whole; `canon migrate` re-pins still-current reviews and accepted divergences.

### Changed
- A review pins its scenario digest plus per-role latest verdict and check identity; a chore is satisfied by one change-level review or current signed reviews of every evidenced scenario.
- Evidence, finding and divergence stage under `.canon/ledger/_staging/` and are committed by `canon gate promote`.

### Upgrading
1. Run `canon migrate` once and commit the result (CI needs `fetch-depth: 0`); until then old reviews do not count.
2. Remove `--drop-attachment`/`--drop-reason` from scripts; add `--history` to `canon query` calls reading superseded versions; read `pins.checks`.
3. Optional: `canon artifacts migrate --to <tier>` moves artifact bytes off git.

## 0.22.0 — 2026-10-17

Gates fail closed on policy drift and unverified fixes, a tracked known gap can be accepted without a false `faithful` verdict, and every refusal has a hint. 0.21 deprecated names are removed.

### Breaking
- Removed (usage errors now): `context-pack create`, `prompt register`, `divergence stage`, `demo attest`, two-positional `subject status <id> <state>`; `meta.deprecated` is gone.
- `canon gate check` exits 1 and `subject status --to shipped`/reopen is refused (`POLICY_CHANGED_SINCE_APPROVAL`, `POLICY_APPROVAL_UNREADABLE`) when the policy differs from its approved digest.
- A blocker closed `fixed` keeps blocking (`open-blocker`, waivable with `--override-reason`); an approved chore refuses shipping (`uncovered-cell`) until every evidenced scenario has newer evidence.
- While the brief has blockers, `canon status` shows `canon brief check --change <slug>`, not the human stop.

### Added
- `canon divergence accept --project-id <root> --scenario-id <id> --reason <text> --tracking <ref> [--expiry YYYY-MM-DD] --reviewer <id> --role reviewer` accepts a `divergent` scenario as a gap.
- `canon policy show [--json]` prints the policy digest and approval state (`none`, `approved`, `unapproved`, `changed`, `unreadable`).
- `canon change new` on a shipped subject prints the reopen command.

### Changed
- `canon brief approve` writes RFC 3339 UTC time (date-only `Approved:` lines still read); every refusal has a `hint` (stderr `hint: …`; JSON `error.hint` string).
- `meta.next` after writes follows the same brief-first rule as `status`.

### Upgrading
1. Switch scripts to the 0.21 names (`context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`); drop any branch reading `meta.deprecated`.
2. If `canon gate check` now exits 1 on policy, have the human run `canon policy approve --by <human> --note "<words>"`, or restore the approved policy files from git.
3. After a blocker closed `fixed`, or an approved chore, re-record with `canon evidence add --all-scenarios …` and re-review before `--to shipped`.

## 0.21.1 — 2026-10-16

Patch release: clearer refusals for outside-the-repo evidence, and `--reason` on every status transition.

### Added
- `canon subject status <id> --to <state> --reason "<text>"` is accepted on any transition, stored as optional `status_reason`. Reopen still requires `--reason`.

### Changed
- Refusals for evidence/review files outside the repository now carry a hint (copy it under the repository) and JSON `error.field` names the flag.

### Removed
- Error code `SUBJECT_REASON_UNEXPECTED`; drop any branch that handled it.

## 0.21.0 — 2026-10-16

One verb grammar, `meta.next` on every write so agents need not call `canon status`, declared agent roles, a core-plus-role skill, and attachments that carry forward on re-attestation.

### Breaking
- `canon evidence add` no longer refuses with `EVIDENCE_REFUSED` when earlier attachments are not repeated; they carry forward. An identical repeat returns unchanged (exit 0).
- Write commands answer `{ data, meta }`; read `.data` (e.g. `dispatch begin` `run_id` is `.data.run_id`). Write output ends with `next: <command> — <why>`; `status`/`meta.next` steps gain `role`.
- `review add` without a provenance flag and invalid enum values are `USAGE` errors; `CANON_ROLE` outside `planner`, `implementer`, `reviewer`, `orchestrator` is `UNKNOWN_ROLE`.

### Added
- Verb grammar: `canon context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>` (without `--to` it shows).
- `meta.next: [{ command, why, actor, role }]` on every successful write; roles via `CANON_ROLE` or global `--as <role>` (flag wins); other roles' steps become `meta.waitingOn`.
- Attachments carry forward (`carried`); a named file replaces its binding; `--drop-attachment <path> --drop-reason <text>` drops one; `EVIDENCE_CARRY_BLOB_MISSING` (exit 2) if a blob is lost.
- `canon schema` gives each command `roles`, `example`, `requiredOneOf`; bare `canon schema` lists the declared role's commands, `--all` all; `--fields a,b` on `status`, `export`, `context`.
- The skill is a shared core plus `reference/role-*.md` per role; `--role` on `evidence add`/`review add` defaults to the declared role.

### Changed
- Deprecated until 0.22.0 (stderr warning): `context-pack create`, `prompt register`, `divergence stage`, `demo attest`, `subject status <id> <state>`; `subject status`, `format` are `write`.

### Upgrading
1. Run `canon skills install` to get the 0.21 skill.
2. Switch to the new spellings before 0.22.0 (`context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`).
3. Read `.data` and `.meta` of write JSON; match the leading lines of write output, since `next:` is now last.

## 0.20.0 — 2026-10-15

Agents can drive the CLI without `--help`: `canon schema` describes every command, all take `--json`, every refusal has a stable code, lists are paged, and `purge` deletes only a reviewed plan.

### Breaking
- `canon purge --dry-run` is removed; use `--plan`, then `--confirm <planDigest>`. A bare `purge` is refused with `PURGE_PLAN_REQUIRED`. `canon learn request` is removed; use `canon learn approve`.
- Stderr errors are `canon <command path>: <CODE> — <message>` plus optional `hint:`; match the code, not old `refused —` forms. A multi-violation refusal is one error.
- `query`, `finding list`, `export`, `divergence status` return at most 100 records by default; pass `--all` or follow `nextCursor`.
- A closed stdout (`canon … | head`) ends canon by SIGPIPE (exit 141) instead of panicking with exit 101.

### Added
- `canon schema [<command path>…] [--json] [--errors]` describes the command tree; unknown path is `UNKNOWN_COMMAND`. Exit codes keep meaning (0 ok, 1 gate/operational, 2 refusal/usage).
- Every command accepts `--json`: `{ "data", "meta" }` or `{ "error": { "code", "message", "hint", "retryable" } }`; `statusVersion` stays `3`. Errors have stable codes; clap usage errors are `USAGE`.
- `query`, `finding list`, `divergence status`, `export` take `--limit <n>` (default 100), `--cursor`, `--all`; `meta` has `hasMore`, `nextCursor`; `--fields a,b,c` on `query`, `finding list`.
- `canon purge --kind <kind> --before <time> --plan` prints a `planDigest`; `--confirm <digest>` deletes only if the plan is unchanged, else `PLAN_CHANGED`.

### Fixed
- `canon … | head` no longer panics; `canon schema "evidence add"` (quoted path) is accepted.

### Upgrading
1. Run `canon skills install` to get the 0.20 skill.
2. Replace `canon purge --dry-run` with `--kind <kind> --before <time> --plan`, then rerun with `--confirm <planDigest>`; replace `canon learn request` with `canon learn approve`.
3. Scripts reading all records from `query`, `finding list`, `export`, `divergence status` must pass `--all` or follow `meta.nextCursor`; match error codes, not `refused —`.

## 0.19.0 — 2026-10-15

The brief becomes a PRD that canon can check: a fixed skeleton with numbered flow steps, `canon brief check`, a digest-bound approval, flow-step coverage, a chore brief, and a configurable language.

### Breaking
- `canon change new` no longer writes `tasks.md`; an existing one is still read, and `gate task` and `evidence add --task` are unchanged for imported plans.

### Added
- `canon change new` writes a fixed brief skeleton (Intent … Out of scope) with flow ids `F1`, `F2`, …; `--kind chore` writes a short brief.
- `canon brief check [--change <slug>] [--json]` reports blockers (`brief.*`) and advisories (`prose.*`); exit 2 on blockers; `<!-- canon:waive <rule-id> <section-key> "<reason>" -->` waives one rule.
- `canon brief` prints the brief's state; `canon brief approve --change <slug> --by <human> --note "<words>"` binds `Approved:` to the brief's sha256 and refuses while blockers remain.
- `canon scenario new --flow <F-id>` tags it `@flow:<change>.<F-id>`; `canon status` lists each flow step with its scenarios and review state (`--json` adds `flow[]`).
- `canon init --language <tag>` writes `language:` to `canon.yaml`; templates and prose rules ship for `en` and `ko`.
- The skill gains five references: `brief-shape`, `brief-clarify`, `brief-harden`, `brief-align`, `brief-polish`.

### Changed
- A brief changed after approval reads as unapproved (`BRIEF_CHANGED_SINCE_APPROVAL`); the ship guard also refuses on brief blockers.
- An `Approved:` line without a digest stays approved with an advisory; coverage rules no longer fire on a new scope before it is approved.

### Upgrading
1. Run `canon skills install`, and re-approve briefs whose `Approved:` line has no digest with `canon brief approve`.
2. Make scripts that read `tasks.md` treat it as optional.

## 0.18.0 — 2026-10-13

One `evidence add` now attests every scenario a test report covers, one `review add` reviews a batch, a shipped subject can be reopened in its approved scope, and a change can have several subjects.

### Breaking
- `Change.subject_id` is now `subject_ids`, a list (`Change` schema generation `2`); generation-1 records read as a one-element list.
- `canon query --kind change` returns only each change's current version; it used to return every version.

### Added
- `canon evidence add --project-id <root> --all-scenarios --report <fmt>:<path> …` writes one record per indexed scenario with a case in the report; all or nothing.
- `canon review add` takes repeated `--scenario-id` and `--scenario-ids-from <file|->` for an all-or-nothing batch with one reviewer and pin.
- `evidence add --task --report` with no scenario id binds every case in the report; `--report swift-testing:<path>` reads `swift test --experimental-event-stream-output`.
- `canon subject status <id> --to verifying --reason "<why>"` reopens a `shipped` subject; outside the approved scope it exits 1 naming the brief to approve.
- `canon subject adopt <change> --subject <other>` adds a second subject to a change; `canon status` flags a shipped subject whose ship guard would now fail (`statusVersion` stays `3`).
- `canon finding list --change-id <id> [--open] [--json]` prints a change's committed findings; `canon skills install` and `doctor` end with an `owned-paths:` list of globs for ignore lists.

### Changed
- `--reason` is required for a reopen and refused on any other transition (exit 2); `canon skills check` reports a formatter-only rewrite as `formatter-rewritten`, not `stale` (still exit 1).
- Fabrication markers `"would pass"`, `"TBD"`, `"n/a"` match only as whole words; `canon report` fails (exit 1) on a non-sqlite `hot` db file.

### Upgrading
1. Run `canon skills install` to refresh the skill and see the `owned-paths:` ignore list.
2. Make scripts expect one row per change from `canon query --kind change`, and match `formatter-rewritten` as well as `stale` from `canon skills check`.

## 0.17.0 — 2026-10-12

canon keeps its specs, its changes and every approval under `.canon/`, reads external plans without writing into them, asks which harnesses get the skill, and `canon migrate` moves a 0.16 repo over.

### Breaking
- The spec layout drops the `features/` level and the default spec root is `.canon/specs` (was `specs`); a root still holding `features/` is refused, naming `canon migrate`.
- `Approved:` is read only from `.canon/changes/<change>/brief.md`; an external plan gets a pointer `brief.md` (`Source: <path>`).
- `canon status --json` is `statusVersion` 3: the `brief` object is `{approved, enforced, brief?, briefExists, source?, …}` and `proposal` is gone.
- `canon change new` writes `.canon/changes/<slug>/brief.md` and `tasks.md` instead of `openspec/changes/<slug>/`; `canon init` no longer creates `openspec/changes/`, and `plans:` is `sources: []`.
- Skill providers are no longer inferred from `.agents` or a Claude+Codex fallback; with no install lock and only `.agents`, pass `--providers`.

### Added
- `canon migrate [--dry-run] [--repo <dir>]` moves a 0.16 or earlier repo to the 0.17 layout, idempotently; it exits 2 and writes nothing on uncommitted changes under a path it moves.
- `.canon/changes/` is a native plan source (dialect `canon`), read first by `ingest plans`, `gate task`, `evidence add --task`, `dispatch --task` and `status`.
- `canon.yaml` gains `skills: { providers: [claude, codex, omp, pi] }`; `canon init` asks which harnesses get the skill, or takes `--providers a,b|none`; `skills doctor` flags `stray:`.

### Changed
- `canon skills install|check|doctor` follow `skills.providers`, else the install lock, then detected harnesses; `--providers` on install rewrites `canon.yaml`.
- `canon init` on an existing repo also adds missing `.gitignore` lines.

### Upgrading
1. Install 0.17.0, run `canon migrate --dry-run`, commit anything uncommitted under a path it moves, run `canon migrate`, and commit the result.
2. Run `canon skills check` and `canon skills doctor`; update `canon status --json` readers from `brief.proposal` to `brief.brief`, `brief.briefExists`, `brief.source`.
3. Record approvals in `.canon/changes/<change>/brief.md`; repos with `{ dialect: openspec, root: . }` keep it as an external source.

## 0.16.0 — 2026-10-12

After one human approval the agent can drive on its own: one approval per scope, promote on write, a self-refreshing scenario index, policy levels, review artifacts, and checked legacy reviews.

### Breaking
- `evidence add` under an unapproved brief and `subject status --to shipped` without `Approved:` refuse with `unreviewed-promotion <subject>` (exit 1); `--override-reason` waives.
- `gate check` and shipping exit 1 with `uncovered-cell <root>` when a changed root's index refresh is refused.
- `canon status --json` is `statusVersion: 2` (`next[].actor`, `subjects[].brief`, `policy.level`); the human step text changed.
- The `strict` starter policy now begins `# canon policy level: strict` (new `policy_digest`); `canon skills check` exits 1 when the lock's `canon_version` is missing or not this canon's.

### Added
- One approval per scope: only a new subject or scope change needs the human; `Approved:` line plus `canon policy approve --by <human> --note "<words>"` approves both.
- `canon evidence add --override-reason <line>` attests despite a brief refusal; the record gains `brief_override`, listed by `status` and `gate check` without failing.
- `--promote`/`--no-promote` on write commands; `write: { promote: auto }` in `canon.yaml` promotes on write (`canon init` sets it for new repos).
- A stale scenario index is rebuilt from `.feature` files by `gate check`, `status`, `subject status`, `review add`, `evidence add`; removed scenarios get a `removed: true` generation.
- `canon init --policy light|standard|strict` (default `strict`); `review add --artifact <path>` stores files under `.canon/artifacts/sha256/`; the skills lock records `canon_version`.

### Changed
- Pre-0.15 reviews are checked against the scenario at the pinned commit (undecidable is never a violation); `evidence add --task` and `gate task` read `tasks.md` as is.

### Upgrading
1. Re-run `canon skills install` (with your `--providers`) and `canon init` (refreshes the `AGENTS.md` block); until then `skills check` exits 1.
2. Add `.canon/audit/`, `.canon/scenario-index.lock`, `.canon/artifact-store.lock` to `.gitignore`; set `fetch-depth: 0` on the CI checkout running `gate check`.
3. Record each brief approval as an `Approved:` line or pass `--override-reason`; update scripts matching `status` step text or parsing `--json`; optionally add `write: { promote: auto }`.

## 0.15.0 — 2026-10-11

Reviews pin what they saw, re-attesting cannot silently drop bound files, external audits sit beside declared evidence, and the policy gets an approver.

### Breaking
- A new latest `canon evidence add` record omitting files the previous one bound is refused (exit 2); re-bind them or pass `--drop-attachment <path>` with `--drop-reason <text>`.
- The failure-class list has ten classes: `false-success` is added; a latest `refuted` audit is `false-success`, refused by the ship guard even with `--override-reason`.
- Under `require_review`, only a `signed` review qualifies, and a pinned one only while its pins match; otherwise `unreviewed-promotion` ("stale review").

### Added
- `canon review add` pins the scenario digest and latest evidence records; `--outcome signed|indeterminate|unavailable` (default `signed`), `--reviewer-provider|-model|-harness`.
- `canon evidence add --kind audit` records an external re-check (`--evaluator-id|-version|-digest`, `--audit-result confirmed|refuted|indeterminate`).
- `spec_coverage.require_audit: { rate, seed, scope }` samples scenarios deterministically; a sampled one without a confirmed audit is `uncovered-cell`.
- `canon policy approve --by <who> [--note]` writes `.canon/policy.approval.json`; `gate check` and `status` advise while the policy is unapproved or changed.
- `canon status` counts stale/unsigned reviews and audits (`--json`: `staleReviews`, `unsignedReviews`); `gate task` warns when evidence has no attachment and its ref is a canon command.

### Changed
- Pre-0.15 reviews carry no pins and keep qualifying; `finding add|close` resolve short SHAs (unknown refused); the access audit is written beside the `canon.yaml` read.

### Upgrading
1. Re-review (`canon review add`, same reviewer) to pin a scenario; until then it can change without the review going stale.
2. Have a human run `canon policy approve --by <who>` and commit `.canon/policy.approval.json`.
3. Update scripts that parse the failure-class list or re-attest with fewer files; re-run `canon skills install`.

## 0.14.0 — 2026-10-11

The skill becomes a working loop for agents, `canon init` sets that loop up, bound evidence stays provable, units and reviews carry a session, and `canon status` says what to run next.

### Breaking
- Write-command success lines now end with the write mode (e.g. ``— written directly; nothing to promote``); `finding add --introduced-by self` is refused (use `--reviewer <me>`).
- `evidence add --report` refuses a `faithful` verdict when a bound case failed; `require_review.distinct_actor` also rejects a review whose session equals an evidence session.

### Added
- `canon status [--json]` lists subjects, gaps, policy warnings, ends with `next:`; `canon change new <slug> --subject <id> --title <t>` scaffolds, imports, adopts a change.
- `canon init` writes a starter `.canon/policy.yaml`, an `AGENTS.md` block and an `openspec` plans source (`--no-agents-md`, `--no-policy` opt out); `--session-id` on evidence/review/finding.
- `evidence add --artifact/--report` copies bound files into git-tracked `.canon/artifacts/`; `canon evidence vault` backfills; `--max-artifact-mib <N>` raises the 25 MiB limit.

### Changed
- `gate check` re-checks bound files against stored blobs (`stale-evidence`); a report case binds a scenario only by its own id; `gate task` re-imports the flipped change.
- Re-running `canon init` leaves `canon.yaml` alone, refreshes only the `AGENTS.md` block, exits 0; the skill opens with an 8-step loop.

### Fixed
- `gate task` is no longer slow with large plans-root directories; a plan import keeps the change's `subject_id` when the plan names none.

### Upgrading
1. Add `{ dialect: openspec, root: . }` to `plans.sources` (pre-0.14 repos keep `[]`) for `change new`; commit `.canon/artifacts/`; run `canon evidence vault` once.
2. Re-run `canon skills install`; update scripts parsing write-command output; if an adopted change lost its subject, run `canon subject adopt <change> --subject <id>`.

## 0.13.1 — 2026-10-10

Codex agents can now discover canon: the Codex skill is installed where Codex reads it.

### Fixed
- `canon skills install` projects Codex to `.agents/skills/canon/SKILL.md` (the old `.codex/skills/canon.md` was never read) and migrates the legacy projection, keeping edited files.
- `canon skills check` (exit 1) and `canon skills doctor` report any remaining legacy projection with the fix command.

### Upgrading
1. Re-run `canon skills install`; Codex does not see canon until you do.
2. If you use the developer-only `canon/skills-dev` materializer (now writes `.agents/skills/<name>/SKILL.md`), delete old `.codex/skills/<name>.md` files by hand.

## 0.13.0 — 2026-10-09

`canon context --json` carries a typed policy summary, and unknown `policy.yaml` keys fail the gate.

### Breaking
- In `canon context --json`, `spec_coverage`, `evidence_binding`, `review.requireReview` are JSON objects, not strings; unusable is `{"invalid": "<detail>"}`. `capabilityVersion` is 5.
- `canon gate check` reports an unknown top-level `policy.yaml` key (e.g. the typo `spec_coverag`) as `uncovered-cell <key>`, exit 1.

### Upgrading
1. Update consumers that parsed the old policy strings to read the fields directly and check for an `invalid` key instead of an `INVALID` prefix.
2. Fix or remove any unknown top-level key in `.canon/policy.yaml`.

## 0.12.0 — 2026-10-09

Ledger commands fail loudly on an invalid `canon.yaml`, and an opt-in independent-review gate is added.

### Breaking
- Commands that read or write the ledger exit 2 naming `canon.yaml` and the parse error instead of silently using the default ledger; a legacy `tiers: git:` layout now refuses.

### Added
- Opt-in `spec_coverage.require_review` (`scope`, `distinct_actor`, `block_on_findings`): `gate check` reports in-scope scenarios without another actor's review as `unreviewed-promotion`.
- Ninth failure class `open-blocker`: an open `blocker` finding on a change the subject adopted.
- `canon subject status` runs the same checks on entering a scoped status and refuses on gaps; `--override-reason "<one line>"` (with `--actor-id`) waives only `unreviewed-promotion`/`open-blocker`.
- With `block_on_findings`, an unreadable finding record is a `malformed-evidence` violation that no override waives.
- `canon context` lists finding and review fields and the `require_review` setting; `--json` adds a `review` object and `capabilityVersion` 4.

### Upgrading
1. Fix any `canon.yaml` that only worked through the old fallback; rewrite a legacy `tiers: git:` layout to `tiers: { local: { backend: git, root: .canon/ledger } }`.

## 0.11.0 — 2026-10-09

Experimental, off-by-default binding of evidence to files your own runner or agent produced; canon still never runs a test.

### Added
- `canon evidence add --artifact <path>` binds any repository file by sha256.
- `canon evidence add --report junit:<path>` or `cucumber:<path>` parses the report, records the matched case and outcome, and refuses a `faithful` verdict over a failed case.
- Policy `experimental.evidence_binding` with `mode: off | warn | require`, minimum `strength`, and optional filters; repos that do not set it are unaffected, and the shape may change.

## 0.10.0 — 2026-10-08

Golden-path-only specs become visible through scenario case tags.

### Added
- `@case:<value>` scenario tag (`happy`, `failure`, `edge`), set by `canon scenario new --case` and indexed by `canon inventory sync`.
- Opt-in `spec_coverage.require_cases: [failure]` reports feature surfaces with no failure-path scenario; `canon subject status <id> --to shipped` refuses on the same rule.
- `canon context` prints `exclude_lanes` and `require_cases`.

### Upgrading
1. Before setting `require_cases`, tag existing failure-path scenarios; untagged scenarios satisfy no case.

## 0.9.1 — 2026-10-07

Fixes the `@journeykit/canon` launcher's exit status.

### Fixed
- The launcher in 0.8.0–0.9.0 always exited 0, hiding refusals and a red `canon gate check`; it now passes the binary's exit code through.

## 0.9.0 — 2026-10-07

The Gherkin `@subject:<id>` tag becomes the only subject-to-scenario link.

### Breaking
- `canon report`'s Subjects panel and the `verifying → shipped` gate count scenarios carrying `@subject:<id>` in the latest `canon inventory sync`; shipping is refused when a subject owns none.

### Removed
- The never-populated `Subject.scenario_ids` list (older records still read).

### Upgrading
1. Tag each subject's scenarios with `@subject:<id>`.
2. Re-run `canon inventory sync` before moving a subject to `shipped`.

## 0.8.0 — 2026-10-03

Provider projections consolidate into a single `canon` skill, with stricter signed-approval and quarantine boundaries.

### Breaking
- Ingest is metadata-only: source bytes stay external while canon records metadata and provenance.
- Signed approval and quarantine workflows have stricter requirements.

### Changed
- Provider projections are consolidated into a single `canon` skill.

### Upgrading
1. Update integrations to the current parser generations before migrating.
2. Review existing approval and quarantine workflows against the stricter requirements.
