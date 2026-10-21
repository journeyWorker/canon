# canon CLI audit — output, JSON contract, errors, exit codes, versioning

Auditor: OutputAudit. Binary: installed `canon 0.25.0` (main e347991f). Skill: `cli-interface-design`
(Output + JSON contract + Errors sections). Scratch repo: `git init` + `canon init --providers=omp` + subject
`login`/`logout`, feature + 6 scenarios, 2 changes (one approved brief), evidence, review, findings, divergences,
dispatch run, context pack, prompt bundle, an openspec plan. Every one of the 70 leaf commands in `canon schema` was run with
`--json`; 64 had a success run, 6 did not (see §9). Evidence = pasted outputs below; source reads are
`crates/canon-cli/src/{main.rs,cli_error.rs,json_out.rs,removed.rs}` and `tests/json_contract.rs`. Guesses are marked [INFERENCE].
The scratch tree was deleted afterwards.

## 0. Headline numbers

| fact | value |
|---|---|
| leaf commands / all accept `--json` | 70 / 70 (`schema` → `supportsJson:true`; `tests/json_contract.rs` sweeps this) |
| success shapes in `--json` | **57** `{data,meta}` (compact, one line) · **13 bare** reports with no `data`/`meta`/`command`: `status brief-check context query finding-list dispatch-diff context-pack-show context-pack-verify prompt-show retrieve(array!) adapter-validate adapter-authorize selftest` · 9 of the bare ones are pretty-printed (2-space) |
| failure shapes in `--json` | 6 different: `{error}` only (the norm: 157 such documents in my ~400-run log) · `{data,error,meta}` (gate check, format, report --check, scenario new --from) · bare report + inline `error` (brief check, adapter authorize, selftest) · `{error}` where the report is lost (gate task blocked, learn *) |
| `ok` field | **nowhere** (only `init --check-config` has an unrelated `data.ok`; reports use `clean`, `verified`, `flipped`, `state`) |
| command id in the JSON | `meta.command` on the 57 enveloped successes and 4 data+error failures. **Absent** on the 13 bare reports and on every error-only failure (usage errors included) |
| schemaVersion | **nowhere**. Only per-feature fields: `statusVersion:3`, `capabilityVersion:5`, `formatVersion`/`inputManifestVersion`/`manifestVersion`, `protocol_version`, export/purge `schema:"canon.retention.*"`+`version:1`, record `schema:1` |
| error codes | 151 registered (`schema --errors`); each error carries `code,message,hint,retryable` (+`field`,`expected` sometimes); `retryable:true` is used once in the code base (`DISPATCH_END_IN_PROGRESS`) |
| per-command exit tables | 70; **37 share one boilerplate** ("1 operational failure, or a blocking finding…" / "2 refused…") — the tables disagree on what 1 and 2 mean (§6) |
| `--help` size | top-level 4 307 B / 68 lines (`-h` 4 112 B); all 70 leaf `--help` = 96.9 KB; largest `evidence add` 9 675 B/101 lines, `finding add` 7 883 B, `finding close` 3 844 B, `review add` 3 382 B; only 9/70 leaves have an `Examples:` section; 3 top-level summaries >120 chars (`migrate` 216, `purge` 150, `schema` 121) |

## 1. Table: every command × shape

Legend. `ENV` = `{"data":…,"meta":{"command":…}}` on one compact line. `BARE` = report is the whole document (pretty unless noted).
`stderr` = the command narrates on stderr while in `--json` mode (the same line text mode prints on stdout). "Failure shapes seen" lists `CODE→exit`; `ERR` = `{error}` only.
`data` keys for the subject commands are the raw stored record (snake_case + `kind`+`schema:1`).

