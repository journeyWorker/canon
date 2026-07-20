# Design — s19 authoring ergonomics

## Current state (verified against the built binary + source)

- **`canon scenario new`'s `--feature` is a bare, unvalidated `PathBuf`.**
  `crates/canon-cli/src/main.rs:762-766` declares `#[arg(long)] feature:
  PathBuf` — clap REQUIRES it (confirmed: `canon scenario new --help`
  shows `--feature <FEATURE>` with no `[default: …]`, and the usage line
  reads `--title <TITLE> --feature <FEATURE> <TAG>`). `run_scenario_new`
  (`scaffold.rs:179-248`) resolves it with a single `if feature.is_absolute()
  { .. } else { repo_root.join(feature) }` (line 201) and writes there —
  no check that the result falls under any `specs.roots[]` entry.
- **`canon feature new` already derives the identical path.**
  `run_feature_new` (`scaffold.rs:272-326`) resolves ONE `specs.roots[]`
  entry (refusing an ambiguous multi-root config, lines 282-293) and joins
  `root.root.join("features").join("kind=feature").
  join(format!("area={}", area_surface.area)).
  join(format!("{}.feature", area_surface.surface))` (lines 295-296) — this
  IS `FamilyKind::Feature::layout_descriptor`'s shape, hand-built rather
  than called through the descriptor type (the descriptor is a VALIDATION
  table consumed by `layout_problem`/`canon fmt`, not a path constructor —
  `canon-model/src/family/mod.rs:201-236`), so scaffold.rs already owns the
  one authoritative constructor for this shape; s19 factors it out and
  reuses it, never re-derives from the descriptor a second way.
- **`ScenarioId` already carries `area`/`surface`.** `tag: ScenarioId`
  (parsed via `ScenarioId::parse`, `scaffold.rs:106-108`) exposes
  `.area()`/`.surface()` — `run_scenario_new`'s own "new file" branch
  already calls `tag.area()`/`tag.surface()` at line 230 to build the
  `Feature: <area> <surface>` header. The derivation input already exists
  in-hand at the point `--feature` is currently required.
- **`FmtFailureClass` is a CLOSED, audited 11-member set.**
  `crates/canon-fmt/src/report.rs:14-62` + `:66-79`'s `ALL` array; the
  module doc (`report.rs:1-6`) states the count is "chosen so the audit's
  own gap list is exactly `FmtFailureClass::ALL`'s cardinality, no more, no
  less" and `lib.rs:44-48`'s selftest oracle asserts every one of the 11 is
  independently SURFACED by the fixture corpus — a spec-level invariant,
  not incidental. A fresh `feature new` stub's fmt violation is emitted as
  `FmtFailureClass::LayoutGrammar` (`check.rs`'s `layout_problem` call
  sites, e.g. line 96) with message text built from `layout_problem`'s own
  `v.expected`/`v.detail` — generic grammar-mismatch phrasing, not
  stub-aware.
- **`canon query` has exactly five flags, none scope-narrowing.**
  `main.rs:77-98` (`--kind`, `--since`, `--canon-yaml`, `--json`,
  `--plugin`); `run` (`query.rs:70-81`) builds one `TierQuery::kind(kind)
  [.since(since)]` and merges by `at` — no post-filter, no sort override.
  `Change`/`Task` (`canon-model/src/records.rs:43-49`, `:82-90`) carry
  `change_id: ChangeId`/`task_id: TaskId` with `status: ChangeStatus`/
  `TaskStatus` (`#[serde(rename_all = "snake_case")]`, wire strings
  `proposed`/`in_progress`/`completed`/`archived` and `open`/`done`).
  `TaskId::change_id()` (`ids.rs:323-327`) already derives the owning
  change from a task id — the exact join key `--change-id` needs.
