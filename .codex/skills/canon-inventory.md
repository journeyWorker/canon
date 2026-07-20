# canon-inventory

> How to run canon inventory sync — the s15 (spec-ledger-unification) validate-then-materialize step that turns an S11-validated `.feature` corpus into the native `Scenario` ledger index, and how that index feeds the rest of the unified loop (`canon review add` / `canon divergence {stage,promote,resolve,defer}` for attestation, `canon gate check` for trust-ladder/staleness enforcement, `canon ingest artifacts` with `artifacts.native_records: true` for the S6/S7 ReasoningBank flywheel). Use when authoring a new spec corpus, wiring a monorepo's `specs.roots[]` config, debugging a sync abort (S11 violation or a duplicate scenario_id), or explaining how canon's ledger records get produced in the first place.

# canon-inventory

s15 (`s15-spec-ledger-unification`) closes canon's own governance gap:
`Scenario`/`Review`/`Divergence` existed in `canon-model` since S1 but
had ZERO producers — every construction site was a test fixture. This
skill covers the FIRST stage of the now-unified pipeline:
`canon inventory sync` (`crates/canon-cli/src/inventory.rs`), which
turns a hand-authored, S11-validated `.feature` corpus into the native
`Scenario` ledger-INDEX record every downstream stage joins against.

## The unified loop

```
author .feature corpus (S11-validated)
        │
        ▼
canon inventory sync            # THIS skill — materializes Scenario index records
        │
        ▼
canon review add                # attests a Review record (native-verdict-lifecycle spec)
canon divergence {stage,promote,resolve,defer}   # tracks/resolves a divergence
canon gate check [--release]    # trust-ladder / staleness / coverage enforcement
        │
        ▼
canon ingest artifacts          # with artifacts.native_records: true in canon.yaml —
                                 # feeds Review/Divergence verdicts into the S6/S7
                                 # ReasoningBank flywheel (native-record-flywheel spec)
```

Every stage after `sync` joins on the SAME `(project_id, scenario_id)`
pair `sync` materializes — there is no second identity scheme anywhere
in this loop (design D2/D6).

## `canon inventory sync [--repo <dir>] [--spec-root <dir>]`

```bash
canon inventory sync                        # every canon.yaml specs.roots[] entry
canon inventory sync --repo ../svc           # a specific repo
canon inventory sync --spec-root ./specs     # ad hoc: ignore canon.yaml entirely
```

Per configured root, in order:

1. **Validate** — runs `canon-fmt::check` (S11) over the root. ANY
   violation (missing provenance, layout-grammar, …) ABORTS THE WHOLE
   ROOT: zero `Scenario` records written for it, the violation(s)
   reported. Never a partial sync.
2. **Scan** — walks `<root>/features/**/*.feature` via
   `canon-fmt::gherkin::scan`, pairing each `@<area>.<surface>.<nn>`
   tag with its header's label as `title`, and computing a
   `source_digest` (a full sha256-hex over the `.feature` file's raw
   bytes — a `SpecDigest`, never `ids::Sha`, which is a 40-hex git
   sha). `<root>/inventory/**` is validated by `canon-fmt::check` as
   ordinary S11 hygiene but is NEVER read here — the index derives from
   the `.feature` corpus alone (an `InventoryEntry.covered_by`
   read is a donor-porting concern reserved for a future s16 plugin,
   never core canon).
3. **Materialize** — upserts ONE `Scenario` index record per
   `(project_id, scenario_id)` via the normal append-only `GitTier`
   write. **Logically idempotent**: an unchanged `source_digest` (and
   its derived `title`) is a no-op; a changed `.feature` file appends
   exactly one new record (the OLD one is never overwritten — Hive
   append-only). A `.feature` edit re-materializes every scenario in
   that file (file granularity, not per-scenario — the line-scan can't
   isolate a single scenario's body without becoming a full parser;
   bounded churn under idempotence, design D4).
4. **Duplicate guard** — a `scenario_id` scanned more than once WITHIN
   one root's corpus can't pick a winning title/digest, so that root
   aborts (0 writes) too — reported via a `sync_errors` entry, NOT a
   frozen `canon-fmt::FmtFailureClass` violation (that 11-class set is
   closed; a duplicate scenario_id is a D5 sync-level fault, a
   different lane). Two DIFFERENT roots sharing the same `scenario_id`
   stay distinct — `project_id` isolates them (design D6).

`Scenario`'s index shape is deliberately GENERAL: `title` +
`source_digest`, nothing else. It carries no `covered`/`surface_ref`
field — coverage stays `canon gate check`'s own `uncovered-cell`
authority, never a sync-populated fact (design D2/proposal.md's
"covered ≠ coverage").

## `specs.roots[]` config (`canon.yaml`, design D3)

```yaml
specs:
  roots:
    - id: app-a         # STABLE LITERAL — never the checkout directory
      root: apps/a/specs #   name (that would split identity across clones)
    - id: app-b
      root: apps/b/specs
```

- **Absent `specs:` key** → the single default root, `{id: root, root:
  specs}` (relative to the repo root).
- **Present `specs:` with an empty/missing `roots[]`** → fails LOUD
  (`InventoryError::Config`) — a present-but-incomplete config must
  never silently resolve to zero roots at a hollow exit 0. Only an
  ABSENT `specs:` key gets the default.