| command | effect | JSON success shape | `data` / top-level keys | `meta` keys | stderr narration in --json | failure shapes seen (code→exit) |
|---|---|---|---|---|---|---|
| `adapter authorize` | read | BARE (snake_case, pretty) | capabilities,execution_authorized,execution_performed,reasons,sandbox_enforced (+error inline on exit 1) | - | yes (error line) | exit 1 ADAPTER_REFUSED / ADAPTER_EXECUTION_NOT_AUTHORIZED (BARE+error) |
| `adapter validate` | read | BARE (snake_case, pretty) | protocol_version,run_id,provider,model,context_pack_id,status,capabilities,evidence_count,telemetry,extension_keys,context_join_verified | - | no | exit 1 ADAPTER_REFUSED (ERR) |
| `artifacts migrate` | write | (no success run) | - | - | - | ARTIFACTS_MIGRATE_REFUSED→2 |
| `artifacts verify` | read | ENV {data,meta} | backend,bound,full,problems,stored,workingTreeOnly | command | yes | — |
| `brief approve` | write | ENV {data,meta} | approvedBy,brief,change,digest,policyApproved | command,next | no | BRIEF_HAS_BLOCKERS→2 |
| `brief check` | read | BARE (pretty) | approved,brief,change,counts,findings,language,legacy,phase,waivers | - | no | BRIEF_NOT_FOUND→1; BRIEF_HAS_BLOCKERS→2 (BARE) |
| `change new` | write | ENV {data,meta} | brief,changeId,subjectId | command,next | yes | USAGE→2 |
| `context` | read | BARE (pretty) | capabilityVersion,cel,enums,joinKeys,kinds,policy,review,vocab | - | no | — |
| `context-pack new` | write | ENV {data,meta} | capabilityVersion,formatVersion,id,inputManifestVersion,knowledgeMapDigest,nonReplayable,policy,prompts,repo,selectedDocs,skillBundleDigest,tools,w… | command,next | no | CONTEXT_UNSAFE_PATH→2; CONTEXT_INPUT_MISSING→2 |
| `context-pack show` | read | BARE (pretty) | capabilityVersion,formatVersion,id,inputManifestVersion,knowledgeMapDigest,nonReplayable,policy,prompts,repo,selectedDocs,skillBundleDigest,tools,w… | - | no | CONTEXT_PACK_NOT_FOUND→2 |
| `context-pack verify` | read | BARE | id,verified | - | no | CONTEXT_PACK_NOT_FOUND→2 |
| `dashboard` | write | ENV then serves (no stream) | data{app,briefsUrl,snapshot,snapshotAvailable,url}, meta{command,next:[],warnings:[]} | - | yes (4 lines) | not reproduced: DASHBOARD_PORT_IN_USE (a held 127.0.0.1 socket did not block it) |
| `demo add` | write | ENV {data,meta} | next,scenario | command,next | yes | — |
| `demo init` | write | ENV {data,meta} | next,repo,scenario | command,next | yes | — |
| `dispatch begin` | write | ENV {data,meta} | injected_guidance,lineage,manifest,run_id | command,next | no | USAGE→2 |
| `dispatch diff` | read | BARE (pretty) | counts,declared,edges,notes,observed | - | no | — |
| `dispatch end` | write | ENV {data,meta} | ended_at,manifest,run_id,status | command,next | no | USAGE→2; DISPATCH_ALREADY_ENDED→2; DISPATCH_RUN_NOT_FOUND→2 |
| `divergence accept` | write | ENV {data,meta} | accepted,expiry,tracking | command,next | yes | USAGE→2 (runtime `--role` check); DIVERGENCE_ACCEPT_REFUSED→2 |
| `divergence add` | write | ENV {data,meta} | committed,staged | command,next | yes | USAGE→2 |
| `divergence defer` | write | ENV {data,meta} | committed | command,next | yes | — |
| `divergence promote` | write | ENV {data,meta} | dryRun,promoted,recovered,refused | command,next | yes | — |
| `divergence resolve` | write | ENV {data,meta} | committed | command,next | yes | — |
| `divergence status` | read | ENV {data,meta} | as_of,scenarios,summary | command,hasMore,nextCursor,returned,total | no | — |
| `evidence add` | write | ENV {data,meta} | records,staleReviews | command,next | yes | EVIDENCE_INVALID_ARGUMENT→2; USAGE→2 |
| `evidence approval-payload` | read | ENV {data,meta} | effects,payload,payloadHex,surface | command | no | USAGE→2; EVIDENCE_REFUSED→2 |
| `evidence vault` | write | ENV {data,meta} | alreadyStored,boundFiles,notStored,stored | command,next | yes | — |
| `export` | write | ENV {data,meta} | after,before,content_digests,kind,record_count,records,schema,sensitive,source_scope,version | command,hasMore,next,nextCursor,returned,total | no | — |
| `feature new` | write | ENV {data,meta} | feature,next | command,next | yes | — |
| `finding add` | write | ENV {data,meta} | changeId,committed,disposition,round,seq,severity,staged,unsourced | command,next | yes | — |
| `finding close` | write | ENV {data,meta} | changeId,committed,disposition,from,noop,round,seq,staged | command,next | yes | FINDING_RESOLUTION_SHA_REQUIRED→2 |
| `finding list` | read | BARE (pretty) | change_id,findings,meta | hasMore,nextCursor,returned,total | no | USAGE→2 |
| `format` | read | ENV {data,meta} | clean,filesChecked,violations | command | yes | FORMAT_VIOLATIONS→1 (ENV+ERR) |
| `gate check` | read | ENV {data,meta} | advisories,clean,violations | command | yes | GATE_VIOLATIONS→1 (ENV+ERR) |
| `gate install-hooks` | write | ENV {data,meta} | installed,preCommitScriptWritten | command,next | yes | — |
| `gate promote` | write | ENV {data,meta} | dryRun,promoted,recovered,refused | command,next | yes | — |
| `gate selftest` | read | ENV {data,meta} | clean,fixtures | command | yes | — |
| `gate task` | write | ENV {data,meta} | document,flipped,taskId | command,next | yes | TASK_NOT_FOUND→2; GATE_TASK_INVALID_ARGUMENT→2; GATE_TASK_BLOCKED→1 (ERR only) |
| `ingest artifacts` | write | ENV {data,meta} | adapters,convergence_conflicts,strategy_items_rebuilt,trajectories_converged_deleted,trajectories_left_pending,trajectories_marked,trajectories_per… | command,next | no | — |
| `ingest plans` | write | ENV {data,meta} | changes_persisted,duplicate_change_id,non_clean_sources,sources,tasks_persisted,unwritten_changes,unwritten_tasks | command,next | no | — |
| `ingest sessions` | write | ENV {data,meta} | adapters,degradeReason,eventsWritten,malformedRecords,rowsSkipped,runsWritten,scope,sessionsNormalized,unwritten | command,next | yes | — |
| `init` | write | ENV {data,meta} | steps | command,next | yes | CONFIG_MISSING→2; USAGE→2; INIT_INVALID_ARGUMENT→2 |
| `inventory sync` | write | ENV {data,meta} | clean,roots,totalWritten | command,next | yes | — |
| `learn approve` | write | (no success run) | - | - | - | USAGE→2; LEARN_STRATEGY_NOT_FOUND→1 |
| `learn promote` | write | (no success run) | - | - | - | USAGE→2; LEARN_PROMOTION_FAILED→1 |
| `learn rollback` | write | (no success run) | - | - | - | USAGE→2; LEARN_SIGNATURE_UNREADABLE→1 |
| `ledger compact` | write | ENV {data,meta} | archive,bytes,dryRun,files,kinds,live,moved,superseded | command,next | yes | — |
| `migrate` | write | ENV {data,meta} | dryRun,indexRecordsWritten,steps | command,next | yes | — |
| `plugin sync` | write | (no success run) | - | - | - | PLUGIN_SYNC_FAILED→2 |
| `policy approve` | write | ENV {data,meta} | approval,approvedAt,approvedBy,policy,policyDigest | command,next | yes | USAGE→2 |
| `policy show` | read | ENV {data,meta} | approval,approvedAt,approvedBy,approvedDigest,detail,note,policy,policyDigest,state | command | no | — |
| `prompt add` | write | ENV {data,meta} | digest,name,prompts,version | command,next | no | — |
| `prompt show` | read | BARE (pretty) | digest,name,prompts,version | - | no | PROMPT_BUNDLE_INVALID→2 |
| `purge` | destructive | ENV {data,meta} | before,dry_run,kind,planDigest,rungs,schema,source_scope,unavailable,version | command,next | no | USAGE→2; RETENTION_PURGE_FAILED→2; PURGE_PLAN_REQUIRED→2; PLAN_CHANGED→2 |
| `query` | read | BARE (pretty) | count,kind,meta,records,since,violations | hasMore,nextCursor,returned,total | no | USAGE→2; UNKNOWN_FIELD→2; INVALID_CURSOR→2 |
| `regime-key` | read | ENV {data,meta} | regimeKey | command | no | REGIME_KEY_INVALID→2 |
| `report` | write | ENV {data,meta} | mode,report | command,next | yes | USAGE→2; REPORT_STALE→1 (ENV+ERR) |
| `retrieve` | read | BARE ARRAY | [] (array of items) | - | no | RETRIEVE_FAILED→2 |
| `review add` | write | ENV {data,meta} | blobsStored,outcome,reviews | command,next | yes | USAGE→2 |
| `scenario new` | write | ENV {data,meta} | feature,scenarioId | command,next,note | yes | USAGE→2; SCAFFOLD_INVALID_ARGUMENT→2 (ENV+ERR) |
| `schema` | read | ENV {data,meta} | commands[,role] (list) | command,about,aliases,arguments,effect,errorCodes,example,exits,options,path,roles,supportsJson (one) | errors[] (--errors) | command | no | UNKNOWN_COMMAND→2; UNKNOWN_ROLE→2 |
| `selftest` | read | BARE (pretty) | clean,suites[,error] | - | yes? (error line only) | exit 1 SELFTEST_FAILED (BARE+error); false-fails outside the canon checkout |
| `skills check` | read | ENV {data,meta} | clean,manifestOk,orphans,outdated,providers,remnants,selection,statuses | command | yes | — |
| `skills doctor` | read | ENV {data,meta} | lines | command | yes | — |
| `skills install` | write | ENV {data,meta} | changed,configWritten,kept,ownedPaths,providers,pruned,selection | command,next | yes | SKILLS_FAILED→1 |
| `status` | read | BARE (pretty) | canonVersion,next,nextOmitted,policy,statusVersion,subjects,unowned,warnings | - | no | USAGE→2; UNKNOWN_ROLE→2; UNKNOWN_FIELD→2 |
| `subject adopt` | write | ENV {data,meta} | actor,at,change_ids,domain,kind,owner_role,schema,status,subject_id,summary,title | command,next | no | SUBJECT_ADOPT_REFUSED→2 |
| `subject new` | write | ENV {data,meta} | actor,at,domain,kind,owner_role,schema,status,subject_id,summary,title | command,next | no | SUBJECT_EXISTS→2; USAGE→2 |
| `subject status` | write | ENV {data,meta} | actor,at,change_ids,domain,kind,owner_role,schema,status,subject_id,summary,title | command,next | no | SUBJECT_NOT_FOUND→2; SUBJECT_INVALID_TRANSITION→2; USAGE→2 |
| `subject unadopt` | write | ENV {data,meta} | actor,at,change_ids,domain,kind,owner_role,schema,status,subject_id,summary,title | command,next | no | — |
| `tier age` | write | ENV {data,meta} | dryRun,rules | command,next | yes | — |

