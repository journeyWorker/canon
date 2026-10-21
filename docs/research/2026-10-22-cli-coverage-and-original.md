# canon CLI: original research report vs the draft plan, and a coverage mechanism for `canon schema` / `canon context`

Author: CoverageAndOriginal. Read-only on the canon checkout (`main`, 0.25.0). The only canon commands I ran were the installed 0.25.0 `canon schema`, `canon context`, `canon --help` and bare group invocations, in `<scratch>` (deleted afterwards).

Citation keys:
- `R:n` = `docs/research/deep-research-report.md:n`.
- `PLAN:n` = `origin/docs/cli-grammar-plan:docs/superpowers/plans/2026-10-22-canon-cli-grammar.md:n`.
- `AUD-out:n` = `docs/research/2026-10-22-cli-audit-output.md` on the same branch, section number given where I use it.
- `SKILL:n` = `~/.omp/agent/managed-skills/cli-interface-design/SKILL.md:n`.
- `README:n` = `README.md:n` in the canon checkout.
- Code paths are under `crates/canon-cli/` unless noted.
- [INFERENCE] marks a guess I did not verify.

---

## 0. Headline

1. **The plan covers the report's grammar, error and envelope items well. It misses the report items listed in A3 and the thing the owner now asks for, a mechanical guarantee that nothing is missing from the self-description.** The report items that matter most: refused means the tree is unchanged, an idempotency declaration, `error.received`, resolved context in `meta`, `argv` for next steps, an operand rule.
2. **The report and the skill disagree on 10 topics (A2).** canon follows the skill on 5 (output flag, defaults, exit codes, the dangerous-action verb, idempotency), the report on 3 (the `{data, meta}` envelope, `--fields`, specific error codes), and 2 are open or compatible (verb set, positional count).
3. **`brief` is missing from `schema --all` because `leaf_paths` defines "command" as "has no subcommands".** `brief` has two subcommands and is also runnable (`Option<BriefCommand>`). It is the only runnable group today (I ran all 24 groups bare: 23 exit 2 with usage, `brief` ran). Details are in B2.
4. **`canon context` lists no commands.** It lists 14 record kinds, 11 enums and the join keys, and nothing else about the CLI. It also omits the 11 artifact families (`FamilyKind::ALL`). One more gap: `canon context` names the evidence kind `evidence_record`, while `query|export|purge --kind` accept `evidence` for the same kind.
5. **Recommended mechanism: one generated spec, `docs/cli/canon.cli.json`.**
   - It is built from the clap tree, META, the error registry, and JSON Schemas derived from typed `Report` structs (schemars 1.2.1 and jsonschema 0.47 are already in `Cargo.lock`).
   - `json_out::data` becomes generic over `R: Report`, so an undeclared output is a compile error.
   - A drift test follows the existing `CANON_BLESS_SCHEMA_CODES` / `cargo xtask check-generated` pattern.
   - Website reference, completions, man pages and skill-table lint are derived from it.
   - **Estimate: about 24-27 engineer-days serial, about 8-9 calendar days fanned out by module.** The cost is in typing about 49 leaves whose payload is built as `serde_json::Value` (B4).

---

# Part A. The original report against the draft plan

Column meanings:
- **Plan** is judged against `PLAN:*` with the owner's feedback applied (everything in 0.28).
- **canon 0.25** is what ships today.
- Status words: covered / partly / missing / conflicts (n/a = does not apply to a local tool).

## A1. Every concrete recommendation

### Command grammar and parsing

| # | Report recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.1 | One canonical grammar `<cli> <resource> <verb> [id] [options]`, kept across the whole CLI | R:17, 293 | **covered**: D1 grammar doc + vocabulary tables (PLAN:33), Q2 renames (PLAN:56) | `group verb` for 24 groups plus the remaining top-level commands (`status`, `format`, `query`, `purge`…) (`canon --help`) | The plan's Q2 moves toward one grammar (`records` group, `task done`, `regime show`) but leaves the final shape open. |
| A1.2 | Small canonical verb set `list/get/create/update/delete` (+ `search/apply/execute/watch` when needed); no synonyms | R:306-322 | **conflicts, unstated**. D1 will publish "a verb list" (PLAN:33), but nothing says whether it is the report's | 0.21 chose `new` (create a resource), `add` (append a ledger record), `list`, `show`, plus domain verbs (README:894-905) | The skill agrees with the report for resource-style CLIs and allows toolchain verbs (SKILL:38-41). The plan's Q2 introduces `update` (`finding close`→`finding update`), `move` (`subject move`), `show`, `list`. That is two near-synonyms (`update`/`move --to`) and breaks 0.21's rule. **Owner decision: state in D1 that canon keeps its 0.21 verbs (a ledger is append-only, so `create/update/delete` do not describe it) [INFERENCE], or adopt the report's set.** |
| A1.3 | Avoid overlapping verbs (update/upgrade/modify/edit/patch/set/change) | R:60-72 | **partly**: R5 denylist covers valued options only (PLAN:34) | `update` appears nowhere today; `migrate` vs `artifacts migrate` is the overlap (PLAN:24) | Q2 handles the known overlaps (`artifacts migrate`→`artifacts move`). A rule over command *names* is missing. R3/R5 are option-only. |
| A1.4 | At most one positional, the primary id; everything else is a named long flag | R:74-103, 999 | **missing as a rule.** Audit P1-P5 find the problems (surface audit §2.3), but R1-R16 (PLAN:34) contain no operand rule | 0.21 removed two-positional `subject status <id> <state>` (README:884-887, 905). SKILL:66-70 allows two same-kind operands | Add rule R17 "one operand kind per command; context is an option" to D2. `format <ROOT> --repo` (P1) is the live violation. |
| A1.5 | Strict parser: unknown flag/enum is a hard error before any effect; suggest, never auto-correct | R:105-114 | **covered**. clap does it; D3 says `hint` is "a working invocation, not clap's usage blob" (PLAN:35) | I ran `canon status --jsno --json`: exit 2, `code: USAGE`, `field: "--jsno"`, hint is clap's `tip: a similar argument exists…Usage:…` blob | The report wants `UNKNOWN_OPTION` and a structured `did_you_mean`. canon uses the one code `USAGE` plus `field`. Fixing the hint (PLAN:35) is enough; a second code is not needed. |
| A1.6 | One boolean convention: `--x` / `--no-x`; never `--x=true`, `--disable-x` | R:116-132, 1000 | **missing** | clap `SetTrue` flags reject `=true`. canon has `--no-agents-md`, `--no-policy` (`main.rs`, `init`) | Add a rule to D2: no `--disable-*`/`--without-*`; a negation is `--no-<flag>`. Cheap, one test. |
| A1.7 | `--` passthrough boundary | R:134-140 | n/a | no canon command wraps a subprocess [INFERENCE] | skip |
| A1.8 | Alias = input compatibility only; schema/errors/telemetry use the canonical command | R:165-173, 1002 | **covered**: R14 (`meta.command` = schema id, PLAN:34); `fmt` dropped in Q2 | `fmt` alias; `schema` and errors name `format` (README:1240-1241); `schema` shows `aliases` (schema.rs:261) | covered |
| A1.9 | Minimise hidden defaults; expose the resolved account/project/region in the result (`meta.context`) | R:142-163, 1001 | **partly**: X2/D5 fix discovery and fail-closed `CONFIG_MISSING` (PLAN:19, 37) but nothing is *reported* | `status --json` keys: `canonVersion,next,nextOmitted,policy,statusVersion,subjects,unowned,warnings` (I ran it). The resolved repo root and role are not in any output | Add `meta.context {repo, role, actor}` to the envelope (D3). Paths must be normalised in conformance. Also lets an agent detect a wrong `--repo`. |
| A1.10 | Named agent mode `--agent` / `ACME_AGENT=1` as a protocol profile (json, no prompt/color/pager, strict, bounded, stable order) | R:353-380 | **partly**: `--no-input`, `--json` implies it (PLAN:38) | One TTY-only prompt (`init`, AUD-out §2). `CANON_ROLE` is the only ambient mode | **Do not add `--agent`**: SKILL:89-92 forbids env-driven format changes. Document that `--json --no-input` is canon's profile. Caveat: `CANON_ROLE` already changes `schema` (70→17 commands) and `status` (AUD-out §2), which violates "same command line, same output". Spec generation must always pass `--all` (B3). |

