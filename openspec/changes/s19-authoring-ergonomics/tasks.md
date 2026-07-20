# Tasks — s19 authoring ergonomics

The four capabilities are independent CLI/message-surface fixes over
existing machinery — no sequencing dependency forces one before another —
but P1 (the scenario/feature derivation) and P2 (the fmt message) share
`canon-cli::scaffold`, so P1 lands first to avoid two concurrent edits of
the same module. P3 (`query`) and P4 (`init`) are independent of P1/P2 and
of each other.

## 1. derived-validated-scenario-feature (P1)

- [x] 1.1 `canon-cli/src/scaffold.rs`: extract `run_feature_new`'s inline
      `features/kind=feature/area=<area>/<surface>.feature` join
      (`scaffold.rs:295-296`) into a standalone `resolve_feature_path(root:
      &SpecRoot, area: &str, surface: &str) -> PathBuf`; rewire
      `run_feature_new` to call it — pure code motion, `run_feature_new`'s
      existing test suite must pass byte-identical before/after.
- [x] 1.2 `main.rs`'s `ScenarioCommand::New.feature` becomes `Option<PathBuf>`
      (`#[arg(long)]` stays, drops the requiredness); `run_scenario_new`'s
      dispatch fn threads the `Option` through unchanged otherwise.
- [x] 1.3 `scaffold.rs::run_scenario_new`: when `feature` is `None`, resolve
      `ctx.spec_roots(None)`, require exactly one root (mirroring
      `run_feature_new`'s `[one] => one, many => refuse` match at
      `scaffold.rs:282-293`, exit `2` on ambiguity), then call
      `resolve_feature_path(root, tag.area(), tag.surface())` as the target.
- [x] 1.4 `scaffold.rs::run_scenario_new`: when `feature` is `Some(path)`,
      resolve it to an absolute path (unchanged), then validate it falls
      under SOME configured `specs.roots[]` entry's canonicalized root
      directory (path-component-wise prefix, not a string prefix) BEFORE
      the existing duplicate-tag/target-file checks; a path outside every
      root refuses (exit `2`, names the attempted path + configured
      roots), zero bytes written.
- [x] 1.5 Tests (`crates/canon-cli/tests/scaffold.rs`): omitted `--feature`
      derives the identical path `feature new` would scaffold; omitted
      `--feature` under a two-root config refuses loud; a `--feature` path
      outside every configured root refuses loud with zero bytes written
      anywhere; a `--feature` path under a configured root in a
      non-canonical subpath still succeeds; the existing duplicate-tag
      tests pass unchanged for both a derived and an explicit path.

## 2. wip-feature-stub-class (P2 — after P1, same module)

- [x] 2.1 `scaffold.rs::run_feature_new`: success stdout gains a next-step
      hint line naming `canon scenario new <area>.<surface>.01 --title
      '<label>' [--feature <path>]`, derived from the same
      `area_surface`/`title`/`feature_path` already in scope.
- [x] 2.2 `canon-fmt/src/check.rs` (or `report.rs`, wherever the
      `LayoutGrammar` message for a feature-family resolve is constructed):
      detect the empty-feature-stub shape via the existing
      `canon_fmt::gherkin::scan` result already computed for the file (a
      `Feature:` header + one provenance comment + zero `@`-tagged
      scenarios) and rewrite that ONE violation's message text to lead
      with "empty feature stub (not yet a valid corpus entry)"; every
      other `LayoutGrammar` cause keeps its current phrasing untouched.
- [x] 2.3 Confirm (no code change expected) `FmtFailureClass::ALL` stays
      exactly 11 members and `--check`'s exit code for this violation
      stays nonzero.
- [x] 2.4 Tests: `canon-fmt` unit test asserting the reworded message for a
      fresh `feature new`-shaped stub; a second unit test asserting an
      UNRELATED `LayoutGrammar` cause (e.g. a flat pre-migration path) is
      unaffected; a `canon-cli` integration test running `feature new` then
      `fmt --check` and asserting both the stdout hint and the reworded
      violation text; the existing `lib.rs` selftest oracle (11-class
      surfacing) still green.

## 3. query-scope-filters (P3)

- [x] 3.1 `main.rs`'s `Command::Query` gains `--change-id <String>` (parsed
      via `ChangeId::parse`) and `--status <String>` args (both
      `Option<...>`); usage validation (kind-gating, D5) happens in
      `canon_cli::query`, not clap, so the error message can name the
      queried kind.
- [x] 3.2 `query.rs`: kind-gate `--change-id`/`--status` — any `--kind`
      other than `change`/`task` with either flag set fails loud (exit
      `2`, naming the two supported kinds) before any tier read.