Not enveloped but document-per-run facts: `dashboard --json` prints one ENV document then serves until killed (no stream; narration on stderr).
No command writes NDJSON or an event stream to stdout. NDJSON exists only as **input** (`scenario new --from -`, the item format has a stale example, §8-E) and as the audit file `.canon/audit/access.jsonl`.
Artifacts: `export --out FILE` (`canon.retention.export`), `report --snapshot DIR` (10 `.parquet` + manifest), `report` → `.canon/REPORT.md`,
`evidence approval-payload` (raw signing bytes in text mode, no trailing newline), `context-pack new` / `prompt add` (immutable JSON files under `.canon/`).

Key-style mix in `data` (same release): camelCase (`changeId`, `filesChecked`, `dryRun`), snake_case (`run_id`, `injected_guidance`, `as_of`,
`record_count`, `dry_run`, `changes_persisted`, subject record `subject_id`/`owner_role`), and both in one document (`status`: `statusVersion` but `policy.spec_coverage`;
`purge`: `planDigest` beside `dry_run`; `export`: `record_count` beside `source_scope`).

## 2. stdout / stderr / TTY / color (text mode)

* Text success output (including notes, `next:` lines, warnings and the `scenario new` provenance reminder) all goes to **stdout**; stderr is silent on every
  success run I made except `export --out` (`canon export: wrote 2 record(s)`). Errors go to stderr only (`canon <cmd>: CODE — msg` + `hint: …`).
  A failing `gate check`/`format` in text mode prints the violations on stdout and **nothing** on stderr (`emit_with_data`: "human mode adds nothing"), so no code is visible.