### Discovery and schema

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.11 | `schema [resource [verb]]`, narrowing without dumping the tree, deterministic order | R:175-221, 446-484, 1003 | **covered** | `canon schema [path…] [--all] [--errors]`, sorted leaves (schema.rs:1-26, 514-554) | One hole: runnable group `brief` (B2). |
| A1.12 | Schema fields: type, required, enum, default, `effect`, `id positional`, `supports{dry_run,idempotency}` | R:189-219, 453-473 | **partly**: PLAN:26 adds real defaults, `possibleValues`, help on all 64 options | `ArgSchema` has name/kind/short/aliases/valueName/required/repeatable/possibleValues/default/help (schema.rs:239-253). `effect`, `exits`, `roles`, `errorCodes`, `example` exist. No `supports` | Add per-command `idempotency` and derive `supportsDryRun` from options (B3). |
| A1.13 | `output_schema` per command (`issue.v1`) | R:217 | **partly**: `outputs` entry per `--json` leaf = `{flag, kind, schemaVersion}` (PLAN:34 R8, PLAN:46), the Lute form (SKILL:147-149) | none | The plan names an output. It does not give its *shape*. Part B closes that with a JSON Schema per output. |
| A1.14 | Discovery makes zero remote/side-effect calls | R:223, 971, 983, 1003 | **covered** | `schema` reads only the in-memory clap tree (schema.rs:17-20); `tests/schema.rs` runs it in a non-repo temp dir (tests/schema.rs:6-9) | covered |
| A1.15 | Help: short synopsis + minimal example; help matches installed version; command schema without reading 400 lines | R:536, 1004 | **partly**: one-sentence `about` + `long_about`, top-level help ≤3 KB (PLAN:26, 46) | `example` per leaf in `schema`, each run by `tests/schema_examples.rs` (README:968-970). Only 9 of 70 `--help` pages have `Examples:` (AUD-out §0) | covered through `schema` |

### Output

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.16 | Bounded JSON; success `{data, meta}`, failure `{error, meta}` | R:382-403, 565-591, 1069-1097 | **covered, and stronger**: D3 `{ok, data, meta}` / `{ok:false, error, meta}` (PLAN:35) | 57 leaves `{data,meta}`, 13 bare, 6 failure shapes (AUD-out §0) | **The report supports the plan's D3 over the skill's flat envelope** (SKILL:110-113), which answers Q4 (PLAN:58). |
| A1.17 | `--output=json\|ndjson\|text` | R:327 | **conflicts, deliberately**: `-o/--output` is an output *file* (PLAN:34 R7) | `--json` on every leaf (README:1202) | Follow SKILL:95-96. Not a defect. |
| A1.18 | NDJSON for streams and bulk results | R:408-416, 1005 | **missing** | none: "No command writes NDJSON … to stdout" (AUD-out §1). NDJSON exists as *input* only (`scenario new --from -`) | Not needed until a streaming output exists. Only per-item batch results (A1.30) would use it. Backlog. |
| A1.19 | `--fields` projection as a first-class feature | R:244-250, 418-428, 1006 | **n/a in plan** | on `status`, `export`, `divergence status`, `context`, `query`, `finding list` (README:963, 1220-1222); unknown field = `UNKNOWN_FIELD` | SKILL:100-102 calls it a candidate, not a default. canon is already ahead of the plan. |
| A1.20 | Small filter vocabulary, hard error on unknown field | R:430-444 | n/a | typed filters on `query` (`--domain`, `--status`…); no DSL | covered by absence |
| A1.21 | `meta` minimal by default, `--meta=none\|minimal\|full` | R:406 | missing | `meta` always carries `command`, `next`, `warnings`, page state | Skip: over-engineering for a local tool. |
| A1.22 | Bounded lists: small default `--limit`, opaque `--cursor`, explicit `--all` | R:689-726, 1007, 986 | **covered** | `query`, `finding list`, `divergence status`, `export` (README:1211-1222). `retrieve -k` renamed `--limit` by O22 (PLAN:47) | There is no test that *every list-like leaf* is bounded (acceptance R:986 "list bounded 100%"). Mark paged leaves in the spec (B3) and test it. |
| A1.23 | Stable ordering where semantically possible; stable timestamps | R:376 | partly | `schema` sorted; `context` BTreeMap (context.rs:43-46) | spec file is deterministic by construction |
| A1.24 | JSON stdout only; ANSI/pager/spinner off when machine mode | R:968, 1005 | **covered** | one JSON doc on stdout, narration on stderr (README:1200-1207); `tests/json_contract.rs:112-187` | covered |
| A1.25 | Shell safety: pass argv vectors, not strings; bulk untrusted data as JSON stdin | R:812-824 | **missing** | `meta.next[].command` is a shell string `{command, why, actor, role}` (README:918-923) | Add `argv: [..]` beside `command` in `next` steps; one additive field. |

