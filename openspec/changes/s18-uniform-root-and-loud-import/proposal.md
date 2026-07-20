## Why

canon's own usability review (`target/usage-review/najun-art-dummy/SYNTHESIS.md`,
four independent personas, unanimous **conditional** adoption) names exactly
two **BLOCKERS** — not gaps, blockers — and both are reproduced live against
the built binary below, not merely asserted from source reading.

**B2 — `canon query` breaks from a subdirectory (hit by all four personas).**
`canon query` is the ONE verb that resolves `canon.yaml` from a literal
`--canon-yaml <path>` flag defaulting to the bare string `"canon.yaml"`
(`crates/canon-cli/src/query.rs:85-87`), fed straight into
`tiers::build_tiers`'s `std::fs::read_to_string(canon_yaml_path)`
(`crates/canon-cli/src/tiers.rs:78-81`) — no ancestor walk. Every sibling
verb this review's loop actually drove (`canon context`, `canon gate check`,
`canon inventory sync`, `canon ingest artifacts`, `canon ingest plans`,
`canon report`, `canon dashboard`, `canon retrieve`) instead takes `--repo
<dir>` (default `.`) resolved through the ONE shared
`canon_cli::context::resolve_repo_root` (`context.rs:247-252`): `repo == "."`
walks `cwd.ancestors()` for the nearest `canon.yaml` (git's own convention,
per that function's doc); any OTHER explicit `--repo` is used as-is. Live
repro (this session, built binary, `s18-repro` fixture repo with `canon.yaml`
at its root):

```
$ cd s18-repro/sub/dir && canon query --kind change
canon query: reading `canon.yaml`: No such file or directory (os error 2)
$ echo $?
1
$ cd s18-repro/sub/dir && canon context --json | head -3   # sibling verb, same repo, same cwd
{ "capabilityVersion": 1, "kinds": { … } }
$ echo $?
0
```

A Makefile target, editor task, or git hook that invokes `canon query` from
whatever cwd it happens to run at (exactly how every OTHER canon verb is
already invoked, per this review's own loop transcript) breaks the moment
cwd is not the repo root — while every sibling verb in the same loop keeps
working. Traced to `query.rs:70-71` (`run(canon_yaml: &Path, …)` takes the
literal path straight through, never calling `resolve_repo_root`) and
`tiers.rs:78-81` (`build_tiers` reads that literal path with no fallback).

**B1 — `canon ingest plans`'s `root:` near-miss exits 0 (Developer's #1 ask,
reproduced fresh by Developer + Planner).** `plans.sources[].root: openspec`
— the natural first guess for "point at my openspec dir" when the correct
value is `openspec/changes` — makes `discover_change_dirs` fall through its
`openspec/changes` substructure check (`root.join("openspec").join
("changes")` does not exist under `openspec` itself), scan `openspec`'s own
immediate children instead, and find exactly one: `changes/`. `ChangeId::
parse("changes")` succeeds (a valid kebab slug), so `parse_change_dir` treats
`openspec/changes` as ONE change directory, finds no `proposal.md` at
`openspec/changes/proposal.md`, and increments `malformed` by 1 — while the
17 real change dirs one level below are never visited
(`crates/canon-ingest/src/plan_adapters/openspec.rs:99-108` for discovery,
`:135-151` for the malformed-proposal skip). The driver
(`crates/canon-cli/src/plans.rs`) surfaces `malformed` as one JSON field
alongside `changes_persisted: 0`/`tasks_persisted: 0`, and `main.rs::
run_ingest_plans` returns `ExitCode::SUCCESS` unconditionally on `Ok(_)`
(`main.rs:1161-1178`) — the outcome's contents never feed the exit code.
Live repro (this session, built binary, the same `s18-repro` fixture with
`root: openspec` misconfigured one level too high above one real change
dir with `proposal.md`+`tasks.md`):

```
$ canon ingest plans --repo . --json
{ "sources": [ { "root": ".../openspec", "changes_parsed": 0, "tasks_parsed": 0,
  "changes_persisted": 0, "tasks_persisted": 0, "malformed": 1, … } ],
  "changes_persisted": 0, "tasks_persisted": 0, … }
$ echo $?
0
```

*Contrast, proving canon already knows how to do this:* a non-list
`plans.sources`, an unregistered dialect id, or a nonexistent configured
root all fail the command LOUD today (`plans.rs`'s own `PlansError` variants,
pinned by s17's `openspec-plan-dialect`/`plan-import-connector` spec
scenarios "A typo'd key in a present plans section fails loud" / "A
nonexistent configured source root fails loud"). The `root:`-one-level-off
near-miss is the one malformed-but-technically-valid-YAML shape that still
exits clean — the single failure mode in the whole s17 surface that is
*silent*, in a tool whose entire pitch (s15/s16/s17's shared framing) is
"never silently drop evidence." A `canon ingest plans && deploy.sh` CI chain
ships green while importing nothing.

**Why these two, together, now:** the SYNTHESIS ranks both as adoption
BLOCKERS (distinct from its three GAPs, B3-B5) and its punch-list ranks
"make B1 loud" and "unify root resolution" as the top two fixes by
adoption impact — "lowest effort, highest trust-recovery" for B1, "unblocks
scripted/subdir use" for B2. Neither touches the closed 12-kind model, gate
authority, or the single-`Scenario`-producer discipline; both are surface
fixes to CLI plumbing s17/s12/S2 already built the correct pattern for
elsewhere in the same binary.

## What Changes

- **`canon query` gains `--repo <REPO>` (default `.`), resolved through the
  SAME `canon_cli::context::resolve_repo_root` ancestor walk `canon
  context`/`canon gate check`/`canon ingest artifacts`/`canon ingest plans`
  already share** — `repo == "."` walks `cwd.ancestors()` for the nearest
  `canon.yaml`; any other explicit `--repo <dir>` is used as-is, identical
  semantics, zero new resolution logic invented. `canon.yaml` is then
  `<resolved-repo>/canon.yaml`.
- **`--canon-yaml <path>` is KEPT as an explicit low-level override, not
  removed** — a caller who already pins an exact non-standard `canon.yaml`
  path (a snapshot copy, a test fixture, a CI cache) keeps working
  byte-for-byte. Precedence: `--canon-yaml`, when explicitly supplied,
  is used AS-IS (the literal path, no walk) and wins over `--repo`; when
  `--canon-yaml` is absent, `--repo`'s resolution governs. The two flags
  are never both meaningful at once — one wins, deterministically, and the
  existing "no flags at all" invocation shape callers already use for every
  sibling verb starts working for `query` too, with no flag added to any
  existing script that already passes `--canon-yaml` explicitly.
- **Every OTHER verb is UNCHANGED** — `canon context`, `canon gate *`,
  `canon inventory sync`, `canon plugin sync`, `canon ingest artifacts`,
  `canon ingest plans`, `canon report`, `canon dashboard`, `canon retrieve`,
  `canon feature new`, `canon scenario new` already take `--repo` resolved
  via `resolve_repo_root`; `query` is the one verb this change brings into
  that convention. (`canon tier age` and `canon ingest sessions` also take a
  literal `--canon-yaml`/`--canon-yaml`-shaped flag with no ancestor walk —
  see non-goals for why they are deliberately out of this change's scope.)
- **`canon ingest plans`'s malformed diagnostics gain a path + reason, not
  just a count.** Each malformed construct (an unreadable directory, a
  basename failing `ChangeId`'s grammar, a directory with no readable
  `proposal.md`) is named in the pass summary by its relative path and its
  specific reason — matching the bar every other loud path in the same
  command already holds (typo'd key names the key; unregistered dialect
  names the id and the registered set; nonexistent root names the source).
  A malformed dir whose basename is literally `changes` (the exact
  root-one-level-too-high signature) carries an additional actionable hint:
  the `root:` value may be pointing at the changes directory's PARENT
  rather than at (or above) `openspec/changes` itself.
- **A source yielding `malformed > 0` and zero persisted records
  (`changes_persisted == 0 && tasks_persisted == 0`) is non-clean at the
  process level.** The driver emits an unconditional stderr WARN — visible
  regardless of `--json`, mirroring `run_ingest_plans`'s existing
  always-print-`unwritten` precedent — naming the source's dialect, root,
  and malformed count; `canon ingest plans` exits non-zero (reusing the
  established `0`-clean/non-zero-non-clean convention `canon gate check`'s
  own exit code already establishes for this binary, distinct from the
  already-loud `PlansError` config-failure path, which keeps its own exit
  code). A source that is legitimately empty (`malformed == 0`, zero
  changes found because there genuinely are none yet) is UNCHANGED: still a
  clean, silent, zero-source no-op — this condition targets ONLY the
  malformed-but-YAML-valid near-miss, never a fresh/empty plan tree.
- **s17's `openspec-plan-dialect`/`plan-import-connector` spec scenarios for
  the malformed-per-construct behavior are extended, not replaced** — "one
  malformed change dir does not sink the pass" still holds (siblings still
  import normally); this change adds WHAT the malformed entry now carries
  and WHETHER a wholly-unproductive pass now exits non-zero, on top of the
  existing skip-and-count discipline.

### Added Capabilities

- `uniform-repo-resolution`: `canon query` accepts `--repo` (default `.`)
  resolved through the shared `resolve_repo_root` ancestor walk, achieving
  subdirectory-invocation parity with every sibling verb; `--canon-yaml`
  is preserved as an explicit, precedence-winning override for callers that
  already pin a literal path.
- `loud-plan-import-diagnostics`: every malformed plan-import construct
  carries a named path + reason (plus a root-near-miss hint for the
  `changes`-basename signature), and a source that is malformed-nonzero
  with zero persisted records makes the whole `canon ingest plans` process
  non-clean (unconditional stderr WARN + non-zero exit) rather than silently
  exiting 0.

### Explicit non-goals

- No change to `canon tier age`'s or `canon ingest sessions`'s
  `--canon-yaml` flag. Both share `query`'s literal-path-no-walk shape
  (`tier.rs:43-44` calls `tiers::build_tiers(canon_yaml)` directly;
  `ingest.rs:224-225` derives `repo_root` from `canon_yaml.parent()`, no
  ancestor walk either) but neither was named a BLOCKER by any of the four
  personas' live loops in the SYNTHESIS — `canon query` is the one verb
  this review's own evidence reproduces breaking from a subdirectory.
  Widening the fix to those two verbs is a plausible FOLLOW-UP (same shape,
  same fix), deliberately deferred here to keep this change's diff scoped
  to the actually-reported blocker; it does not require a design decision
  this change would need to make differently.
- No change to `resolve_repo_root` itself (`context.rs:247-252`) — the
  ancestor-walk algorithm, its `repo != "."` used-as-is short-circuit, and
  every existing caller are UNCHANGED. This change adds ONE new caller
  (`canon_cli::query::run`), never a new resolution strategy.
- No change to `canon.yaml` config surface, `plans:` section grammar,
  `PlanAdapter` trait, `plan_registry`, or the openspec dialect's mapping
  rules (change-dir discovery, `ChangeId`/`TaskId` derivation, status
  derivation, drop-diagnostic categories) — s17 already spec'd and shipped
  all of that; this change only enriches WHAT a malformed entry reports and
  adds a process-level non-clean signal on top.
- No retroactive exit-code change for a genuinely empty/fresh plan source
  (`malformed == 0`) — that path stays the clean, silent no-op s17 already
  specs ("An absent plans section is a clean no-op"). This change's
  non-clean condition is `malformed > 0 && persisted == 0`, never
  `persisted == 0` alone.
- No change to `--dialect`/`--source` one-shot override semantics, the
  watermark-cursor gate, `TierRegistry::persist`, the `unwritten` seam, or
  cross-source `duplicate-change-id` resolution — all s17-shipped and
  untouched.
- No change to `canon-gate`, `canon-store`, `canon-model`, `canon-plugin`,
  or `canon-vocab` — connector-never-authority holds unchanged: neither
  fix touches gate verdicts, the closed 12-`RecordKind` set
  (`RecordKind::ALL.len() == 12` stays asserted at its three sites), or
  `canon inventory sync`'s single-`Scenario`-producer discipline.
- No new `canon` subcommand — both fixes are surface changes to two
  EXISTING commands (`canon query`, `canon ingest plans`), never a new verb.

## Impact

- **`canon-cli`**: `query.rs`'s `run` signature gains repo resolution ahead
  of `tiers::build_tiers` (mirroring `plans.rs::run`'s own
  `resolve_repo_root(repo)` call at its top); `main.rs`'s `Command::Query`
  variant gains a `--repo` arg beside the retained `--canon-yaml` (now
  `Option<PathBuf>`, no default, so its presence/absence is distinguishable
  from `--repo`'s always-present default `.`).
- **`canon-ingest`**: `plan_adapters/openspec.rs`'s `parse_change_dir`
  (and any other malformed-producing site in the adapter) reports a path +
  reason per malformed construct instead of an anonymous increment;
  `PlanParseOutcome`'s malformed field carries that detail instead of (or
  alongside) the existing bare count, consumed identically by any other
  `PlanAdapter` implementation (the shape is on the shared outcome type,
  not openspec-specific).
- **`canon-cli`**: `plans.rs`'s driver and `PlanSourceSummary`/
  `PlansOutcome` surface the named malformed entries in both `format_human`
  and `format_json`; `main.rs::run_ingest_plans` inspects the outcome for
  the new non-clean condition and returns a non-zero `ExitCode` instead of
  the current unconditional `ExitCode::SUCCESS` on `Ok(_)`.
- **`canon-model` / `canon-store` / `canon-gate` / `canon-learn` /
  `canon-vocab` / `canon-plugin`**: UNCHANGED. No new `RecordKind`, no new
  core field, no gate-authority change — the same acceptance bar every
  prior wave in this repo held itself to.
- **s17's specs** (`plan-import-connector`, `openspec-plan-dialect`): their
  existing malformed-handling scenarios remain valid; this change's
  `loud-plan-import-diagnostics` spec sits alongside them as an ADDED
  capability, not a modification to s17's own spec files.