* JSON mode: stdout is exactly one JSON document in every `--json` run in my log, ~400 runs (`tests/json_contract.rs` pins that). 31/70 commands also narrate on stderr
  (`gate selftest` prints 10 `ok <fixture>` lines; `skills doctor` repeats the whole report; `demo init` ~10 lines). Error order: stderr line first, then stdout JSON.
* ANSI: canon-authored text is never styled. Only clap output (usage errors, `--help`) is coloured, and only on a TTY. Verified in a pty: `canon statuss` → `\x1b[1m\x1b[31merror:`; with `NO_COLOR=1` plain;
  `CLICOLOR_FORCE=1` forces colour even piped; piped output plain. `canon status`, `gate check`, `subject status` in a pty: plain.
* Prompts: one — `canon init` when stdin **and** stderr are TTYs asks "which harnesses should get the canon skill?" on stderr (Enter = detected ones). Non-TTY or `--providers` skips it. There is no `--no-input`,
  `--yes`, `--force`, `--quiet`, `--verbose`, `--commit`, `-o/--output` anywhere (union of all option names). SIGPIPE is handled (`canon context --json | head -c 20` exits 0). stdin is read only by `scenario new --from -` and `review add --scenario-ids-from -`.
* Text-mode lists, one record per line? `finding list` (header + rows) yes; `divergence status` (summary line + one per scenario) yes; `skills check` yes; `schema` yes (70 lines);
  `query` = header line + blank + table + footer, `AT` column unaligned, the 150-char hex cursor printed **twice** in the footer; `status` is a 25-line nested outline; `context` 282 lines;
  `export`/`purge --plan` print a summary line followed by a pretty JSON artifact on stdout (not parseable as either); `adapter authorize` prints `key: value` lines;
  `--fields` is silently ignored in text mode.
* Same command line, different output by environment: `CANON_ROLE`/`--as` changes `schema` (70 → 17 commands, adds `data.role`), `status` (adds `waitingOn`, empties `next` for another role) and `evidence/review add --role`; `canon init` differs TTY vs non-TTY; `CANON_ROLE=bogus` makes **every** command including `schema` and `schema --errors` fail with `UNKNOWN_ROLE` (only `--help`/`--version` survive), so the error catalogue is unreachable exactly when the env var is wrong.
* Undeclared side effect: `query` and `export` append to `.canon/audit/access.jsonl` (`{"actor":"<os user>",…}` — OS user name) although `query` is declared `read`. gitignored. The 19 other `read`-effect commands I diffed wrote nothing.

## 3. Error contract

Source of truth: `cli_error.rs`. Rendering: human `canon <command path>: <CODE> — <message>` + `hint: …` on stderr; JSON `{"error":{"code","message","hint","retryable"[,"field"][,"expected"]}}` on stdout.
`hint` is always present (`null` when none); `field`/`expected` only when a refusal names one.

| property | observed |
|---|---|
| code, message, hint, retryable | always present on `emit` errors |
| `field` | usage errors and many `*_INVALID_ARGUMENT`; several formats: `--verdict`, `<ID>`/`[TAG]`/`<STRATEGY_ID>` (clap bracket decoration), `CANON_ROLE`, bare `subject` (batch item), the raw unexpected token (`status --json -- x` → `"field":"x"`), `--subject` (removed spelling) |
| `expected` | closed sets give the values (`--verdict`, `--kind`, `--as`, `--to`, `--fields`); removed spellings give the **replacement flag** (`["--subject-id"]`, `["--kind evidence"]`) — a different meaning for the same key; `[]` when a value is missing (`report --snapshot`, `subject new --title --json`) |
| missing required args | `message:"the following required arguments were not provided:"`; the names are glued into `hint` together with clap's `Usage:` line and `For more information, try '--help'.` — no `field`, no `expected`: `{"hint":"--kind <KIND> --ref <REF> Usage: canon evidence add --kind <KIND> --ref <REF> --json For more information, try '--help'."}` |
| invalid typed value | message holds the grammar regex (`invalid Sha: … exactly 40 lowercase hex characters`), `hint` is the useless `For more information, try '--help'.` |
| similar-name suggestion | **preserved in both modes**: `canon statuss`, `canon evidence ad`, `canon status --jsn`, `canon schema evidenc add` all name the near match (text: clap `tip: a similar subcommand exists: 'status'`; JSON: inside `hint`, flattened with the usage line, no structured field). Not reproduced: the "text mode loses the suggestion" hypothesis. `canon nonsense` has no near match in either mode |
| `command` in the JSON error | never. Usage errors print stderr prefix `canon:` (id `"canon"` hard-coded in `usage_error`), others `canon <cmd>:` on stderr only |
| `path`, `argument` | do not exist; paths are inside `message`; `field` plays `argument` |
| `retryable` | `false` on every error I produced; only `DISPATCH_END_IN_PROGRESS` can be `true` |
| `ok:false`/`schemaVersion` | absent |
| text vs JSON differences | clap errors only: text = clap's native multi-line block (`error: …`, `tip:`, `Usage:`; ANSI on a TTY); JSON-mode stderr = canon's two-line form. Domain errors are byte-identical on stderr in both modes. `gate check`/`format` failures: stderr empty in text mode, `error` object in JSON |
| raised before parsing | (a) clap parse failures → `usage_error` (JSON if literal `--json` precedes `--`); (b) removed spellings detected only after a parse failure (`--subject`, `--kind evidence_record`) → `USAGE`, `field` + `expected`=replacement, message "`--subject` was removed in 0.25.0" (release number hard-coded in `removed.rs` `Removed::message`, so it must be edited every release); (c) `CANON_ROLE` validation after parse, before dispatch, uses `emit("canon", argv_wants_json())` |
| `--json=true`, `--json --json` | `--json=true` rejected by clap (text error, no JSON); duplicate → `USAGE` JSON |