### Errors

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.26 | Stable string `error.code` as primary contract; exit code is a coarse class | R:594-605, 1008 | **covered** | 151 registered codes (`schema --errors`, I counted 151 entries), one `CliError` (cli_error.rs:33-141) | covered |
| A1.27 | Error fields `code, message, field/path, received, expected, retryable, retry_after_ms, hint, request_id` | R:607-619 | **partly**: D3 keeps `code, message, field, expected, hint, retryable` (PLAN:35) | `CliError { code, message, hint, retryable, exit, field, expected }` (cli_error.rs:35-46) | **Missing `received`** (the offending value). `retry_after_ms`/`request_id` are n/a (no remote). `retryable:true` is used once in the code base (AUD-out §0), so S3/store failures (`ARTIFACTS_STORE_UNREACHABLE`) say "do not retry". Fix the flag. |
| A1.28 | Exit mapping 0/2/10/20/30/40 | R:596-603 | **conflicts, deliberately**: D4 0/1/2 (PLAN:36) | 0/1/2 (cli_error.rs:17-21) | The report itself says its numbers are "a proposal, not a standard" (R:605). Keep 0/1/2. |
| A1.29 | No stack traces by default | R:621 | n/a | none | covered |
| A1.30 | Batch: stdin JSON/NDJSON, per-item result lines, atomic vs non-atomic policy stated, partial-failure class | R:541-559, 965, 1012 | **partly** | `scenario new --from -` NDJSON in, validated all-or-nothing (role-implementer.md Commands block); `review add --scenario-ids-from -`; `evidence add --all-scenarios` carries per scenario (README:994-997); `evidence vault` exits 0 with `notStored` (README:975-977) | The policy is stated per command in prose. The spec's `inputs` and `idempotency` fields (B3) record it. |

### Mutation safety

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.31 | Validation before any side effect; a refusal leaves nothing written | R:1009 | **partly**: R11 only checks that `read` leaves leave the tree unchanged (PLAN:34) | `REFUSES` exits promise "nothing is left written" (schema.rs:107-109); no test sweeps it | Add rule: every exit-2 run in the sweep leaves `git status --porcelain` and `.canon/` unchanged. |
| A1.32 | `--plan`/`--dry-run` through the same validation path as the real run | R:654-655 | **partly**: `--dry-run` coverage widened (PLAN:38) | `--dry-run` on 7 of 46 writing leaves (PLAN:25) | Add a test: for each `--dry-run` leaf, a refusing input refuses with the same code with and without it. |
| A1.33 | High-risk action = plan (effects, `risk`, digest) then `--confirm=<digest>`; stale plan rejected | R:656-687, 963-964, 1049-1058 | **conflicts in spelling, keeps the substance**: D6 = preview by default, `--commit --plan-digest <DIGEST>` (PLAN:38); R5 denylist bans valued `--confirm` (PLAN:34) | `purge --plan` then `--confirm <planDigest>`, `PLAN_CHANGED`, `PURGE_PLAN_REQUIRED` (README:1226-1235) | The plan follows the skill (SKILL:75-78) and still binds the digest, as the report wants. Gap: the report's plan payload carries `effects[]` and `risk` (R:663-673). canon's purge plan returns rungs and record keys. Add `risk` to the plan output schema. Dangerous set is Q1. |
| A1.34 | Idempotency: key-based retry safety; same key + different payload rejected; prefer set/apply over toggle/append | R:623-652, 1010 | **missing** | content-based: an identical `evidence add`/`review add` writes nothing and answers `meta.unchanged: true` (README:971-973). No key. Creates refuse on repeat (`SUBJECT_EXISTS`, `CHANGE_EXISTS`, `FEATURE_EXISTS`, `FINDING_SEQ_OCCUPIED` in `schema_codes.rs`) | A key is not needed because writes are content-addressed, which fits SKILL:85. After a lost response a `subject new` retry returns `SUBJECT_EXISTS` rather than the record [INFERENCE: I did not run it]. Add `idempotency` to every write's spec row (`content` / `refuses-on-repeat` / `none`) and a test that repeats each write in the sweep. |
| A1.35 | Preconditions/stale-plan detection | R:1011 | n/a | `PLAN_CHANGED`, `BRIEF_CHANGED_SINCE_APPROVAL`, `BRIEF_CHANGED_SINCE_PAGE_LOAD`, `POLICY_CHANGED_SINCE_APPROVAL` (`schema_codes.rs`) | covered |
| A1.36 | No prompts/editor/browser; every operation has a timeout | R:1015 | **covered** for prompts (`--no-input`, PLAN:38); timeout n/a | `init` is the only prompt | covered |

### Remote-service items

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.37 | Rate-limit error with `retry_after`, bounded auto-retry | R:728-756, 1013 | n/a | S3 tier only | Only the `retryable` truthfulness (A1.27) applies. |
| A1.38 | `auth status`, non-interactive credentials, secret never as argv | R:758-794, 1014 | n/a | S3 credentials come from `CANON_S3_*` env (README "Live tiers: env contract") | covered by absence |
| A1.39 | Sandbox: filesystem/network/credential scope, budgets | R:796-810, 1016 | n/a (runner concern) | `adapter authorize` reports `sandbox_enforced` (AUD-out §1) | out of scope |

### Telemetry, evaluation, compatibility

| # | Recommendation | R cite | Plan | canon 0.25 | Note |
|---|---|---|---|---|---|
| A1.40 | Local telemetry: canonical command, latency, exit class, no argument values; opt-out | R:826-857, 984, 1017 | **missing** | none. Only `query`/`export` append `.canon/audit/access.jsonl` (AUD-out §2). Dogfood evals count calls from session transcripts (`evals/dogfood/trace_check.py`, README:999-1001) | SKILL:163-172 specifies the local-only form. Per your "backlog, not over-engineering" stance: **file an issue, do not build in 0.28.** |
| A1.41 | Metrics (task success, first-command validity, discovery calls, tokens/success, recovery depth, …) | R:902-947 | **partly**: each release gets a dogfood eval of wrong-shape calls and `--help`/`schema` probes (PLAN:51) | evals 8-9b report calls/scenario and `schema` probes (README:1297-1313, 1094-1120) | covered enough. |
| A1.42 | Regression suite of 19 scenarios (cold discovery, field minimisation, unknown flag, invalid enum, huge collection, pagination, destructive, stale plan, batch mixed, special characters, no TTY, output purity, compatibility, alias canonicalization, discovery isolation, injection, …) | R:949-972 | **missing as a list**; the plan only has a per-release eval | Partly in tests: output purity `json_contract.rs`; discovery isolation `schema.rs`; pagination `paging.rs`; stale plan `purge`; alias `schema.rs` (README:1291-1293). **No test** for special characters in names/`--`, or argv injection | Turn rows into CI tests where they can be deterministic: special characters and injection (an id with space/newline/`--`/`;`), refused ⇒ unchanged (A1.31), repeat-write (A1.34). |
| A1.43 | Nine hard acceptance targets (JSON parse 100 %, unknown-option fail-fast 100 %, 0 interactive stalls, 0 unapproved destructive effects, 0 duplicate effects, 0 remote calls in discovery, 0 secrets in telemetry, error code 100 %, bounded list 100 %) | R:974-988 | **missing** | `json_contract.rs` already pins parse and coded errors on every leaf | Make the nine into assertions over the spec: error code present on every exit ≠ 0, bounded default on every paged leaf, destructive ⇒ plan/commit. |
| A1.44 | Additive schema evolution; field/enum removal needs a deprecation period; contract regression tests in CI | R:1018-1020 | **conflicts**: R3 is "a documented hard cut" of 13 reports (PLAN:49, 59), and the owner now wants renames + envelope in one release | conformance corpus pins exit/stdout/stderr (`conformance/README.md:1-20`); deprecation precedent: 0.21 warn → 0.22 remove (README:907-913), 0.24 → 0.25 refuse-with-replacement (`tests/one_name.rs:202-248`) | In one release a deprecation window cannot exist. Use the repo's own 0.25 pattern for renames (old spelling refused with a usage error naming the replacement, `removed.rs`). For the envelope, additive fields (`ok`, `schemaVersion`) plus moving 13 bare reports under `data` stay a hard cut. **Owner decision (Q5), now the only breaking surface.** |
| A1.45 | Model it as typed RPC first, shell syntax second; command/option/effect/output/error/context relations (the ER diagram) | R:486-524, 1151 | n/a | | **This is the design brief for Part B.** The spec document is that ER model, serialised. |

