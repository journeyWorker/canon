//! `canon` — single entrypoint binary (design §4). S0 ships `canon
//! --version` (design D5, the literal acceptance-criterion surface),
//! `canon skills install` (task group 5, S0), S2 (`s2-tiered-
//! storage`) adds `canon tier age` (task 3.3) and `canon query` (task
//! 4.1), S11 (`s11-format-authority-migration`) adds `canon fmt
//! --check` (task 2.1), S3 (`s3-session-ingest`, Wave 1) adds
//! `canon ingest sessions [--watch]` (task 5.1), S12
//! (`s12-canon-context`) adds `canon context [--repo][--json]` — a
//! capability QUERY over the same schema/policy registry `canon fmt`/
//! `canon gate` validate against, never validation itself (see
//! `canon_cli::context`'s module doc for the three invariants) — and S5
//! wave-2-part2 (`s5-trust-spine-gate`) adds `canon gate
//! check/task/promote/install-hooks/selftest` (`canon_cli::gate`'s own
//! module doc). S10 part2 (`s10-typed-authoring-vocabulary`) wires
//! `canon-vocab`'s capability-snapshot resolution into two of THOSE
//! existing subcommands rather than adding a new one: `canon context`'s
//! surface now also carries the typed authoring vocabulary's
//! directive/enum/evidence-kind index (`canon_cli::context`'s module doc,
//! invariant 2), and `canon gate task` gains a typed-evidence path
//! alongside its existing free-form one (`canon_cli::gate::run_task`'s own
//! doc, design.md D4). S8 part2 (`s8-retrieve-before-task`) adds `canon
//! — the CLI surface over `canon_learn::guidance::retrieve_guidance`
//! (S8Core's library core); see `canon_cli::retrieve`'s own module doc.
//! S9 part2 (`s9-unified-surface`) adds `canon report [--repo][--check]
//! [--snapshot <dir>]` (see `canon_cli::report`'s module doc); S9 part3
//! adds `canon dashboard [--repo][--snapshot <dir>][--port <n>]`,
//! serving the built `packages/dashboard` app locally against a
//! snapshot (see `canon_cli::dashboard`'s module doc). s16 P5
//! (`corpus-authoring-scaffold`, INDEPENDENT of s16's plugin
//! machinery) adds `canon scenario new <tag> --title <label>
//! --feature <path>` and `canon feature new <area>.<surface> --title
//! <label>` — see `canon_cli::scaffold`'s module doc. Every other
//! subcommand is a later spec's responsibility.

use std::path::PathBuf;
use std::process::ExitCode;