Misclassified or misleading codes found: `SERIALIZE_FAILED` ("this is a canon bug: report it") for a user manifest missing `capabilityVersion` (then `examples`; `context-pack new --manifest`, exit 2); `RETRIEVE_FAILED` for the missing `--regime|--domain`
(exit 2 — a `USAGE`; its hint talks about the ledger); `SKILLS_FAILED` for an invalid `--providers` value (hint: "check that the target directory is writable"; `init --providers bogus` is `INIT_INVALID_ARGUMENT`, and silently exits 0 in a repo that already has canon.yaml);
`PROMPT_BUNDLE_INVALID` for an unregistered version (no NOT_FOUND code); `LEARN_PROMOTION_FAILED` for an unknown strategy id (message leaks `demote_strategy:`; `learn approve` says `LEARN_STRATEGY_NOT_FOUND`);
`PLUGIN_SYNC_FAILED` for an unknown plugin id; `RETENTION_PURGE_FAILED` for `purge --kind subject` although `--kind` advertises 14 values and only 4 are allowed; `ARTIFACTS_MIGRATE_REFUSED` for a bad `--to`
(message `canon.yaml TierPolicy: canon.yaml artifact_store.backend: bogus …`); `purge --before 2020-01-01` → `USAGE` "invalid --since timestamp … premature end of input" (wrong flag name, no example format; needs `2020-01-01T00:00:00Z`);
`learn rollback` hint says `--signature <file>` but the flag is `--signature-file` (`learn promote` does have `--signature`); `LEARN_PROMOTION_FAILED` hint offers `--dry-run` (exists).
A programmatic scan of every `.hint("…")` in the source for unknown flags found only the learn one (plus two false positives in a bun command).

## 4. Exit codes: schema `exits` vs behaviour

Rule in code (`cli_error.rs`): `1` = operational error or blocking finding (`failed`), `2` = refusal/usage (`refused`), `with_exit` overrides. 79 codes are built as `refused`(2), 50 as `failed`(1), 6 as both.

| area | declared in `canon schema` | actual | verdict |
|---|---|---|---|
| `gate check` | 1 violations, 2 "could not run: bad --repo or unreadable ledger/config" | `--repo /nonexistent` → **0** `clean`; empty dir without `canon.yaml` → **0** `clean`; real violation → 1 | **declared 2 is dead; gate is fail-open when unconfigured** |
| `format` | `[0,1]` | usage error → 2 (undeclared); `format /nonexistent` → **0** `filesChecked:0 clean:true` | table incomplete; typo'd path passes |
| `status`, `context`, `dispatch diff`, `selftest`, `gate selftest` | `[0]`/`[0,1]` | usage → 2 | 6 tables omit exit 2 (all 70 emit 2 for `USAGE`) |
| `brief check` | 1 operational, **2 blockers** | matches (`BRIEF_NOT_FOUND`→1, `BRIEF_HAS_BLOCKERS`→2) | consistent with itself, **inverted** vs `gate check`/`format`/`evidence add`/`report` where findings=1 and refusal=2 |
| not-found family | — | `SUBJECT_NOT_FOUND` 2, `CONTEXT_PACK_NOT_FOUND` 2, `FINDING_NOT_FOUND` 2, `TASK_NOT_FOUND` 2, `DISPATCH_RUN_NOT_FOUND` 2, `BRIEF_NOT_FOUND` **1**, `LEARN_STRATEGY_NOT_FOUND` **1** | inconsistent |
| unreadable input file | — | `CONTEXT_INPUT_MISSING` 2, `ADAPTER_REFUSED` **1**, `LEARN_SIGNATURE_UNREADABLE` **1**, `LEARN_INVALID_INPUT` **1** | inconsistent |
| `*_INVALID_ARGUMENT` | — | all 2 except `LEARN_INVALID_ARGUMENT` **1**; `SKILLS_FAILED` (invalid provider) **1** | inconsistent |
| store/IO failure | — | `EVIDENCE_STORE_FAILED`/`EVIDENCE_VAULT_FAILED` 2 ("refused"), `INGEST_FAILED`/`DASHBOARD_FAILED` 1 | inconsistent |
| same precondition, no `canon.yaml` (empty dir) | — | `gate check`,`status`,`report`,`inventory sync`,`policy show`,`skills check`,`context`,`finding list`,`artifacts verify`,`gate promote`,`divergence status` → **0**; `query` 1 `QUERY_FAILED`; `tier age` 1 `TIER_FAILED`; `brief check` 1 `NO_BRIEF`; `export` 2 `RETENTION_EXPORT_FAILED`; `migrate` 2 `CONFIG_MISSING`; `subject status` 2 `REPO_UNREADABLE`; `report` writes `.canon/REPORT.md` + seed parquet files into the unconfigured dir | no single "not a canon repo" behaviour |
| 37 commands | identical boilerplate "1 operational failure, or a blocking finding the command reports / 2 refused: …" | e.g. `retrieve` declares 1 but its only failure is exit 2; `adapter validate` input errors really exit 1 | tables are templates, not measured |
| `report --check` | 1 for drift | `REPORT_STALE`→1, `meta.command` becomes `"report --check"` | ok, but id includes a flag |
| JSON `ok` | none | exit is the only success signal | |