**Tally of the 45 rows by the Plan column:** covered 10 (A1.1, 5, 8, 11, 14, 16, 22, 24, 26, 36), partly 12 (3, 9, 10, 12, 13, 15, 23, 27, 30, 31, 32, 41), conflicts 5 (2, 17, 28, 33, 44), missing 9 (4, 6, 18, 21, 25, 34, 40, 42, 43), n/a 9 (7, 19, 20, 29, 35, 37, 38, 39, 45). Of the 5 conflicts, A1.17 and A1.28 are deliberate and right (the skill wins); A1.2 and A1.44 need an owner decision; A1.33 is spelling only.

## A2. Where the report and the skill disagree, and what canon follows

| Topic | Report | Skill | canon today | Plan | Verdict |
|---|---|---|---|---|---|
| Machine-output flag | `--output=json\|ndjson\|text` (R:327) | `--json`; `-o/--output` is a file/format (SKILL:95-96) | `--json` | R7 follows the skill (PLAN:34) | Skill. |
| Default format driven by mode/env | `ACME_AGENT=1` implies json (R:359-378) | never change the default by env or TTY (SKILL:89-92) | explicit `--json`; but `CANON_ROLE` changes `schema`/`status` (AUD-out §2) | `--json` implies `--no-input` only (PLAN:38) | Skill. `CANON_ROLE` is the one remaining violation; spec generation must neutralise it. |
| Envelope | `{data, meta}` / `{error, meta}` (R:384-403, 567-591) | flat `{command, schemaVersion, ok, …fields}` (SKILL:110-113) | `{data, meta}` | D3 `{ok, data, meta}` (PLAN:35) | Report (and canon). Settles Q4. |
| Exit codes | 0/2/10/20/30/40 (R:596-603) | 0/1/2 (SKILL:106-107) | 0/1/2 | D4 0/1/2 | Skill. |
| Dangerous action | `--plan` + `--confirm=<digest>` (R:659-683) | preview by default + `--commit` (SKILL:76-78); `--confirm` is a denylisted valued name (PLAN:34) | `purge --plan/--confirm` (README:1226-1235) | D6 `--commit --plan-digest` | Skill for the verb, report for the digest binding. |
| Verb set | `list/get/create/update/delete` (R:306-320) | same for resource CLIs; `new/fmt/lint/check…` for toolchain CLIs (SKILL:38-41) | `new/add/list/show` + domain verbs | Q2 mixes | Open (A1.2). |
| `--fields` | first-class (R:418) | candidate only (SKILL:100-102) | on 6 commands | silent | canon's choice stands. |
| Error code vocabulary | specific strings (`UNKNOWN_OPTION`, `INVALID_ENUM`) (R:109, 581) | a small fixed set (`E-CLI-USAGE/IO/INPUT`) (SKILL:119-123) | 151 specific codes, one generic `USAGE` for clap | keeps registry | Report, except clap errors. |
| Idempotency | keys (R:625-650) | "a second run is a no-op" (SKILL:85) | content-based `meta.unchanged` | silent | Skill. |
| Telemetry | central (R:826-857) | local JSONL only, opt-out (SKILL:163-172) | none | none | Backlog. |
| Positional count | ≤1 primary id (R:92-99) | one operand kind; two of the same kind allowed (SKILL:66-70) | 0.21 enforces single kind | no rule | Compatible; add the rule. |

## A3. What to add to the plan because of the report

Everything below is additive to the draft, in order of value. The plan is one release now (0.28), so each item lands in the same release.

1. **Part B** (the generated spec with per-output JSON Schemas). It replaces the weak `outputs` of R8.
2. **R17 operand rule** (A1.4) and **R18 boolean rule** (A1.6) in the D2 test.
3. **Refused ⇒ tree unchanged** and **repeat-write** sweeps (A1.31, A1.34), with `idempotency` declared per write.
4. **`error.received`**, `retryable` made truthful for store/network codes, and clap hints rewritten to a working invocation (A1.5, A1.27).
5. **`meta.context`** (resolved repo root, role, actor) and **`meta.next[].argv`** (A1.9, A1.25).
6. **`risk` and `effects[]` in the `purge` plan** (A1.33).
7. **CI tests for the report's acceptance table** (A1.42, A1.43) where deterministic; special characters and injection first.
8. **Issue only:** telemetry (A1.40), NDJSON outputs (A1.18).

Also found while reading:
- README:911-913 says "`format` is `write`" (0.21), but META declares `format` as `Read` (schema.rs:150) and the output audit lists it as `read`. Prose and table already disagree. The generated spec removes that class of drift.
- `canon-policy/src/registry.rs` says "twelve closed RecordKinds"; the code has 14 (`canon-model/src/envelope.rs:51-66`). Stale comment, same class.

---

# Part B. Coverage design

## B1. What exists today

