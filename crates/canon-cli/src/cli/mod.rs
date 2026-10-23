//! The `canon` command line: one clap tree, one module per command group.
//!
//! [`Cli`] is the root; [`Command`] lists the top-level commands. Each group
//! (`canon gate ...`) keeps its enum and its per-leaf `Args` structs in its own
//! file, so adding or changing a leaf touches one group file. Groups are never
//! runnable: every group enum is a required subcommand. [`meta`] maps every
//! parsed command to its metadata row, exhaustively.

pub mod adapter;
pub mod artifacts;
pub mod brief;
pub mod change;
pub mod context_pack;
pub mod demo;
pub mod dispatch;
pub mod divergence;
pub mod evidence;
pub mod feature;
pub mod finding;
pub mod gate;
pub mod ingest;
pub mod inventory;
pub mod ledger;
pub mod meta;
pub mod plugin;
pub mod policy;
pub mod prompt;
pub mod regime;
pub mod report;
pub mod review;
pub mod scenario;
pub mod skills;
pub mod strategy;
pub mod subject;
pub mod task;
pub mod tier;
pub mod top;

pub use top::*;
pub use policy::PolicyCommand;
pub use context_pack::ContextPackCommand;
pub use adapter::AdapterCommand;
pub use prompt::PromptCommand;
pub use strategy::StrategyCommand;
pub use regime::RegimeCommand;
pub use report::ReportCommand;
pub use task::TaskCommand;
pub use demo::DemoCommand;
pub use dispatch::DispatchCommand;
pub use ingest::IngestCommand;
pub use gate::GateCommand;
pub use evidence::EvidenceCommand;
pub use finding::FindingCommand;
pub use review::ReviewCommand;
pub use divergence::DivergenceCliCommand;
pub use inventory::InventoryCommand;
pub use plugin::PluginCommand;
pub use scenario::ScenarioCommand;
pub use feature::FeatureCommand;
pub use brief::BriefCommand;
pub use change::ChangeCommand;
pub use subject::SubjectCommand;
pub use skills::SkillsCommand;
pub use ledger::LedgerCommand;
pub use tier::TierCommand;
pub use artifacts::ArtifactsCommand;

use clap::{CommandFactory, Parser, Subcommand};

/// The clap tree of the whole CLI.
pub fn command() -> clap::Command {
    Cli::command()
}