Documented in `cli.mdx`: "Exit 2 = invocation or inputs wrong and fixable; exit 1 = operational failure or blocking finding" — the generic text is right, the per-command exceptions (brief check, learn, adapter, skills, unreadable files) break it. `schema` lists `errorCodes` and `exits` separately: no code→exit map, and `schema --errors` gives `{code, meaning}` only (no exit, no commands, no `retryable`).

## 5. Versioning — how could a consumer detect a payload change?

* Per-report: `status` `statusVersion:3` + `canonVersion:"0.25.0"` (top level); `context` `capabilityVersion:5` (top level; also `dispatch begin` `lineage.context.capability_version`);
  `context-pack` `formatVersion`/`inputManifestVersion`; adapter `protocol_version`; `export`/`purge` `data.schema:"canon.retention.export|purge"` + `data.version:1`; stored records `schema:1` (an int, same key, other meaning).
* Nothing else is versioned. `schema --json` (the introspection report itself) has no version; `meta` has neither `schemaVersion` nor the canon version. `canon --version` is text-only (no `--json`); `status --json` is the only JSON that says which canon produced it.
* `schemaVersion` as defined in the skill (`<release>.<report-name>`, unchanged while the payload is unchanged) is absent, so a consumer cannot detect a change except by comparing `canon --version` (coarse) or diffing keys. `tests/json_contract.rs` pins only "one JSON value; coded error", not any success shape.
* `schema <cmd>` `outputs` (skill: `{flag, kind: report|artifact|stream, schemaVersion}`) does not exist; only `supportsJson`. `options[].aliases`, `default` are lists (`[]` for none).

## 6. Deviations from the skill (Output + JSON contract + Errors)

Severity: H = breaks a stated rule and agents hit it; M = inconsistent; L = cosmetic.