- [x] 3.3 `query.rs`: `--status`'s value is validated against the queried
      kind's own domain (`open`/`done` for `task`; the four `ChangeStatus`
      strings for `change`) — an out-of-domain value fails loud naming the
      valid set for that kind.
- [x] 3.4 `query.rs`: post-tier-merge filtering — `--change-id` narrows
      `Change` records by `change_id` equality and `Task` records by
      `TaskId::change_id()` equality (parsed from each raw record's own
      `task_id`/`change_id` field); `--status` narrows by the record's own
      `status` field.
- [x] 3.5 `query.rs`: `--kind task` output computes a `done`/`total` rollup
      over the (post-filter) result set; human `format_human` prints a
      `<done>/<total> done` line, `format_json` adds a `rollup` object.
      `--kind change` (and every other kind) carries no rollup.
- [x] 3.6 `query.rs`: `--kind change`/`--kind task` output is sorted
      ascending by `(change_id, task_id-natural-order)` before
      printing/emitting, reusing the existing `resolve_partition`-derived
      natural key (`query.rs:102-107`); every other kind's `at`-merge order
      is unchanged (an acceptance test pins the existing
      `merges_records_split_across_the_routed_tier_and_its_aging_
      destination` fixture's order byte-identical).
- [x] 3.7 Tests (`crates/canon-cli/tests/query.rs`): kind-gating refusal for
      both flags; `--change-id` scoping on `task` and on `change`;
      `--status` scoping + cross-kind-domain refusal; rollup reflects the
      filtered set (not the whole ledger) and reflects the unfiltered set
      when no filter is given; deterministic `(change_id, task_id)` sort
      against an interleaved raw-merge fixture; the existing trajectory
      `at`-merge test unchanged.

## 4. canon-init-scaffold (P4)

- [x] 4.1 New `canon-cli/src/init.rs`: `run_init(repo: &Path) -> i32`
      writes the skeleton `canon.yaml` (tiers.git + commented pg/r2;
      routing: all twelve `RecordKind::ALL` wire strings → `git`; specs:
      one `{id: root, root: specs}` entry; `plans: { sources: [] }`) via
      `create_new` (atomic create-fails-if-exists), mirroring
      `run_feature_new`'s refusal convention on an existing file (exit `2`,
      zero bytes touched).
- [x] 4.2 `init.rs`: `run_check_config(repo: &Path) -> i32` — read-only;
      missing `canon.yaml` fails loud (exit `2`); otherwise calls
      `TierPolicy::from_yaml`, `canon_cli::inventory::load_spec_roots`,
      `canon_cli::plans::load_plan_sources_from_config` in sequence
      against the same file text, collecting one PASS/FAIL/"not
      configured" line per section (`plans`/`specs` fail-soft-on-absent
      per their own existing contracts) without stopping at the first
      failure; exits `0` only when every present section passes.
- [x] 4.3 `main.rs`: new `Command::Init { repo: PathBuf, check_config: bool
      }` (`--repo` default `.`, `--check-config` a bool flag) dispatching
      to `run_init`/`run_check_config` (mutually exclusive: `--check-config`
      never writes, plain `init` never validates-only).
- [x] 4.4 Any visibility changes needed on `TierPolicy::from_yaml`,
      `load_spec_roots`, `load_plan_sources_from_config` to be callable
      from `init.rs` — signatures and error types unchanged, visibility
      only.
- [x] 4.5 Tests (new `crates/canon-cli/tests/init.rs`): fresh-repo `init`
      writes a working skeleton; refuse-overwrite on an existing
      `canon.yaml`; `init` immediately followed by `inventory sync`
      (with one added `.feature` file) succeeds with zero further edits;
      `init` immediately followed by `ingest plans` exits `0` as a clean
      no-op; `check-config` on the fresh skeleton reports all sections
      PASS/`plans` as configured-empty; `check-config` on a missing file
      fails loud; `check-config` surfaces one malformed section (e.g. an
      unregistered plan dialect) while still reporting the other two
      sections PASS; `check-config` treats an absent `plans:` key as "not
      configured", not FAIL.

## 5. Verification

- [x] 5.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green.
- [x] 5.2 `bunx openspec validate --strict s19-authoring-ergonomics` green.
- [x] 5.3 `canon selftest` all suites still green (no suite touches
      scaffold/query/fmt-message/init behavior this change alters in a way
      the fixture oracle pins — confirm none regress).
- [x] 5.4 Structural invariants re-asserted green: `RecordKind::ALL.len()
      == 12`; `FmtFailureClass::ALL.len() == 11`; every existing
      `run_feature_new`/`format_human`/trajectory-merge-order test passes
      byte-identical.
