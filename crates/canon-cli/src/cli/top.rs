//! Arguments of the top-level leaf commands (`canon init`, `canon query`, ...).

use canon_model::envelope::RecordKind;
use canon_model::{ChangeId, RegimeKey, RoleId, SubjectId};
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct InitArgs {
    /// Directory to set up (used as-is)
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Validate an existing canon.yaml instead of writing one
    #[arg(long, conflicts_with_all = ["no_agents_md", "no_policy", "policy", "providers", "language", "dry_run"])]
    pub check_config: bool,
    /// Do not write or refresh the canon block in AGENTS.md
    #[arg(long)]
    pub no_agents_md: bool,
    /// Level of the starter .canon/policy.yaml [default: strict]
    #[arg(long, value_enum, value_name = "LEVEL", conflicts_with = "no_policy")]
    pub policy: Option<crate::policy_level::PolicyLevel>,
    /// Do not write the starter .canon/policy.yaml
    #[arg(long)]
    pub no_policy: bool,
    /// Harnesses to install the canon skill for, without asking (`claude`, `codex`, `omp`, `pi`, comma-separated; `none` for none)
    #[arg(long, value_name = "LIST")]
    pub providers: Option<String>,
    /// Language the briefs are written in (`en`, `ko`, or any tag: `en` headings, structure rules only); recorded as `language:` in canon.yaml
    #[arg(long, value_name = "TAG")]
    pub language: Option<String>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct MigrateArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// With --json, keep only these top-level report fields (comma-separated); an unknown field is refused
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
}

#[derive(Args)]
pub struct ContextArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// With --json, keep only these top-level surface sections (comma-separated); an unknown field is refused
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
}

#[derive(Args)]
pub struct FormatArgs {
    /// Corpus directory, e.g. a consumer repo's spec/ directory (default: every configured specs.roots[] entry). A relative path resolves under --repo when given, else the current directory
    #[arg(value_name = "DIR")]
    pub dir: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct RetrieveArgs {
    /// Role scope; with --regime must equal its leading segment
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: RoleId,
    /// Full regime key (<role>/<repo>/<area>/<hash>); mutually exclusive with --domain/--subject-id
    #[arg(long, value_parser = crate::retrieve::parse_regime)]
    pub regime: Option<RegimeKey>,
    /// Derive the regime from a domain slug; mutually exclusive with --regime
    #[arg(long)]
    pub domain: Option<String>,
    /// Narrow --domain to one subject_id; requires --domain, mutually exclusive with --regime
    #[arg(long, value_parser = crate::retrieve::parse_subject)]
    pub subject_id: Option<SubjectId>,
    /// Top-k cap (default: canon's DEFAULT_K)
    #[arg(short, long)]
    pub k: Option<usize>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct RegimeKeyArgs {
    /// The <role> segment (canonicalized)
    #[arg(long)]
    pub role: String,
    /// The <repo> segment (canonicalized)
    #[arg(long = "repo-name", value_name = "NAME")]
    pub repo_name: String,
    /// The <area> segment (canonicalized)
    #[arg(long)]
    pub area: String,
    /// The <hash> segment (6-64-char lowercase hex; passed through, never re-hashed)
    #[arg(long)]
    pub hash: String,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct QueryArgs {
    /// Record kind to read (e.g. handoff, strategy_item)
    #[arg(long, value_parser = crate::query::kind_parser())]
    pub kind: RecordKind,
    /// Only records with at >= <since> (RFC3339/ISO-8601)
    #[arg(long, value_parser = crate::query::parse_since)]
    pub since: Option<DateTime<Utc>>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Project a plugin's overlay fields onto each record (fail-soft)
    #[arg(long)]
    pub plugin: Option<String>,
    /// Every version of the kind, the ledger's and the archive's, oldest first, instead of the current records
    #[arg(long, conflicts_with = "plugin")]
    pub history: bool,
    /// Scope --kind change/task to one ChangeId (other kinds exit 2)
    #[arg(long, value_parser = crate::query::parse_change_id)]
    pub change_id: Option<ChangeId>,
    /// Scope --kind change/task by status field (other kinds exit 2)
    #[arg(long)]
    pub status: Option<String>,
    /// Scope --kind subject by domain (other kinds exit 2)
    #[arg(long)]
    pub domain: Option<String>,
    /// Permit sensitive query output only when policy explicitly allows it
    #[arg(long)]
    pub include_sensitive: bool,
    /// Return at most this many records per page, `--cursor` to continue, `--all` for every record
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// With --json, keep only these top-level record fields (comma-separated); an unknown field is refused
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
}

#[derive(Args)]
pub struct ExportArgs {
    #[arg(long, value_parser = crate::query::kind_parser())]
    pub kind: RecordKind,
    #[arg(long, value_parser = crate::query::parse_since)]
    pub before: Option<DateTime<Utc>>,
    #[arg(long, value_parser = crate::query::parse_since)]
    pub after: Option<DateTime<Utc>>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub include_sensitive: bool,
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// Keep only these fields of each exported record (comma-separated): kind, at, id, digest; an unknown field is refused
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
}

#[derive(Args)]
pub struct PurgeArgs {
    #[arg(long, value_parser = crate::query::purgeable_kind_parser())]
    pub kind: RecordKind,
    #[arg(long, value_parser = crate::query::parse_since)]
    pub before: DateTime<Utc>,
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ReportArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Byte-diff against .canon/REPORT.md (0 clean, 1 drift); --snapshot wins if both given
    #[arg(long)]
    pub check: bool,
    /// Export panel marts to <dir>/*.parquet + manifest.json instead
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DashboardArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Snapshot dir to serve (default: regenerated fresh each run)
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
    /// Local port to bind (0 = OS-assigned free port)
    #[arg(long, default_value_t = 4173)]
    pub port: u16,
    /// Print one JSON document naming the address it serves (`data.url`), then serve until stopped; the human lines go to stderr
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SelftestArgs {
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SchemaArgs {
    /// Command path, e.g. `evidence add` (omit to list every command)
    pub path: Vec<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Print every error code any command can emit, with its meaning, instead of a command
    #[arg(long, conflicts_with = "path")]
    pub errors: bool,
    /// List every command instead of only the declared role's
    #[arg(long)]
    pub all: bool,
}

/// The shells `canon completion` writes a script for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Args)]
pub struct CompletionArgs {
    /// Shell to write the completion script for
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: CompletionShell,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
