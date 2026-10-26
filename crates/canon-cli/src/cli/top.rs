//! Arguments of the top-level leaf commands (`canon init`, `canon query`, ...).

use canon_model::envelope::RecordKind;
use canon_model::{ChangeId, RegimeKey, RoleId, SubjectId};
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to set up (used as-is)
    #[arg(long, default_value = ".", value_name = "DIR")]
    pub repo: PathBuf,
    /// Validate an existing canon.yaml instead of writing one
    #[arg(long, conflicts_with_all = ["no_agents_md", "no_policy", "policy", "providers", "language", "dry_run"])]
    pub check: bool,
    /// Do not write or refresh the canon block in AGENTS.md
    #[arg(long)]
    pub no_agents_md: bool,
    /// Level of the starter .canon/policy.yaml [default: strict]
    #[arg(long, value_enum, value_name = "LEVEL", default_value = "strict", conflicts_with = "no_policy")]
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
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// With --json, keep only these top-level report fields (comma-separated); an unknown field is refused (camelCase and snake_case both match)
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', action = clap::ArgAction::Set)]
    pub fields: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// With --json, keep only these top-level surface sections (comma-separated); an unknown field is refused (camelCase and snake_case both match)
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', action = clap::ArgAction::Set)]
    pub fields: Option<Vec<String>>,
}

#[derive(Debug, Args)]
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

#[derive(Debug, Args)]
pub struct RetrieveArgs {
    /// Role scope; with --regime must equal its leading segment
    #[arg(long = "for-role", value_name = "ROLE", value_parser = crate::retrieve::parse_role)]
    pub for_role: RoleId,
    /// Full regime key (<role>/<repo>/<area>/<hash>); mutually exclusive with --domain/--subject-id
    #[arg(long, value_parser = crate::retrieve::parse_regime, value_name = "KEY")]
    pub regime: Option<RegimeKey>,
    /// Derive the regime from a domain slug; mutually exclusive with --regime
    #[arg(long, value_name = "NAME")]
    pub domain: Option<String>,
    /// Narrow --domain to one subject_id; requires --domain, mutually exclusive with --regime
    #[arg(long, value_parser = crate::retrieve::parse_subject, value_name = "SUBJECT_ID")]
    pub subject_id: Option<SubjectId>,
    /// Cap on the guidance items returned (default: 5)
    #[arg(long, value_name = "N", default_value = "5")]
    pub limit: Option<usize>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Record kind to read (e.g. handoff, strategy_item)
    #[arg(long, value_parser = crate::query::kind_parser(), value_name = "KIND")]
    pub kind: RecordKind,
    /// Only records with at >= <since> (RFC3339/ISO-8601)
    #[arg(long, value_parser = crate::timestamp::parse, value_name = "TIMESTAMP")]
    pub since: Option<DateTime<Utc>>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Project a plugin's overlay fields onto each record (fail-soft)
    #[arg(long, value_name = "PLUGIN_ID")]
    pub plugin_id: Option<String>,
    /// Every version of the kind, the ledger's and the archive's, oldest first, instead of the current records
    #[arg(long, conflicts_with = "plugin_id")]
    pub history: bool,
    /// Scope --kind change/task to one ChangeId (other kinds exit 2)
    #[arg(long, value_parser = crate::query::parse_change_id, value_name = "CHANGE_ID")]
    pub change_id: Option<ChangeId>,
    /// Scope --kind change/task by status field (other kinds exit 2)
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,
    /// Scope --kind subject by domain (other kinds exit 2)
    #[arg(long, value_name = "NAME")]
    pub domain: Option<String>,
    /// Permit sensitive query output only when policy explicitly allows it
    #[arg(long)]
    pub include_sensitive: bool,
    /// Return at most this many records per page, `--cursor` to continue, `--all` for every record
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// With --json, keep only these top-level record fields (comma-separated); an unknown field is refused (camelCase and snake_case both match)
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', action = clap::ArgAction::Set)]
    pub fields: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// The record kind to export
    #[arg(long, value_parser = crate::query::kind_parser(), value_name = "KIND")]
    pub kind: RecordKind,
    /// Only records strictly before this time (RFC3339 or YYYY-MM-DD, UTC)
    #[arg(long, value_parser = crate::timestamp::parse, value_name = "TIMESTAMP")]
    pub before: Option<DateTime<Utc>>,
    /// Only records at or after this time (RFC3339 or YYYY-MM-DD, UTC)
    #[arg(long, value_parser = crate::timestamp::parse, value_name = "TIMESTAMP")]
    pub after: Option<DateTime<Utc>>,
    /// Write the manifest to this file (mode 0600) instead of stdout
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Include records marked sensitive (only when policy explicitly allows it)
    #[arg(long)]
    pub include_sensitive: bool,
    #[command(flatten)]
    pub page: crate::paging::PageFlags,
    /// Keep only these fields of each exported record (comma-separated): kind, at, id, digest; an unknown field is refused (camelCase and snake_case both match)
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', action = clap::ArgAction::Set)]
    pub fields: Option<Vec<String>>,
    #[command(flatten)]
    pub dry_run: crate::cli::DryRunArgs,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct PurgeArgs {
    /// The record kind to purge (session, run, event or trajectory)
    #[arg(long, value_parser = crate::query::purgeable_kind_parser(), value_name = "KIND")]
    pub kind: RecordKind,
    /// Remove records strictly before this time (RFC3339 or YYYY-MM-DD, UTC)
    #[arg(long, value_parser = crate::timestamp::parse, value_name = "TIMESTAMP")]
    pub before: DateTime<Utc>,
    #[command(flatten)]
    pub safety: crate::cli::SafetyArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct DashboardArgs {
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Snapshot dir to serve (default: regenerated fresh each run)
    #[arg(long, value_name = "DIR")]
    pub snapshot: Option<PathBuf>,
    /// Local port to bind (0 = OS-assigned free port)
    #[arg(long, default_value_t = 4173, value_name = "N")]
    pub port: u16,
    /// Print one JSON document naming the address it serves (`data.url`), then serve until stopped; the human lines go to stderr
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub input: crate::cli::InputArgs,
}

#[derive(Debug, Args)]
pub struct SelftestArgs {
    /// Run only this suite (default: every suite)
    #[arg(long, value_name = "NAME", value_parser = clap::builder::PossibleValuesParser::new(crate::selftest::SUITE_NAMES.iter().copied()))]
    pub suite: Option<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    /// Command path, e.g. `evidence add` (omit to list every command)
    pub path: Vec<String>,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
    /// Print every error code any command can emit, with its meaning, instead of a command
    #[arg(long, conflicts_with_all = ["path", "error"])]
    pub errors: bool,
    /// Show one error code: its meaning, exit, retryable and fix (an unknown code exits 2)
    #[arg(long, value_name = "CODE", conflicts_with = "path")]
    pub error: Option<String>,
}

/// The shells `canon completion` writes a script for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// Shell to write the completion script for
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: CompletionShell,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