#[derive(Parser)]
#[command(
    name = "canon",
    version,
    about = "Specs, evidence gates, and agent memory for your repo",
    long_about = "canon keeps a repo's specs, review evidence, and agent strategy \
memory in one place: author .feature specs, task done completion on real evidence, \
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
pub struct Cli {
    /// The role you act as: planner, implementer, reviewer or orchestrator. Overrides `CANON_ROLE`. `schema` and `context` list only the role's commands, `status` keeps its steps, and `--role` on `evidence add` / `evidence audit` / `review scenario` / `review change` defaults to it
    #[arg(long = "as", global = true, value_enum, value_name = "ROLE")]
    pub declared_role: Option<crate::role::Role>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    // ── Getting started ──
    /// Set up canon in a repo (canon.yaml, a starter policy, the AGENTS.md block, a plans home)
    #[command(after_help = "Examples:\n  canon init\n  canon init --policy light\n  canon init --providers claude,codex\n  canon init --no-agents-md --no-policy\n  canon init --check\n\nPolicy levels (--policy, default strict): light requires evidence on every\nscenario; standard adds a `@case:failure` scenario per feature surface; strict\nadds independent review. The level is the first line of .canon/policy.yaml and\n`canon status` shows it.\n\nHarnesses (--providers): on a terminal, init asks which harnesses get the canon\nskill, with the detected ones (.claude, .codex, .omp, .pi directories) checked.\nWithout a terminal or --providers it takes the detected ones; with none detected\nit installs nothing and says how to choose. The answer is recorded as\n`skills: { providers: [...] }` in canon.yaml (`--providers none` records `[]`).\n\nRerunning in a repo that has canon.yaml leaves canon.yaml and .canon/policy.yaml\nalone and only refreshes the canon block in AGENTS.md.")]
    Init(InitArgs),
    /// Bring a repo from an earlier canon up to date: the 0.17 layout (specs, changes, approvals, skills, .gitignore), the 0.23 latest-only ledger (superseded versions to .canon/archive) and 0.23 review pins
    #[command(after_help = "Examples:\n  canon migrate\n  canon migrate --commit --plan-digest <DIGEST>\n\nThe bare run prints the plan and its planDigest and writes nothing; --commit --plan-digest applies exactly that plan. Idempotent:\na second run finds nothing to move, and a run that stopped partway is finished\nby the next one.\n\n- A spec root's features/kind=feature/ moves to <root>/kind=feature/ (git mv in a\n  git work tree). A root at the repo-level `specs` moves to .canon/specs, and\n  canon.yaml records `former_roots: [specs]` so reviews pinned before the move\n  still find their scenarios.\n- Changes `canon change new` wrote under openspec/changes move to\n  .canon/changes/<slug>/ (proposal.md -> brief.md, tasks.md alongside); every\n  other change stays and is named. A change counts as canon's when its files are\n  still that command's scaffold and its first stored version was already adopted:\n  0.16 kept no durable provenance. A false positive keeps its content and ids and\n  is one `git mv` to undo; --dry-run shows exactly which changes move.\n  canon init's `{ dialect: openspec, root: . }` source goes once nothing unread is\n  left under it.\n- An external proposal.md's Approved: line is copied into a canon pointer\n  brief, .canon/changes/<change>/brief.md, with a Source: line.\n- skills.providers is written from the install lock; missing canon .gitignore\n  lines are added; the scenario index is rebuilt (full sync).\n- Superseded ledger versions move to .canon/archive (`canon ledger compact`);\n  `canon query --history` still reads them.\n- Reviews written before 0.23 are re-pinned to what their evidence says: a\n  review current under the old rule stays current, a stale one stays stale.\n\nRefuses (exit 2, nothing written) on uncommitted changes under a path it\nmoves, on a destination that already exists, on a change directory that could\nstop halfway, and on a canon.yaml edit it cannot make with every other byte\nkept. canon.yaml is written atomically.")]
    Migrate(MigrateArgs),
    /// Record who approved the gate's policy (.canon/policy.yaml) and which bytes
    #[command(after_help = "Examples:\n  canon policy approve --approver alice --note 'reviewed the starter policy'\n\nWrites .canon/policy.approval.json (commit it): the policy's sha256, who\napproved it, when, and the note. `canon gate check` and `canon status` print an\nadvisory while the policy was never approved. A policy that changed since its\napproval (or whose approval cannot be read) fails closed: `gate check` exits 1\nand `subject move` refuses to ship or reopen; `--override-reason` does not\nwaive it. `canon policy show` reads the approval back.")]
    Policy {
        #[command(subcommand)]
        action: PolicyCommand,
    },
    /// Try the evidence loop end-to-end in a throwaway demo repo
    #[command(after_help = "Examples:\n  canon demo init --repo /tmp/canon-demo && cd /tmp/canon-demo\n  canon gate check   # RED\n  canon demo review && canon gate check   # GREEN")]
    Demo {
        #[command(subcommand)]
        action: DemoCommand,
    },
    /// Show where this repo stands: subjects by status, their gaps, and the next commands
    #[command(after_help = "Examples:\n  canon status\n  canon status --json\n\nA read: exits 0 whatever it finds, and writes nothing. The counts come from the\nsame joins `canon gate check` and `canon subject move` use.")]
    Status(StatusArgs),
    /// Show what you can author here: record kinds, fields, enums, policies
    Context(ContextArgs),
    /// Create, inspect, and verify reproducible context packs
    #[command(after_help = "Examples:\n  canon context-pack add --manifest .canon/context-manifest.json\n  canon context-pack show sha256:<pack-digest> --repo . --json\n  canon context-pack verify sha256:<pack-digest>")]
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
        after_help = "Examples:\n  canon prompt add reviewer@v1 --manifest .canon/reviewer.json\n  canon prompt show reviewer@v1 --json"
    )]
    Prompt {
        #[command(subcommand)]
        action: PromptCommand,
    },
    // ── Specs & authoring ──
    /// Validate a spec/artifact corpus against canon's format
    #[command(name = "format", after_help = "Examples:\n  canon format                  Every configured spec root\n  canon format spec\n  canon format spec --repo ~/work/myrepo")]
    Format(FormatArgs),
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
    /// Write a plan change (.canon/changes/<slug>/brief.md) and adopt it into a subject
    #[command(after_help = "Examples:\n  canon change new add-login --subject-id auth --title 'Add login'")]
    Change {
        #[command(subcommand)]
        action: ChangeCommand,
    },
    /// Check a change's brief (PRD format) and see the next steps; never edits
    #[command(after_help = "Examples:\n  canon brief show                     State and the next one to three steps\n  canon brief check                    Findings; exit 0 clean, 2 blockers, 1 error\n  canon brief check add-login --json\n\nThe brief is the PRD `canon change new` writes: sections are found by their\n`<!-- canon:section <key> role=… -->` marker, never by heading text. Blockers\n(structure) are a missing or out-of-order section, a duplicate flow id, step\ndetail that does not match the flow ids, leftover placeholders, and, once the\nsubject is building, a flow step with no scenario or no failure scenario.\nAdvisories (prose) apply to normative sections, Intent and Questions, in the\nlanguage each is written in (`language:` in canon.yaml overrides); only en and\nko have rule lists.\nWaive one rule in one section with\n  <!-- canon:waive <rule-id> <section-key> \"<reason>\" -->\nA brief without markers (written before 0.19) checks as `brief.legacy`, an\nadvisory. Without --change-id, the repo's only live change is checked.")]
    Brief {
        #[command(subcommand)]
        action: BriefCommand,
    },
    /// Index a validated .feature corpus into the scenario ledger
    Inventory {
        #[command(subcommand)]
        action: InventoryCommand,
    },
    // ── Evidence loop ──
    /// Run evidence gates, promote staged records, install hooks
    #[command(after_help = "Examples:\n  canon gate check\n  canon task done my-change#3\n  canon gate install-hooks")]
    Gate {
        #[command(subcommand)]
        action: GateCommand,
    },
    /// Flip a plan task's checkbox once committed evidence supports it
    #[command(after_help = "Examples:\n  canon task done my-change#3")]
    Task {
        #[command(subcommand)]
        action: TaskCommand,
    },
    /// Author a staged, attributed evidence attestation for a plan task
    #[command(after_help = "Examples:\n  canon evidence add --task-id my-change#3.1 --class test-run --ref 'cargo test -p canon-cli' --role implementer\n\nThe loop:\n  canon evidence add ...    Record an EvidenceRecord (promoted in the same run under\n                            `write: { promote: auto }`, which `canon init` writes)\n  canon task done <id>      Flip the checkbox on it\nUnder `write.promote: manual` (no `write:` key), run `canon gate promote` after\neach add to commit it (assigns run_seq).\n\nSee `canon evidence add --help` for exactly what the gate does and does not check.")]
    Evidence {
        #[command(subcommand)]
        action: EvidenceCommand,
    },
    /// Check or move the evidence artifact store (local, or an object-store tier)
    Artifacts {
        #[command(subcommand)]
        action: ArtifactsCommand,
    },
    /// Record one code-review finding as a durable record
    #[command(after_help = "Examples:\n  canon finding add --change-id s43-findings-are-records --round 1 --seq 1 \\\n      --severity blocker --disposition open --actor-id review-voice \\\n      --summary 'a release metric is asserted, never computed'\n  canon finding close s43-findings-are-records --round 1 --seq 1 \\\n      --disposition fixed --resolution-sha $(git rev-parse HEAD)\n\nThe loop:\n  canon finding add ...     Record a Finding\n  canon finding close ...   Record its disposition TRANSITION, once fixed\nEach is promoted in the same run under `write: { promote: auto }`, which\n`canon init` writes. Under `write.promote: manual` (no `write:` key), run\n`canon gate promote` after each to commit it.\n\nThe committed finding STAYS when it is closed: the ledger is append-only and\nthe pair IS the history. Every reader folds by natural key to the latest\nversion, so the finding is still counted once.\n\nRECORDED OBSERVATION, NOT PROOF — canon never verifies that the defect\nexisted, that it was fixed, or that --introduced-by is the true cause.\nSee `canon finding add --help` for exactly what it does and does not establish.")]
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
    /// Import agent sessions, records, and plans into canon's store
    #[command(after_help = "Examples:\n  canon ingest sessions --watch\n  canon ingest records\n  canon ingest plans")]
    Ingest {
        #[command(subcommand)]
        action: IngestCommand,
    },
    /// Fetch role-scoped strategy guidance
    Retrieve(RetrieveArgs),
    /// Promote a proven strategy into the git-tracked tier
    Strategy {
        #[command(subcommand)]
        action: StrategyCommand,
    },
    /// Begin an agent run with retrieved guidance recorded
    Dispatch {
        #[command(subcommand)]
        action: DispatchCommand,
    },
    /// Build a canonical regime key for hook scripts
    Regime {
        #[command(subcommand)]
        action: RegimeCommand,
    },
    // ── Reading & reporting ──
    /// Read stored records across every storage tier
    Query(QueryArgs),
    /// Export a redacted retention manifest without deleting records
    Export(ExportArgs),
    /// Remove allowlisted records older than a cutoff: the bare run previews the removal set and its planDigest, `--commit --plan-digest <DIGEST>` deletes it
    Purge(PurgeArgs),
    /// Build, check, or snapshot the status report
    Report {
        #[command(subcommand)]
        action: ReportCommand,
    },
    /// Serve the local status dashboard
    Dashboard(DashboardArgs),
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
    /// Keep the ledger to the current version of each record
    #[command(after_help = "Examples:\n  canon ledger compact\n  canon ledger compact --commit --plan-digest <DIGEST>\n\nThe bare run previews the move and prints its planDigest.\n\nMoves every superseded version of a record (an evidence record a newer one replaced, a\nscenario or finding rewritten since) from .canon/ledger to .canon/archive, once. The\nledger then holds the current decision per key, and every default read (status, gate\ncheck, guards, query) reads only that; `canon query --kind <kind> --history` reads the\narchive too. Appending a new version moves the versions it supersedes on its own, so\nthis is for a ledger written before 0.23, or after an interrupted move. Idempotent;\n`canon migrate` runs it. A record that carries a standing claim (a flagged overlay, a\nbrief override, an approval at the current evidence) stays in the ledger.")]
    Ledger {
        #[command(subcommand)]
        action: LedgerCommand,
    },
    /// Run canon's built-in fixture self-tests
    Selftest(SelftestArgs),
    /// Describe canon's own commands: list them, or show one command's arguments, options, effect and exit codes
    #[command(after_help = "Examples:\n  canon schema\n  canon schema --json\n  canon schema evidence add --json\n  canon schema --as reviewer --json\n  canon schema --errors --json\n  canon schema --error CONFIG_MISSING\n\nGenerated from the command tree itself, so it never drifts from what the parser accepts.\nTouches no repo and no network: it works from any directory. An unknown path exits 2 and names the\nnearest matches.\n\nThe bare listing is unfiltered: every command. `--as <ROLE>` lists only that role's commands; the\nCANON_ROLE variable never filters or fails `schema` (an invalid value is a warning in meta.warnings).\nA command whose effect depends on its arguments carries `effectNote`.")]
    Schema(SchemaArgs),
    /// Write the shell completion script for canon (bash, zsh or fish)
    #[command(after_help = "Examples:\n  canon completion zsh > ~/.zfunc/_canon\n  canon completion bash > /etc/bash_completion.d/canon\n  canon completion fish > ~/.config/fish/completions/canon.fish\n\nThe script is generated from the command tree, so it completes exactly what the parser accepts. It needs no repo and no network.")]
    Completion(CompletionArgs),
}