| Piece | Where | What it guarantees |
|---|---|---|
| Command tree from clap | `schema.rs:414-481` (`arg_schema`, `command_schema`) | Arguments, options, aliases, enums, defaults cannot drift from the parser. |
| Effects, exits, roles | `META` (schema.rs:118-189), one row per **leaf**; `check_metadata` (:214-235) fails on a leaf without a row or a row without a leaf | Effect/exit/roles stay in step with clap, for leaves. |
| Examples | `schema_examples.rs` (84 lines); `check_examples` (schema.rs:386-408); `tests/schema_examples.rs:114-174` runs each in a scratch repo | One runnable example per leaf. 19 need state a scratch repo lacks (`NEEDS_STATE`, tests/schema_examples.rs:16-36); `dashboard` is skipped. |
| Error registry | `cli_error.rs:159-163` `all_codes()`, `CODES` (:163-315): 151 `(code, meaning)` pairs. `emit` debug-asserts the code is registered (:117, :132) | A code that is not registered cannot be emitted in a debug build. |
| Per-command codes | `schema_codes.rs` (generated); `tests/schema_codes.rs` derives it by scanning sources and fails on any difference (`CANON_BLESS_SCHEMA_CODES=1` rewrites) | Sets are at module granularity: a superset, not exact (schema.rs:281-283). Also `every_registered_code_is_emitted_by_some_command` (tests/schema_codes.rs:223-229). |
| `--json` sweep | `tests/json_contract.rs:112-187` | Every leaf in bare `schema --json` accepts `--json`, prints exactly one JSON value, and a non-zero exit has a registered `error.code`. It does **not** check the payload's shape. |
| One-name rule | `tests/one_name.rs:111-169` reads the tree through `schema --all` | Flag spellings and `--kind` values. |
| Record schemas | `canon-model/src/schema_export.rs` (14 record kinds via `schema_for!`, 11 family kinds); committed `canon-model/schemas/*.schema.json` (25 files); `cargo xtask check-generated` and the test `canon_model::gen::tests::committed_generated_output_matches_current_source` (`xtask.rs:1-9`); `.cargo/config.toml` alias | **The model for the new drift test.** |
| `canon context` | `context.rs:312-343` `AuthoringSurface`: `capabilityVersion, kinds, enums, joinKeys, policy, vocab, review, cel` | Built from the same `SchemaRegistry::load()` the validator uses (context.rs:402-431). Tested for the 14 kinds (tests/context.rs:35-38). |
| Conformance corpus | `conformance/cases/*` (103 cases) replayed by `tests/conformance.rs` | Pins stdout/stderr/exit, including about 30 JSON bodies. |
| Deps already locked | `schemars 1.2.1`, `jsonschema 0.47.0` (`Cargo.lock:2878`, `:1886`); canon-cli does not yet depend on either (`canon-cli/Cargo.toml`) | No new crates for the schema work. |

## B2. What is missing, and why `brief` is absent from `schema --all`

**Why `brief` is missing.**
- `leaf_paths` (schema.rs:192-211) walks the clap tree and records a path only when `subs.is_empty()` (:195). `brief` has subcommands (`approve`, `check`), so it is a "group" and is not recorded.
- But `brief` is also runnable: `action: Option<BriefCommand>` (main.rs:225-227), and `None => canon_cli::brief_check::run_next(...)` (main.rs:2203-2206). Bare `canon brief` prints state and the next steps.
- META has only `brief approve` and `brief check` (schema.rs:123-124). `check_metadata` compares META against `leaf_paths`, so it cannot notice the missing row.
- `canon schema brief --json` resolves to a `Group` (schema.rs:363-364) and lists the two children. The runnable `brief` itself has no effect, exits, options (`--change-id`, `--repo`, `--json`), example or error codes anywhere.
- Evidence: `canon schema --all --json` lists 70 commands, 2 under `brief`. I ran every group bare: 23 print usage and exit 2, `brief` ran (exit 1, `NO_BRIEF`). `brief` is the only runnable group.
- The existing sweeps (`json_contract`, `schema_examples`, `one_name`) all start from this leaf list, so `brief` escapes all of them too. The plan's R12 ("no runnable group", PLAN:34) removes the case by renaming bare `brief`→`brief status`. The coverage test below should still walk groups, so any future runnable group fails loudly.

**Other gaps.**

| Gap | Evidence |
|---|---|
| No `group` nodes: a group's `about`/long text is not retrievable | `Resolved::Group` lists leaf entries only (schema.rs:531-543) |
| No output shapes at all: no `outputs`, no JSON Schema, no report ids | `CommandSchema` fields (schema.rs:255-284) |
| No input documents (NDJSON batch line, context manifest, adapter response, canon.yaml, policy.yaml, learn evaluation) | `BatchLine` (scaffold.rs:500, `deny_unknown_fields`), `AdapterResponse` (adapter.rs:87) are typed Rust structs but unexposed |
| No environment variables (`CANON_ROLE`, `CANON_ACTOR`, `CANON_S3_*`, `CANON_SKILLS_SOURCE`, `CANON_GATE_ADVISORY`, `CANON_DASHBOARD_DIST`…) | only two use clap `env =` (main.rs:1486, 1514); the rest are `std::env::var` literals scattered across the crate |
| Global option `--as` appears per command only via clap `global=true` propagation, no top-level "global" section | `canon schema status` shows `as` among options |
| Per-command error codes are a module-level superset | schema.rs:281-283 |
| Exit meaning per code missing: `CODES` is `(code, meaning)` only (cli_error.rs:163) | exit class is chosen per call site (`refused` vs `failed`, cli_error.rs:50, 55) |
| Role filtering: bare `schema` hides commands of other roles (`CANON_ROLE`/`--as`) | schema.rs:532 (`role = if all { None } else { Role::current(None) }`). `json_contract.rs:21` does not `env_remove("CANON_ROLE")`, so a CI environment with the variable set would shrink the sweep. It would fail loudly on `leaves.len() > 60` (:118), not silently. |
| `canon context` has no commands and no artifact families | `AuthoringSurface` fields above. `family_schemas()` (11) are exported (schema_export.rs) but not surfaced. |
| `evidence` vs `evidence_record` | `canon context` key `evidence_record`; `query`/`export`/`purge --kind` accept `evidence` (`canon schema query --json`). The same record kind has two names. |
| Prose command tables are hand-written | website `cli.mdx` "Surface" table (packages/website/src/content/docs/cli.mdx:184-200, en + ko); `Commands` blocks of `role-*.md` (canon/skills/reference/role-implementer.md:9-30) |
| No completions and no man pages | no `clap_complete`/`clap_mangen` in `Cargo.toml` |
| The clap tree lives in the binary (`main.rs:52` to about `:1740`) | tests can only reach it by spawning `canon` |

## B3. Proposed mechanism: one generated command spec

### Document

