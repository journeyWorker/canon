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
//! <label>` — see `canon_cli::scaffold`'s module doc. s42
//! (`close-the-open-loops`) adds `canon evidence add` — the missing
//! AUTHORING half of the gate loop `canon gate promote`/`canon gate
//! task` already implemented (see `canon_cli::evidence`'s module doc) —
//! and `canon ingest artifacts --run <RUN_ID>`, the explicit trajectory
//! attribution edge. Every other subcommand is a later spec's
//! responsibility.

use std::path::PathBuf;
use std::process::ExitCode;

use canon_model::envelope::RecordKind;
use canon_model::{regime_key, Actor, ChangeId, EvidenceVerdict, ProjectId, RegimeKey, RoleId, RunId, RunStatus, ScenarioId, Sha, SubjectId, TaskId};
use canon_learn::StrategyId;
use canon_cli::scaffold::AreaSurface;
use chrono::{DateTime, Timelike, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "canon",
    version,
    about = "Specs, evidence gates, and agent memory for your repo",
    long_about = "canon keeps a repo's specs, review evidence, and agent strategy \
memory in one place: author .feature specs, gate task completion on real evidence, \
ingest agent sessions, and retrieve role-scoped guidance.",
    arg_required_else_help = true,
    propagate_version = true,
    after_help = "\
Examples:
  canon init                Set up canon in the current repo
  canon demo init           Scaffold a throwaway demo repo to try the evidence loop
  canon format spec         Validate a spec corpus
  canon gate check          Run the evidence gate

Learn more:
  Use `canon <command> --help` for details on any command.
  `canon skills install` projects one `canon` skill per selected provider,
  with lazy reference and script sidecars; `skills check` and `skills doctor`
  inspect the target without deleting user files."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    // ── Getting started ──
    /// Set up canon in a repo (writes a starter canon.yaml)
    #[command(after_help = "Examples:\n  canon init\n  canon init --check-config")]
    Init {
        /// Directory to set up (used as-is)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Validate an existing canon.yaml instead of writing one
        #[arg(long)]
        check_config: bool,
    },
    /// Try the evidence loop end-to-end in a throwaway demo repo
    #[command(after_help = "Examples:\n  canon demo init --repo /tmp/canon-demo && cd /tmp/canon-demo\n  canon gate check   # RED\n  canon demo attest && canon gate check   # GREEN")]
    Demo {
        #[command(subcommand)]
        action: DemoCommand,
    },
    /// Show what you can author here: record kinds, fields, enums, policies
    Context {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Create, inspect, and verify reproducible context packs
    #[command(after_help = "Examples:\n  canon context-pack create --manifest .canon/context-manifest.json\n  canon context-pack show sha256:<pack-digest> --repo . --json\n  canon context-pack verify sha256:<pack-digest>")]
    ContextPack {
        #[command(subcommand)]
        action: ContextPackCommand,
    },
    /// Validate a provider-neutral adapter response without executing a provider
    #[command(after_help = "Examples:\n  canon adapter validate --response response.json\n  canon adapter validate --response response.json --repo . --json")]
    Adapter {
        #[command(subcommand)]
        action: AdapterCommand,
    },
    /// Register and select versioned prompt bundles
    #[command(
        disable_version_flag = true,
        after_help = "Examples:\n  canon prompt register --name reviewer --version v1 --manifest .canon/reviewer.json\n  canon prompt show --name reviewer --version v1 --json"
    )]
    Prompt {
        #[command(subcommand)]
        action: PromptCommand,
    },

    // ── Specs & authoring ──
    /// Validate a spec/artifact corpus against canon's format
    #[command(name = "format", visible_alias = "fmt", after_help = "Examples:\n  canon format spec\n  canon format spec --repo ~/work/myrepo")]
    Format {
        /// Validate and report violations (the default; kept for compatibility)
        #[arg(long)]
        check: bool,
        /// Corpus root, e.g. a consumer repo's spec/ directory
        root: PathBuf,
        /// Repo root the corpus is resolved under (default: root used as-is)
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Scaffold a new .feature spec file
    Feature {
        #[command(subcommand)]
        action: FeatureCommand,
    },
    /// Add a tagged scenario to a .feature spec file
    Scenario {
        #[command(subcommand)]
        action: ScenarioCommand,
    },
    /// Create and manage subjects (durable product units)
    Subject {
        #[command(subcommand)]
        action: SubjectCommand,
    },
    /// Index a validated .feature corpus into the scenario ledger
    Inventory {
        #[command(subcommand)]
        action: InventoryCommand,
    },

    // ── Evidence loop ──
    /// Run evidence gates, flip task checkboxes, install hooks
    #[command(after_help = "Examples:\n  canon gate check\n  canon gate task my-change#3\n  canon gate install-hooks")]
    Gate {
        #[command(subcommand)]
        action: GateCommand,
    },
    /// Author a staged, attributed evidence attestation for a plan task
    #[command(after_help = "Examples:\n  canon evidence add --task my-change#3.1 --kind test-run --ref 'cargo test -p canon-cli' --role implementer\n\nThe loop:\n  canon evidence add ...    Stage an EvidenceRecord\n  canon gate promote        Commit it (assigns run_seq)\n  canon gate task <id>      Flip the checkbox on it\n\nSee `canon evidence add --help` for exactly what the gate does and does not check.")]
    Evidence {
        #[command(subcommand)]
        action: EvidenceCommand,
    },
    /// Record one code-review finding as a durable record
    #[command(after_help = "Examples:\n  canon finding add --change-id s43-findings-are-records --round 1 --seq 1 \\\n      --severity blocker --disposition open --reviewer review-voice \\\n      --summary 'a release metric is asserted, never computed'\n  canon finding close --change-id s43-findings-are-records --round 1 --seq 1 \\\n      --disposition fixed --resolution-sha $(git rev-parse HEAD)\n\nThe loop:\n  canon finding add ...     Stage a Finding\n  canon gate promote        Commit it\n  canon finding close ...   Stage its disposition TRANSITION, once fixed\n  canon gate promote        Commit that too\n\nThe committed finding STAYS when it is closed: the ledger is append-only and\nthe pair IS the history. Every reader folds by natural key to the latest\nversion, so the finding is still counted once.\n\nRECORDED OBSERVATION, NOT PROOF — canon never verifies that the defect\nexisted, that it was fixed, or that --introduced-by is the true cause.\nSee `canon finding add --help` for exactly what it does and does not establish.")]
    Finding {
        #[command(subcommand)]
        action: FindingCommand,
    },
    /// Record an attributed review verdict
    Review {
        #[command(subcommand)]
        action: ReviewCommand,
    },
    /// Stage, promote, resolve, and inspect spec divergences
    Divergence {
        #[command(subcommand)]
        action: DivergenceCliCommand,
    },

    // ── Ingest & memory ──
    /// Import agent sessions, artifacts, and plans into canon's store
    #[command(after_help = "Examples:\n  canon ingest sessions --watch\n  canon ingest artifacts\n  canon ingest plans")]
    Ingest {
        #[command(subcommand)]
        action: IngestCommand,
    },
    /// Fetch role-scoped strategy guidance
    Retrieve {
        /// Role scope; with --regime must equal its leading segment
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Full regime key (<role>/<repo>/<area>/<hash>); mutually exclusive with --domain/--subject
        #[arg(long, value_parser = canon_cli::retrieve::parse_regime)]
        regime: Option<RegimeKey>,
        /// Derive the regime from a domain slug; mutually exclusive with --regime
        #[arg(long)]
        domain: Option<String>,
        /// Narrow --domain to one subject_id; requires --domain, mutually exclusive with --regime
        #[arg(long, value_parser = canon_cli::retrieve::parse_subject)]
        subject: Option<SubjectId>,
        /// Top-k cap (default: canon's DEFAULT_K)
        #[arg(short, long)]
        k: Option<usize>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Promote a proven strategy into the git-tracked tier
    Learn {
        #[command(subcommand)]
        action: LearnCommand,
    },
    /// Begin an agent run with retrieved guidance recorded
    Dispatch {
        #[command(subcommand)]
        action: DispatchCommand,
    },
    /// Build a canonical regime key for hook scripts
    RegimeKey {
        /// The <role> segment (canonicalized)
        #[arg(long)]
        role: String,
        /// The <repo> segment (canonicalized)
        #[arg(long)]
        repo: String,
        /// The <area> segment (canonicalized)
        #[arg(long)]
        area: String,
        /// The <hash> segment (6-64-char lowercase hex; passed through, never re-hashed)
        #[arg(long)]
        hash: String,
    },

    // ── Reading & reporting ──
    /// Read stored records across every storage tier
    Query {
        /// Record kind to read (e.g. handoff, strategy_item)
        #[arg(long, value_parser = canon_cli::query::parse_kind)]
        kind: RecordKind,
        /// Only records with at >= <since> (RFC3339/ISO-8601)
        #[arg(long, value_parser = canon_cli::query::parse_since)]
        since: Option<DateTime<Utc>>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Explicit canon.yaml path (overrides --repo resolution)
        #[arg(long)]
        canon_yaml: Option<PathBuf>,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
        /// Project a plugin's overlay fields onto each record (fail-soft)
        #[arg(long)]
        plugin: Option<String>,
        /// Scope --kind change/task to one ChangeId (other kinds exit 2)
        #[arg(long, value_parser = canon_cli::query::parse_change_id)]
        change_id: Option<ChangeId>,
        /// Scope --kind change/task by status field (other kinds exit 2)
        #[arg(long)]
        status: Option<String>,
        /// Scope --kind subject by domain (other kinds exit 2)
        #[arg(long)]
        domain: Option<String>,
        /// Permit sensitive query output only when policy explicitly allows it
        #[arg(long)]
        include_sensitive: bool,
    },
    /// Export a redacted retention manifest without deleting records
    Export {
        #[arg(long, value_parser = canon_cli::query::parse_kind)]
        kind: RecordKind,
        #[arg(long, value_parser = canon_cli::query::parse_since)]
        before: Option<DateTime<Utc>>,
        #[arg(long, value_parser = canon_cli::query::parse_since)]
        after: Option<DateTime<Utc>>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        include_sensitive: bool,
    },
    /// Permanently remove allowlisted records older than a cutoff
    Purge {
        #[arg(long, value_parser = canon_cli::query::parse_kind)]
        kind: RecordKind,
        #[arg(long, value_parser = canon_cli::query::parse_since)]
        before: DateTime<Utc>,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },

    /// Generate the status report (write, --check, or --snapshot)
    Report {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Byte-diff against .canon/REPORT.md (0 clean, 1 drift); --snapshot wins if both given
        #[arg(long)]
        check: bool,
        /// Export panel marts to <dir>/*.parquet + manifest.json instead
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// Serve the local status dashboard
    Dashboard {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Snapshot dir to serve (default: regenerated fresh each run)
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Local port to bind (0 = OS-assigned free port)
        #[arg(long, default_value_t = 4173)]
        port: u16,
    },

    // ── Maintenance ──
    /// Sync plugin overlay records onto the ledger
    Plugin {
        #[command(subcommand)]
        action: PluginCommand,
    },
    /// Install canon's agent guides into a repo
    Skills {
        #[command(subcommand)]
        action: SkillsCommand,
    },
    /// Age records between storage tiers
    Tier {
        #[command(subcommand)]
        action: TierCommand,
    },
    /// Run canon's built-in fixture self-tests
    Selftest {
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ContextPackCommand {
    /// Create an immutable context pack from a repository-local JSON manifest
    Create {
        /// Repository-relative input manifest
        #[arg(long)]
        manifest: PathBuf,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output the complete pack manifest as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show a context pack after verifying its manifest and content objects
    Show {
        /// Context pack id (sha256:<64 hex digits>)
        id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output the complete pack manifest as JSON
        #[arg(long)]
        json: bool,
    },
    /// Verify a context pack's manifest and immutable content objects
    Verify {
        /// Context pack id (sha256:<64 hex digits>)
        id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output a JSON verification result
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AdapterCommand {
    /// Read and validate one provider-neutral response envelope
    Validate {
        /// JSON response file to validate
        #[arg(long)]
        response: PathBuf,
        /// Optional repo root for context-pack join verification
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Output the normalized core summary as JSON
        #[arg(long)]
        json: bool,
    },
    /// Authorize declared capabilities against repository policy (preflight only)
    Authorize {
        /// JSON response file to validate and authorize
        #[arg(long)]
        response: PathBuf,
        /// Repository root containing `.canon/policy.yaml`
        #[arg(long)]
        repo: PathBuf,
        /// Output authorization result as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PromptCommand {
    /// Register a prompt bundle from a repository-local JSON manifest
    Register {
        /// Bundle name
        #[arg(long)]
        name: String,
        /// Bundle version
        #[arg(long)]
        version: String,
        /// Repository-relative input manifest
        #[arg(long)]
        manifest: PathBuf,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output the complete bundle manifest as JSON
        #[arg(long)]
        json: bool,
    },
    /// Select a registered prompt bundle version
    Show {
        /// Bundle name
        #[arg(long)]
        name: String,
        /// Bundle version
        #[arg(long)]
        version: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output the complete bundle manifest as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LearnCommand {
    /// Promote a strategy after paired evaluation and signed approval
    Promote {
        /// The StrategyId (ULID) to promote
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Evaluation bundle produced by the honest promotion adapter
        #[arg(long)]
        evaluation: Option<PathBuf>,
        /// Approval JSON containing the detached SSH signature
        #[arg(long)]
        approval: Option<PathBuf>,
        /// Optional detached signature file to attach to approval JSON
        #[arg(long)]
        signature: Option<PathBuf>,
        /// Preview without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Produce an unsigned, externally signable approval payload
    Approve {
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        #[arg(long)]
        evaluation: PathBuf,
        #[arg(long)]
        principal: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Alias for approve: produce an unsigned approval request payload
    Request {
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        #[arg(long)]
        evaluation: PathBuf,
        #[arg(long)]
        principal: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Disable an active strategy with durable rollback provenance
    Rollback {
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        signature_file: PathBuf,
        #[arg(long)]
        approved_at: String,
        #[arg(long)]
        contradicting_trajectory_id: Option<String>,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum DemoCommand {
    /// Scaffold the demo repo (gate starts RED)
    Init {
        /// Directory to set up (used as-is)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Record the missing reviewer evidence (gate turns GREEN)
    Attest {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum DispatchCommand {
    /// Mint a Run manifest with retrieved guidance and context/policy lineage
    Begin {
        /// Role about to run; must equal --regime's leading segment
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Full regime key (<role>/<repo>/<area>/<hash>) to retrieve guidance for
        #[arg(long, value_parser = canon_cli::retrieve::parse_regime)]
        regime: RegimeKey,
        /// The dispatching agent's id (recorded as the run's actor)
        #[arg(long, default_value = "canon")]
        agent_id: String,
        /// Declared provider metadata (copied verbatim; never inferred from --agent-id)
        #[arg(long)]
        provider: Option<String>,
        /// Declared model metadata (copied verbatim)
        #[arg(long)]
        model: Option<String>,
        /// Skill that contributed to this run
        #[arg(long)]
        skill_id: Option<String>,
        /// Declared skill digest; requires --skill-id
        #[arg(long)]
        skill_digest: Option<String>,
        /// Explicit repository-relative ContextPack input manifest
        #[arg(long)]
        context_manifest: Option<PathBuf>,
        /// Registered prompt bundle to select (`name@version`)
        #[arg(long, value_parser = canon_cli::dispatch::parse_prompt_bundle)]
        prompt_bundle: Option<canon_cli::context_pack::PromptBundleSelection>,
        /// Plan task this run serves (<change_id>#<n>); validated against the plan corpus
        #[arg(long, value_parser = canon_cli::dispatch::parse_task_id)]
        task: Option<TaskId>,
        /// RunId (ULID) of the run that dispatched this one
        #[arg(long, value_parser = canon_cli::dispatch::parse_run_id)]
        parent_run: Option<RunId>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Close a dispatched run with a terminal status and an ended_at
    End {
        /// The RunId (ULID) minted by `canon dispatch begin`
        #[arg(long, value_parser = canon_cli::dispatch::parse_run_id)]
        run: RunId,
        /// Terminal outcome: succeeded or failed
        #[arg(long, value_parser = canon_cli::dispatch::parse_run_status)]
        status: RunStatus,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Diff the declared plan DAG against the observed execution graph
    Diff {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum IngestCommand {
    /// Import agent CLI session transcripts (omp/pi, Claude Code, Codex, Hermes)
    Sessions {
        /// Keep polling instead of exiting after one pass
        #[arg(long)]
        watch: bool,
        /// Seconds between polls
        #[arg(long, default_value_t = 30)]
        interval_secs: u64,
        /// The scan root's home directory (defaults to $HOME)
        #[arg(long)]
        home: Option<PathBuf>,
        /// This repo's canon.yaml (tier-policy source)
        #[arg(long, default_value = "canon.yaml")]
        canon_yaml: PathBuf,
        /// Ignore watermark cursors and re-parse every in-scope file
        #[arg(long)]
        full: bool,
        /// Scan every workspace on this machine, not just this project
        #[arg(long)]
        all_workspaces: bool,
    },
    /// Import review/divergence/task/handoff artifacts and derive verdicts
    Artifacts {
        /// Keep polling instead of exiting after one pass
        #[arg(long)]
        watch: bool,
        /// Seconds between polls
        #[arg(long, default_value_t = 30)]
        interval_secs: u64,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
        /// Attribute this pass's trajectories to a dispatched run (a RunId minted by `canon dispatch begin`)
        #[arg(long, value_name = "RUN_ID", value_parser = canon_cli::dispatch::parse_run_id)]
        run: Option<RunId>,
    },
    /// Import a plan corpus (openspec, superpowers) as Change/Task records
    Plans {
        /// One-shot override: import this dialect's --source root (requires --source)
        #[arg(long)]
        dialect: Option<String>,
        /// One-shot override's source root (paired with --dialect)
        #[arg(long)]
        source: Option<PathBuf>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum GateCommand {
    /// Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
    Check {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Additionally engage the release-scoped trust check
        #[arg(long)]
        release: bool,
    },
    /// Flip one task checkbox, gated on real evidence (fails closed)
    Task {
        /// The openspec task id (<change_id>#<n>)
        task_id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Promote staged evidence records to the committed ledger
    Promote {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Preview without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Install the gate hook into .claude/settings.json / .codex/hooks.json
    InstallHooks {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// The hook event name (e.g. PreToolUse, Stop)
        #[arg(long, default_value = "PreToolUse")]
        event: String,
        /// Omitted for a matcher-less event
        #[arg(long)]
        matcher: Option<String>,
        /// The command the hook runs
        #[arg(long, default_value = "canon gate task")]
        command: String,
        /// Hook timeout in seconds
        #[arg(long, default_value_t = 30)]
        timeout: u32,
    },
    /// Run the gate's self-contained fixture self-test
    Selftest,
}

#[derive(Subcommand)]
enum EvidenceCommand {
    /// Emit exact bytes for external `ssh-keygen -Y sign -n canon-approval-v1`
    ApprovalPayload {
        #[arg(long, value_parser = canon_cli::dispatch::parse_task_id)]
        task: Option<TaskId>,
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: Option<ProjectId>,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: Option<ScenarioId>,
        #[arg(long, value_parser = canon_cli::dispatch::parse_run_id)]
        run_id: Option<RunId>,
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        artifact_sha: canon_model::Sha,
        #[arg(long = "surface-ref", value_parser = canon_cli::evidence::parse_surface_ref)]
        surface_ref: Vec<String>,
        #[arg(long)]
        approval_by: String,
        #[arg(long)]
        approval_at: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Stage one attributed EvidenceRecord for a task (commit it with `canon gate promote`)
    #[command(after_help = "ATTESTATION, NOT PROOF.\nThe evidence command never runs, resolves, or checks --ref. The attestor can authorize its own checkbox; this is an attestation, not independent approval.\nThe gate skips the staleness, trust-ladder, release-trust, and divergence dimensions.\nDetached approvals require an external signature; canon never signs on the user's behalf.\n\nEXPERIMENTAL BINDING (--artifact, --report).\nCanon reads files your runner or agent already wrote; it never runs a test. --artifact binds any file by sha256; --report junit:<path> or cucumber:<path> also records the matched case and its outcome (a faithful verdict needs a passing case). Enforcement is opt-in via policy experimental.evidence_binding (off by default).")]
    Add {
        /// Plan task this evidence attests to (<change_id>#<n>); required unless --scenario-id is given
        #[arg(long, value_parser = canon_cli::dispatch::parse_task_id)]
        task: Option<TaskId>,
        /// Spec corpus id for --scenario-id (canon keys a scenario by the composite (project_id, scenario_id))
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: Option<ProjectId>,
        /// What CLASS of evidence is attested to (test-run, review, ...) — the `evidence.kind` companion
        #[arg(long)]
        kind: String,
        /// The reference a reader can resolve: a command line, commit sha, or report path. NEVER run or resolved by canon
        #[arg(long = "ref", value_name = "REF")]
        evidence_ref: String,
        /// faithful / not-applicable / divergent
        #[arg(long, default_value = "faithful", value_parser = canon_cli::evidence::parse_verdict)]
        verdict: EvidenceVerdict,
        /// One-line note; becomes the flipped row's ✅ suffix (scanned for fabrication markers; line breaks refused)
        #[arg(long)]
        summary: Option<String>,
        /// The real captured output backing --summary (requires --summary; scanned for fabrication markers)
        #[arg(long)]
        command_result: Option<String>,
        /// Scenario this evidence also joins to
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: Option<ScenarioId>,
        /// RunId (ULID) of the run that produced this evidence
        #[arg(long, value_parser = canon_cli::dispatch::parse_run_id)]
        run_id: Option<RunId>,
        /// Explicit risk binding: repository-relative path or effect:<kebab-slug>; persisted as surface_ref
        #[arg(long = "surface-ref", value_parser = canon_cli::evidence::parse_surface_ref)]
        surface_ref: Vec<String>,
        /// The attesting actor's id (recorded as the attestation's author; line breaks refused)
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Required: `canon gate promote` derives its run_seq partition key from it
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Human approval identity; authentication comes only from the detached signature.
        #[arg(long)]
        approval_by: Option<String>,
        /// Approval role; only human is eligible for risk approval.
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        approval_role: Option<RoleId>,
        /// Armored SSH detached signature file. Canon never signs on the user's behalf.
        #[arg(long)]
        approval_signature_file: Option<PathBuf>,
        /// RFC3339 timestamp included in the signed approval payload.
        #[arg(long)]
        approval_at: Option<String>,
        /// Exact Git SHA bound by the evidence and any approval.
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        artifact_sha: Option<canon_model::Sha>,
        /// EXPERIMENTAL: bind any file (trace, screenshots, agent QA log, report) by sha256; repeatable; must be inside the repo
        #[arg(long = "artifact", value_name = "PATH")]
        artifacts: Vec<PathBuf>,
        /// EXPERIMENTAL: bind and parse a test report, `junit:<path>` or `cucumber:<path>`; repeatable
        #[arg(long = "report", value_name = "FORMAT:PATH", value_parser = canon_cli::evidence_attach::parse_report_spec)]
        reports: Vec<(canon_model::ReportFormat, PathBuf)>,
        /// EXPERIMENTAL: the report case to bind when its name does not carry the scenario id (a Rust test: its function name)
        #[arg(long)]
        report_case: Option<String>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum FindingCommand {
    /// Stage one code-review finding (commit it with `canon gate promote`)
    #[command(after_help = "RECORDED OBSERVATION, NOT PROOF. canon does not verify that the defect\nexisted, that it was real, that it was fixed, or that --introduced-by names\nthe commit that actually caused it. This records that a named reviewer, at a\nstamped time, RAISED this finding against this change.\n\nAn agent that can run this command can record its own review round, with its\nown severities: the author of a finding and its beneficiary are the same\nparty, with no signature and no second party — the same gap `canon evidence\nadd` states about attestations.\n\nWhat canon does NOT check:\n  - --summary, against the diff, the code, or the review transcript. The\n    record is identical whether the defect was real, misdiagnosed, or invented\n  - --resolution-sha: never resolved, never read. `--disposition fixed`\n    records that someone SAID this commit closed it, not that it did\n  - --introduced-by: never verified, and never guessed (below)\n  - --reviewed-sha: optional, because most rounds review an uncommitted\n    working tree. When absent, canon does not infer one\n  - nothing here is gated. No `canon gate check` check consumes findings, and\n    no checkbox flips on one\n\nWhat the record IS good for:\n  - attribution: an append-only row naming who raised what, in which round,\n    against which change, at which severity and disposition. Before this\n    command, a review round left nothing at all once its transcript was gone\n  - a DERIVED count that stops being typed from memory. v0.4.0's release note\n    hand-typed its issue counts into a published tag and got them wrong; that\n    is why this command exists. No count is quoted here and none is asserted\n    in its place -- read them off the 'Review rounds' panel in\n    .canon/REPORT.md or the dashboard, which derive them from these records\n\n--introduced-by must be SOURCED:\n  Set it only when the introducing commit was actually established — someone\n  read the diff, or the fixing commit names it. Never from timing, commit\n  adjacency, or `git blame`. Leaving it UNSET is the correct record when it is\n  not known, not an incomplete one. Unset means UNSOURCED, never 'no cause'; a\n  guessed value would inflate the exact number this command exists to make\n  trustworthy.\n\nWhat a derived fix-of-fix count MEANS, stated here word for word as every\nother surface states it (canon_report::render::FIX_OF_FIX_MEANING) rather\nthan paraphrased, because paraphrase is how the previous wording went wrong\non seven surfaces at once:\n\n  `fix_of_fix` bounds NOTHING — not from below, not from above: it\n  UNDER-counts, because an unsourced finding is never counted and a fix in\n  one change that breaks something first found while reviewing a DIFFERENT\n  change is not counted at all; it OVER-counts, because a `resolution_sha`\n  commit may carry work BEYOND the fix and every finding recording that\n  commit is counted regardless; and for any individual match the data cannot\n  say whether the fix or the other work in that commit introduced the\n  defect.\n\nFix-of-fix is DERIVED, never recorded: a finding is one when its\nintroduced_by equals an EARLIER finding's resolution_sha (lower round/seq in\nthe same change). There is no flag for it and there will not be one — a flag\nwould relocate the hand-typing from the release notes into the record.\n\n--seq is REQUIRED, never auto-assigned: two concurrent adds would read the\nsame highest seq and both pick the next one, landing two records under one\nfinding's identity and inflating the count. An already-occupied\n(--change-id, --round, --seq) is refused, naming it.\n\nRefused before anything is staged:\n  - --disposition fixed with no --resolution-sha, and --resolution-sha with\n    any other disposition\n  - a line separator in --summary, --reviewer, --file-ref or --actor-id: a\n    finding is rendered as ONE row, so a separator appends a second row that\n    no record backs\n\nThe loop:\n  canon finding add ...     Stage a Finding\n  canon gate promote        Commit it")]
    Add {
        /// The change under review (e.g. s43-findings-are-records) — first component of the natural key
        #[arg(long, value_parser = canon_cli::finding::parse_change_id)]
        change_id: ChangeId,
        /// Which review round on this change raised it (1-based)
        #[arg(long)]
        round: u32,
        /// This finding's index within the round (1-based). Required, never auto-assigned; a collision is refused
        #[arg(long)]
        seq: u32,
        /// blocker / should-fix / note — the reviewer's own judgement, never a score canon derives
        #[arg(long, value_parser = canon_cli::finding::parse_severity)]
        severity: canon_model::FindingSeverity,
        /// open / fixed / rejected / deferred (fixed requires --resolution-sha, and only fixed may carry one)
        #[arg(long, default_value = "open", value_parser = canon_cli::finding::parse_disposition)]
        disposition: canon_model::FindingDisposition,
        /// Who raised it (line breaks refused)
        #[arg(long)]
        reviewer: String,
        /// One line of what the finding IS, in the reviewer's own words. NEVER checked against the code (line breaks refused)
        #[arg(long)]
        summary: String,
        /// The commit whose state this round reviewed. Omit when the round reviewed an uncommitted working tree — canon never infers one. Must name a COMMIT object this repo holds (existence only; never its message or diff)
        #[arg(long, value_parser = canon_cli::finding::parse_sha)]
        reviewed_sha: Option<Sha>,
        /// The commit that closed it. Required by, and permitted only with, --disposition fixed. Must name a COMMIT object this repo holds (existence only; never its message or diff)
        #[arg(long, value_parser = canon_cli::finding::parse_sha)]
        resolution_sha: Option<Sha>,
        /// The SOURCED introducing commit. Leave unset when it is not known — never guess (see --help). Must name a COMMIT object this repo holds (existence only; never its message or diff)
        #[arg(long, value_parser = canon_cli::finding::parse_sha)]
        introduced_by: Option<Sha>,
        /// Where in the tree, as path/to/file.rs:120-134 (line breaks refused)
        #[arg(long)]
        file_ref: Option<String>,
        /// The authoring actor's id (recorded as the record's author; line breaks refused)
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Attribution only — unlike `canon evidence add`, no partition key derives from it
        #[arg(long, default_value = "reviewer", value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Move an already-committed finding's disposition (commit it with `canon gate promote`)
    #[command(after_help = "A finding is raised `open` and later closed. This is the ONLY way to record\nthat second state: `canon finding add` refuses an occupied natural key,\nbecause two DIFFERENT findings under one (change, round, seq) are two\nreviewers' work collapsed into one identity.\n\nWhat it changes: the disposition, and `resolution_sha` with it. NOTHING else.\nSeverity, reviewer, summary, --reviewed-sha, --introduced-by and --file-ref\nare READ from the committed record and copied byte-identically — this command\ncannot edit them, and no command can. A finding recorded wrongly stays\nwrongly recorded; that is the append-only ledger working, not a gap.\n\nThe committed record STAYS. This appends a second version at the same\nnatural key, and the pair IS the history: `open` at one instant, `fixed` at\nanother, each with its own author and timestamp. Every reader folds by\nnatural key to the latest version, so the finding is still counted once.\n\n`canon gate promote` re-derives the transition rule itself before committing\nthe second version, so a hand-written body must clear the same bar.\n\nExamples:\n  canon finding close --change-id s44-spec-derived-worklist --round 1 --seq 1 \\\n      --disposition fixed --resolution-sha $(git rev-parse HEAD)\n  canon finding close --change-id s44-spec-derived-worklist --round 1 --seq 9 \\\n      --disposition rejected")]
    Close {
        /// The change under review — first component of the natural key
        #[arg(long, value_parser = canon_cli::finding::parse_change_id)]
        change_id: ChangeId,
        /// Which review round raised it (1-based)
        #[arg(long)]
        round: u32,
        /// The finding's index within the round (1-based)
        #[arg(long)]
        seq: u32,
        /// Where it lands: fixed / rejected / deferred / open (reopening is a real transition). fixed requires --resolution-sha, and only fixed may carry one
        #[arg(long, value_parser = canon_cli::finding::parse_disposition)]
        disposition: canon_model::FindingDisposition,
        /// The commit that closed it. Required by, and permitted only with, --disposition fixed. Must name a COMMIT object this repo holds
        #[arg(long, value_parser = canon_cli::finding::parse_sha)]
        resolution_sha: Option<Sha>,
        /// The actor recorded as authoring the TRANSITION — not the original finding's author, which is preserved on the record this supersedes
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Attribution only — no partition key derives from it
        #[arg(long, default_value = "reviewer", value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReviewCommand {
    /// Write one attributed Review record (exactly one provenance ref required)
    Add {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        pin: String,
        /// Provenance ref (mutually exclusive with --original-spec-ref; exactly one required)
        #[arg(long)]
        upstream_ref: Option<String>,
        /// Provenance ref (mutually exclusive with --upstream-ref; exactly one required)
        #[arg(long)]
        original_spec_ref: Option<String>,
        /// The invoking actor's id
        #[arg(long, default_value = "canon")]
        actor_id: String,
        #[arg(long, value_parser = canon_cli::retrieve::parse_role)]
        role: RoleId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum DivergenceCliCommand {
    /// Stage a divergence candidate (no run_seq yet)
    Stage {
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project_id: ProjectId,
        #[arg(long, value_parser = canon_cli::review::parse_scenario_id)]
        scenario_id: ScenarioId,
        #[arg(long, value_parser = canon_cli::divergence::parse_sha)]
        sha: Sha,
        /// open / still-divergent / resolved / deferred (deferred needs --reason/--expiry)
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
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Promote all staged candidates, assigning run_seq
    Promote {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Preview without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Directly record a resolved divergence
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
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Directly record a deferred divergence (requires --reason/--expiry)
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
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Show the current divergence burn-down state
    Status {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Governs Deferred expiry; defaults to now
        #[arg(long, value_parser = canon_cli::divergence::parse_timestamp)]
        as_of: Option<DateTime<Utc>>,
    },
}

#[derive(Subcommand)]
enum InventoryCommand {
    /// Validate each spec root, then materialize scenario index records
    Sync {
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Sync exactly one ad hoc root, overriding canon.yaml's specs.roots[]
        #[arg(long)]
        spec_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PluginCommand {
    /// Validate and write a plugin's overlay records
    Sync {
        /// A .canon/plugins/<id>/plugin.yaml manifest id (e.g. porting)
        plugin_id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Sync exactly one ad hoc root, overriding canon.yaml's specs.roots[]
        #[arg(long)]
        spec_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ScenarioCommand {
    /// Append a tagged scenario stub to its .feature file (created if missing)
    New {
        /// <area>.<surface>.<nn> scenario tag
        #[arg(value_parser = canon_cli::scaffold::parse_scenario_tag)]
        tag: ScenarioId,
        /// The Scenario: header label
        #[arg(long)]
        title: String,
        /// Optional subject id tag to pin above the scenario
        #[arg(long, value_parser = canon_cli::subject::parse_subject_id)]
        subject: Option<SubjectId>,
        /// Optional lane tag to classify the scenario
        #[arg(long, value_parser = canon_cli::scaffold::parse_lane_slug)]
        lane: Option<String>,
        /// Optional case tag: which path of the behavior this scenario
        /// specifies (`happy`, `failure`, `edge` in the base vocabulary)
        #[arg(long, value_parser = canon_cli::scaffold::parse_lane_slug)]
        case: Option<String>,
        /// Agent id written into provenance (env: CANON_ACTOR)
        #[arg(long, env = "CANON_ACTOR", default_value = "canon-scaffold")]
        actor: String,
        /// Target .feature file (default: derived from <tag>; must live under a specs.roots[] entry)
        #[arg(long)]
        feature: Option<PathBuf>,
        /// Which configured `specs.roots[]` entry to write under, by its id. Required only when the repo configures more than one
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project: Option<ProjectId>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum FeatureCommand {
    /// Create a fresh .feature file for a new area.surface
    New {
        /// <area>.<surface> the fresh .feature file scaffolds
        #[arg(value_parser = canon_cli::scaffold::parse_area_surface)]
        surface: AreaSurface,
        /// The Feature: header label
        #[arg(long)]
        title: String,
        /// Agent id written into provenance (env: CANON_ACTOR)
        #[arg(long, env = "CANON_ACTOR", default_value = "canon-scaffold")]
        actor: String,
        /// Which configured `specs.roots[]` entry to write under, by its id. Required only when the repo configures more than one
        #[arg(long, value_parser = canon_cli::review::parse_project_id)]
        project: Option<ProjectId>,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum SubjectCommand {
    /// Author a new subject at status `proposed`
    New {
        /// The subject's kebab-slug id
        #[arg(value_parser = canon_cli::subject::parse_subject_id)]
        id: SubjectId,
        /// The subject's domain (kebab slug)
        #[arg(long)]
        domain: String,
        /// The subject's title
        #[arg(long)]
        title: String,
        /// The subject's summary (optional)
        #[arg(long, default_value = "")]
        summary: String,
        /// The accountable owning role (default: implementer)
        #[arg(long, value_parser = canon_cli::retrieve::parse_role, default_value = "implementer")]
        owner_role: RoleId,
        /// The invoking actor's id
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Link an imported plan change to a subject
    Adopt {
        /// The imported Change's id
        #[arg(value_parser = canon_cli::subject::parse_change_id)]
        change_id: ChangeId,
        /// The subject to adopt the change under
        #[arg(long, value_parser = canon_cli::subject::parse_subject_id)]
        subject: SubjectId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
    /// Transition a subject's lifecycle status (shipping is evidence-gated; with spec_coverage.require_review, entering its scope is review-gated)
    Status {
        /// The subject to transition
        #[arg(value_parser = canon_cli::subject::parse_subject_id)]
        id: SubjectId,
        /// Target state (proposed/specced/building/verifying/shipped/retired)
        #[arg(value_parser = canon_cli::subject::parse_status)]
        state: canon_model::SubjectStatus,
        /// Move anyway when only the require_review checks (unreviewed-promotion, open-blocker) refuse; the reason is recorded on the subject and `canon gate check` lists the gaps as advisories
        #[arg(long)]
        override_reason: Option<String>,
        /// The invoking actor's id, recorded with an --override-reason waiver
        #[arg(long, default_value = "canon")]
        actor_id: String,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Output JSON instead of the human-readable form
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SkillsCommand {
    /// Install the canonical `canon` bundle or legacy developer skills.
    Install {
        /// Source directory; defaults to CANON_SKILLS_SOURCE or canon/skills.
        #[arg(long)]
        source: Option<PathBuf>,
        /// Consumer repo root to materialize into.
        #[arg(long, default_value = ".")]
        target: PathBuf,
        /// Providers to project (`claude`, `codex`, `omp`, `pi`, or a comma-separated list).
        /// Omitted: detected from `.claude`, `.agents` or `.codex` (codex), `.omp`, `.pi`;
        /// none present selects claude,codex. Codex is projected to `.agents/skills/canon/`.
        #[arg(long)]
        providers: Option<String>,
    },
    /// Check canonical projections without writing.
    Check {
        #[arg(long)]
        source: Option<PathBuf>,
        /// Consumer repo root to inspect.
        #[arg(long, default_value = ".")]
        target: PathBuf,
        /// Providers to inspect (`claude`, `codex`, `omp`, `pi`, or a comma-separated list).
        #[arg(long)]
        providers: Option<String>,
    },
    /// Print structured diagnostics for canonical projections and legacy remnants.
    Doctor {
        #[arg(long)]
        source: Option<PathBuf>,
        /// Consumer repo root to inspect.
        #[arg(long, default_value = ".")]
        target: PathBuf,
        /// Providers to inspect (`claude`, `codex`, `omp`, `pi`, or a comma-separated list).
        #[arg(long)]
        providers: Option<String>,
    },
}

#[derive(Subcommand)]
enum TierCommand {
    /// Apply canon.yaml aging rules, moving old records to their destination tier
    Age {
        /// Preview without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Explicit canon.yaml path (overrides --repo resolution)
        #[arg(long)]
        canon_yaml: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Skills { action } => match action {
            SkillsCommand::Install { source, target, providers } => run_skills_install(source.as_deref(), &target, providers.as_deref()),
            SkillsCommand::Check { source, target, providers } => run_skills_check(source.as_deref(), &target, providers.as_deref()),
            SkillsCommand::Doctor { source, target, providers } => run_skills_doctor(source.as_deref(), &target, providers.as_deref()),
        },
        Command::Tier { action } => match action {
            TierCommand::Age { dry_run, repo, canon_yaml } => run_tier_age(&repo, canon_yaml.as_deref(), dry_run),
        },
        Command::Query { kind, since, repo, canon_yaml, json, plugin, change_id, status, domain, include_sensitive } => {
            run_query(&repo, canon_yaml.as_deref(), kind, since, json, plugin, change_id, status, domain, include_sensitive)
        }
        Command::Export { kind, before, after, out, repo, json, include_sensitive } => ExitCode::from(canon_cli::retention::run_export(&repo, kind, before, after, out.as_deref(), json, include_sensitive)),
        Command::Purge { kind, before, repo, dry_run, json } => ExitCode::from(canon_cli::retention::run_purge(&repo, kind, before, dry_run, json)),
        Command::Format { check: _, root, repo } => run_fmt(&root, repo.as_deref()),
        Command::Context { repo, json } => run_context(&repo, json),
        Command::ContextPack { action } => match action {
            ContextPackCommand::Create { manifest, repo, json } => run_context_pack_create(&repo, &manifest, json),
            ContextPackCommand::Show { id, repo, json } => run_context_pack_show(&repo, &id, json),
            ContextPackCommand::Verify { id, repo, json } => run_context_pack_verify(&repo, &id, json),
        },
        Command::Adapter { action } => match action {
            AdapterCommand::Validate { response, repo, json } => run_adapter_validate(&response, repo.as_deref(), json),
            AdapterCommand::Authorize { response, repo, json } => run_adapter_authorize(&response, &repo, json),
        },
        Command::Prompt { action } => match action {
            PromptCommand::Register { name, version, manifest, repo, json } => {
                run_prompt_register(&repo, &name, &version, &manifest, json)
            }
            PromptCommand::Show { name, version, repo, json } => run_prompt_show(&repo, &name, &version, json),
        },
        Command::Ingest { action } => match action {
            IngestCommand::Sessions { watch, interval_secs, home, canon_yaml, full, all_workspaces } => run_ingest_sessions(&canon_yaml, home.as_deref(), watch, interval_secs, full, all_workspaces),
            IngestCommand::Artifacts { watch, interval_secs, repo, json, run } => run_ingest_artifacts(&repo, watch, interval_secs, json, run.as_ref()),
            IngestCommand::Plans { dialect, source, repo, json } => run_ingest_plans(&repo, dialect.as_deref(), source.as_deref(), json),
        },
        Command::Gate { action } => match action {
            GateCommand::Check { repo, release } => ExitCode::from(canon_cli::gate::run_check(&repo, release) as u8),
            GateCommand::Task { task_id, repo } => ExitCode::from(canon_cli::gate::run_task(&repo, &task_id) as u8),
            GateCommand::Promote { repo, dry_run } => ExitCode::from(canon_cli::gate::run_promote(&repo, dry_run) as u8),
            GateCommand::InstallHooks { repo, event, matcher, command, timeout } => {
                ExitCode::from(canon_cli::gate::run_install_hooks(&repo, &event, matcher.as_deref(), &command, timeout) as u8)
            }
            GateCommand::Selftest => ExitCode::from(canon_cli::gate::run_selftest() as u8),
        },
        Command::Evidence { action } => match action {
            EvidenceCommand::ApprovalPayload {
                task, project_id, scenario_id, run_id, artifact_sha, surface_ref,
                approval_by, approval_at, repo,
            } => ExitCode::from(canon_cli::evidence::run_approval_payload(
                &repo,
                &canon_cli::evidence::ApprovalPayloadArgs {
                    task_id: task, scenario_id, project_id, run_id, artifact_sha,
                    surface_ref, approval_by, approval_at,
                },
            ) as u8),
            EvidenceCommand::Add {
                task,
                project_id,
                kind,
                evidence_ref,
                verdict,
                summary,
                command_result,
                scenario_id,
                run_id,
                surface_ref,
                actor_id,
                role,
                approval_by,
                approval_role,
                approval_signature_file,
                approval_at,
                artifact_sha,
                artifacts,
                reports,
                report_case,
                repo,
            } => ExitCode::from(
                canon_cli::evidence::run_add(
                    &repo,
                    &canon_cli::evidence::EvidenceArgs {
                        task_id: task,
                        project_id,
                        kind,
                        evidence_ref,
                        verdict,
                        summary,
                        command_result,
                        scenario_id,
                        run_id,
                        surface_ref,
                        actor_id,
                        role,
                        approval_by,
                        approval_role,
                        approval_signature_file,
                        approval_at,
                        artifact_sha,
                        artifacts,
                        reports,
                        report_case,
                    },
                ) as u8,
            ),
        },
        Command::Finding { action } => match action {
            FindingCommand::Add {
                change_id,
                round,
                seq,
                severity,
                disposition,
                reviewer,
                summary,
                reviewed_sha,
                resolution_sha,
                introduced_by,
                file_ref,
                actor_id,
                role,
                repo,
            } => ExitCode::from(canon_cli::finding::run_add(
                &repo,
                &canon_cli::finding::FindingArgs {
                    change_id,
                    reviewed_sha,
                    round,
                    seq,
                    severity,
                    disposition,
                    reviewer,
                    summary,
                    resolution_sha,
                    introduced_by,
                    file_ref,
                    actor_id,
                    role,
                },
            ) as u8),
            FindingCommand::Close { change_id, round, seq, disposition, resolution_sha, actor_id, role, repo } => {
                ExitCode::from(canon_cli::finding::run_close(
                    &repo,
                    &canon_cli::finding::FindingCloseArgs { change_id, round, seq, disposition, resolution_sha, actor_id, role },
                ) as u8)
            }
        },
        Command::Review { action } => match action {
            ReviewCommand::Add { project_id, scenario_id, reviewer, pin, upstream_ref, original_spec_ref, actor_id, role, repo } => ExitCode::from(
                canon_cli::review::run_add(&repo, &project_id, &scenario_id, &reviewer, &pin, upstream_ref.as_deref(), original_spec_ref.as_deref(), &actor_id, &role) as u8,
            ),
        },
        Command::Divergence { action } => match action {
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
        Command::Inventory { action } => match action {
            InventoryCommand::Sync { repo, spec_root } => run_inventory_sync(&repo, spec_root.as_deref()),
        },
        Command::Plugin { action } => match action {
            PluginCommand::Sync { plugin_id, repo, spec_root } => run_plugin_sync(&repo, &plugin_id, spec_root.as_deref()),
        },
        Command::Scenario { action } => match action {
            ScenarioCommand::New { tag, title, subject, lane, case, actor, feature, project, repo } => {
                let axes = canon_cli::scaffold::ScenarioAxes {
                    subject: subject.as_ref().map(SubjectId::as_str),
                    lane: lane.as_deref(),
                    case: case.as_deref(),
                };
                run_scenario_new(&repo, &tag, &title, feature.as_deref(), project.as_ref(), axes, &actor)
            }
        },
        Command::Feature { action } => match action {
            FeatureCommand::New { surface, title, actor, project, repo } => run_feature_new(&repo, &surface, &title, project.as_ref(), &actor),
        },
        Command::Subject { action } => match action {
            SubjectCommand::New { id, domain, title, summary, owner_role, actor_id, repo, json } => {
                ExitCode::from(canon_cli::subject::run_new(&repo, &id, &domain, &title, &summary, &owner_role, &actor_id, json) as u8)
            }
            SubjectCommand::Adopt { change_id, subject, repo, json } => ExitCode::from(canon_cli::subject::run_adopt(&repo, &change_id, &subject, json) as u8),
            SubjectCommand::Status { id, state, override_reason, actor_id, repo, json } => {
                ExitCode::from(canon_cli::subject::run_status(&repo, &id, state, override_reason.as_deref(), &actor_id, json) as u8)
            }
        },
        Command::Init { repo, check_config } => run_init(&repo, check_config),
        Command::Demo { action } => match action {
            DemoCommand::Init { repo } => ExitCode::from(canon_cli::demo::run_demo_init(&repo) as u8),
            DemoCommand::Attest { repo } => ExitCode::from(canon_cli::demo::run_demo_attest(&repo) as u8),
        },
        Command::Retrieve { role, regime, domain, subject, k, repo, json } => run_retrieve(&repo, &role, regime.as_ref(), domain.as_deref(), subject.as_ref(), k, json),
        Command::Report { repo, check, snapshot } => run_report(&repo, check, snapshot.as_deref()),
        Command::Dashboard { repo, snapshot, port } => run_dashboard(&repo, snapshot.as_deref(), port),
        Command::RegimeKey { role, repo, area, hash } => run_regime_key(&role, &repo, &area, &hash),
        Command::Learn { action } => match action {
            LearnCommand::Promote { strategy_id, repo, evaluation, approval, signature, dry_run } =>
                canon_cli::learn::run_promote_with_evidence(&repo, &strategy_id, dry_run, evaluation.as_deref(), approval.as_deref(), signature.as_deref()),
            LearnCommand::Approve { strategy_id, evaluation, principal, repo, json } =>
                canon_cli::learn::run_approve(&repo, &strategy_id, &evaluation, &principal, json),
            LearnCommand::Request { strategy_id, evaluation, principal, repo, json } =>
                canon_cli::learn::run_request(&repo, &strategy_id, &evaluation, &principal, json),
            LearnCommand::Rollback { strategy_id, reason, actor, signature_file, approved_at, contradicting_trajectory_id, repo } =>
                canon_cli::learn::run_rollback(&repo, &strategy_id, &reason, &actor, contradicting_trajectory_id.as_deref(), &signature_file, &approved_at),
        },
        Command::Dispatch { action } => match action {
            DispatchCommand::Begin { role, regime, agent_id, provider, model, skill_id, skill_digest, context_manifest, prompt_bundle, task, parent_run, repo, json } => {
                let binding = canon_cli::dispatch::DispatchBinding { task_id: task, parent_run_id: parent_run };
                let metadata = canon_cli::dispatch::DispatchMetadata {
                    provider,
                    model,
                    skill_id,
                    skill_digest,
                    context_manifest,
                    prompt_bundle,
                };
                canon_cli::dispatch::run_begin(&repo, &role, &regime, &agent_id, &binding, &metadata, json)
            }
            DispatchCommand::End { run, status, repo, json } => canon_cli::dispatch::run_end(&repo, run, status, json),
            DispatchCommand::Diff { repo, json } => canon_cli::dispatch::run_diff(&repo, json),
        },
        Command::Selftest { json } => canon_cli::selftest::run_selftest(json),
    }
}

fn run_skills_install(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>) -> ExitCode {
    let source = canon_cli::skills::resolve_source(source);
    if !source.join("SKILL.src.md").is_file() {
        return match canon_cli::skills::install(&source, target) {
            Ok(report) => {
                for skill in &report.installed {
                    let status = if skill.changed { "installed" } else { "unchanged" };
                    println!("{} v{} — {}", skill.name, skill.version, status);
                }
                ExitCode::SUCCESS
            }
            Err(err) => { eprintln!("canon skills install: {err}"); ExitCode::FAILURE }
        };
    }
    match canon_cli::skills::install_canonical(&source, target, providers) {
        Ok(report) => {
            println!("canon v1 — {} ({})", if report.changed { "installed" } else { "unchanged" }, report.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(","));
            ExitCode::SUCCESS
        }
        Err(err) => { eprintln!("canon skills install: {err}"); ExitCode::FAILURE }
    }
}

fn run_skills_check(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>) -> ExitCode {
    let source = canon_cli::skills::resolve_source(source);
    match canon_cli::skills::check(&source, target, providers) {
        Ok(report) => {
            for status in &report.statuses { println!("{} — {} ({})", status.path.display(), status.state, status.provider.as_str()); }
            for remnant in &report.remnants { println!("{} — legacy-remnant (codex reads .agents/skills; fix: `{}`)", remnant.display(), canon_cli::skills::legacy_codex_fix(&report.providers)); }
            if report.manifest_ok && report.remnants.is_empty() && report.statuses.iter().all(|status| status.state == "ok") { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Err(err) => { eprintln!("canon skills check: {err}"); ExitCode::FAILURE }
    }
}

fn run_skills_doctor(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>) -> ExitCode {
    let source = canon_cli::skills::resolve_source(source);
    match canon_cli::skills::doctor(&source, target, providers) {
        Ok(lines) => { for line in lines { println!("{line}"); } ExitCode::SUCCESS }
        Err(err) => { eprintln!("canon skills doctor: {err}"); ExitCode::FAILURE }
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

/// `plugin: None` calls the EXACT SAME [`canon_cli::query::run`] path this
/// function used before sensitive query output was added. The selected
/// formatter receives the already-policy-validated `include_sensitive` bit,
/// while the default formatter wrappers remain redacted. `plugin: Some(id)`
/// calls [`canon_cli::query::run_with_plugin`] instead; every diagnostic it
/// returns is printed to stderr regardless of whether a projection actually
/// resolved, and stdout uses the matching core/overlay formatter with the
/// same sensitive-output authorization.
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
    include_sensitive: bool,
) -> ExitCode {
    // s19 `query-scope-filters` design D5: kind-gating + status-domain
    // validation runs BEFORE any tier read (task 3.2/3.3) -- a usage
    // fault here is a clean, nothing-read `2`, never a store error.
    if let Err(e) = canon_cli::query::validate_scope(kind, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        eprintln!("canon query: {e}");
        let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, 0);
        return ExitCode::from(2);
    }

    if let Err(err) = canon_cli::retention::validate_sensitive(repo, include_sensitive) {
        eprintln!("canon query: {err}");
        let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, 0);
        return ExitCode::from(2);
    }
    let Some(plugin_id) = plugin else {
        return match canon_cli::query::run(repo, canon_yaml, kind, since, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
            Ok(outcome) => {
                let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, outcome.records.len());
                if json {
                    println!("{}", canon_cli::query::format_json_with_sensitive(&outcome, include_sensitive));
                } else {
                    print!("{}", canon_cli::query::format_human_with_sensitive(&outcome, include_sensitive));
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, 0);
                eprintln!("canon query: {err}");
                ExitCode::FAILURE
            }
        };
    };

    match canon_cli::query::run_with_plugin(repo, canon_yaml, kind, since, &plugin_id, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        Ok((outcome, plugin_outcome)) => {
            let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, outcome.records.len());
            for msg in &plugin_outcome.diagnostics {
                eprintln!("canon query --plugin {plugin_id}: {msg}");
            }
            if plugin_outcome.projections.is_empty() {
                if json {
                    println!("{}", canon_cli::query::format_json_with_sensitive(&outcome, include_sensitive));
                } else {
                    print!("{}", canon_cli::query::format_human_with_sensitive(&outcome, include_sensitive));
                }
            } else if json {
                println!("{}", canon_cli::query::format_json_with_overlay_and_sensitive(&outcome, &plugin_id, &plugin_outcome.projections, include_sensitive));
            } else {
                print!("{}", canon_cli::query::format_human_with_overlay_and_sensitive(&outcome, &plugin_id, &plugin_outcome.projections, include_sensitive));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = canon_cli::retention::audit_query(repo, kind, since, include_sensitive, 0);
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
fn run_fmt(root: &std::path::Path, repo: Option<&std::path::Path>) -> ExitCode {
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

/// `canon scenario new <tag> --title <label> [--subject <id>] [--lane
/// <v>] [--case <v>] [--actor <id>] [--feature <path>] [--project <id>]` (s16 P5,
/// `canon_cli::scaffold::run_scenario_new`'s own doc): the ONE
/// `Utc::now()` call for this command — computed here, at the
/// dispatch boundary, so a brand-new `.feature` file's `Feature:` +
/// first `Scenario:` provenance comments never straddle two different
/// timestamps (`canon_cli::scaffold`'s module doc, "deterministic
/// provenance"). `2` on a refused invocation (config fault, a spec
/// root the `--project` rules can't pin down for the derived path, an
/// out-of-root explicit `--feature`, or a duplicate tag), `0` on a
/// successful append/create.
fn run_scenario_new(
    repo: &std::path::Path,
    tag: &ScenarioId,
    title: &str,
    feature: Option<&std::path::Path>,
    project: Option<&ProjectId>,
    axes: canon_cli::scaffold::ScenarioAxes<'_>,
    actor_id: &str,
) -> ExitCode {
    let resolved = canon_cli::context::resolve_repo_root(repo);
    for (axis, value) in [("lane", axes.lane), ("case", axes.case)] {
        if let Some(value) = value {
            if let Some(violation) = canon_cli::subject::enum_membership_violation(&resolved, axis, value) {
                eprintln!("canon scenario new: refused — {violation}");
                return ExitCode::from(2);
            }
        }
    }
    let at = Utc::now().with_nanosecond(0).expect("0 is a valid nanosecond");
    let actor = Actor::new_unattributed(actor_id);
    ExitCode::from(
        canon_cli::scaffold::run_scenario_new(
            repo,
            tag,
            title,
            feature,
            project,
            axes,
            &actor,
            at,
        ) as u8,
    )
}

/// `canon feature new <area>.<surface> --title <label> [--project
/// <id>]` (s16 P5, `canon_cli::scaffold::run_feature_new`'s own doc)
/// — same single-`Utc::now()`-call discipline as [`run_scenario_new`]
/// above. `2` on a refused invocation (config fault, a spec root the
/// `--project` rules can't pin down, or an already-existing target
/// file), `0` on a fresh file
/// written.
fn run_feature_new(repo: &std::path::Path, surface: &AreaSurface, title: &str, project: Option<&ProjectId>, actor_id: &str) -> ExitCode {
    let at = Utc::now().with_nanosecond(0).expect("0 is a valid nanosecond");
    let actor = Actor::new_unattributed(actor_id);
    ExitCode::from(canon_cli::scaffold::run_feature_new(repo, surface, title, project, &actor, at) as u8)
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

fn context_pack_error_code(error: &canon_cli::context_pack::ContextPackError) -> ExitCode {
    use canon_cli::context_pack::ContextPackError;
    let code = match error {
        ContextPackError::UnsafePath(_)
        | ContextPackError::InvalidManifestVersion(_)
        | ContextPackError::MissingInput(_)
        | ContextPackError::MissingPack(_)
        | ContextPackError::PromptBundle { .. }
        | ContextPackError::SecretDetected(_)
        | ContextPackError::Serialize(_) => 2,
        ContextPackError::Io { .. }
        | ContextPackError::ObjectConflict { .. }
        | ContextPackError::Tampered { .. }
        | ContextPackError::Git(_) => 1,
    };
    ExitCode::from(code)
}

/// `canon adapter validate` is deliberately read-only: it parses one response
/// envelope and prints a redacted summary. Provider execution and capability
/// enforcement remain outside canon.
fn run_adapter_validate(response: &std::path::Path, repo: Option<&std::path::Path>, json: bool) -> ExitCode {
    let mut summary = match canon_cli::adapter::validate_response(response) {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("canon adapter validate: {error}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(repo) = repo {
        let repo = canon_cli::context::resolve_repo_root(repo);
        summary.context_join_verified = Some(verify_adapter_context_join(&repo, &summary));
    }

    if json {
        match serde_json::to_string_pretty(&summary) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("canon adapter validate: cannot render summary: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        println!("adapter response: valid");
        println!("  protocol_version: {}", summary.protocol_version);
        println!("  run_id: {}", summary.run_id);
        println!("  provider: {}", summary.provider);
        println!("  model: {}", summary.model);
        println!("  context_pack_id: {}", summary.context_pack_id);
        println!("  status: {}", format!("{:?}", summary.status).to_lowercase());
        println!("  evidence_refs: {}", summary.evidence_count);
        println!("  extensions: {}", if summary.extension_keys.is_empty() {
            "(none)".to_string()
        } else {
            summary.extension_keys.join(", ")
        });
        if let Some(verified) = summary.context_join_verified {
            println!("  context_join_verified: {verified}");
        }
    }
    ExitCode::SUCCESS
}

fn run_adapter_authorize(response: &std::path::Path, repo: &std::path::Path, json: bool) -> ExitCode {
    let result = match canon_cli::adapter::authorize_response(response, repo) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("canon adapter authorize: {error}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into()));
    } else {
        println!("execution_authorized: {}", result.execution_authorized);
        println!("execution_performed: false");
        println!("sandbox_enforced: false");
        for reason in &result.reasons { println!("reason: {reason}"); }
    }
    if result.execution_authorized { ExitCode::SUCCESS } else {
        eprintln!("execution_not_authorized");
        ExitCode::FAILURE
    }
}

fn verify_adapter_context_join(repo: &std::path::Path, summary: &canon_cli::adapter::ValidationSummary) -> bool {
    if canon_cli::context_pack::verify(repo, &summary.context_pack_id).is_err() {
        return false;
    }
    let Ok(outcome) = canon_cli::query::run(repo, None, RecordKind::Run, None, None, None, None) else {
        return false;
    };
    outcome.records.iter().any(|record| {
        record.0.get("run_id").and_then(serde_json::Value::as_str) == Some(summary.run_id.as_str())
            && record
                .0
                .get("lineage")
                .and_then(|lineage| lineage.get("context"))
                .and_then(|context| context.get("pack_id"))
                .and_then(serde_json::Value::as_str)
                == Some(summary.context_pack_id.as_str())
    })
}

fn run_context_pack_create(repo: &std::path::Path, manifest: &std::path::Path, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    match canon_cli::context_pack::create_from_manifest(&repo, manifest) {
        Ok(pack) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&pack).expect("context pack is serializable"));
            } else {
                println!("{}", pack.id);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon context-pack create: {error}");
            context_pack_error_code(&error)
        }
    }
}

fn run_context_pack_show(repo: &std::path::Path, id: &str, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    match canon_cli::context_pack::show(&repo, id) {
        Ok(pack) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&pack).expect("context pack is serializable"));
            } else {
                println!("{}", pack.id);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon context-pack show: {error}");
            context_pack_error_code(&error)
        }
    }
}

fn run_context_pack_verify(repo: &std::path::Path, id: &str, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    match canon_cli::context_pack::verify(&repo, id) {
        Ok(()) => {
            if json {
                println!("{}", serde_json::json!({ "id": id, "verified": true }));
            } else {
                println!("context pack {id} verified");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon context-pack verify: {error}");
            context_pack_error_code(&error)
        }
    }
}

fn run_prompt_register(repo: &std::path::Path, name: &str, version: &str, manifest: &std::path::Path, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    match canon_cli::context_pack::register_prompt_bundle_from_manifest(&repo, name, version, manifest) {
        Ok(bundle) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&bundle).expect("prompt bundle is serializable"));
            } else {
                println!("prompt bundle {}@{} registered ({})", bundle.name, bundle.version, bundle.digest);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon prompt register: {error}");
            context_pack_error_code(&error)
        }
    }
}

fn run_prompt_show(repo: &std::path::Path, name: &str, version: &str, json: bool) -> ExitCode {
    let repo = canon_cli::context::resolve_repo_root(repo);
    match canon_cli::context_pack::select_prompt_bundle(&repo, name, version) {
        Ok(bundle) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&bundle).expect("prompt bundle is serializable"));
            } else {
                println!("prompt bundle {}@{} ({})", bundle.name, bundle.version, bundle.digest);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon prompt show: {error}");
            context_pack_error_code(&error)
        }
    }
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
///
/// `--run` is the explicit attribution edge (s42
/// `close-the-open-loops`, task 3.2) — omitted, a trajectory records no
/// run rather than a guessed one.
fn run_ingest_artifacts(repo: &std::path::Path, watch: bool, interval_secs: u64, json: bool, run_id: Option<&RunId>) -> ExitCode {
    loop {
        match canon_cli::artifact_ingest::run(repo, run_id) {
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
    // A present but unusable `canon.yaml` is a usage error (exit 2),
    // the same refusal `canon gate check` makes — never a report of
    // the default ledger.
    let (repo, inputs) = match canon_cli::report::resolve_inputs(repo) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("canon report: {err}");
            return ExitCode::from(2);
        }
    };

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
            match err {
                canon_cli::dashboard::DashboardError::Config(_) => ExitCode::from(2),
                _ => ExitCode::FAILURE,
            }
        }
    }
}