- **A malformed entry** (missing `id`, `roots` not a list, `id` not a
  valid `ProjectId`) → fails LOUD, never a silent fallback to the
  default root.
- `--spec-root <dir>` bypasses `specs.roots[]` ENTIRELY and syncs
  exactly that one ad hoc directory, under the same stable literal
  `root` id the absent-`specs:` default uses.

Only the fail-loud-on-malformed / default-on-missing SEMANTICS reuse
`IngestSourceConfig::load`'s pattern — the named multi-root LIST shape
itself is new to `specs.roots[]`.

## The `SyncCtx` seam (s15 P5, spec-ledger-selftest Req 2)

Sync logic is driven through `crate::inventory::SyncCtx` — the SAME
`repo`/`ledger_root` rebindable-roots shape `canon_gate::GateCtx` uses,
composed directly over it (never a second, hand-rolled resolution of
`<repo>/canon.yaml`'s `tiers.git.root`):

```rust
use canon_cli::inventory::{SyncCtx, run_sync_with_ctx};

let ctx = SyncCtx::from_repo(repo_path);       // production: resolves canon.yaml
// or, fully offline against a fresh tempdir corpus:
let ctx = SyncCtx::from_fixture(fixture_dir);

let outcome = run_sync_with_ctx(&ctx, /* spec_root_override */ None)?;
```

`run_sync_with_ctx` is the ONE downstream sync entry point — a
production `canon inventory sync` (`SyncCtx::from_repo`, via the
public `run_sync(repo, spec_root)` wrapper) and `canon selftest`'s own
inventory fixture corpora (`SyncCtx::from_fixture`) both call it;
neither branches on which constructor built its `ctx`. `SyncCtx::
spec_roots(spec_root_override)` is the one `specs.roots[]` resolver
both constructors run through.

## Selftest coverage (`canon selftest`, `spec-ledger-selftest` suite)

`crates/canon-cli/src/inventory_selftest.rs` registers a 9th `canon
selftest` suite covering:

- **Two-sided exact-set oracles** (`crates/canon-cli/fixtures/
  inventory/{clean-root,missing-provenance,duplicate-scenario}/`) —
  mirrors `canon gate selftest`'s own discipline: a fixture's actual
  violation/`sync_errors` set is diffed against a checked-in
  `expected_*.txt` oracle, reporting BOTH a missing expected entry AND
  an extra unexpected one (over-triggering is a failure, never
  silently accepted as a superset).
- **A frozen-incident fold fixture**
  (`crates/canon-cli/fixtures/inventory/frozen-incident/`) — pins the
  REAL `world.firstbuy-hotdeal.26` divergence-fold case (a
  round-8 backfill with `run_seq: 1` that a fresh round-3 review
  campaign with `run_seq: 3` correctly outranks) through the REAL
  `fold_to_current_state`, plus the resolved-then-invalidated
  live-binding re-check (design D8).

Run it with `canon selftest` (all 9 suites) or `cargo test -p
canon-cli inventory_selftest`.

## What this skill does NOT cover

- **`canon review add` / `canon divergence {stage,promote,resolve,defer}`**
  (the native VERDICT producers `sync` feeds into) — see the
  native-verdict-lifecycle spec and each command's own `--help`; this
  skill only covers the INDEX stage upstream of them. Briefly: `canon
  review add --project-id <p> --scenario-id <s> --reviewer <r> --pin
  <sha> (--upstream-ref <ref> | --original-spec-ref <ref>) --role <r>`
  writes one attributed `Review`; `canon divergence stage` writes an
  unordered staging candidate, `canon divergence promote` assigns the
  monotonic `run_seq`, `canon divergence {resolve,defer}` direct-commit
  outside the staging batch, and `canon divergence status` renders the
  S9 burn-down's current-state view.
- **`canon gate check`/`canon gate selftest`** — see the
  `trust-spine-gate` skill for the full gate contract (trust ladder,
  staleness, the flag ratchet, staging→promote).
- **`canon ingest artifacts`'s native-record flywheel wiring** — the
  `artifacts.native_records: bool` `canon.yaml` switch (NOT a CLI
  flag) that enables the `Review`/`Divergence` records-source adapters
  against canon's own tiers, XOR-exclusive with the raw-artifact path
  fields (`ledger_root`/`divergences_root`/`openspec_root`) — see the
  `canon-artifact-ingest` skill for the adapter/verdict-derivation
  mechanics this pipes into.
- **A Gherkin parser beyond the existing line-scan subset.** `sync`
  reuses `canon-fmt::gherkin::scan` unchanged — no new parsing surface,
  by design (D4).
- **`canon scenario new` / `canon feature new`** (s16 P5,
  `corpus-authoring-scaffold` spec — NO LONGER hypothetical, now
  built) — a `.feature`-file TEMPLATE generator only, writing NO
  ledger record; the author still runs `canon fmt --check` then
  `canon inventory sync` afterward, exactly as for a hand-authored
  file. See `crates/canon-cli/src/scaffold.rs`'s own doc comment and
  the `canon-plugins` skill's P1-P6 surface map.