`docs/cli/canon.cli.json`, checked in, generated, deterministic (sorted keys via `serde_json::Value`'s BTreeMap; arrays in declared/sorted order):

```text
{
  "specVersion": 1,
  "cli": { "name": "canon", "version": "0.28.0" },
  "global": {
    "options":   [ { --as … } ],                       // clap global args
    "env":       [ { "name": "CANON_ROLE", "effect": "…", "values": [...] }, … ],
    "exitCodes": [ { "code": 0, … }, { "code": 1, … }, { "code": 2, … } ]
  },
  "envelope": { "success": <JSON Schema>, "failure": <JSON Schema>, "meta": <JSON Schema> },
  "errors": [ { "code", "meaning", "exit", "retryable", "commands": [...] } ],   // all 151
  "commands": [ {
      "id": "brief check",  "path": ["brief","check"],
      "nodeKind": "leaf" | "group" | "runnable-group",
      "aliases": [], "deprecatedAliases": [],  "hidden": false,
      "about", "longAbout"?,
      "effect", "effectNote"?, "idempotency", "roles", "paged": bool,
      "arguments": [...], "options": [...], "requiredOneOf": [...],
      "exits": [...], "errorCodes": [...],
      "outputs": [ { "flag": "--json", "kind": "report|artifact|stream",
                     "when": "…", "schemaRef": "#/schemas/BriefCheckReport", "schemaVersion": "0.28.brief-check" } ],
      "inputs":  [ { "flag": "--from", "format": "ndjson", "schemaRef": "#/schemas/ScenarioBatchLine" } ],
      "examples": [ { "args": "canon brief check", "needs": "…", "exit": 2 } ],
      "subcommands": ["brief approve","brief check"]
  }, … ],
  "schemas": { "BriefCheckReport": { …JSON Schema 2020-12… }, … },
  "records": { "kinds": ["change", …14], "families": [ …11 ], "schemaDir": "crates/canon-model/schemas" }
}
```

It is the report's ER model (R:486-524) serialised: CLI → command → option; command → effect; command → output schema; command → error schema; command → context.

### Generator

- **Where.**
  - Step 0: move the clap definitions out of `main.rs` (lines 52-1735: `Cli`, `Command`, 24 subcommand enums and the `Args` structs) into `canon_cli::cli`. `main.rs` keeps `run`. This makes `Cli::command()` reachable from tests and a spec binary, and is mechanical (`canon_cli::` → `crate::`). About 0.5 day.
  - New module `schema/spec.rs`, run from a **hidden** dev entry. For example `canon schema --spec` (hidden from `--help`, but present in the spec as a hidden command) or a `canon-cli/src/bin/spec.rs`.
- **Input.** It walks *all* nodes: leaves, groups and runnable groups. A node's `nodeKind` comes from `subcommands.is_empty()` and whether the enum is `Option`/`arg_required_else_help`. A runnable group is a clap command with subcommands and `!is_subcommand_required_set()`.
- **Role-independent.** It always uses the `--all` path (`Role` ignored) and ignores `CANON_ROLE`.
- **META rows for groups.** Change `META` to cover every node: `brief` gets its own row (read; exits 0/1; roles planner/reviewer). `check_metadata` is rewritten over *all* nodes, so the `brief` hole becomes a failing test. Pure groups need no row; they carry only `about`.

### Drift test and bless

- `tests/cli_spec.rs`: regenerate in memory, byte-diff against `docs/cli/canon.cli.json`.
- `CANON_BLESS_CLI_SPEC=1` rewrites the file, following `CANON_BLESS_SCHEMA_CODES` (tests/schema_codes.rs:15) and `cargo xtask write` (xtask.rs:12-15).
- A `cargo xtask`-style alias (`cargo xtask spec`) is optional. `canon-model`'s xtask cannot depend on `canon-cli`, so the bless stays a test env var.
- Version policy: `cli.version` is in the file, so each release regenerates it; this is part of the release checklist.

### Derived artifacts (nothing hand-written that restates the command list)

| Artifact | Derivation | Check |
|---|---|---|
| `canon schema` / `canon schema --errors` | the same builder, reading the same structures | existing tests plus equality with the spec |
| **`canon context` command section** | `commands` index from the spec: id, effect, roles, outputs, `example` (see B6) | drift test: every spec command in `context.commands` |
| Website CLI reference (en + ko) | a bun script `scripts/gen-cli-reference.ts` renders `packages/website/src/content/docs/generated/cli-reference.mdx` from the JSON (tables: command, effect, roles, options, exits, error codes, example, output schema link). The hand-written narrative in `cli.mdx` keeps prose and links to the generated page. | `bun run docs:check` (regenerate and `git diff --exit-code`) in `test.yml`. The ko page gets the generated English tables and its own prose [INFERENCE: translating `about` strings is a separate decision]. |
| Skill command tables | The curated `Commands` blocks (`role-*.md`) are workflow-ordered lines with placeholders, so **generate a lint, not the text**. Each line must parse against the spec: command exists, every flag exists, required flags present, enum values valid. A generated appendix `reference/commands.md` lists the rest. This is the plan's R16 doc lint (PLAN:34) using the spec as its source. | test in `tests/cli_docs.rs` |
| Shell completions | new `canon completion <bash\|zsh\|fish>` (clap_complete), as `gh`/`kubectl`/`wrangler` do | it is a command, so the spec test covers it |
| Man pages | `clap_mangen` from `Cli::command()` at release packaging into `docs/cli/man/` | optional; same drift test pattern |
| Conformance expectations | schema-validated: every JSON `expected/N.out` is checked against the declared schema of the step's command | `tests/conformance.rs` addition |

## B4. How output payloads get schemas

### Where things stand

I counted the emission sites (`json_out::data`, `data_with_next`, `data_with_meta`, `envelope`, `envelope_with_next`; grep in `src/*.rs`, about 55 call sites). They all hand `serde_json::Value` to `json_out`. About 21 leaves build the `Value` from a typed `Serialize` struct; about 49 build it with `serde_json::json!` or a helper that returns `Value` (`tier::format_json`, `ingest::summary_json`, `outcome.json()`). Some of those helpers wrap typed internals, so the conversion may be cheaper than a rewrite.

| Style | Leaves (from `AUD-out` §1 keys + grep) | Evidence |
|---|---|---|
| **Typed `Serialize` struct** | `status` (status.rs, 10 derives), `brief check` (brief_check.rs, 10), `brief approve` (brief_approve.rs), `context` (context.rs, 15), `context-pack new/show/verify` and `prompt add/show` (context_pack.rs, 11), `adapter validate/authorize` (adapter.rs, 11), `schema` (schema.rs, 7), `subject new/adopt/status/unadopt` (canon-model `Subject`, already `JsonSchema`), `skills install/check/doctor` (skills.rs, 3), `ingest plans` (plans.rs, 4), `ingest artifacts` (artifact_ingest.rs, 4) | `grep -c 'derive(.*Serialize'` per file; main.rs:2733, 2785 (`to_value(&pack)`, `to_value(&bundle)`); subject.rs:1016 |
| **Built with `json!`** | `artifacts verify/migrate`, `change new`, `demo add/init`, `dispatch begin/end/diff`, `divergence add/accept/defer/resolve/promote/status`, `evidence add/vault/approval-payload`, `export`, `feature new`, `finding add/close/list`, `format`, `gate check/install-hooks/promote/selftest/task`, `ingest sessions`, `init`, `inventory sync`, `learn approve/promote/rollback`, `ledger compact`, `migrate`, `plugin sync`, `policy approve/show`, `purge`, `query`, `regime-key`, `report`, `retrieve`, `review add`, `scenario new`, `selftest`, `tier age`, `dashboard` | `json!(` appears 247 times in 36 files (including unit tests); non-test: `evidence.rs` 21, `dispatch.rs` 15, `finding.rs` 15, `gate.rs` 14, `retention.rs` 11, `divergence.rs` 10 |

The typed/`json!` split is my classification from the grep and the audit's `data` keys, not a per-site read. [INFERENCE] ±5 leaves.

### Mechanism

1. **Add `schemars` to `canon-cli`** (version already locked, `features = ["chrono04"]`).
2. **Introduce a `Report` trait** (new `crates/canon-cli/src/report_schema.rs` or `json_out.rs`):
   ```rust
   pub trait Report: Serialize + JsonSchema {
       const COMMAND: &'static str;          // "gate check"
       const NAME: &'static str;             // "GateCheckReport"
       const VERSION: &'static str;          // "0.28.gate-check"
   }
   pub fn data<R: Report>(r: &R) { … }       // replaces data(command, Value)
   pub fn data_with_next<R: Report>(r: &R, after: &After) { … }
   ```
   - `json_out::data(command: &str, data: Value)` (json_out.rs:30) is deleted; **every emitter must name a `Report` type**, so an undeclared output is a compile error. This is the "OpenAPI-like" step: the handler's return type *is* the contract.
   - `to_value(&r)` stays inside `envelope`, so key order is unchanged. Without serde_json's `preserve_order` (not enabled: `Cargo.toml:47` has no features), `Value` objects print with sorted keys. **The 30 JSON-pinned conformance bodies do not change** (the audit's "bare reports" move under `data` per D3, a separate, planned diff).