/// `--promote`/`--no-promote`, flattened into every staged write
/// (0.16 D2; `crate::write_mode`).
#[derive(clap::Args, Debug, Clone, Copy)]
pub struct PromoteArgs {
    /// Promote the record this run staged, in the same run; records others staged stay staged (the default under canon.yaml `write: { promote: auto }`)
    #[arg(long, conflicts_with = "no_promote")]
    pub promote: bool,
    /// Keep this write staged even under canon.yaml `write: { promote: auto }`
    #[arg(long)]
    pub no_promote: bool,
}

impl PromoteArgs {
    pub fn flag(self) -> crate::write_mode::PromoteFlag {
        crate::write_mode::PromoteFlag::from_flags(self.promote, self.no_promote)
    }
}

/// `--dry-run`, declared once with one help string and flattened into every
/// writing leaf that offers a preview (0.28 D6): full validation (it refuses
/// with the code the real run would), nothing written, a report of what the
/// run would write. The five destructive leaves take [`SafetyArgs`] instead.
#[derive(clap::Args, Clone, Copy, Debug, Default)]
pub struct DryRunArgs {
    /// Validate fully and report what would be written, without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

/// `--commit --plan-digest <DIGEST>`, flattened into exactly the five
/// destructive leaves (`purge`, `artifacts move`, `migrate`, `ledger compact`,
/// `tier age`; 0.28 D6). The bare run is the preview: it prints the plan with
/// its `planDigest`, exits 0 and writes nothing. `--commit` acts only when the
/// plan recomputed now still has `--plan-digest` (`PLAN_CHANGED`, exit 2).
/// [`SafetyArgs::mode`] is what a handler reads.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct SafetyArgs {
    /// Act on the previewed plan; needs --plan-digest
    #[arg(long, requires = "plan_digest")]
    pub commit: bool,
    /// The `planDigest` the preview printed; the run acts only if the plan recomputed now has this digest
    #[arg(long, value_name = "DIGEST", requires = "commit")]
    pub plan_digest: Option<String>,
}

