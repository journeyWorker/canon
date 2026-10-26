---
status: Proposed
---

# Ledger file naming

- **Scope:** the name every stored record gets, in every store: the git tier
  (`.canon/ledger`, `.canon/archive`, `.canon/ledger/_staging`), plugin
  overlay kinds (`kind=<ns.kind>/`), the sqlite and Postgres tiers'
  `(kind, id, digest)` keys, the R2 object key, and the places that read a
  name back (report views, archive scoping, the conformance normalizer).
- **Change:** `canon-0-30-ledger-naming` (`.canon/changes/`), release 0.30.
- **Measured at:** `main` 0977bce1 (canon 0.29.0). Canon's own ledger and
  archive: 3,145 files. Every file matches today's code (digest suffix equals
  the content digest, `area=` matches the content's `scenario_id`). The data
  is consistent with the code; the code has no single rule.
- **Reviewed:** two independent reviews, 2026-10-26 (findings folded in
  below).

## Today

Path: `kind=<kind>/[area=<area>/]<natural_key>__<digest12>.json`, with
`natural_key` written kind by kind in `canon-store/src/partition.rs:88-162`.
Area-scoped kinds: scenario, review, divergence.

| Kind | Area | Natural key today |
|---|---|---|
| change | — | `<change_id>` |
| task | — | `<task_id>` |
| scenario | scenario area | `<project_id>__<scenario_id>` |
| session | — | `<session_id>` with `/` → `_` |
| run | — | `<run_id>` |
| event | — | `<run_id>-<seq:010>` |
| handoff | — | `<id>` |
| review (scenario) | scenario area | `<project_id>__<scenario_id>__<pin>` |
| review (change) | literal `change` | `<project_id>__change-<change_id>__<pin>` |
| divergence | scenario area | `<project_id>__<scenario_id>__<run_seq:010>__<round:06>` |
| trajectory | — | `<run_id>` |
| strategy_item | — | `<regime_key>` with `/` → `_` |
| evidence_record | — | first of `task_id`, `scenario_id`, `run_id`, else `unscoped` |
| subject | — | `<subject_id>` |
| finding | — | `<change_id>__<round:04>__<seq:04>` |
| overlay `<ns.kind>` | — | join-key values joined by `__` |

### What is inconsistent

1. **Project scope.** Scenario, review and divergence start with
   `project_id`; evidence carries it and leaves it out.
2. **Which key.** Evidence names do not say which id they hold; `unscoped`
   is a fourth form; only the change review marks its key (`change-<id>`).
3. **Area.** Three kinds take `area=`; evidence with a scenario does not; a
   change review goes into a fake area `change`.
4. **Separators.** `__` everywhere except event (`-`).
5. **Number widths.** 4, 6 and 10 digits.
6. **Escaping.** Only session and strategy_item escape, and only `/`.
7. **Kind spelling.** `evidence_record` on disk and in records; `evidence`
   on the CLI. Every other kind is a bare noun.
8. **`area=` does nothing.** No reader selects by it; it costs a validation
   path and the change-review special case.
9. **Names are read back in places that assume today's shape:** report
   views extract the digest with `__([0-9a-f]{12})\.json$` and filter
   `kind = 'evidence_record'`; archive scoping parses stems; overlay keys are
   split on `__`; the conformance normalizer matches `__<12 hex>.json`.

## Rule

```
kind=<kind>/<component>_<component>…_<digest8>.json
```

Token cost, measured with `o200k_base` over the 3,145 paths: 28.5 tokens per
path; the 12-hex digest is 25%, the review's 40-hex pin 23%, `area=` 4%. A
separator costs 1 token as `_` or `__`; a 10-digit padded number 4, a small
plain number 1. Example: today's review path is 48 tokens, the new one 18.