3. **One `OUTPUTS` table**, the single table that both `envelope` and `schema` read (SKILL:149-150's rule). One entry per `(command, flag, variant)`: `{flag, kind: Report|Artifact|Stream, report: Option<fn() -> Schema>, version}`. Commands with several shapes declare each as its own entry with `when` text: `purge` plan vs commit, `report` write/check/snapshot, `schema` list vs one, `export` file vs page, `context --fields`. The `data` schema is the `oneOf` of its entries.
4. **Artifacts and streams** get an entry with `kind` and, when JSON, a schema of their own:
   - `export --out` manifest (`canon.retention.export`), `report --snapshot` manifest.
   - `evidence approval-payload` (raw bytes, `kind: artifact`, no schema).
   - `context-pack`/`prompt` files under `.canon/`.
5. **`ErrorBody`**: one derived schema for `{error:{code,message,hint,retryable,field?,expected?,received?}}`, plus the `data`+`error`+`meta` failure form (gate check, format, report --check, scenario new --from). It lives in `envelope.failure`.
6. **No `serde_json::Value` inside a `Report`** except through a named `OpenObject` wrapper whose schema says so. The generated schema is scanned: a subschema without `type`/`properties`/`$ref`/`enum`/`oneOf`, outside an allow-list, fails the test.
7. **Input documents**: derive `JsonSchema` on `BatchLine` (scaffold.rs:500), `AdapterResponse` (adapter.rs:87), the context manifest, prompt manifest and learn evaluation types. They are listed under `inputs` in the spec. `policy.yaml` already has `family-policy.schema.json`; `canon.yaml` has no schema [INFERENCE: a `CanonYaml` struct is outside this report's scope; flag it as a gap].

### The migration cost (what the days buy)

- About 21 typed reports: add `derive(JsonSchema)` and `impl Report`. Cheap, but some use `#[serde(flatten)]`, `skip_serializing_if`, or `Value` fields that schemars renders loosely.
- About 49 `Value`-built payloads: write a struct per payload. Small ones (`regime-key`, `gate install-hooks`, `gate selftest`, `tier age`, `migrate`, `feature new`) take about 15 minutes each. Large ones (`evidence add`, `dispatch begin/end`, `gate check`, `gate promote`, `purge`, `export`, `query`, `ingest sessions`, `init`, `report`) take 1-3 hours, mostly because their fields are inconsistent (camelCase and snake_case in one document, AUD-out §1). Do not "fix" the casing in this pass; the struct must reproduce today's bytes. The re-casing is a separate, breaking choice.
- This work **is** the plan's R3 (the 13 bare reports moving into `data`): do both in one touch of each emitter.

## B5. How a test proves 100 % coverage

Tests live in a new `tests/cli_spec.rs` plus additions to `json_contract.rs`/`conformance.rs`. Each row says what fails.

| # | Assertion | Mechanism | Fails when |
|---|---|---|---|
| T1 | Every clap node is in the spec: leaves, groups, runnable groups | walk `Cli::command()` recursively (not only leaves), compare ids with `spec.commands[].id`, both directions | a command is added without a spec row, or the spec names a command clap does not have. This is the `brief` hole. |
| T2 | Every node has metadata | `check_metadata` rewritten over all nodes; `nodeKind: runnable-group` requires a META row | `brief` without a row |
| T3 | Every option and argument is in the spec, with help, value name, default, `possibleValues` for closed sets | recursive walk of `get_arguments()` incl. globals; R9/R10 of the plan | an option without help, an enum with empty `possibleValues` |
| T4 | Spec equals regeneration (drift) | in-memory generation vs `docs/cli/canon.cli.json` | any change not blessed |
| T5 | Every error code is in `errors[]` with meaning and exit; every code a command lists is registered; every registered code is emitted by some command | `all_codes()` vs spec; existing `tests/schema_codes.rs` checks | unregistered or unreachable code |
| T6 | Every command has ≥1 `outputs` entry, or an explicit `outputs: []` with a reason (`group`, `serve`) | spec scan | a leaf with `--json` and no output declared |
| T7 | **Every JSON document emitted is valid against its declared schema** | (a) **in the process:** in debug builds (`cfg!(debug_assertions)` or `CANON_VALIDATE_OUTPUT=1`) `json_out::envelope` validates the document against its `Report` schema with `jsonschema` and panics on mismatch. Because `cargo test` runs the debug `canon` binary, **every `--json` invocation in the whole suite becomes a validation** (the plan counts about 293 test invocations parsing `--json`, PLAN:27). (b) **outside the process:** `json_contract.rs` validates each leaf's stdout against `spec.commands[leaf].outputs` / `envelope.failure` | an emitted field is not in the schema, or a required one is missing |
| T8 | **Conformance bodies validate** | `tests/conformance.rs`: for each step with JSON in `expected/N.out`, validate against the command's schema | a re-blessed body that no longer matches the schema |
| T9 | **Witness coverage**: every declared output entry was emitted at least once by the suite | debug builds append `{command, output}` to `$CANON_OUTPUT_WITNESS`; a final test runs the example set and the fixture-based sweep and compares the witness set to `outputs[]` | a declared output nobody ever exercises. Today 6 of 70 leaves have no success run in the audit's scratch repo (`artifacts migrate`, `learn approve/promote/rollback`, `plugin sync`, `dashboard`), and `NEEDS_STATE` lists 19. Each needs a richer fixture, or sits in an explicit shrinking `UNWITNESSED` list with a reason. |
| T10 | No untyped `data()` | compile-time (generic `Report` bound). Plus a grep test that no module outside `json_out.rs` calls `serde_json::json!` to print to stdout | a new `println!("{}", json!(..))` |
| T11 | No `any` subschema in any `Report` | scan `schemas` | a `Value` field slipped in |
| T12 | Context coverage (B6) | see below | a kind, family or command missing from `canon context` |
| T13 | Doc coverage: every command or option named in `README`, website, skills, AGENTS block, hints and `meta.next` strings exists in the spec | the plan's R16 (PLAN:34), sourced from the spec | a doc names a removed spelling |
| T14 | Mutation tests | like Lute's `mutated()` pattern (PLAN:34): delete a META row, drop an option's help, add a `json!` print, add a runnable group; each must fail the right test | the test is weaker than it looks |

Honest limit on T7/T9: for a typed report, the schema comes from the same struct that serialises, so validation proves the *envelope and the `oneOf` selection*, not the field shapes. Its power is where the type is looser than the schema (`serde(skip)`, `flatten`, `Value` fields, hand-patched `Value` after `to_value`). T11 closes the `Value` loophole. The value of the sweep is catching a path that bypasses the type, which is why T10 exists.

## B6. `canon context` coverage

What `canon context` lists today: `capabilityVersion, kinds (14), enums (11), joinKeys (9), policy, vocab, review, cel` (I ran `canon context --json | jq keys`). It has no commands and no families. Proposed (`capabilityVersion` 5→6):

1. **`commands`**: compact index generated from the spec: `[{id, nodeKind, effect, roles, outputs:[ids], example}]`, sorted. About 70 rows. Detail stays in `schema <id>`; the outline gets one line per command. Size: the JSON is about 22 KB today (AUD-out §2), so this adds a fifth. Alternative: only under `--fields commands` (`--fields` exists on `context`, README:963). **Recommendation: always include it; this is the first thing an agent needs.** Owner can choose.
2. **`families`**: the 11 `FamilyKind::ALL` entries with their layout templates and a pointer to `crates/canon-model/schemas/family-*.schema.json`.
3. **`records`**: a pointer to each kind's `schemas/<kind>.schema.json` (14 files exist).
4. **One name per kind**: `evidence_record` (context key) vs `evidence` (`query|export|purge --kind`). Decide one; the `--kind` value set must equal the context `kinds` keys. Test T12 asserts that.
5. **Tests (T12):**
   - `context.kinds` keys == `RecordKind::ALL` (14) and `context.families` == `FamilyKind::ALL` (11), both directions.
   - Every `--kind` `possibleValues` set in the spec ⊆ `context.kinds`.
   - Every spec command appears in `context.commands`, and the reverse.
   - JSON and outline agree (the existing `check_json_outline_agreement`, context.rs:892, extended).
   - Fix the stale "twelve" comments in `canon-policy/src/registry.rs:191-194` while there.

## B7. Effort

One engineer-day = 6 productive hours. "Fanned" = parallel agents by module, one integration owner.

| # | Work | Days | Notes |
|---|---|---|---|
| 0 | Extract clap tree into `canon_cli::cli`; main.rs keeps dispatch | 0.5 | mechanical, unblocks everything |
| 1 | Node model: groups + runnable groups in `leaf_paths`/`META`/`check_metadata`/`resolve`/`list_entries`; `brief` row + example + codes | 1 | fixes the bug independent of the rest |
| 2 | Spec builder + bless/drift test (`global`, `env`, `errors`, `envelope`, `commands`, `records`) | 2 | env-var registry plus a scan test for `std::env::var("CANON_` literals (the `schema_codes.rs` pattern) is 0.5 of it |
| 3 | `Report` trait, `OUTPUTS` table, `ErrorBody`; `json_out` signature change | 1.5 | |
| 4 | Type the ~20 already-typed reports (derive + impl) | 1.5 | |
| 5 | Type the ~49 `Value`-built payloads | 9–12 | the bulk. Fan out by module (evidence, dispatch, gate, divergence, finding, retention/export/purge, ingest, init/migrate/skills, misc). With 5 agents about 3 calendar days, plus integration. |
| 6 | Input-document schemas (`BatchLine`, manifests, adapter response, evaluation) | 1 | |
| 7 | Tests T1-T14, mutation tests, witness fixtures for the 19 `NEEDS_STATE` leaves (or allow-list) | 3 | the fixtures are 2 of the 3 |
| 8 | `canon context`: `commands`, `families`, kind-name unification, capability 6, outline, tests | 1 | the `evidence`/`evidence_record` rename is a user-visible change |
| 9 | Derived docs: website generator (en/ko), skill-line linter (R16), completions command, man pages, CI step | 2.5 | |
| 10 | Re-bless conformance, README release section, skill/AGENTS updates | 1 | folds into the plan's D8 |
| | **Total** | **≈ 24-27 serial** | about **8-9 calendar days** fanned out; steps 5 and 9 parallelise, 0-3 do not |

The total is the sum of the rows above: 0.5+1+2+1.5+1.5+(9 to 12)+1+3+1+2.5+1 = 24 to 27.

**Risks, in order:**
1. Typing the `json!` payloads can change bytes (key order is safe via `to_value`; but `skip_serializing_if`, `null` vs absent, `Option` fields and number formats are not). Each emitter needs a before/after diff on its conformance and test bodies. Mitigation: the existing 293 `--json` test invocations are the oracle, and T8 catches schema drift.
2. Typed schemas expose today's inconsistent casing (camelCase and snake_case in one document, AUD-out §1). The schema will faithfully document the mess. Deciding to fix it is a breaking change to add to the Q5 list, not a coverage task.
3. schemars is looser than some needs: `flatten`, `untagged` enums, `u8` exits. T11 forces the issue early.
4. Witness coverage for the 19 stateful leaves is the long tail; an explicit allow-list that only shrinks is the escape hatch, and it should be visible in review.
5. The website ko page translates prose, not generated tables. [INFERENCE] a decision for you.

## B8. Decisions needed from you (new, beyond Q1-Q5)

| # | Question | My recommendation |
|---|---|---|
| Q6 | Verb set: keep canon's `new/add/list/show` + domain verbs, or the report's `list/get/create/update/delete` (A1.2)? | Keep canon's, record the reason in D1, and drop `update`/`move` from Q2 where they would create near-synonyms. |
| Q7 | `canon context`: always include the `commands` index, or only on `--fields commands` (B6.1)? | Always. |
| Q8 | Unify the kind name `evidence`/`evidence_record` (B6.4); which wins? | `evidence_record` is wrong under the 0.24 "no `_record` suffix" rule (tests/one_name.rs:149-169). Use `evidence` in `context` too. User-visible, so list it in the 0.28 Upgrading section. |
| Q9 | Add `canon completion <shell>` as a public command? | Yes: one short command that `gh`/`kubectl`/`wrangler` all have. It is then covered by the spec like any other. |
| Q10 | Per-output JSON Schemas required for every report in 0.28, or ship the spec first with `outputs[].schema` for the typed ~20 and an explicit `UNTYPED` list for the rest? | The owner asked for completeness: require all. Fall back only if risk 1 bites. |
| Q11 | Telemetry and NDJSON: issue only (A1.18, A1.40)? | Yes. |