| # | skill rule | canon today | sev |
|---|---|---|---|
| J1 | every report carries `command`, `schemaVersion`, `ok` | `command` only in `meta`, not on 13 bare reports / any error-only document; `schemaVersion` and `ok` nowhere | H |
| J2 | "no payload field reuses those names" | `schema` is an int (record) in subjects/evidence and a string in `export`/`purge`; `version` in export/purge; `status` means subject state / adapter status / etc.; `ok` only in `init --check-config` | M |
| J3 | envelope only reports; artifacts/streams unwrapped | wrapping decided per command: `export --json` wraps the artifact, text prints it bare; `context-pack new` wraps, `show` bare; `prompt add` wraps, `show` bare; `approval-payload` wraps (payload string + hex) in JSON, raw bytes in text | H |
| J4 | JSON mode from arguments | satisfied (`argv_wants_json`, before `--`) | ok |
| J5 | error envelope `{command, schemaVersion:"<rel>.error", ok:false, error:{code,message,path?,argument?,hint?}}`, stdout exactly one object, human message on stderr, exit unchanged | stdout one object ✓, stderr ✓, exit ✓; no `command`/`schemaVersion`/`ok`; extra `retryable`; `field` for `argument`; clap hint is the flattened `tip + Usage + try --help` string | H |
| J6 | failure vs report shape | six failure shapes (see §0); `brief check` blockers = bare report with inline `error`; `adapter authorize` and `selftest` the same; `gate task` blocked and `learn *` lose their report; `format`/`gate check`/`report --check`/`scenario new --from` return `{data,error,meta}` | H |
| J7 | fixed small code set (Lute 3 codes) | 151 codes, many near-duplicates (`*_INVALID_ARGUMENT` ×16 families + `USAGE` + `REPO_UNREADABLE`/`CONFIG_UNREADABLE`/`WRITE_FAILED`); explainable via `schema --errors`, but no `canon explain CODE`, and the registry has no exit/command columns | M |
| O1 | text default, `--json` explicit, no TTY/env switching | format ✓; `init` prompts on a TTY only; `CANON_ROLE` changes `schema`/`status` content | M |
| O2 | compact JSON | 13 bare reports are pretty-printed; compact would save 29% (`context` 21 901→15 538 B), 34% (`status` 2 865→1 894 B), 24% (`query`), 33% (`finding list`) | M |
| O3 | text one record per line | `query` header+blank+table+duplicate cursor; `status` nested outline; `export`/`purge --plan` line + pretty blob | M |
| O4 | bound large output, say when truncated + how to narrow (`truncated`+`hint`) | paging with `--limit` (default 100 in help only), `--all`, `--cursor`, `meta.{returned,total,hasMore,nextCursor}` on `query`, `finding list`, `divergence status`, `export`; no `truncated`/`hint` field; **unbounded**: `context` (21.9 KB), `status.subjects`, `gate check.violations`, `brief check.findings`, `skills check.statuses`, `schema --all` (12.5 KB); `--fields` on 6 commands but ignored in text mode; `--limit 0` refused | M |
| O5 | diagnostics on stderr | all text-mode narration/notes/warnings on stdout; JSON mode duplicates narration to stderr on 31 commands | L |
| O6 | stable exit-code table | declared per command, inconsistent (§4) | H |
| O7 | `schema` derives `outputs` from the same table as the envelope | no `outputs`; `supportsJson` only | M |
| E1 | validate before side effects | `report` writes into a non-canon dir; `gate check`/`format` accept a missing repo/path as clean; `init --providers bogus` ignored when canon.yaml exists; `purge --kind subject` accepted by the parser, refused at runtime | H |
| E2 | invalid value names the valid set + a working invocation | enum sets ✓ via `expected`; grammar errors show the regex but not an example; `--before` date format never shown; missing-required lists names only inside `hint` | M |
| E3 | dangerous commands preview by default, `--commit` | `purge`: bare run is refused (`PURGE_PLAN_REQUIRED`, exit 2), `--plan` previews, `--confirm <planDigest>` acts — safe, but a different grammar from `--commit` | L |
| E4 | `--yes`/`--force`/`--no-input` | none exist; `init` prompt only on TTY | L |
| E5 | identifiers reject `?`/`#`/`%` | `TaskId` = `<change>#<n>`; the `#` ends up in ledger filenames (`kind=evidence_record/oc#1.1__….json`) | L |
| E6 | `--json` never means input | ✓; but `--json` help text varies per command ("Output the complete bundle manifest as JSON") | L |
| E7 | NDJSON stays unwrapped | no NDJSON output; input help example stale (`--from` text shows `"subject":"demo"`, the key now fails with ``` `subject` was removed in 0.25.0 ``` — must be `subject-id`) | M |

## 7. Proposed unified envelope

Decision principle: keep the 57-command `{data, meta}` shape (it is documented in `cli.mdx`, used by the dashboard/skills/conformance cases and by `tests/json_contract.rs`), add what the skill requires, and wrap the 13 bare reports.

Success (exit 0):
```json
{"ok":true,
 "data":{ ...the report... },
 "meta":{"command":"subject new","schemaVersion":"0.26.subject",
         "next":[...],"nextOmitted":2,"waitingOn":"human","warnings":["..."],"unchanged":true,
         "returned":50,"total":212,"hasMore":true,"nextCursor":"..."}}
```
Report-level finding failure (exit 1, a report worth reading):
```json
{"ok":false,"error":{"code":"GATE_VIOLATIONS","message":"1 violation(s)","hint":"…","retryable":false},
 "data":{...same report as success...},"meta":{"command":"gate check","schemaVersion":"0.26.gate-check"}}
```
Refusal / usage / I/O (exit 2 or 1, no report):
```json
{"ok":false,"error":{"code":"USAGE","message":"…","field":"--verdict","expected":["faithful","not-applicable","divergent"],"hint":"…","retryable":false},
 "meta":{"command":"evidence add","schemaVersion":"0.26.error"}}
```
Rules:
1. `ok` is top level and is `true` exactly when exit is 0 (skill rule). It replaces ad-hoc `clean`/`verified` for branching; those stay inside `data`.
2. `command` stays in `meta` (already there on 57 commands) = canonical id from `schema` (path joined by single spaces). **Flags never change it** (`report --check` → `report`). Errors gain `meta.command` too; for parse failures derive the id from argv (the `removed::find` walker already computes the command path `here`); fall back to `"canon"` only when no subcommand resolved.
3. `meta.schemaVersion` = `<release>.<report-name>` (`0.26.subject`, `0.26.gate-check`, `0.26.error`). Unchanged while the payload shape is unchanged; for `data`+`error` documents it names `data`. Fold `statusVersion`, `capabilityVersion`, export/purge `schema`/`version` into it (keep those numbers inside `data` only if consumers need the finer number; **remove `data.schema:"canon.retention.*"`** to stop the `schema` key meaning two things).
4. `meta.next`, `meta.nextOmitted`, `meta.waitingOn`, `meta.warnings`, `meta.unchanged`, `meta.deprecated` (reserved: removed in 0.25.0, re-add only through `meta`) and paging `meta.{returned,total,hasMore,nextCursor}` stay in `meta`, flat, as today. `query`/`finding list` already emit this paging as top-level `meta`; moving their report into `data` makes it identical to `divergence status`/`export`. Add `meta.truncated` only if the owner wants the skill's `truncated`+`hint` (hasMore + a narrowing hint).
5. `error` keeps `{code,message,hint,retryable[,field][,expected]}`. Fix the flattening: parser suggestion → `hint` text only; add `expected` for missing-required (`field` = first missing arg, or a list) instead of burying names in `hint`; `expected` always means values (move removed-spelling replacement to `hint`/a new `replacement`); `field` always `--flag`, `<POSITIONAL>` or env name, never a raw token; drop `hint` null (omit) or always present — pick one.
6. One mapping from code to exit class stored in the registry (`all_codes()` gains `exit` and `commands`) and printed by `schema --errors --json`; `exits` tables generated from it (no boilerplate templates).
7. Artifacts vs reports: classify each output in `schema <cmd>` as `outputs:[{flag,kind:report|artifact|stream,schemaVersion}]`. Reports (`export` summary, `purge --plan`, `context-pack new/show/verify`, `prompt add/show`, `evidence approval-payload`) are always enveloped under `--json` and **text mode prints a summary, not the artifact**; the raw artifact is written by `--out FILE` (export already) or, for `approval-payload`, `--out`/`--raw` (bytes exactly as signed). No NDJSON/stream exists today, so none is declared.
8. Compact everywhere (no pretty-print); humans use `jq`.

