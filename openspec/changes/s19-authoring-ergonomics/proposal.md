## Why

The `najun-art-dummy` usability review (`target/usage-review/najun-art-dummy/SYNTHESIS.md`)
ran canon's shipped CLI end to end against a real consumer project and
returned **conditional adoption from all four personas** — the core loop
(`feature new → scenario new → fmt --check → inventory sync → gate check`)
is production-grade, but the review also surfaced a specific, low-effort
cluster of DAILY-AUTHORING friction that the synthesis explicitly separates
from the two hard CI/scale blockers (B1/B2, owned by sibling changes): the
"secondary friction (real, shallow)" section (`SYNTHESIS.md:125-140`) plus
the Designer's own `highest_leverage_improvement` (`reviews/designer.json`)
and the Developer's/Planner's day-one-onboarding and status-tracking
complaints (`reviews/developer.json`, `reviews/planner.json`). None of these
four items requires a new architecture decision — each is reuse of a
derivation, a message, or a filter that already exists one function away.

**1. `scenario new --feature` forces a path the tag already encodes, and a
typo writes an invisible orphan (Designer's TOP ask, "fixes three findings
at once").** `canon scenario new <tag> --title <t> --feature <path>`
REQUIRES `--feature` today (`crates/canon-cli/src/main.rs:762-766`,
`PathBuf`, no `Option`) even though `<tag>` is already a parsed `ScenarioId`
carrying `area`+`surface`
(`crates/canon-cli/src/scaffold.rs:70-101`'s `AreaSurface`), and the
sibling command `canon feature new <area>.<surface>` already DERIVES the
exact same `features/kind=feature/area=<area>/<surface>.feature` path from
those two segments
(`crates/canon-cli/src/scaffold.rs:295-296`, doc comment: "via the SAME …
layout `FamilyKind::Feature::layout_descriptor` declares"). Verified live
(`reviews/designer.json` weakness #1): `canon scenario new wall.render.01
--title … --feature specs/features/kind=feature/area=wall/render.feature`
retypes a skeleton the tag already implies. Worse (weakness #2, a NEW
finding beyond USAGE-NOTES #3): `canon scenario new wall.render.03 --title
guess --feature wall.render` exits `0` and writes a fully-formed feature
file at the repo ROOT — `canon fmt --check .` then reports "0 file(s)
checked" because `--feature` is accepted as a raw path with zero validation
that it resolves under `specs.roots[]`
(`crates/canon-cli/src/scaffold.rs:201`, `feature_path` built directly from
the caller's `feature: &Path` argument with no root-membership check). The
scenario is invisible to `fmt`, `inventory sync`, AND the duplicate-tag
guard (which only scans configured roots,
`crates/canon-cli/src/scaffold.rs:189-199`) — a silent design-integrity
hole, not just a UX tax.

**2. `feature new`'s empty stub reads as corruption, not a first step
(Designer).** `canon feature new wall.render` writes a `Feature:` header +
provenance with zero scenarios BY DESIGN (`scaffold.rs:264-271`'s own doc:
"a starting point for subsequent `canon scenario new` calls"), but the very
next `canon fmt --check specs` reports it as a `[layout-grammar]` violation
— `canon-fmt`'s `FmtFailureClass::LayoutGrammar`
(`crates/canon-fmt/src/report.rs:14-18`), the SAME class flat-path
violations and partition-key smears use. A designer's first command with
the tool produces a red indistinguishable from real corpus damage.

**3. `canon query` has no scope filter and no rollup (Planner's top-ask
runner-up).** `canon query --help` lists exactly `--kind`, `--since`,
`--canon-yaml`, `--json`, `--plugin`
(`crates/canon-cli/src/main.rs:77-98`) — no `--change-id`, no `--status`,
no aggregate. Verified live (`reviews/planner.json`): answering "what's
left on `add-audio-reactive`" requires dumping every `Task` record and
`jq`-filtering `task_id` prefixes client-side, and the raw order isn't even
change/task-sorted (`mart_trust_matrix`'s `ORDER BY change_id, task_id` in
`canon-store/sql/views.sql:226` applies only inside `canon report`, never
`canon query`). s17 already populates exactly the `Change`/`Task` records
this would filter over
(`openspec/changes/s17-plan-import/specs/plan-import-connector/spec.md`) —
the join spine exists; the read surface over it doesn't scope.

**4. No `canon init` (Developer).** `canon --help`'s 20-entry command list
(verified: `./target/debug/canon --help`) has no `init`/`new`/
`scaffold-project`. `najun-art-dummy/canon.yaml` had to be hand-typed
against four separate skill docs' worth of section grammar
(`tiers:`/`routing:`/`aging:` from `tiered-storage`, `specs:` from
`canon-inventory`, `plans:` from `canon-plan-import`), and there is no
single command that validates a fresh `canon.yaml` end to end — today the
first `canon inventory sync`/`canon ingest plans` does that validation as
an unplanned side effect (`crates/canon-store/src/policy.rs::TierPolicy::
from_yaml`, `crates/canon-cli/src/inventory.rs::load_spec_roots`,
`crates/canon-cli/src/plans.rs::load_plan_sources_from_config` are each
independently fail-loud, strict, `deny_unknown_fields` — but nothing
chains them into one preflight check a developer can run BEFORE wiring a
sync).

**Scope discipline.** All four fixes are additive CLI/message surface over
EXISTING machinery — `FamilyKind::Feature::layout_descriptor`'s derivation,
`canon-store`'s three independent strict config loaders, the `Change`/
`Task` records s17 already writes. None touches the closed 12-`RecordKind`
set, the closed 11-member `FmtFailureClass` set (`report.rs:14-62`, "the
audit's own gap list is exactly `FmtFailureClass::ALL`'s cardinality, no
more, no less"), gate/promotion authority, or `canon-gate`'s checkbox
format authority.

## What Changes

- **`canon scenario new`'s `--feature` becomes OPTIONAL, tag-derived by
  default, validated when given.** Omitted: the target path is derived
  from `<tag>`'s `area`+`surface` via the SAME join `canon feature new`
  already builds (`root.root.join("features").join("kind=feature").
  join("area=<area>").join("<surface>.feature")`) against the ONE
  configured `specs.roots[]` entry — an ambiguous multi-root config
  refuses loud (exit `2`), mirroring `run_feature_new`'s own existing
  ambiguity refusal, never a guessed root. Given: the resolved path MUST
  fall under some configured `specs.roots[]` entry's root directory or the
  command refuses (exit `2`, names the attempted path and the configured
  roots) — never a repo-root orphan write again. Existing duplicate-tag and
  target-file guards are unchanged and still run.
- **`feature new`'s empty-stub result stops reading as corruption.**
  `canon feature new` prints an explicit next-step hint
  (`` next: `canon scenario new <area>.<surface>.01 --title '<label>' [--feature <path>]` to make it fmt-clean ``)
  on success. `canon fmt --check`'s `LayoutGrammar` message for the
  SPECIFIC shape a fresh `feature new` stub produces (a `Feature:` header +
  provenance, zero `@`-tagged scenarios) is reworded to lead with "empty
  feature stub (not yet a valid corpus entry)" instead of generic
  grammar-violation phrasing. The violation's CLASS stays
  `FmtFailureClass::LayoutGrammar` and `--check`'s exit code is UNCHANGED
  (still `1`, still a real corpus-completeness gap) — the closed 11-member
  `FmtFailureClass` set is not widened.
- **`canon query` gains `--change-id <ChangeId>` and `--status <s>`.** Both
  flags require `--kind change` or `--kind task` (any other `--kind` fails
  loud, exit `2`, naming the two supported kinds) — `--change-id` filters
  `Change` records by `change_id` equality and `Task` records by
  `TaskId::change_id()` equality; `--status` validates against the
  QUERIED kind's own status domain (`open`/`done` for `task`,
  `proposed`/`in_progress`/`completed`/`archived` for `change`) and fails
  loud naming the valid set on a cross-kind value (e.g. `--kind task
  --status archived`). `--kind task` output additionally carries a
  `<done>/<total> done` rollup (human line + JSON `rollup` object) computed
  over the (possibly `--change-id`-filtered) result set. `--kind change`/
  `--kind task` output is sorted deterministically by `(change_id,
  task_id)` before printing — every other `--kind`'s existing `at`-merge
  order is UNCHANGED.
- **New `canon init [--repo <dir>]` + `canon init --check-config`.**
  `canon init` writes a fresh, WORKING `canon.yaml` skeleton (`tiers:` git
  root + commented pg/r2 stanzas; `routing:` — all twelve
  `RecordKind::ALL` wire strings routed `git`, the only zero-env-var tier;
  `specs:` with one `id: root, root: specs` entry; `plans: { sources: [] }`
  — a legitimate, `deny_unknown_fields`-clean present-but-empty section)
  at `<repo>/canon.yaml`, refusing to overwrite an existing file
  (`create_new`, exit `2`, mirrors `run_feature_new`'s own refusal
  convention — zero bytes touched either way). `canon init --check-config`
  is READ-ONLY: it loads an EXISTING `canon.yaml` (missing file fails loud,
  exit `2`) and chains the same three independent strict loaders a real
  sync/ingest would use — `TierPolicy::from_yaml`, `load_spec_roots`,
  `load_plan_sources_from_config` — reporting one PASS/FAIL line per
  section, exiting `0` only when every present section parses clean; it
  reimplements NONE of their validation logic.

### Added Capabilities

- `derived-validated-scenario-feature`: `--feature` optional on `canon
  scenario new`, tag-derived default via the shared `FamilyKind::Feature`
  layout join, and `specs.roots[]`-membership validation on an explicit
  `--feature`, closing the orphan-write footgun.
- `wip-feature-stub-class`: a distinguishing next-step hint on `feature
  new` plus a reworded `LayoutGrammar` message for the empty-stub shape —
  no new `FmtFailureClass` variant, no exit-code change.
- `query-scope-filters`: `--change-id`/`--status` scoping (kind-gated,
  domain-validated) plus a `done/total` rollup and deterministic
  `(change_id, task_id)` ordering for `canon query --kind change`/`--kind
  task`.
- `canon-init-scaffold`: `canon init` (write, refuse-overwrite) +
  `canon init --check-config` (read-only, chains the three existing strict
  config loaders) for `canon.yaml`.

### Explicit non-goals

- No change to `canon query`'s root-resolution algorithm (the
  `--canon-yaml`-vs-`--repo` split, B2 in `SYNTHESIS.md:84-91`) — a
  DIFFERENT, sibling change owns unifying root resolution across every
  verb; this change only adds scope-filter FLAGS to the existing `--kind`/
  `--since`/`--canon-yaml`/`--json`/`--plugin` surface.
- No Task↔Scenario join (B3, `SYNTHESIS.md:93-104`, the Planner's actual
  `top_ask`) — `--change-id`/`--status` scope an EXISTING `Task`/`Change`
  read; they do not add a new field, a new join, or a new record kind. A
  planner asking "is this DONE and VERIFIED" still needs the (separately
  owned) Task↔Scenario join; this change only makes "is this DONE" fast.
- No new `FmtFailureClass` variant and no change to `canon fmt --check`'s
  exit-code contract for an empty feature stub — the 11-member closure
  (`FmtFailureClass::ALL`, `report.rs:66-79`) stays exactly 11, structurally
  asserted; an empty stub remains a real, exit-`1` corpus-completeness gap,
  presented more legibly, never waived.
- No `--repo` alias on `canon query` (Designer's secondary ask,
  `reviews/designer.json`'s `secondary_asks[1]`) — folded into the same B2
  root-resolution unification named above, not duplicated here.
- No `canon.yaml` `pg:`/`r2:` auto-provisioning in `canon init` — those
  tiers need operator-supplied `dsn_env`/`bucket_env` values `init` cannot
  guess; the skeleton documents their shape as commented stanzas and routes
  every kind to `git` (the zero-config tier) by default, matching this
  repo's OWN root `canon.yaml`'s working example (`canon.yaml:15-27`) for
  what a minimal REAL config looks like.
- No `canon inventory sync`/`canon ingest plans` behavior change — `canon
  init --check-config` calls the SAME three loaders those commands already
  call; it adds a preflight entry point, never a fourth validation path.
- No change to the closed 12-`RecordKind` set, gate/promotion authority, or
  `canon-gate`'s checkbox format authority — every change here is CLI
  surface + message text over already-existing derivations and records.

## Impact

- **`canon-cli`**: `src/scaffold.rs` (`--feature` → `Option<PathBuf>`
  handling, a shared `default_feature_path`/root-membership validator
  reused by `run_scenario_new` and `run_feature_new`, the `feature new`
  next-step hint), `src/main.rs` (`ScenarioCommand::New.feature` optional,
  `Query`'s new `--change-id`/`--status` args, a new `Command::Init` +
  `run_init`/`run_check_config`), `src/query.rs` (change/task filtering,
  rollup, deterministic sort), new `src/init.rs` (skeleton writer +
  `--check-config` driver chaining `canon-store::policy::TierPolicy`,
  `canon-cli::inventory::load_spec_roots`, `canon-cli::plans::
  load_plan_sources_from_config`).
- **`canon-fmt`**: `src/check.rs`/`src/report.rs` — `Violation`'s rendered
  message text for the empty-feature-stub shape only; `FmtFailureClass`'s
  variant set and `--check`'s exit-code logic are UNCHANGED.
- **`canon-model` / `canon-store` (beyond the read-only `init
  --check-config` call-through) / `canon-gate` / `canon-learn` /
  `canon-vocab` / `canon-plugin` / `canon-ingest`**: UNCHANGED.
- **Tests**: `crates/canon-cli/tests/scaffold.rs` (default-derivation,
  orphan-rejection, ambiguous-multi-root-refusal scenarios), `tests/
  query.rs` (`--change-id`/`--status` filtering, rollup, deterministic
  sort, kind-gating refusal), a new `tests/init.rs` (scaffold-then-
  check-config round trip, refuse-overwrite, missing-file `--check-config`
  refusal), `crates/canon-fmt` unit tests (reworded message, unchanged
  class/exit code).