- **R1. One directory level:** `kind=<kind>/`. No `area=` (decided
  2026-10-26: a directory level exists only in trees people author, i.e.
  `specs/kind=feature/area=<area>/`; an area's source is always the scenario
  id's first segment, which the name carries).
- **R2. Components are typed, escaping happens only at the path.** A table
  in `canon-model`, next to `RecordKind`, gives each kind's components as
  typed values (below). Folds, query output, cursors and error messages use
  the typed components; only the path builder escapes and joins them. A
  kind whose schema has `project_id` starts with it; an absent optional
  `project_id` is `-`.
- **R3. Alternative keys are tagged** with the field they hold: `scenario`,
  `change`, `task`, `run`. Evidence precedence is today's: `task`, then
  `scenario`, then `run`. Evidence with none is refused by the model.
- **R4. Separator:** one `_`. Ids of kebab/dot grammars never contain `_`;
  free-text ids are escaped (R6).
- **R5. Integers:** plain decimal, no padding. Nothing orders by name:
  every place that orders records sorts by body fields, then by the typed
  components in table order, then by the body digest (today
  `TierRegistry::query` sorts by `at` only, keeping scan order for ties, and
  promote breaks ties by staging scan order; both get explicit tie-breaks).
  Records that share every body sort field may come out in a different order
  than in 0.29; canon's ledger has 16 evidence records sharing one `at`.
- **R6. Canonical text, then escape.** A typed component is written in its
  canonical lowercase form (a ULID such as `RunId` is case-insensitive
  Crockford, so it is written lowercase and stays 26 characters). A
  free-text component (session id, regime key segment, handoff id) has every
  byte outside `[a-z0-9.-]` (uppercase and `_` included) replaced by `+` and
  two lowercase hex digits. `+` is chosen because R2's object-store client
  leaves it as is (it percent-encodes `%` and `~`); the key is the same string
  in every store, and names are safe on case-insensitive file systems.
  Reversible. A name over 200 bytes replaces its free-text component with
  `h` and the first 16 hex digits of that value's SHA-256; the value stays in
  the body.
- **R7. Digest:** `_<digest8>` last, the first 8 hex digits of SHA-256 over
  the canonical JSON (sorted keys, compact, UTF-8). No seed: every body
  carries its own `at`. 8 hex = 2^32 values; the measured maximum is 9 files
  under one key (ledger + archive), collision chance about 1e-8
  (n²/2^33). A collision is never silent in any store: the git tier refuses
  an existing path and a ledger path that the archive holds with different
  bytes; R2 compares the stored bytes instead of reporting `deduped`; sqlite
  and Postgres compare canonical JSON on a key conflict instead of
  `DO NOTHING` (Postgres keeps `body` as JSONB, not bytes). A batch write
  runs in one transaction, so a collision leaves no row of that batch.
- **R8. Kind names are bare nouns:** `evidence_record` → `evidence` in the
  directory, the record's `kind` field, the schema, the Rust variant,
  `canon.yaml` routing/aging keys and report views. The kind value is in the
  body, so evidence digests change.
- **R9. Read side:** a file whose path is not the one R1–R7 compute is a
  layout violation. Old-rule files get their own violation code whose hint
  is `canon migrate`; `canon.yaml` with an `evidence_record` key parses
  only far enough to send the user to `canon migrate`. Before migration,
  read commands warn and write commands refuse with the same hint, so no
  new-rule file is written next to old ones.
- **R10. Name parsers take the digest from the body**, never from the name:
  report views compute it from the record body; archive scoping uses the
  typed components; overlay validation matches typed components.

### Component table

| Kind | Components |
|---|---|
| change | `change_id` |
| task | `task_id` |
| scenario | `project_id`, `scenario_id` |
| session | `session_id` |
| run | `run_id` |
| event | `run_id`, `seq` |
| handoff | `id` |
| review | `project_id`, `scenario` + `scenario_id` \| `change` + `change_id` |
| divergence | `project_id`, `scenario_id`, `run_seq`, `round` |
| trajectory | `run_id` |
| strategy_item | `regime_key` |
| evidence | `project_id`, `task` + `task_id` \| `scenario` + `scenario_id` \| `run` + `run_id` |
| subject | `subject_id` |
| finding | `change_id`, `round`, `seq` |
| overlay `<ns.kind>` | its declared join-key fields, in declaration order |

The review's 40-hex `pin` leaves the name: it is in the body, and the digest
separates two reviews of the same target.

### Examples

| Kind | Today | New |
|---|---|---|
| evidence | `kind=evidence_record/vocab.typed.01__570796b2d76a.json` | `kind=evidence/canon_scenario_vocab.typed.01_<d8>.json` |
| review (scenario) | `kind=review/area=gate/canon__gate.check.104__addf4e86…f9ae__e7f52dd152fe.json` | `kind=review/canon_scenario_gate.check.104_e7f52dd1.json` |
| review (change) | `kind=review/area=change/canon__change-canon-0-28__<pin>__<d12>.json` | `kind=review/canon_change_canon-0-28_<d8>.json` |
| finding | `kind=finding/canon-0-28__0002__0002__b10258db7621.json` | `kind=finding/canon-0-28_2_2_b10258db.json` |
| event | `kind=event/<run_id>-0000000007__<d12>.json` | `kind=event/<run_id>_7_<d8>.json` |
| session | `kind=session/<id with / → _>__<d12>.json` | `kind=session/<id, R6-escaped>_<d8>.json` |

(The evidence digest is recomputed because R8 changes the body.)

### Why each part is there

| Part | Needed for | Measured |
|---|---|---|
| `kind=` | one directory per kind; readers scan one kind | — |
| project | the project id, not the tool name; scenario ids are unique only per project | this repo: `canon` 1,546 records, `platformer` 48 |
| digest | two worktrees appending for the same key produce two files, not a git add/add conflict; the archive keeps every superseded version; an identical resubmission lands on the same path | live ledger: 2,083 of 2,085 keys hold one file; ledger + archive: up to 9 |

A sequence number would not separate concurrent branches (both pick the same
next number), and a timestamp costs more tokens than 8 hex digits.

## User-visible text that changes

Specified as scenarios in the change, since they are output:

- `canon query`'s ID column and its cursor tokens (a 0.29 cursor is refused
  with a hint to start over).