/// `--repo <DIR>`, declared once and flattened into every leaf that reads a
/// repo (0.28 D5). Absent: the nearest ancestor of the current directory
/// holding a `canon.yaml`, else the current directory. Present: that
/// directory, as given (no ancestor walk). `init` and `demo init` set up the
/// directory they are given and keep their own option.
///
/// Dereferences to the resolved root, so a handler passes `&repo` wherever a
/// path is wanted; the walk runs at most once per parse.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct RepoArgs {
    /// The repo to act on (default: the nearest ancestor directory holding a canon.yaml)
    #[arg(long = "repo", value_name = "DIR")]
    pub(crate) repo: Option<std::path::PathBuf>,
    #[arg(skip)]
    resolved: std::sync::OnceLock<std::path::PathBuf>,
}

impl RepoArgs {
    /// `--repo` as typed, when present.
    pub fn given(&self) -> Option<&std::path::Path> {
        self.repo.as_deref()
    }
}

impl std::ops::Deref for RepoArgs {
    type Target = std::path::PathBuf;
    fn deref(&self) -> &std::path::PathBuf {
        self.resolved.get_or_init(|| match &self.repo {
            // `--repo .` names the current directory itself, not "search upward".
            Some(dir) if dir.as_os_str() == "." => std::env::current_dir().unwrap_or_else(|_| dir.clone()),
            Some(dir) => dir.clone(),
            None => crate::context::discover_repo_root(),
        })
    }
}
