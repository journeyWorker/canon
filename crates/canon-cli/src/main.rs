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
  `canon skills install` materializes the full guides into .claude/skills/ and .codex/skills/."
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
        #[arg(long)]
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
enum LearnCommand {
    /// Promote a distilled strategy (by id) into .canon/strategies/
    Promote {
        /// The StrategyId (ULID) to promote
        #[arg(value_parser = canon_cli::learn::parse_strategy_id)]
        strategy_id: StrategyId,
        /// Repo root (default: nearest ancestor with a canon.yaml)
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Preview without writing anything
        #[arg(long)]
        dry_run: bool,
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
    /// Mint a Run manifest with retrieved guidance recorded into it
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
        /// Target .feature file (default: derived from <tag>; must live under a specs.roots[] entry)
        #[arg(long)]
        feature: Option<PathBuf>,
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
    /// Transition a subject's lifecycle status (shipping is evidence-gated)
    Status {
        /// The subject to transition
        #[arg(value_parser = canon_cli::subject::parse_subject_id)]
        id: SubjectId,
        /// Target state (proposed/specced/building/verifying/shipped/retired)
        #[arg(value_parser = canon_cli::subject::parse_status)]
        state: canon_model::SubjectStatus,
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
    /// Copy canon's skill guides into .claude/skills/ and .codex/skills/
    Install {
        /// Directory holding <name>/SKILL.md sources
        #[arg(long, default_value = "canon/skills")]
        source: PathBuf,
        /// Consumer repo root to materialize .claude/ and .codex/ into
        #[arg(long, default_value = ".")]
        target: PathBuf,
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
            SkillsCommand::Install { source, target } => run_skills_install(&source, &target),
        },
        Command::Tier { action } => match action {
            TierCommand::Age { dry_run, repo, canon_yaml } => run_tier_age(&repo, canon_yaml.as_deref(), dry_run),
        },
        Command::Query { kind, since, repo, canon_yaml, json, plugin, change_id, status, domain } => {
            run_query(&repo, canon_yaml.as_deref(), kind, since, json, plugin, change_id, status, domain)
        }
        Command::Format { check: _, root, repo } => run_fmt(&root, repo.as_deref()),
        Command::Context { repo, json } => run_context(&repo, json),
        Command::Ingest { action } => match action {
            IngestCommand::Sessions { watch, interval_secs, home, canon_yaml, full, all_workspaces } => run_ingest_sessions(&canon_yaml, home.as_deref(), watch, interval_secs, full, all_workspaces),
            IngestCommand::Artifacts { watch, interval_secs, repo, json } => run_ingest_artifacts(&repo, watch, interval_secs, json),
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
            ScenarioCommand::New { tag, title, feature, repo } => run_scenario_new(&repo, &tag, &title, feature.as_deref()),
        },
        Command::Feature { action } => match action {
            FeatureCommand::New { surface, title, repo } => run_feature_new(&repo, &surface, &title),
        },
        Command::Subject { action } => match action {
            SubjectCommand::New { id, domain, title, summary, owner_role, actor_id, repo, json } => {
                ExitCode::from(canon_cli::subject::run_new(&repo, &id, &domain, &title, &summary, &owner_role, &actor_id, json) as u8)
            }
            SubjectCommand::Adopt { change_id, subject, repo, json } => ExitCode::from(canon_cli::subject::run_adopt(&repo, &change_id, &subject, json) as u8),
            SubjectCommand::Status { id, state, repo, json } => ExitCode::from(canon_cli::subject::run_status(&repo, &id, state, json) as u8),
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
            LearnCommand::Promote { strategy_id, repo, dry_run } => canon_cli::learn::run_promote(&repo, &strategy_id, dry_run),
        },
        Command::Dispatch { action } => match action {
            DispatchCommand::Begin { role, regime, agent_id, repo, json } => canon_cli::dispatch::run_begin(&repo, &role, &regime, &agent_id, json),
        },
        Command::Selftest { json } => canon_cli::selftest::run_selftest(json),
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