- `FINDING_SEQ_OCCUPIED` and promote refusal messages that quote a key.
- `canon tier age` effect ids and `canon export` / `canon purge` manifest
  ids.
- Every write's "wrote/staged <path>" line.
- `--kind` values are unchanged (`evidence` already).

## Migration

`canon migrate` (preview, then `--commit --plan-digest`):

1. **Refuses** while a moved path has uncommitted changes, or while
   `_staging` holds the trace of an interrupted promote (a committed record
   whose `staging_id` is the 12-hex digest of a staged body). Other staged
   records are moved like the ledger.
2. **Records the old world first:** for every evidence file, its 0.29 key
   (task id, then scenario id, then run id; no project, no tag), its 12-hex
   digest and its old body go into the plan. The pre-0.23 review re-pinning
   then judges legacy pins (81 in canon's ledger) against this record with a
   frozen copy of the 0.29 key and digest functions, which restores
   `kind: "evidence_record"` before hashing.
3. **Renames** before every other existing migrate step (the re-pinning and
   the final `inventory sync` read through the tier, which no longer returns
   old-rule files).
4. **Rewrites `canon.yaml`** routing/aging keys (`evidence_record` →
   `evidence`) textually, as it already edits `specs:`.
5. **Moves** ledger, archive, staging and overlay files with `git mv`; rewrites
   evidence bodies (kind value) and recomputes their digests.
6. **Rekeys** sqlite rows (`hot.db`) and, for repos that route kinds there,
   Postgres rows and R2 objects, through the same plan.
7. **Rewrites `.gitignore`** negations that name a renamed kind directory
   (`!.canon/ledger/kind=evidence_record/`), or reports them when it cannot.
8. **Backs up** every rewritten file under `.canon/migrate-backup/`, and
   `hot.db` as a whole file. Down path: restore the backup and revert the
   moves; R2 and Postgres rows keep their old keys until the migrate commit
   step and are restored from the plan's recorded old keys.
9. Running it again reports nothing to move.

Path references inside records stay as strings: free-text `file_ref`s
(about 20) and plan links, as in the plans cutover. 0.23+ review pins hold
no paths.

Conformance: every case whose expected output changes is regenerated and
reviewed (about 30 cases print a digest-suffixed path; 69 fixture
`canon.yaml`s and 41 fixture `kind=evidence_record` ledgers change); the
normalizer's `__<12 hex>.json` pattern changes with them.

## Open questions

- **Q1. Drop `area=`:** decided, yes.
- **Q2. Kind spelling:** decided, `evidence`.
- **Q3. Release:** decided, 0.30.
- **Q4. Keyless evidence:** proposed refused by the model.
- **Q5. Phasing.** canon's own planner rule cuts a migration into expand
  (readers accept both), migrate, contract, one change each. This design
  is one change: only `canon migrate` reads old names, as in the 0.27→0.28
  cut. It needs the human's explicit waiver of the phasing rule.