- **No `canon init` exists.** `canon --help`'s 20-entry list has none;
  `canon.yaml`'s three independently-strict section loaders already exist
  and are individually well-tested: `TierPolicy::from_yaml`
  (`canon-store/src/policy.rs:181-202`, parses `tiers:`/`routing:`/
  `aging:`, ignores unrelated top-level keys via `#[serde(default)]` with
  NO `deny_unknown_fields` at the top level — only nested sections are
  strict), `load_spec_roots` (`canon-cli/src/inventory.rs:109-155`, fails
  loud on non-YAML or a malformed/empty-but-present `specs:`),
  `load_plan_sources_from_config` (`canon-cli/src/plans.rs:432-466`, fails
  loud on non-YAML, a malformed `plans:`, or an unregistered dialect id).
  This repo's own root `canon.yaml` (`canon.yaml:1-52`) is the reference
  shape for what a hand-typed, working config looks like.

## Decisions

- **D1 — `--feature`'s default derivation is a REUSE of
  `run_feature_new`'s join, extracted once, called twice.** A new function
  `fn resolve_feature_path(root: &SpecRoot, area: &str, surface: &str) ->
  PathBuf` in `scaffold.rs` holds the exact
  `features/kind=feature/area=<area>/<surface>.feature` join currently
  inlined at `scaffold.rs:295-296`; `run_feature_new` is rewired to call
  it (zero behavior change, pinned by its existing tests) and
  `run_scenario_new`'s new "no `--feature`" branch calls the SAME
  function against `tag.area()`/`tag.surface()`. Rejected alternative:
  duplicating the join inline in `run_scenario_new` — the entire premise
  of this change is that a second hand-typed copy of a derivable path is
  the bug class being fixed; a two-owner copy would repeat it one layer
  down.
- **D2 — root selection for the default mirrors `run_feature_new`'s
  existing ambiguity refusal; `--feature` explicit still scans ALL
  roots.** When `--feature` is omitted, `run_scenario_new` resolves
  `ctx.spec_roots(None)` and requires exactly ONE root (same `[one] =>
  one, many => refuse` match `run_feature_new` already performs,
  `scaffold.rs:282-293`) before calling `resolve_feature_path` — an
  ambiguous multi-root config fails loud (exit `2`) rather than guessing
  which root's `.feature` file the tag belongs under. This is a
  DELIBERATE parity choice: `feature new` and the derived branch of
  `scenario new` scaffold the SAME file shape under the SAME
  disambiguation rule, so a designer never sees `feature new` refuse
  while `scenario new`'s default silently picks a root (or the reverse).
  When `--feature` IS given explicitly, the EXISTING multi-root duplicate-
  tag scan (`corpus_tags` over every root, `scaffold.rs:189-199`) is
  untouched — an explicit path only needs to fall under SOME configured
  root (D3), not resolve which one is "the" root.
- **D3 — explicit `--feature` validation is root-MEMBERSHIP, not exact
  layout conformance.** The resolved absolute path MUST have some
  configured `specs.roots[]` entry's canonicalized root directory as a
  path-component-wise prefix (never a naive string-prefix compare — a
  root `specs` must not falsely accept a sibling `specs2`); a path
  failing every root refuses loud (exit `2`, names the attempted absolute
  path and every configured root). This is the MINIMAL closure of the
  orphan hole: once a `.feature` file sits under a configured root,
  `canon fmt --check` (which walks configured roots) and `canon inventory
  sync`'s duplicate-tag scan both SEE it — the existing validators take
  over from there. Rejected alternative: forcing `--feature` to match
  `resolve_feature_path`'s own derived shape exactly — that would make
  `--feature` pointless to ever pass explicitly (D2's default already
  covers the canonical shape); an explicit `--feature` exists precisely
  for an operator who wants a DIFFERENT filename/subdirectory under a
  real root (e.g. grouping several small surfaces in one hand-named
  file), which root-membership validation still permits while closing the
  repo-root-escape footgun the review found.