Why not the flat Lute envelope: it would move all 57 `data` payloads to top level (every `.data.x` consumer breaks) and collide with canon's own `status`, `kind`, `schema`, `next` keys; `{ok,data,meta}` meets the skill's intent (command, schemaVersion, ok present in every document, no collisions since payload lives under `data`). Record this as an owner-approved deviation in the grammar doc.

## 8. Migration risk

| change | breaks | scale | mitigation |
|---|---|---|---|
| add top-level `ok`; add `meta.command`+`meta.schemaVersion` to every document; errors gain `meta.command` | nobody (additive); parsers that assert the exact key set of `meta` or `{error}` | tests/conformance only | none needed |
| wrap the 13 bare reports in `{ok,data,meta}` | every reader of top-level fields: `status` (`.next`, `.subjects`, `.statusVersion` — used by skill text in 4 provider copies + `canon/skills/reference/canon-subject.md`, `README.md`, website `cli.mdx` en+ko, conformance `status-gaps`), `query` (51 files mention `canon query … --json`), `context` (24), `brief check` (15), `retrieve` (15; bare **array** → `data` array), `finding list` (11), `prompt show`/`context-pack show` (9 each), adapter (10) | ~20 of 103 conformance cases use `--json`; website/skills docs updated in the same PR (repo rule: migrate callers in the change) | one clean cutover in a minor release with a release-note table `old path → new path`; add the new shape's `schemaVersion` so a consumer can assert it; `--fields` keeps selecting top-level `data` keys |
| move `statusVersion`/`capabilityVersion`/export-purge `schema`+`version` | consumers of those keys (dashboard reads export/status? [INFERENCE]; `canon/skills` text mentions `statusVersion`) | small | keep the numeric version fields inside `data` for one release if wanted |
| failure documents: `{data,error,meta}` for report failures; `brief check`/`adapter authorize`/`selftest` leave the bare+`error` shape | consumers of `brief check`'s inline `error` | brief check is read by hooks/skills | covered by the same cutover |
| exit-code harmonisation (`LEARN_*`, `ADAPTER_REFUSED`, `SKILLS_FAILED`, `BRIEF_NOT_FOUND`, `LEARN_STRATEGY_NOT_FOUND` 1→2; `gate check`/`format`/`status`/`report`… with no repo → non-zero) | CI that treats 1 as "gate red" and 2 as "broken"; any pipeline running `gate check` in a directory without `canon.yaml` goes from green to red (intended: it is fail-open today) | highest consequence of the list | ship fail-closed `gate check` with its own note; decide `brief check` blockers 2→1 (skill: 1 = finding) as an explicit exception or flip; README documents `2 = blockers`, so flipping is a documented-contract break |
| `report` stops writing into non-canon dirs; `format /missing` fails | scripts that pipe `format` over optional paths | low | `CONFIG_MISSING`/`USAGE` with hint |
| drop pretty-printing | none semantically; golden files in conformance for pretty outputs (`status-gaps/expected/2.out`, `gate-check-audit-sampling-fixed-seed/expected/3.out`) need re-bless | ~10 cases [INFERENCE] | re-bless |
| `schema` gains `outputs` + `schemaVersion`; `--errors` gains `exit`,`commands` | additive | none | — |

## 9. Gaps in this audit

* No success run for `artifacts migrate` (needs an s3 tier), `learn promote|approve|rollback` (needs a stored strategy), `plugin sync` (needs a plugin), `selftest` (always failed here: false-fails with fixtures missing outside the canon checkout: `reading EXPECTED-violations.json failed`; it also needs no `--repo`, so its result depends on cwd [INFERENCE]), `dashboard` port-in-use (binding 127.0.0.1:41733 while another socket held it did not fail: `DASHBOARD_PORT_IN_USE` not reproduced), `adapter validate/authorize` success was run in text+JSON via shell, not in the table harness. Failure shapes for those come from the source/`schema` only.
* `evidence add --all-scenarios` batch success not run (needs `--report`).
* `canon status` on an unconfigured dir and in my scratch repo wrote nothing; SurfaceAudit's report of `status` refreshing the scenario index in the canon checkout is consistent with "a stale scenario index is rebuilt first" in `cli.mdx`; it is declared `read` [INFERENCE: effect label ignores index rebuild].