use canon_model::envelope::RecordKind;
use canon_model::{regime_key, ChangeId, ProjectId, RegimeKey, RoleId, ScenarioId, Sha, SubjectId};
use canon_learn::StrategyId;
use canon_cli::scaffold::AreaSurface;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "canon",
    version,
    about = "canon — harness knowledge substrate",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Materialize `canon/skills/` companion skills into consumer repos.
    Skills {
        #[command(subcommand)]
        action: SkillsCommand,
    },
    /// Tier-storage maintenance (S2 `s2-tiered-storage`).
    Tier {
        #[command(subcommand)]
        action: TierCommand,
    },
    /// Fan out a record kind's read across every tier it may currently
    /// live in and merge by `at` (S2 task 4.1, unified-query spec).
    /// `--plugin <id>` (s16 P3, tasks.md 3.3) projects a resolved
    /// plugin's declared overlay fields onto each queried record --
    /// fail-soft (`canon_cli::query`'s module doc): an unresolved
    /// plugin, or a `--kind`/overlay `core_kind` mismatch, degrades to
    /// the unmodified core view plus a stderr diagnostic, never a
    /// process error.
    Query {
        /// A `RecordKind` wire string (`RecordKind::as_str()`, e.g.
        /// `handoff`, `strategy_item`).
        #[arg(long, value_parser = canon_cli::query::parse_kind)]
        kind: RecordKind,
        /// Only records with `at >= <since>` (RFC3339/ISO-8601).
        #[arg(long, value_parser = canon_cli::query::parse_since)]
        since: Option<DateTime<Utc>>,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`,
        /// the SAME ancestor walk every sibling verb (`canon context`,
        /// `canon gate check`, `canon ingest plans`, …) already uses:
        /// `--repo == "."` (the default) walks `cwd.ancestors()` for the
        /// nearest `canon.yaml`; any other explicit `--repo <dir>` is used
        /// as-is. Ignored whenever `--canon-yaml` is also supplied
        /// (`--canon-yaml` wins — see that flag's own doc).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// An explicit, literal `canon.yaml` path override that BYPASSES
        /// `--repo`'s ancestor walk entirely — read AS-IS from exactly this
        /// path, regardless of cwd. Takes precedence over `--repo` when
        /// supplied. Omitted (the ordinary invocation shape every sibling
        /// verb already supports): `--repo`'s ancestor-walk resolution
        /// governs instead.
        #[arg(long)]
        canon_yaml: Option<PathBuf>,
        /// Machine-readable output (the merged record bodies) instead
        /// of the default human table.
        #[arg(long)]
        json: bool,
        /// A `canon/plugins/<id>/plugin.yaml` manifest id (s16 P3) --
        /// project that plugin's declared overlay fields onto each
        /// queried record before printing/emitting. Omitted: output is
        /// byte-identical to the pre-s16 `canon query`.
        #[arg(long)]
        plugin: Option<String>,
        /// s19 `query-scope-filters` (design D5): scopes `--kind
        /// change`/`--kind task` to one `ChangeId`'s own row(s) —
        /// `Change.change_id` equality, or `TaskId::change_id()`
        /// equality for `--kind task`. Any other `--kind` fails loud
        /// (exit `2`) — kind-gating happens in `canon_cli::query`, not
        /// here, so the error can name the queried kind
        /// (`canon_cli::query::validate_scope`).
        #[arg(long, value_parser = canon_cli::query::parse_change_id)]
        change_id: Option<ChangeId>,
        /// s19 `query-scope-filters`: scopes `--kind change`/`--kind
        /// task` by their own `status` field. Kept a raw `String` here
        /// (never a clap value parser) because its valid domain
        /// depends on the QUERIED kind (`open`/`done` for `task`; the
        /// four `ChangeStatus` values for `change`) — validated in
        /// `canon_cli::query::validate_scope`, which can name the
        /// kind-specific valid set on a mismatch.
        #[arg(long)]
        status: Option<String>,
        /// s36 `subject-domain-loop`: scopes `--kind subject` to one
        /// `domain` (its own `domain` field equality). Any other
        /// `--kind` fails loud (exit `2`) — kind-gating in
        /// `canon_cli::query::validate_scope`, mirroring `--change-id`.
        #[arg(long)]
        domain: Option<String>,
    },
    /// Validate a consumer-repo corpus (e.g. `spec/`) against the
    /// artifact-family schemas + layout descriptors (S11 task 2.1).
    Fmt {
        /// Report every violation found — `canon fmt`'s only
        /// supported mode today.
        #[arg(long)]
        check: bool,
        /// Corpus root, e.g. a consumer repo's `spec/` directory.
        root: PathBuf,
        /// Repo root the corpus `root` is resolved under (s26 D1). Omitted
        /// (the default, i.e. every existing invocation): `root` is used
        /// EXACTLY as given -- byte-identical to today, no new code path
        /// runs. Given: resolved via the SAME
        /// `canon_cli::context::resolve_repo_root` ancestor walk every
        /// sibling verb's `--repo` uses (`--repo .` walks `cwd.ancestors()`
        /// for the nearest `canon.yaml`; any other explicit `--repo <dir>`
        /// is used as-is), and the corpus actually checked becomes
        /// `resolve_repo_root(repo).join(root)` -- `root` stays the
        /// corpus-relative suffix, never replaced.
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Emit the project-resolved AUTHORING SURFACE (S12
    /// `context-authoring-surface`): record kinds + envelope fields,
    /// enum domains, join-key grammars, partition layout,
    /// policy-derived requirements, and a capability version. A
    /// capability QUERY, never validation — exits 0 with the full
    /// surface even when `canon fmt --check`/`canon gate` would
    /// report diagnostics against the same repo.
    Context {
        /// Repo root to resolve the schema/policy registry against
        /// (`<repo>/canon/policy.yaml`). Omitted, or the literal `.`
        /// default, resolves the PROJECT root instead — the nearest
        /// ancestor of cwd carrying a `canon.yaml` (design D7, task 1.4;
        /// `canon_cli::context::resolve_repo_root`), matching `canon
        /// fmt`/`canon gate`'s own `canon.yaml`-anchored root convention —
        /// so `canon context` run from any subdirectory still surfaces the
        /// repo root's policy, never a subdirectory's absence of one. Any
        /// OTHER explicit `--repo <dir>` is used as-is (no walk), same as
        /// `canon gate`'s own `GateCtx::from_repo`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the machine-readable JSON surface instead of the
        /// default compact human outline — both render from the
        /// identical resolved surface (design D5).
        #[arg(long)]
        json: bool,
    },
    /// Scan + parse + normalize agent-CLI session transcripts into
    /// canon-model records, project-scoped by default (s31
    /// `s31-scoped-session-ingest`). See `canon_cli::ingest`'s module
    /// doc.
    Ingest {
        #[command(subcommand)]
        action: IngestCommand,
    },
    /// The trust-spine gate (S5 wave-2-part2, `s5-trust-spine-gate`):
    /// evidence-gated coverage/verdict-ledger/staleness/trust-ladder
    /// checking, task-checkbox flips, staging→promote, and hook-seam
    /// installation. See `canon_cli::gate`'s module doc.
    Gate {
        #[command(subcommand)]
        action: GateCommand,
    },
    /// `canon review add` (s15 P3b, native-verdict-lifecycle spec):
    /// the native, attributed `Review` producer. See
    /// `canon_cli::review`'s module doc.
    Review {
        #[command(subcommand)]
        action: ReviewCommand,
    },
    /// `canon divergence {stage,promote,resolve,defer}` (s15 P3b,
    /// native-verdict-lifecycle spec): the native `Divergence`
    /// producer + monotonic `run_seq` promotion. See
    /// `canon_cli::divergence`'s module doc.
    Divergence {
        #[command(subcommand)]
        action: DivergenceCliCommand,
    },
    /// `canon inventory sync [--spec-root <dir>]` (s15 P3a,
    /// `inventory-materialization` spec): validates each configured
    /// `specs.roots[]` entry (`canon.yaml`, design D3) with
    /// `canon-fmt::check`, then materializes one `Scenario`
    /// ledger-index record per `(project_id, scenario_id)` — the
    /// general feature-corpus → ledger indexer. See
    /// `canon_cli::inventory`'s module doc.
    Inventory {
        #[command(subcommand)]
        action: InventoryCommand,
    },
    /// `canon plugin sync <plugin-id> [--spec-root <dir>]` (s16 P4,
    /// `openspec/changes/s16-plugin-extensibility/`, tasks.md 4.3,
    /// design.md D5, `porting-plugin` spec): the GENERIC dispatcher —
    /// resolves `<plugin-id>`'s manifest-declared overlay(s) (`canon-
    /// plugin` P1), hands each to its registered `OverlaySource` impl,
    /// and writes every returned candidate through P2's validate-then-
    /// write pipeline (`canon_plugin::overlay::write_overlay`). See
    /// `canon_cli::plugin_sync`'s module doc — this command never
    /// string-matches a specific plugin id itself.
    Plugin {
        #[command(subcommand)]
        action: PluginCommand,
    },
    /// `canon scenario new <area>.<surface>.<nn> --title <label>
    /// --feature <path>` (s16 P5, `corpus-authoring-scaffold` spec,
    /// tasks.md 5.1): appends (creating if absent) an S11-conformant
    /// `.feature` stub — the exact tag-then-header shape
    /// `canon-fmt::gherkin::scan` already reads (s15 D4). Writes NO
    /// ledger record; see `canon_cli::scaffold`'s module doc.
    /// INDEPENDENT of every s16 P1-P4 plugin concern — a
    /// corpus-authoring convenience only.
    Scenario {
        #[command(subcommand)]
        action: ScenarioCommand,
    },
    /// `canon feature new <area>.<surface> --title <label>` (s16 P5,
    /// `corpus-authoring-scaffold` spec, tasks.md 5.2): scaffolds a
    /// fresh, zero-scenario `.feature` file for a not-yet-started
    /// surface — fails loud rather than overwriting an existing one.
    /// The stub carries only a `Feature:` header + `# canon:`
    /// provenance; an empty feature is not yet a valid corpus entry, so
    /// `canon fmt --check` flags it until the first `canon scenario new`
    /// adds a `@<area>.<surface>.<nn>`-tagged scenario — the spec ties
    /// the fmt-clean round-trip to `scenario new`'s output, never the
    /// bare stub. See `canon_cli::scaffold`'s module doc.
    Feature {
        #[command(subcommand)]
        action: FeatureCommand,
    },
    /// `canon subject {new,adopt,status}` (s36 `subject-domain-loop`):
    /// author + evolve the durable product/management unit (the
    /// reviewed 13th record kind). See `canon_cli::subject`'s module
    /// doc.
    Subject {
        #[command(subcommand)]
        action: SubjectCommand,
    },
    /// `canon init [--repo <dir>]` + `canon init --check-config
    /// [--repo <dir>]` (s19 `canon-init-scaffold`): scaffolds a fresh,
    /// WORKING `canon.yaml` skeleton (`tiers:`/`routing:`/`specs:`/
    /// `plans:`, design D8/D9) at `<repo>/canon.yaml` -- refuses to
    /// overwrite an existing one -- or, with `--check-config`,
    /// READ-ONLY validates an existing `canon.yaml` by chaining the
    /// SAME three strict loaders `canon inventory sync`/`canon ingest
    /// plans`/`canon tier age` already use (design D7). See
    /// `canon_cli::init`'s module doc.
    Init {
        /// Repo root `<repo>/canon.yaml` resolves against -- used
        /// AS-IS, literally joined (never an ancestor walk: `init`'s
        /// whole job is bootstrapping the FIRST `canon.yaml`, so
        /// there is no existing repo root to walk up and find).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// READ-ONLY validate an existing `canon.yaml` instead of
        /// writing one -- mutually exclusive in effect with the
        /// default write mode (never both in the same invocation).
        #[arg(long)]
        check_config: bool,
    },
    /// `canon demo {init,attest}`: a self-contained, throwaway
    /// evidence-loop demo for first-time users. `demo init` scaffolds a
    /// real demo repo (the `canon init` skeleton + a `reviewer`-requiring
    /// `policy.yaml`) and seeds one dev-authored evidence record, so
    /// `canon gate check` is RED (`uncovered-cell`); `demo attest` records
    /// the reviewer evidence, turning the same gate GREEN. See
    /// `canon_cli::demo`'s module doc.
    Demo {
        #[command(subcommand)]
        action: DemoCommand,
    },
    /// S8 part2 (`s8-retrieve-before-task`, design.md decisions 1/3):
    /// role+regime-scoped strategy retrieval — the CLI surface over
    /// `canon_learn::guidance::retrieve_guidance`. FAIL-SOFT: once
    /// `--role`/`--regime` parse and agree with each other (a CLI usage
    /// precondition — exit `2` on mismatch, see `canon_cli::retrieve`'s
    /// module doc), retrieval always exits `0`, printing possibly-empty
    /// guidance; a store outage or malformed row degrades to an empty
    /// list internally, never a nonzero exit.
    Retrieve {
        /// A `RoleId` slug — the retrieval's role scope (design
        /// decision 1). With `--regime` it MUST equal that key's own
        /// leading segment; with `--domain`/`--subject` it is the
        /// `<role>` the derived regime candidates are built from.
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// The full `regime_key` string (`<role>/<repo>/<area>/<hash>`)
        /// — the SAME canonical serialization S6's write side produces
        /// (no second key derivation, design decision 1). MUTUALLY
        /// EXCLUSIVE with `--domain`/`--subject` (s36): supply either
        /// an explicit regime OR the derived `<domain>[/<subject>]`
        /// pair, never both — enforced in `canon_cli::retrieve` (exit
        /// `2` on violation), never a clap group.
        #[arg(long, value_parser = canon_cli::retrieve::parse_regime)]
        regime: Option<RegimeKey>,
        /// s36 `subject-domain-loop`: derive the retrieval regime from
        /// a `domain` (kebab slug), trying `<domain>/<subject_id>` then
        /// `<domain>` in the `<area>` segment (fallback hierarchy).
        /// Mutually exclusive with `--regime`.
        #[arg(long)]
        domain: Option<String>,
        /// s36 `subject-domain-loop`: narrows the `--domain`-derived
        /// regime to one `subject_id` (tried before the domain-only
        /// fallback). Requires `--domain`; mutually exclusive with
        /// `--regime`.
        #[arg(long, value_parser = canon_cli::retrieve::parse_subject)]
        subject: Option<SubjectId>,
        /// Top-`k` cap, defaulting to
        /// `canon_learn::guidance::DEFAULT_K` when omitted.
        #[arg(long)]
        k: Option<usize>,
        /// Repo root — resolved via
        /// `canon_cli::context::resolve_repo_root` (design D7), same
        /// as `canon context`/`canon gate`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Machine-readable output (the raw `Vec<StrategyRef>`
        /// snapshot, the exact shape `Run::injected_guidance` would
        /// embed) instead of the default human table.
        #[arg(long)]
        json: bool,
    },
    /// Serialize + validate one canonical `regime_key`
    /// (`<role>/<repo>/<area>/<hash>`) from raw segments — the
    /// SHELL-facing counterpart to `canon_model::ids::regime_key`, the
    /// ONE serializer S4/S6/S7/S8's Rust write+read paths all call
    /// (design decision 1, "no second key derivation"). Exists so hook
    /// / script authors (S8's `pre-dispatch.sh`) assemble the retrieval
    /// `--regime` through this EXACT normalizer instead of a second,
    /// drifting shell derivation that could write `my_repo` yet query
    /// `my-repo` and silently miss the namespace
    /// (`s8-retrieve-before-task` whole-branch-review fix). Prints the
    /// validated key and exits `0`; on a malformed result (empty
    /// segment, or a non-hex/too-short `<hash>`) reports to stderr and
    /// exits `2` with nothing on stdout — a caller's own `|| exit 0`
    /// turns that into a clean fail-soft branch.
    RegimeKey {
        /// The `<role>` segment; canonicalized (trimmed, lowercased,
        /// whitespace/`/` runs collapsed to `-`, all other characters
        /// preserved) exactly as the Rust write path canonicalizes it.
        #[arg(long)]
        role: String,
        /// The `<repo>` segment (same canonicalization) — typically the
        /// repo directory basename.
        #[arg(long)]
        repo: String,
        /// The `<area>` segment (same canonicalization).
        #[arg(long)]
        area: String,
        /// The `<hash>` segment — passed through lowercased/trimmed,
        /// never re-hashed here (a 6-64-char lowercase hex digest the
        /// caller owns, e.g. S3's session digest or an area digest).
        #[arg(long)]
        hash: String,
    },
    /// S9 part2 (`s9-unified-surface`, design D1/D2/D3): the CLI
    /// surface over `canon-report`'s library API — generate the
    /// markdown status report (default), byte-diff it against the
    /// committed copy (`--check`), or export the panel marts to
    /// Parquet + `manifest.json` (`--snapshot <dir>`). See
    /// `canon_cli::report`'s module doc for root/roots resolution.
    Report {
        /// Repo root — resolved via
        /// `canon_cli::context::resolve_repo_root` (design D7), same
        /// as `canon context`/`canon fmt`/`canon gate`/`canon retrieve`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Regenerate the report in memory and byte-diff it against
        /// the existing `canon/REPORT.md` (design D2): exit `0` on no
        /// drift, `1` on `MISSING`/`DRIFT`. Mutually exclusive with
        /// `--snapshot` — when both are given, `--snapshot` wins.
        #[arg(long)]
        check: bool,
        /// Export every panel mart to `<dir>/<table>.parquet` plus a
        /// declared `<dir>/manifest.json` (design D3) instead of
        /// writing/checking the markdown report.
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// S9 part3 (`s9-unified-surface`, tasks.md 6.1): serves the built
    /// `packages/dashboard` app locally, pointed at a snapshot via the
    /// app's own `?snapshot=` override. See `canon_cli::dashboard`'s
    /// module doc for the two-route server shape and the
    /// default-vs-explicit `--snapshot` regeneration rule.
    Dashboard {
        /// Repo root — resolved via
        /// `canon_cli::context::resolve_repo_root` (design D7), same
        /// as every other subcommand.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Directory to serve the snapshot from. Omitted: regenerated
        /// fresh on every run at the conventional
        /// `canon/dashboard-snapshot` scratch dir — "the repo's last
        /// `canon report --snapshot` output" (task 6.1) IS whatever
        /// this run just produced there. Given: served as-is when it
        /// already has a `manifest.json`, else generated there once
        /// (never silently regenerated over a caller-provided
        /// snapshot).
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Local port to bind — `0` picks any OS-assigned free port
        /// (the actually-bound port is always printed regardless of
        /// what was requested). Defaults to the same port
        /// `packages/dashboard`'s own `bun run preview` uses.
        #[arg(long, default_value_t = 4173)]
        port: u16,
    },
    /// S6 (`role-strategy-memory`, task group 4): promote a distilled
    /// strategy from the operator-local parquet warm tier up into the
    /// git-tracked, PR-reviewed tier (`canon/strategies/<role>/<id>.md`).
    /// See `canon_cli::learn`'s module doc.
    Learn {
        #[command(subcommand)]
        action: LearnCommand,
    },
    /// S8 (`retrieve-before-task`, task 2.3): the live run-manifest write
    /// seam — retrieve role+regime guidance at dispatch time and record
    /// it verbatim into a `Run` manifest. See `canon_cli::dispatch`.
    Dispatch {
        #[command(subcommand)]
        action: DispatchCommand,
    },
    /// Run every registered crate's fixture-corpus self-test and diff
    /// against its EXPECTED oracle (design §8, `canon selftest`). Additive
    /// alongside `canon gate selftest`; see `canon_cli::selftest`.
    Selftest {
        /// Emit a machine-readable per-suite JSON summary.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LearnCommand {
    /// Promote one distilled `StrategyItem` (by id) into the git tier
    /// (`canon_learn::promote_strategy`), running the advisory promote
    /// lint (content length + literal absolute paths) as non-blocking
    /// stderr warnings.
    Promote {
        /// The `StrategyId` (ULID) to promote — a malformed id is a
        /// clap usage error (exit `2`), never reaching the command body.
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (design D7), same as every other subcommand.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Preview the promotion (target path + advisory warnings)
        /// without writing the git-tier file.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum DemoCommand {
    /// Scaffold the throwaway demo repo (real `canon init` config + a
    /// `reviewer`-requiring `policy.yaml` + one seeded dev evidence
    /// record). Leaves `canon gate check` RED with `uncovered-cell`.
    Init {
        /// Repo root the demo scaffolds into — used AS-IS (never an
        /// ancestor walk), same as `canon init`. Refuses to overwrite an
        /// existing `canon.yaml`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Record the missing reviewer evidence, turning `canon gate check`
    /// GREEN.
    Attest {
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum DispatchCommand {
    /// Mint a `Run` (status `Running`), retrieve `--role`/`--regime`
    /// guidance, record it into the run's `injected_guidance`, and write
    /// the manifest to `<repo>/.canon/dispatch/<run_id>.json` (a private
    /// side-channel, never canon-store's git tier — see
    /// `canon_cli::dispatch`'s module doc).
    Begin {
        /// The role about to run — MUST equal `--regime`'s leading
        /// segment (design decision 1).
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// The full `regime_key` (`<role>/<repo>/<area>/<hash>`) to
        /// retrieve guidance for.
        #[arg(long, value_parser = canon_cli::retrieve::parse_regime)]
        regime: RegimeKey,
        /// The dispatching agent's id (recorded as the run's actor).
        #[arg(long, default_value = "canon")]
        agent_id: String,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the machine-readable JSON summary (run_id + manifest path
        /// + the recorded guidance) instead of the human line.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum IngestCommand {
    /// Scan every registered `canon-ingest` adapter's session store
    /// (`omp`/`pi`, Claude Code, Codex, Hermes), normalize into
    /// canon-model `Session`/`Run`/`Event` records, and persist through
    /// `canon-store`'s write path (S3 task 5.1). Defaults to THIS
    /// PROJECT's own sessions — the repo's main `git worktree` root
    /// plus every linked one (s31 design D3); see `--all-workspaces`.
    Sessions {
        /// Poll the configured roots on an interval instead of exiting
        /// after one pass. Each pass applies the s31 D1 per-file
        /// watermark gate ([`canon_store::cursor::SourceCursor::diff`]):
        /// a file whose content is byte-identical to its persisted
        /// cursor entry is skipped, so a steady-state `--watch` loop
        /// re-parses only the files that actually changed
        /// (correctness still rests on canon-store's digest-suffixed
        /// idempotent write path — the watermark removes wasted
        /// parse/normalize/persist work above it).
        #[arg(long)]
        watch: bool,
        /// Seconds between `--watch` passes.
        #[arg(long, default_value_t = 30)]
        interval_secs: u64,
        /// The scan root's home directory (defaults to `$HOME`).
        #[arg(long)]
        home: Option<PathBuf>,
        /// This repo's `canon.yaml` (S2's `TierPolicy` source) — the
        /// same tier resolution `canon query`/`canon tier age` use.
        #[arg(long, default_value = "canon.yaml")]
        canon_yaml: PathBuf,
        /// Ignore the persisted watermark cursors and re-parse every
        /// present (in-scope) file this pass (a full rescan / cursor
        /// reset). The digest-idempotent write path keeps a forced
        /// re-ingest from double-writing; cursors are re-advanced
        /// afterward.
        #[arg(long)]
        full: bool,
        /// Scan every workspace on this machine instead of the s31 D3
        /// default project scope (this repo's main worktree + every
        /// linked `git worktree` root) — restores the pre-s31
        /// machine-wide corpus. `Session.project_key` is still stamped
        /// on any session whose workspace resolves into this repo's
        /// project set even with this flag set.
        #[arg(long)]
        all_workspaces: bool,
    },
    /// S14 (`s14-artifact-ingest-cli`): run the artifact/verdict half of
    /// canon's join spine — the `ledger`/`divergence`/`openspec-task`
    /// path-source adapters (config-driven scan) plus the `handoff`
    /// records-source adapter (this driver reads canon's own `Handoff`
    /// records off canon-store's `Tier` and feeds them in) — derive
    /// verdicts (S4), fold into regime-keyed trajectories, and persist
    /// via `canon-learn`'s `store_trajectory` + `rebuild_namespace`
    /// into the SAME store `canon retrieve` (S8) and `canon report`'s
    /// `mart_role_memory`/`mart_flywheel_funnel` (S9) already read. See
    /// `canon_cli::artifact_ingest`'s module doc.
    Artifacts {
        /// Poll on an interval instead of exiting after one pass.
        /// Unlike `sessions`, a re-scan is NOT yet write-idempotent (S4
        /// tasks.md group 6 "Idempotence" is unshipped upstream) — a
        /// repeated pass over an unchanged corpus persists FRESH
        /// trajectories rather than deduping.
        #[arg(long)]
        watch: bool,
        /// Seconds between `--watch` passes.
        #[arg(long, default_value_t = 30)]
        interval_secs: u64,
        /// Repo root — resolved via
        /// `canon_cli::context::resolve_repo_root` (design D7), same
        /// as `canon context`/`canon gate`/`canon retrieve`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the machine-readable JSON outcome instead of the
        /// default human summary.
        #[arg(long)]
        json: bool,
    },
    /// s17 P3 (`s17-plan-import`): the plan-import connector meets
    /// canon-store -- the third instance of `ingest sessions`/`ingest
    /// artifacts`'s adapter -> normalize -> persist shape, generalized
    /// to `canon_ingest::plan_registry`'s `PlanAdapter`s (`openspec`,
    /// s17's reference dialect). No `--watch` (design D2: plans are
    /// operator-pulled, not streamed). See `canon_cli::plans`'s module
    /// doc.
    Plans {
        /// One-shot override: import exactly this dialect's `--source`
        /// root, bypassing `canon.yaml`'s `plans:` section entirely.
        /// REQUIRES `--source` (either flag alone fails loud).
        #[arg(long)]
        dialect: Option<String>,
        /// One-shot override's source root (paired with `--dialect`).
        #[arg(long)]
        source: Option<PathBuf>,
        /// Repo root -- resolved via
        /// `canon_cli::context::resolve_repo_root` (design D7), same
        /// as `canon context`/`canon gate`/`canon ingest artifacts`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the machine-readable JSON outcome instead of the
        /// default human summary.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum GateCommand {
    /// Assemble + run the coverage/ledger/staleness/trust-ladder
    /// `GateCheck` set (task 1.9, `canon_gate::check_set`) over the
    /// resolved repo's evidence corpus, printing violations by failure
    /// class. Exit `0` clean / `1` gate-red / `2` usage-or-load failure.
    Check {
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (design D7), same as `canon context`/`canon fmt`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Additionally engage the release-scoped `ReleaseTrustCheck`
        /// (`trust-below-required`, D7/spec.md "does not block ordinary
        /// (non-release) evaluation") — the always-on trust-ladder
        /// check is never dropped either way (`canon_gate::dispatch`).
        #[arg(long)]
        release: bool,
    },
    /// Evidence-gated checkbox flip for one openspec `task_id`
    /// (`<change_id>#<n>`, S1 join spine) — fails closed (row stays
    /// unflipped) on missing, malformed, or fabricated evidence.
    Task {
        task_id: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Promote `_staging/` evidence records to the committed ledger,
    /// assigning a monotonic per-(role, surface) `run_seq` (O13).
    Promote {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Print the plan (target, assigned `run_seq`) without writing
        /// or deleting anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Idempotent, diff-only hook-seam installation into
    /// `.claude/settings.json`/`.codex/hooks.json` (design D8), plus a
    /// generic pre-commit script for a repo with no existing `canon
    /// gate`-invoking hook entries.
    InstallHooks {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// The hook event name (Claude-Code/Codex convention, e.g.
        /// `PreToolUse`, `Stop`).
        #[arg(long, default_value = "PreToolUse")]
        event: String,
        /// Omitted for a matcher-less event (round-trips with no
        /// `matcher` key at all, never a JSON `null`).
        #[arg(long)]
        matcher: Option<String>,
        /// Defaults to `canon gate task` — the evidence-gated task-flip
        /// entry point (gated-task-completion spec.md "Hook-seam wiring
        /// generation"), not the read-only `canon gate check`.
        #[arg(long, default_value = "canon gate task")]
        command: String,
        #[arg(long, default_value_t = 30)]
        timeout: u32,
    },
    /// Run the shipped fixture corpus (task 5.2): every
    /// `FAILURE_CLASSES` string proven to fire on its own fixture,
    /// exact-set-match against `expected_failures.txt`. Takes no
    /// `--repo` — self-contained, never touches a real repo.
    Selftest,
}

#[derive(Subcommand)]
enum ReviewCommand {
    /// `canon review add` (s15 P3b, native-verdict-lifecycle spec):
    /// writes ONE native, attributed `Review` record. Exactly one of
    /// `--upstream-ref`/`--original-spec-ref` is REQUIRED — neither given
    /// (or both given) refuses the write and exits non-zero, never
    /// synthesizing a ref.
    Add {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        pin: String,
        /// Provenance ref option 1 of 2 — mutually exclusive with
        /// `--original-spec-ref`; exactly one is required.
        #[arg(long)]
        upstream_ref: Option<String>,
        /// Provenance ref option 2 of 2 — mutually exclusive with
        /// `--upstream-ref`; exactly one is required.
        #[arg(long)]
        original_spec_ref: Option<String>,
        /// The invoking actor's id, attributed onto the written
        /// `Review`'s envelope.
        #[arg(long, default_value = "canon")]
        actor_id: String,
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum DivergenceCliCommand {
    /// Write ONE unordered staging candidate carrying no `run_seq` —
    /// `--status` accepts `open`/`still-divergent`/`resolved`/
    /// `deferred` (`deferred` additionally requires `--reason`/
    /// `--expiry`). `canon divergence promote` assigns the monotonic
    /// `run_seq` later.
    Stage {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        sha: Sha,
        #[arg(long, default_value = "open")]
        status: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, value_parser = canon_cli::divergence::parse_timestamp)]
        expiry: Option<DateTime<Utc>>,
        #[arg(long, default_value_t = 1)]
        round: u32,
        #[arg(long)]
        reviewer: String,
        #[arg(long, default_value = "")]
        detail: String,
        #[arg(long, default_value = "canon")]
        actor_id: String,
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Batch-promote every currently-staged `Divergence` candidate,
    /// assigning each a monotonic `run_seq` within its
    /// `(project_id, role, surface)` partition.
    Promote {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Direct-commit a `Resolved` candidate — never touches the batch
    /// staging directory (`canon_cli::divergence`'s module doc).
    Resolve {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        sha: Sha,
        #[arg(long, default_value_t = 1)]
        round: u32,
        #[arg(long)]
        reviewer: String,
        #[arg(long, default_value = "")]
        detail: String,
        #[arg(long, default_value = "canon")]
        actor_id: String,
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Direct-commit a `Deferred { reason, expiry }` candidate — never
    /// touches the batch staging directory.
    Defer {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        sha: Sha,
        #[arg(long, default_value_t = 1)]
        round: u32,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        reason: String,
        #[arg(long, value_parser = canon_cli::divergence::parse_timestamp)]
        expiry: DateTime<Utc>,
        #[arg(long, default_value = "canon")]
        actor_id: String,
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// The S9 divergence burn-down's CURRENT-STATE view (task 4.5):
    /// `canon_report::divergence::current_states`/`summarize` — see
    /// `canon_cli::divergence::run_status`'s own doc.
    Status {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Governs `Deferred` expiry; defaults to now.
        #[arg(long, value_parser = canon_cli::divergence::parse_timestamp)]
        as_of: Option<DateTime<Utc>>,
    },
}

#[derive(Subcommand)]
enum InventoryCommand {
    /// See `Command::Inventory`'s doc.
    Sync {
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (design D7), same as `canon context`/`canon fmt`/`canon gate`.
        /// Its `canon.yaml` supplies BOTH the `specs.roots[]` config (D3)
        /// and the ledger `tiers.git.root` records are written through
        /// (`GateCtx::from_repo`).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Override `canon.yaml`'s `specs.roots[]` config entirely and
        /// sync exactly ONE ad hoc root at this directory, under the
        /// same stable literal id the absent-`specs:` default uses.
        /// Absent: resolve every configured root from `canon.yaml`.
        #[arg(long)]
        spec_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PluginCommand {
    /// See `Command::Plugin`'s doc.
    Sync {
        /// A `canon/plugins/<id>/plugin.yaml` manifest `id` (s16 P4) —
        /// e.g. `porting`. Matched against each registered
        /// `canon_cli::plugin_sync::OverlaySource::plugin_id()` by
        /// string equality (`canon_cli::plugin_sync`'s module doc).
        plugin_id: String,
        /// Repo root — resolved the SAME way `canon inventory sync`
        /// resolves it (`canon_cli::plugin_sync::run_sync`'s own doc:
        /// reuses `canon_cli::inventory::SyncCtx` verbatim), so a
        /// `canon plugin sync porting` run writes into the SAME git
        /// tier `canon query --plugin porting` reads.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Override `canon.yaml`'s `specs.roots[]` config entirely and
        /// sync exactly ONE ad hoc root at this directory — identical
        /// override semantics to `canon inventory sync --spec-root`.
        #[arg(long)]
        spec_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ScenarioCommand {
    /// See `Command::Scenario`'s doc.
    New {
        /// `<area>.<surface>.<nn>` — the scenario tag this stub
        /// carries (`canon_model::ids::ScenarioId`'s own grammar,
        /// reused verbatim via `canon_cli::scaffold::parse_scenario_tag`
        /// — a malformed tag is a clap usage error, exit `2`, never
        /// reaching the command body).
        #[arg(value_parser = canon_cli::scaffold::parse_scenario_tag)]
        tag: ScenarioId,
        /// The `Scenario:` header label.
        #[arg(long)]
        title: String,
        /// The `.feature` file to append to (created fresh, with its
        /// own `Feature:` header, if it doesn't exist yet) — relative
        /// to `--repo` unless absolute. s19 `derived-validated-
        /// scenario-feature`: OPTIONAL — omitted, the target is
        /// derived from `<tag>`'s own `area`/`surface` via the SAME
        /// join `canon feature new` uses (design D1/D2); given, it
        /// MUST resolve under a configured `specs.roots[]` entry or
        /// the command refuses (design D3).
        #[arg(long)]
        feature: Option<PathBuf>,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (design D7), same as every other subcommand. Its
        /// `canon.yaml` `specs.roots[]` (design D3) is the "target
        /// feature corpus" the duplicate-tag rejection scans
        /// (`canon_cli::scaffold::run_scenario_new`'s own doc).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum FeatureCommand {
    /// See `Command::Feature`'s doc.
    New {
        /// `<area>.<surface>` — the not-yet-started surface this
        /// fresh `.feature` file scaffolds.
        #[arg(value_parser = canon_cli::scaffold::parse_area_surface)]
        surface: AreaSurface,
        /// The `Feature:` header label.
        #[arg(long)]
        title: String,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (design D7). The file path is derived from the SAME single
        /// configured `specs.roots[]` entry `canon inventory sync`
        /// would resolve — this command refuses a multi-root config;
        /// no `--spec-root` override exists here to disambiguate
        /// (`canon_cli::scaffold::run_feature_new`'s own doc).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum SubjectCommand {
    /// `canon subject new <id> --domain <d> --title <t>` — author a
    /// fresh `Subject` at status `proposed`. See `canon_cli::subject`.
    New {
        /// The subject's kebab-slug id (`SubjectId` grammar).
        #[arg(value_parser = canon_cli::subject::parse_subject_id)]
        id: SubjectId,
        /// The subject's domain (kebab slug; base vocabulary
        /// `planning`/`design`/`dev`/`data`/`test` lives in
        /// `canon/vocab`). Shape-validated at write.
        #[arg(long)]
        domain: String,
        /// The `Subject.title`.
        #[arg(long)]
        title: String,
        /// The `Subject.summary` (optional).
        #[arg(long, default_value = "")]
        summary: String,
        /// The accountable owning role (`Subject.owner_role`) and the
        /// authoring envelope's role. Defaults to `implementer`.
        #[arg(long, value_parser = canon_cli::retrieve::parse_role, default_value = "implementer")]
        owner_role: RoleId,
        /// The invoking actor's id, stamped onto the envelope
        /// (mirrors `canon review add`'s `--actor-id`).
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the written record as JSON.
        #[arg(long)]
        json: bool,
    },
    /// `canon subject adopt <change_id> --subject <id>` — link an
    /// imported plan change to a subject (stamps `Change.subject_id`,
    /// appends to `Subject.change_ids`). See `canon_cli::subject`.
    Adopt {
        /// The imported `Change`'s id.
        #[arg(value_parser = canon_cli::subject::parse_change_id)]
        change_id: ChangeId,
        /// The `Subject` to adopt the change under.
        #[arg(long, value_parser = canon_cli::subject::parse_subject_id)]
        subject: SubjectId,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the updated subject record as JSON.
        #[arg(long)]
        json: bool,
    },
    /// `canon subject status <id> <state>` — apply a policy-gated
    /// lifecycle transition; `verifying → shipped` is evidence-gated.
    /// See `canon_cli::subject`.
    Status {
        /// The `Subject` to transition.
        #[arg(value_parser = canon_cli::subject::parse_subject_id)]
        id: SubjectId,
        /// The target lifecycle state (`proposed`/`specced`/`building`/
        /// `verifying`/`shipped`/`retired`).
        #[arg(value_parser = canon_cli::subject::parse_status)]
        state: canon_model::SubjectStatus,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Emit the updated subject record as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SkillsCommand {
    /// Materialize `canon/skills/<name>/SKILL.md` into `.claude/skills/`
    /// and `.codex/skills/`, updating the content-hash + version lock.
    Install {
        /// Directory holding `<name>/SKILL.md` sources (the canon
        /// checkout's `canon/skills/` by convention).
        #[arg(long, default_value = "canon/skills")]
        source: PathBuf,
        /// Consumer repo root to materialize `.claude/` and `.codex/` into.
        #[arg(long, default_value = ".")]
        target: PathBuf,
    },
}

#[derive(Subcommand)]
enum TierCommand {
    /// Apply every `canon.yaml` `aging:` rule once, moving records past
    /// their threshold to their configured destination tier (S2 task
    /// 3.3, tier-policy spec).
    Age {
        /// Preview what would move (a read-only threshold scan) without
        /// writing or deleting anything.
        #[arg(long)]
        dry_run: bool,
        /// Repo root — resolved via `canon_cli::context::resolve_repo_root`
        /// (s26 D2), the SAME `repo`/`canon_yaml` precedence pair `canon
        /// query` already ships. Ignored whenever `--canon-yaml` is also
        /// supplied (`--canon-yaml` wins — see that flag's own doc).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// An explicit, literal `canon.yaml` path override that BYPASSES
        /// `--repo`'s ancestor walk entirely — read AS-IS from exactly this
        /// path, regardless of cwd. Takes precedence over `--repo` when
        /// supplied. Omitted: `--repo`'s ancestor-walk resolution
        /// (`resolve_repo_root(repo).join("canon.yaml")`) governs instead.
        #[arg(long)]
        canon_yaml: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        None => ExitCode::SUCCESS,
        Some(Command::Skills { action }) => match action {
            SkillsCommand::Install { source, target } => run_skills_install(&source, &target),
        },
        Some(Command::Tier { action }) => match action {
            TierCommand::Age { dry_run, repo, canon_yaml } => run_tier_age(&repo, canon_yaml.as_deref(), dry_run),
        },
        Some(Command::Query { kind, since, repo, canon_yaml, json, plugin, change_id, status, domain }) => {
            run_query(&repo, canon_yaml.as_deref(), kind, since, json, plugin, change_id, status, domain)
        }
        Some(Command::Fmt { check, root, repo }) => run_fmt(&root, repo.as_deref(), check),
        Some(Command::Context { repo, json }) => run_context(&repo, json),
        Some(Command::Ingest { action }) => match action {
            IngestCommand::Sessions { watch, interval_secs, home, canon_yaml, full, all_workspaces } => run_ingest_sessions(&canon_yaml, home.as_deref(), watch, interval_secs, full, all_workspaces),
            IngestCommand::Artifacts { watch, interval_secs, repo, json } => run_ingest_artifacts(&repo, watch, interval_secs, json),
            IngestCommand::Plans { dialect, source, repo, json } => run_ingest_plans(&repo, dialect.as_deref(), source.as_deref(), json),
        },
        Some(Command::Gate { action }) => match action {
            GateCommand::Check { repo, release } => ExitCode::from(canon_cli::gate::run_check(&repo, release) as u8),
            GateCommand::Task { task_id, repo } => ExitCode::from(canon_cli::gate::run_task(&repo, &task_id) as u8),
            GateCommand::Promote { repo, dry_run } => ExitCode::from(canon_cli::gate::run_promote(&repo, dry_run) as u8),
            GateCommand::InstallHooks { repo, event, matcher, command, timeout } => {
                ExitCode::from(canon_cli::gate::run_install_hooks(&repo, &event, matcher.as_deref(), &command, timeout) as u8)
            }
            GateCommand::Selftest => ExitCode::from(canon_cli::gate::run_selftest() as u8),
        },
        Some(Command::Review { action }) => match action {
            ReviewCommand::Add { project_id, scenario_id, reviewer, pin, upstream_ref, original_spec_ref, actor_id, role, repo } => ExitCode::from(
                canon_cli::review::run_add(&repo, &project_id, &scenario_id, &reviewer, &pin, upstream_ref.as_deref(), original_spec_ref.as_deref(), &actor_id, &role) as u8,
            ),
        },
        Some(Command::Divergence { action }) => match action {
            DivergenceCliCommand::Stage { project_id, scenario_id, sha, status, reason, expiry, round, reviewer, detail, actor_id, role, repo } => {
                match canon_cli::divergence::parse_status(&status, reason.as_deref(), expiry) {
                    Ok(status) => ExitCode::from(canon_cli::divergence::run_stage(&repo, &project_id, &scenario_id, &sha, status, round, &reviewer, &detail, &actor_id, &role) as u8),
                    Err(e) => {
                        eprintln!("canon divergence stage: {e}");
                        ExitCode::from(2)
                    }
                }
            }
            DivergenceCliCommand::Promote { repo, dry_run } => ExitCode::from(canon_cli::divergence::run_promote(&repo, dry_run) as u8),
            DivergenceCliCommand::Resolve { project_id, scenario_id, sha, round, reviewer, detail, actor_id, role, repo } => {
                ExitCode::from(canon_cli::divergence::run_resolve(&repo, &project_id, &scenario_id, &sha, round, &reviewer, &detail, &actor_id, &role) as u8)
            }
            DivergenceCliCommand::Defer { project_id, scenario_id, sha, round, reviewer, reason, expiry, actor_id, role, repo } => {
                ExitCode::from(canon_cli::divergence::run_defer(&repo, &project_id, &scenario_id, &sha, round, &reviewer, &reason, expiry, &actor_id, &role) as u8)
            }
            DivergenceCliCommand::Status { repo, as_of } => ExitCode::from(canon_cli::divergence::run_status(&repo, as_of) as u8),
        },
        Some(Command::Inventory { action }) => match action {
            InventoryCommand::Sync { repo, spec_root } => run_inventory_sync(&repo, spec_root.as_deref()),
        },
        Some(Command::Plugin { action }) => match action {
            PluginCommand::Sync { plugin_id, repo, spec_root } => run_plugin_sync(&repo, &plugin_id, spec_root.as_deref()),
        },
        Some(Command::Scenario { action }) => match action {
            ScenarioCommand::New { tag, title, feature, repo } => run_scenario_new(&repo, &tag, &title, feature.as_deref()),
        },
        Some(Command::Feature { action }) => match action {
            FeatureCommand::New { surface, title, repo } => run_feature_new(&repo, &surface, &title),
        },
        Some(Command::Subject { action }) => match action {
            SubjectCommand::New { id, domain, title, summary, owner_role, actor_id, repo, json } => {
                ExitCode::from(canon_cli::subject::run_new(&repo, &id, &domain, &title, &summary, &owner_role, &actor_id, json) as u8)
            }
            SubjectCommand::Adopt { change_id, subject, repo, json } => ExitCode::from(canon_cli::subject::run_adopt(&repo, &change_id, &subject, json) as u8),
            SubjectCommand::Status { id, state, repo, json } => ExitCode::from(canon_cli::subject::run_status(&repo, &id, state, json) as u8),
        },
        Some(Command::Init { repo, check_config }) => run_init(&repo, check_config),
        Some(Command::Demo { action }) => match action {
            DemoCommand::Init { repo } => ExitCode::from(canon_cli::demo::run_demo_init(&repo) as u8),
            DemoCommand::Attest { repo } => ExitCode::from(canon_cli::demo::run_demo_attest(&repo) as u8),
        },
        Some(Command::Retrieve { role, regime, domain, subject, k, repo, json }) => run_retrieve(&repo, &role, regime.as_ref(), domain.as_deref(), subject.as_ref(), k, json),
        Some(Command::Report { repo, check, snapshot }) => run_report(&repo, check, snapshot.as_deref()),
        Some(Command::Dashboard { repo, snapshot, port }) => run_dashboard(&repo, snapshot.as_deref(), port),
        Some(Command::RegimeKey { role, repo, area, hash }) => run_regime_key(&role, &repo, &area, &hash),
        Some(Command::Learn { action }) => match action {
            LearnCommand::Promote { strategy_id, repo, dry_run } => canon_cli::learn::run_promote(&repo, &strategy_id, dry_run),
        },
        Some(Command::Dispatch { action }) => match action {
            DispatchCommand::Begin { role, regime, agent_id, repo, json } => canon_cli::dispatch::run_begin(&repo, &role, &regime, &agent_id, json),
        },
        Some(Command::Selftest { json }) => canon_cli::selftest::run_selftest(json),
    }
}

fn run_skills_install(source: &std::path::Path, target: &std::path::Path) -> ExitCode {
    match canon_cli::skills::install(source, target) {
        Ok(report) => {
            for skill in &report.installed {
                let status = if skill.changed { "installed" } else { "unchanged" };
                println!("{} v{} — {}", skill.name, skill.version, status);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon skills install: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_tier_age(repo: &std::path::Path, canon_yaml: Option<&std::path::Path>, dry_run: bool) -> ExitCode {
    let canon_yaml_path = canon_cli::context::resolve_canon_yaml(repo, canon_yaml);
    match canon_cli::tier::run(&canon_yaml_path, dry_run) {
        Ok(reports) => {
            print!("{}", canon_cli::tier::format_report(&reports, dry_run));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon tier age: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `plugin: None` calls the EXACT SAME [`canon_cli::query::run`]/
/// [`canon_cli::query::format_human`]/[`canon_cli::query::format_json`]
/// this function called before s16 P3 -- byte-for-byte the same source
/// (task 3.4's no-`--plugin`-⇒-byte-identical hard test). `plugin:
/// Some(id)` calls [`canon_cli::query::run_with_plugin`] instead;
/// every diagnostic it returns is printed to stderr regardless of
/// whether a projection actually resolved, and stdout falls back to
/// the SAME [`format_human`]/[`format_json`] calls whenever
/// `projections` came back empty (an unresolved plugin, or a
/// `--kind`/overlay `core_kind` mismatch) -- so a degraded `--plugin`
/// run's stdout is ALSO byte-identical to the no-`--plugin` path,
/// exactly as `plugin-overlay-projection`'s own fail-soft scenarios
/// require.
#[allow(clippy::too_many_arguments)]
fn run_query(
    repo: &std::path::Path,
    canon_yaml: Option<&std::path::Path>,
    kind: RecordKind,
    since: Option<DateTime<Utc>>,
    json: bool,
    plugin: Option<String>,
    change_id: Option<ChangeId>,
    status: Option<String>,
    domain: Option<String>,
) -> ExitCode {
    // s19 `query-scope-filters` design D5: kind-gating + status-domain
    // validation runs BEFORE any tier read (task 3.2/3.3) -- a usage
    // fault here is a clean, nothing-read `2`, never a store error.
    if let Err(e) = canon_cli::query::validate_scope(kind, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        eprintln!("canon query: {e}");
        return ExitCode::from(2);
    }

    let Some(plugin_id) = plugin else {
        return match canon_cli::query::run(repo, canon_yaml, kind, since, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
            Ok(outcome) => {
                if json {
                    println!("{}", canon_cli::query::format_json(&outcome));
                } else {
                    print!("{}", canon_cli::query::format_human(&outcome));
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("canon query: {err}");
                ExitCode::FAILURE
            }
        };
    };

    match canon_cli::query::run_with_plugin(repo, canon_yaml, kind, since, &plugin_id, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        Ok((outcome, plugin_outcome)) => {
            for msg in &plugin_outcome.diagnostics {
                eprintln!("canon query --plugin {plugin_id}: {msg}");
            }
            if plugin_outcome.projections.is_empty() {
                if json {
                    println!("{}", canon_cli::query::format_json(&outcome));
                } else {
                    print!("{}", canon_cli::query::format_human(&outcome));
                }
            } else if json {
                println!("{}", canon_cli::query::format_json_with_overlay(&outcome, &plugin_id, &plugin_outcome.projections));
            } else {
                print!("{}", canon_cli::query::format_human_with_overlay(&outcome, &plugin_id, &plugin_outcome.projections));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon query: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `repo: None` (every existing invocation) leaves `root` untouched --
/// zero new function calls, byte-identical to pre-s26 (design D1). `repo:
/// Some(r)` resolves the corpus actually checked as
/// `resolve_repo_root(r).join(root)` -- `root` stays the corpus-relative
/// suffix, `--repo` supplies the base.
fn run_fmt(root: &std::path::Path, repo: Option<&std::path::Path>, check: bool) -> ExitCode {
    if !check {
        eprintln!("canon fmt: only `--check` is currently supported");
        return ExitCode::FAILURE;
    }
    let resolved_root = match repo {
        Some(r) => canon_cli::context::resolve_repo_root(r).join(root),
        None => root.to_path_buf(),
    };
    let report = canon_cli::fmt::run(&resolved_root);
    print!("{}", canon_cli::fmt::format_human(&report));
    if report.is_clean() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// `canon inventory sync [--spec-root <dir>]` (s15 P3a): mirrors `canon
/// fmt`'s own 0-clean/nonzero-on-violation convention — `1` when ANY
/// configured root aborted on an S11 validation violation
/// (`canon_cli::inventory::run_sync`'s module doc, "whole-root abort"),
/// `2` on a fail-loud config error (`specs:` present-but-malformed —
/// mirrors `canon review add`/`canon divergence stage`'s own refused-
/// invocation exit code), `0` otherwise (including the common
/// zero-writes no-op re-sync case).
fn run_inventory_sync(repo: &std::path::Path, spec_root: Option<&std::path::Path>) -> ExitCode {
    match canon_cli::inventory::run_sync(repo, spec_root) {
        Ok(outcome) => {
            print!("{}", canon_cli::inventory::format_human(&outcome));
            if outcome.is_clean() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(err) => {
            eprintln!("canon inventory sync: {err}");
            ExitCode::from(2)
        }
    }
}

/// `canon plugin sync <plugin-id> [--spec-root <dir>]` (s16 P4,
/// `canon_cli::plugin_sync::run_sync`'s own doc): `2` on a resolution
/// error (unresolved plugin id, no registered `OverlaySource`, or a
/// `specs:` config fault — mirrors `canon inventory sync`'s own
/// refused-invocation exit code), `1` when a write attempt itself
/// failed for some candidate (`outcome.is_clean()` false), `0`
/// otherwise (including the common zero-new-writes idempotent re-sync
/// case).
fn run_plugin_sync(repo: &std::path::Path, plugin_id: &str, spec_root: Option<&std::path::Path>) -> ExitCode {
    match canon_cli::plugin_sync::run_sync(repo, plugin_id, spec_root) {
        Ok(outcome) => {
            print!("{}", canon_cli::plugin_sync::format_human(&outcome));
            if outcome.is_clean() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(err) => {
            eprintln!("canon plugin sync: {err}");
            ExitCode::from(2)
        }
    }
}

/// `canon scenario new <tag> --title <label> [--feature <path>]` (s16
/// P5, `canon_cli::scaffold::run_scenario_new`'s own doc): the ONE
/// `Utc::now()` call for this command — computed here, at the
/// dispatch boundary, so a brand-new `.feature` file's `Feature:` +
/// first `Scenario:` provenance comments never straddle two different
/// timestamps (`canon_cli::scaffold`'s module doc, "deterministic
/// provenance"). `2` on a refused invocation (config fault, ambiguous
/// multi-root default derivation, an out-of-root explicit `--feature`,
/// or a duplicate tag), `0` on a successful append/create.
fn run_scenario_new(repo: &std::path::Path, tag: &ScenarioId, title: &str, feature: Option<&std::path::Path>) -> ExitCode {
    ExitCode::from(canon_cli::scaffold::run_scenario_new(repo, tag, title, feature, Utc::now()) as u8)
}

/// `canon feature new <area>.<surface> --title <label>` (s16 P5,
/// `canon_cli::scaffold::run_feature_new`'s own doc) — same
/// single-`Utc::now()`-call discipline as [`run_scenario_new`] above.
/// `2` on a refused invocation (config fault, an ambiguous multi-root
/// config, or an already-existing target file), `0` on a fresh file
/// written.
fn run_feature_new(repo: &std::path::Path, surface: &AreaSurface, title: &str) -> ExitCode {
    ExitCode::from(canon_cli::scaffold::run_feature_new(repo, surface, title, Utc::now()) as u8)
}

/// `canon init [--repo <dir>]` / `canon init --check-config` (s19 P4,
/// `canon_cli::init`'s module doc): `check_config: false` writes a
/// fresh skeleton (`2` on an existing `canon.yaml`, `0` written);
/// `check_config: true` READ-ONLY validates an existing one instead
/// (`2` on a missing file, `0` when every present section parses
/// clean, `1` when a present section fails).
fn run_init(repo: &std::path::Path, check_config: bool) -> ExitCode {
    let code = if check_config { canon_cli::init::run_check_config(repo) } else { canon_cli::init::run_init(repo) };
    ExitCode::from(code as u8)
}

/// A capability query, never validation (invariant 1): always
/// `ExitCode::SUCCESS`, mirroring `resolve_surface`'s own infallibility —
/// there is no corpus check here to fail against. `repo` is first resolved
/// via [`canon_cli::context::resolve_repo_root`] (design D7, task 1.4) —
/// the `--repo`-omitted/`--repo .` nearest-`canon.yaml` ancestor walk —
/// before [`canon_cli::context::resolve_surface`] ever reads it.
fn run_context(repo: &std::path::Path, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    let surface = canon_cli::context::resolve_surface(&repo, canon_cli::context::ContextOptions::default());
    if json {
        println!("{}", canon_cli::context::render_json(&surface));
    } else {
        print!("{}", canon_cli::context::render_outline(&surface));
    }
    ExitCode::SUCCESS
}

fn run_ingest_sessions(canon_yaml: &std::path::Path, home: Option<&std::path::Path>, watch: bool, interval_secs: u64, full: bool, all_workspaces: bool) -> ExitCode {
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h),
            None => {
                eprintln!("canon ingest sessions: no `--home` given and $HOME is unset");
                return ExitCode::FAILURE;
            }
        },
    };

    loop {
        match canon_cli::ingest::run(canon_yaml, &home, true, full, all_workspaces) {
            Ok(outcome) => {
                print!("{}", canon_cli::ingest::format_human(&outcome));
                // The documented JSON fallback (`canon_cli::ingest`'s
                // module doc: "the CLI prints it as JSON rather than
                // failing the whole ingest pass") must actually fire
                // whenever there's unwritten output — ReviewS3Full
                // finding 4: this used to be gated behind a `--json`
                // flag the human-readable summary line ABOVE already
                // unconditionally claims happened ("printing JSON
                // instead"), so a default (flagless) run whose tiers
                // were unreachable discarded the only normalized
                // output. `format_json` already returns `None` when
                // every record was persisted, so this is a no-op on
                // the common path.
                if let Some(body) = canon_cli::ingest::format_json(&outcome) {
                    println!("{body}");
                }
            }
            Err(err) => {
                eprintln!("canon ingest sessions: {err}");
                return ExitCode::FAILURE;
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest artifacts` (S14 `s14-artifact-ingest-cli`): see
/// `canon_cli::artifact_ingest`'s module doc — the artifact/verdict
/// half of canon's join spine, mirroring `run_ingest_sessions`'s
/// scan-loop shape one level up (`--repo`-resolved, never `--home`).
fn run_ingest_artifacts(repo: &std::path::Path, watch: bool, interval_secs: u64, json: bool) -> ExitCode {
    loop {
        match canon_cli::artifact_ingest::run(repo) {
            Ok(outcome) => {
                if json {
                    println!("{}", canon_cli::artifact_ingest::format_json(&outcome));
                } else {
                    print!("{}", canon_cli::artifact_ingest::format_human(&outcome));
                }
            }
            Err(err) => {
                eprintln!("canon ingest artifacts: {err}");
                return ExitCode::FAILURE;
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest plans [--dialect <id> --source <path>] [--repo <dir>]
/// [--json]` (s17 P3, extended s18 P2/B1): see `canon_cli::plans`'s
/// module doc. Prints the human summary (or `--json`'s full structured
/// outcome), then -- mirroring `run_ingest_sessions`'s own ReviewS3Full
/// finding-4 fix -- ALWAYS also prints the documented `unwritten`
/// seam's JSON body when non-empty, regardless of `--json`, so a
/// routed-but-unreachable tier's candidates are never the one copy of
/// output silently discarded by a flagless default run.
///
/// s18 `loud-plan-import-diagnostics` spec's "A malformed-nonzero,
/// zero-persisted source makes canon ingest plans non-clean at the
/// process level": whenever `PlansOutcome::non_clean_sources` is
/// non-empty, an unconditional stderr WARN (regardless of `--json`) is
/// printed per flagged source, naming its dialect, root, and malformed
/// count, and the process exits non-zero -- never the unconditional
/// `ExitCode::SUCCESS` this condition produced before this change. A
/// pass with zero flagged sources keeps exiting `0` exactly as before.
/// Distinct from the `Err(err)` arm below (a malformed CONFIGURATION,
/// s17's own `PlansError` paths), which keeps failing loud with its own
/// exit code before any source is even scanned.
fn run_ingest_plans(repo: &std::path::Path, dialect: Option<&str>, source: Option<&std::path::Path>, json: bool) -> ExitCode {
    match canon_cli::plans::run(repo, dialect, source) {
        Ok(outcome) => {
            if json {
                println!("{}", canon_cli::plans::format_json(&outcome));
            } else {
                print!("{}", canon_cli::plans::format_human(&outcome));
                if let Some(body) = canon_cli::plans::format_unwritten_json(&outcome) {
                    println!("{body}");
                }
            }
            for flagged in &outcome.non_clean_sources {
                eprintln!(
                    "canon ingest plans: WARN {} ({}): {} malformed construct(s), 0 persisted -- this source's pass produced nothing usable; see the named malformed entries above (or in --json) for path + reason + hint, a `root:` misconfiguration is a likely cause",
                    flagged.dialect, flagged.root, flagged.malformed
                );
            }
            if outcome.non_clean_sources.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(err) => {
            eprintln!("canon ingest plans: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `canon retrieve` (S8 part2, design.md decision 3): once
/// `canon_cli::retrieve::run` clears the `--role`/`--regime` usage
/// precondition, this ALWAYS exits `0` — a store outage or malformed
/// row degrades to empty guidance internally (`retrieve_guidance`'s own
/// fail-soft contract), never surfaced as a nonzero exit here. The
/// ONLY nonzero exit is the usage precondition itself (`--role`
/// disagreeing with `--regime`'s own leading segment), reported and
/// exiting `2` (mirrors `canon gate check`'s own 0-clean/1-red/2-usage
/// convention) — never reachable via `retrieve_guidance`, which has no
/// error channel at all (`canon_cli::retrieve`'s own module doc).
fn run_retrieve(repo: &std::path::Path, role: &RoleId, regime: Option<&RegimeKey>, domain: Option<&str>, subject: Option<&SubjectId>, k: Option<usize>, json: bool) -> ExitCode {
    match canon_cli::retrieve::run_scoped(repo, role, regime, domain, subject, k) {
        Ok(o) => {
            if json {
                println!("{}", canon_cli::retrieve::format_json(&o.guidance));
                if let Some(n) = canon_cli::retrieve::serving_note(&o) {
                    eprintln!("{n}");
                }
            } else {
                print!("{}", canon_cli::retrieve::format_human_scoped(role, &o));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("canon retrieve: {e}");
            ExitCode::from(2)
        }
    }
}

/// `canon regime-key` (S8 `s8-retrieve-before-task` whole-branch-review
/// fix): serialize + VALIDATE one canonical `regime_key` so shell hooks
/// route through the identical `canon_model::ids::regime_key`
/// normalizer the Rust write path uses, never a second derivation
/// (design decision 1). Prints the validated key and exits `0`; on a
/// malformed result (empty segment / bad `<hash>`, which `regime_key`
/// can still produce — see its doc) reports to stderr and exits `2`
/// with nothing on stdout, so the hook's own `|| exit 0` degrades it to
/// a silent no-op rather than passing a malformed `--regime` on.
fn run_regime_key(role: &str, repo: &str, area: &str, hash: &str) -> ExitCode {
    match RegimeKey::parse(regime_key(role, repo, area, hash)) {
        Ok(valid) => {
            println!("{}", valid.as_str());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon regime-key: {err}");
            ExitCode::from(2)
        }
    }
}

/// `canon report` (S9 part2, tasks.md 3.1): resolves `--repo` +
/// `canon-report`'s `Roots` (`canon_cli::report::resolve_inputs`) then
/// dispatches to exactly one of `canon-report`'s three library entry
/// points — never reimplements any of them (module doc of
/// `canon_cli::report`). `--snapshot <dir>` takes priority when given
/// (module doc of the `Report` clap variant); otherwise `--check`
/// surfaces `canon_report::CheckOutcome::exit_code()` UNCHANGED (`0`
/// no-drift / `1` `MISSING`/`DRIFT`, design D2); the flagless default
/// writes the report. Before any of those three modes, prints a
/// one-line stderr `canon report: WARN …` naming any record kind
/// routed to a backend that is not read directly by the report (s25
/// `report-pg-tier-boundary` design D3/D4, s27 `tier-role-backend-
/// split` design D2, s28 `rung-backend-capability` design D2/D3) —
/// computed via the SAME
/// `canon_report::tier_boundary::kinds_not_read_directly`
/// derivation the written report's own `## Kinds not read directly`
/// section reads, so the two can never disagree; silent for a repo
/// with nothing routed to a backend that is not read directly.
fn run_report(repo: &std::path::Path, check: bool, snapshot_dir: Option<&std::path::Path>) -> ExitCode {
    let (repo, inputs) = canon_cli::report::resolve_inputs(repo);

    let kinds_not_read_directly = canon_report::tier_boundary::kinds_not_read_directly(&repo);
    if let Some(msg) = canon_report::tier_boundary::warn_line(&kinds_not_read_directly) {
        eprintln!("canon report: WARN {msg}");
    }

    if let Some(dir) = snapshot_dir {
        return match canon_report::snapshot(&inputs, dir) {
            Ok(manifest) => {
                println!("canon report --snapshot: wrote {} table(s) to {}", manifest.tables.len(), dir.display());
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("canon report --snapshot: {err}");
                ExitCode::FAILURE
            }
        };
    }

    let report_path = canon_cli::report::default_report_path(&repo);

    if check {
        return match canon_report::check_report(&inputs, &report_path) {
            Ok(outcome) => {
                eprintln!("{}", outcome.message(&report_path));
                ExitCode::from(outcome.exit_code() as u8)
            }
            Err(err) => {
                eprintln!("canon report --check: {err}");
                ExitCode::FAILURE
            }
        };
    }

    match canon_report::write_report(&inputs, &report_path) {
        Ok(_content) => {
            println!("canon report: wrote {}", report_path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon report: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `canon dashboard` (S9 part3, tasks.md 6.1): resolves + (re)generates
/// the snapshot and binds the static server (`canon_cli::dashboard::prepare`,
/// module doc for the default-vs-explicit `--snapshot` rule), then serves
/// forever — this subcommand never returns on success; the process exits
/// only via the standard SIGINT/SIGTERM default handler (no signal
/// handling installed, matching every other local dev-server tool).
fn run_dashboard(repo: &std::path::Path, snapshot: Option<&std::path::Path>, port: u16) -> ExitCode {
    match canon_cli::dashboard::prepare(repo, snapshot, port) {
        Ok(bound) => {
            println!("canon dashboard: app       = {}", bound.dist_dir.display());
            println!("canon dashboard: snapshot  = {}", bound.snapshot_dir.display());
            println!("canon dashboard: serving {}", bound.url());
            bound.serve_forever();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon dashboard: {err}");
            ExitCode::FAILURE
        }
    }
}