- **D4 — the empty-stub UX fix stays entirely OUTSIDE
  `FmtFailureClass`.** `FmtFailureClass::ALL`'s cardinality is a
  STRUCTURALLY ASSERTED invariant (`lib.rs:44-48`'s selftest oracle,
  `report.rs`'s own module doc) — the S11 acceptance bar was explicitly
  "the audit's gap list is exactly this cardinality, no more, no less."
  Adding a twelfth `EmptyFeatureStub` variant (the designer's literal
  first-listed option, "downgrade … to a distinct WIP/info class") would
  violate that bar for a presentation problem, not a classification gap —
  an empty stub genuinely IS a `LayoutGrammar` violation (no
  `@<area>.<surface>.<nn>` tag to derive `area` from is exactly what that
  class documents, `report.rs:15-18`). The fix is therefore two additive,
  non-structural changes: (a) `run_feature_new`'s success message gains a
  next-step hint (module doc's own suggested phrasing, the designer's
  SECOND listed option); (b) `canon-fmt::check`'s `LayoutGrammar` message
  construction special-cases the EXACT shape a fresh `feature new` stub
  produces (a `Feature:` header, a paired provenance comment, ZERO
  `@`-tagged scenarios anywhere in the file — detectable from the same
  `canon_fmt::gherkin::scan` result `layout_problem`'s caller already
  has) and leads the message with "empty feature stub (not yet a valid
  corpus entry)" instead of generic phrasing. `--check`'s exit code for
  this violation is UNCHANGED (still `1`) — this is legibility, not
  leniency; an empty stub is still an incomplete corpus entry and must
  still fail a strict check.
- **D5 — `--change-id`/`--status` are KIND-GATED, not silently-ignored on
  other kinds.** Passing `--change-id`/`--status` with `--kind` anything
  other than `change`/`task` fails loud (exit `2`, naming the two
  supported kinds) rather than silently returning the unfiltered kind's
  full result set — the review's own theme (fail-loud over fail-silent
  everywhere else in this CLI) applies here: a planner who typos `--kind
  scenario --change-id add-widget` must be told the flag doesn't apply to
  that kind, not shown an unfiltered scenario dump that LOOKS filtered.
  `--status`'s accepted value SET is derived from the queried kind
  (`open`/`done` for `task`, the four `ChangeStatus` strings for
  `change`) — an out-of-domain value (`--kind task --status archived`)
  fails loud naming the kind's own valid set, never silently coerced or
  matched against the wrong enum.
- **D6 — the rollup and the deterministic sort are SCOPED to `--kind
  change`/`--kind task` only; every other kind's existing `at`-merge
  order is untouched.** `format_human`'s own doc (`query.rs:83-86`)
  currently promises "ordered by `at` exactly as `TierRegistry::query`
  returns them" for the general case — changing that globally would be an
  undocumented behavior break for the eight other record kinds `canon
  query` already serves (sessions, evidence, trajectories, …), none of
  which carry a `(change_id, task_id)`-shaped natural key to sort by in
  the first place. `--kind change`/`--kind task` get a NEW, additive
  ordering rule (secondary sort by natural key, computed the same way
  `format_human`'s existing per-row `resolve_partition` call already
  derives a natural key at `query.rs:102-107` — reused, not re-derived) —
  every other kind's `at`-only order is BYTE-IDENTICAL before and after
  this change (an acceptance test pins it). The `done/total` rollup is
  likewise `--kind task`-only (a `--kind change` query has no natural
  "done" fraction of its own beyond its already-visible `status`).
- **D7 — `canon init --check-config` CALLS, never REIMPLEMENTS, the three
  existing loaders.** `TierPolicy::from_yaml`, `load_spec_roots`,
  `load_plan_sources_from_config` are each independently public (or made
  `pub(crate)`-visible to `canon-cli::init` without changing their own
  signatures or error types) and called in sequence against the SAME
  `canon.yaml` text; `--check-config`'s job is exclusively to catch each
  `Result::Err`, format it under its own section label, and continue to
  the next loader rather than stopping at the first failure (all THREE
  sections' health is useful in one pass, mirroring `canon fmt --check`'s
  own "report everything, don't stop at the first violation" convention)
  — never a fourth, hand-rolled parser. A section that is legitimately
  ABSENT (e.g. no `plans:` key) reports as an explicit "not configured"
  line, distinct from a parse failure, matching each loader's own
  fail-soft-on-absent contract.
- **D8 — `canon init`'s skeleton routes every kind to `git`, the one
  zero-env-var tier.** `pg`/`r2` need `dsn_env`/`bucket_env` values only
  an operator can supply (`policy.rs:55-79`) — `init` cannot guess a
  Postgres DSN or an R2 bucket name, and writing a placeholder value would
  make `--check-config` pass while `canon tier age`/a real pg-routed write
  fails at RUNTIME with a misleading "looks configured" signal. The
  skeleton instead ROUTES ALL TWELVE kinds to `git` (this repo's own root
  `canon.yaml` already runs the higher-traffic kinds through `pg`/`r2`
  purely as a scale optimization, not a correctness requirement — every
  kind is valid under `git`) and leaves commented `pg:`/`r2:` `tiers:`
  stanzas plus a comment showing which `routing:` lines to flip once an
  operator has real credentials. This makes a freshly-`init`ed repo
  `--check-config`-clean AND `inventory sync`/`ingest plans`-usable with
  zero additional setup — exactly the "before the first sync does so as a
  side effect" ask.
- **D9 — `plans:` scaffolds as a PRESENT-but-empty `sources: []`, `specs:`
  scaffolds with ONE real root.** `load_plan_sources_from_config` accepts
  an empty `sources: []` list as a legitimate present-but-configured
  zero-source state (`plans.rs:453`, `Vec::with_capacity(0)` loops zero
  times, no error) — so `plans:` can ship active and `--check-config`-
  clean out of the box, ready for an operator to add a `{dialect,root}`
  entry later. `load_spec_roots` is DIFFERENT: a present `specs:` with
  ZERO `roots[]` entries is a HARD error (`inventory.rs:139-143`, "only an
  ABSENT `specs:` key resolves the single default root; a present `specs:`
  must declare at least one root") — so the skeleton's `specs:` section
  ships with one real, working `{id: root, root: specs}` entry rather than
  an empty list, or `--check-config` would fail on the file `init` itself
  just wrote.

## Risks

- **R1 — `resolve_feature_path` extraction touches `run_feature_new`'s
  working code.** Mitigated: it is pure code motion (the same three
  `.join()` calls, same inputs, same output), pinned by
  `run_feature_new`'s existing test suite
  (`crates/canon-cli/src/scaffold.rs`'s `#[cfg(test)] mod tests` +
  `crates/canon-cli/tests/scaffold.rs`) which must pass BYTE-IDENTICAL
  before and after.
- **R2 — a stub-shape detector for D4 could false-positive on a
  legitimately-empty-looking file that isn't `feature new`'s output.**
  Mitigated by keying the detector on the STRUCTURAL condition
  (`gherkin::scan` finds a `Feature:` header, a paired provenance comment,
  and zero `@`-tagged scenarios) rather than a byte-exact match against
  `feature new`'s output — any file in that shape, hand-authored or
  scaffolded, genuinely IS "not yet a valid corpus entry," so the reworded
  message is accurate for every file it fires on, not just scaffold
  output.
- **R3 — kind-gating `--change-id`/`--status` could surprise a future
  caller who expects them to apply more broadly.** Accepted: `Change`/
  `Task` are the only two kinds carrying these fields today (D5); widening
  the gate to a future kind that grows a `change_id`/`status`-shaped field
  is an additive, backward-compatible relaxation of the SAME check, never
  a breaking change to callers who already pass `--kind change`/`--kind
  task`.
- **R4 — `canon init`'s all-`git`-routing default diverges from this
  repo's own root `canon.yaml` (`pg`/`task`, `handoff`, etc.).**
  Accepted and documented (D8): the divergence is a deliberate,
  zero-credentials-required BOOTSTRAP default, not a claim that `git`-for-
  everything is the recommended production shape for every repo — the
  skeleton's own comments point at `pg`/`r2` as the scale-up path once an
  operator has real DSNs/buckets.
