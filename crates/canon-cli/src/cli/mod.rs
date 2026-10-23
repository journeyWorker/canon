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
pub mod learn;
pub mod ledger;
pub mod meta;
pub mod plugin;
pub mod policy;
pub mod prompt;
pub mod review;
pub mod scenario;
pub mod skills;
pub mod subject;
pub mod tier;
pub mod top;

pub use top::*;
pub use policy::PolicyCommand;
pub use context_pack::ContextPackCommand;
pub use adapter::AdapterCommand;
pub use prompt::PromptCommand;
pub use learn::LearnCommand;
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
pub struct Cli {
    /// The role you act as: planner, implementer, reviewer or orchestrator. Overrides `CANON_ROLE`. `schema` lists the role's commands, `status` keeps its steps, and `--role` on `evidence add` / `review add` defaults to it
    #[arg(long = "as", global = true, value_enum, value_name = "ROLE")]
    pub declared_role: Option<crate::role::Role>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    // ── Getting started ──
    /// Set up canon in a repo (canon.yaml, a starter policy, the AGENTS.md block, a plans home)
    #[command(after_help = "Examples:\n  canon init\n  canon init --policy light\n  canon init --providers claude,codex\n  canon init --no-agents-md --no-policy\n  canon init --check-config\n\nPolicy levels (--policy, default strict): light requires evidence on every\nscenario; standard adds a `@case:failure` scenario per feature surface; strict\nadds independent review. The level is the first line of .canon/policy.yaml and\n`canon status` shows it.\n\nHarnesses (--providers): on a terminal, init asks which harnesses get the canon\nskill, with the detected ones (.claude, .codex, .omp, .pi directories) checked.\nWithout a terminal or --providers it takes the detected ones; with none detected\nit installs nothing and says how to choose. The answer is recorded as\n`skills: { providers: [...] }` in canon.yaml (`--providers none` records `[]`).\n\nRerunning in a repo that has canon.yaml leaves canon.yaml and .canon/policy.yaml\nalone and only refreshes the canon block in AGENTS.md.")]
    Init(InitArgs),
    /// Bring a repo from an earlier canon up to date: the 0.17 layout (specs, changes, approvals, skills, .gitignore), the 0.23 latest-only ledger (superseded versions to .canon/archive) and 0.23 review pins
    #[command(after_help = "Examples:\n  canon migrate --dry-run\n  canon migrate\n\nPrints the plan, then applies it; --dry-run stops after the plan. Idempotent:\na second run finds nothing to move, and a run that stopped partway is finished\nby the next one.\n\n- A spec root's features/kind=feature/ moves to <root>/kind=feature/ (git mv in a\n  git work tree). A root at the repo-level `specs` moves to .canon/specs, and\n  canon.yaml records `former_roots: [specs]` so reviews pinned before the move\n  still find their scenarios.\n- Changes `canon change new` wrote under openspec/changes move to\n  .canon/changes/<slug>/ (proposal.md -> brief.md, tasks.md alongside); every\n  other change stays and is named. A change counts as canon's when its files are\n  still that command's scaffold and its first stored version was already adopted:\n  0.16 kept no durable provenance. A false positive keeps its content and ids and\n  is one `git mv` to undo; --dry-run shows exactly which changes move.\n  canon init's `{ dialect: openspec, root: . }` source goes once nothing unread is\n  left under it.\n- An external proposal.md's Approved: line is copied into a canon pointer\n  brief, .canon/changes/<change>/brief.md, with a Source: line.\n- skills.providers is written from the install lock; missing canon .gitignore\n  lines are added; the scenario index is rebuilt (full sync).\n- Superseded ledger versions move to .canon/archive (`canon ledger compact`);\n  `canon query --history` still reads them.\n- Reviews written before 0.23 are re-pinned to what their evidence says: a\n  review current under the old rule stays current, a stale one stays stale.\n\nRefuses (exit 2, nothing written) on uncommitted changes under a path it\nmoves, on a destination that already exists, on a change directory that could\nstop halfway, and on a canon.yaml edit it cannot make with every other byte\nkept. canon.yaml is written atomically.")]
    Migrate(MigrateArgs),
    /// Record who approved the gate's policy (.canon/policy.yaml) and which bytes
    #[command(after_help = "Examples:\n  canon policy approve --by alice --note 'reviewed the starter policy'\n\nWrites .canon/policy.approval.json (commit it): the policy's sha256, who\napproved it, when, and the note. `canon gate check` and `canon status` print an\nadvisory while the policy was never approved. A policy that changed since its\napproval (or whose approval cannot be read) fails closed: `gate check` exits 1\nand `subject status` refuses to ship or reopen; `--override-reason` does not\nwaive it. `canon policy show` reads the approval back.")]
    Policy {
        #[command(subcommand)]
        action: PolicyCommand,
    },
    /// Try the evidence loop end-to-end in a throwaway demo repo
    #[command(after_help = "Examples:\n  canon demo init --repo /tmp/canon-demo && cd /tmp/canon-demo\n  canon gate check   # RED\n  canon demo add && canon gate check   # GREEN")]
    Demo {
        #[command(subcommand)]
        action: DemoCommand,
    },
    /// Show where this repo stands: subjects by status, their gaps, and the next commands
    #[command(after_help = "Examples:\n  canon status\n  canon status --json\n\nA read: exits 0 whatever it finds, and writes nothing. The counts come from the\nsame joins `canon gate check` and `canon subject status` use.")]
    Status(StatusArgs),
    /// Show what you can author here: record kinds, fields, enums, policies
    Context(ContextArgs),
    /// Create, inspect, and verify reproducible context packs
    #[command(after_help = "Examples:\n  canon context-pack new --manifest .canon/context-manifest.json\n  canon context-pack show sha256:<pack-digest> --repo . --json\n  canon context-pack verify sha256:<pack-digest>")]
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
        after_help = "Examples:\n  canon prompt add --name reviewer --version v1 --manifest .canon/reviewer.json\n  canon prompt show --name reviewer --version v1 --json"
    )]
    Prompt {
        #[command(subcommand)]
        action: PromptCommand,
    },
    // ── Specs & authoring ──
    /// Validate a spec/artifact corpus against canon's format
    #[command(name = "format", visible_alias = "fmt", after_help = "Examples:\n  canon format spec\n  canon format spec --repo ~/work/myrepo")]
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
    #[command(after_help = "Examples:\n  canon brief show                     State and the next one to three steps\n  canon brief check                    Findings; exit 0 clean, 2 blockers, 1 error\n  canon brief check --change-id add-login --json\n\nThe brief is the PRD `canon change new` writes: sections are found by their\n`<!-- canon:section <key> role=… -->` marker, never by heading text. Blockers\n(structure) are a missing or out-of-order section, a duplicate flow id, step\ndetail that does not match the flow ids, leftover placeholders, and, once the\nsubject is building, a flow step with no scenario or no failure scenario.\nAdvisories (prose) apply to normative sections, Intent and Questions, in the\nlanguage each is written in (`language:` in canon.yaml overrides); only en and\nko have rule lists.\nWaive one rule in one section with\n  <!-- canon:waive <rule-id> <section-key> \"<reason>\" -->\nA brief without markers (written before 0.19) checks as `brief.legacy`, an\nadvisory. Without --change-id, the repo's only live change is checked.")]
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
    /// Run evidence gates, flip task checkboxes, install hooks
    #[command(after_help = "Examples:\n  canon gate check\n  canon gate task my-change#3\n  canon gate install-hooks")]
    Gate {
        #[command(subcommand)]
        action: GateCommand,
    },
    /// Author a staged, attributed evidence attestation for a plan task
    #[command(after_help = "Examples:\n  canon evidence add --task my-change#3.1 --kind test-run --ref 'cargo test -p canon-cli' --role implementer\n\nThe loop:\n  canon evidence add ...    Record an EvidenceRecord (promoted in the same run under\n                            `write: { promote: auto }`, which `canon init` writes)\n  canon gate task <id>      Flip the checkbox on it\nUnder `write.promote: manual` (no `write:` key), run `canon gate promote` after\neach add to commit it (assigns run_seq).\n\nSee `canon evidence add --help` for exactly what the gate does and does not check.")]
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
    #[command(after_help = "Examples:\n  canon finding add --change-id s43-findings-are-records --round 1 --seq 1 \\\n      --severity blocker --disposition open --reviewer review-voice \\\n      --summary 'a release metric is asserted, never computed'\n  canon finding close --change-id s43-findings-are-records --round 1 --seq 1 \\\n      --disposition fixed --resolution-sha $(git rev-parse HEAD)\n\nThe loop:\n  canon finding add ...     Record a Finding\n  canon finding close ...   Record its disposition TRANSITION, once fixed\nEach is promoted in the same run under `write: { promote: auto }`, which\n`canon init` writes. Under `write.promote: manual` (no `write:` key), run\n`canon gate promote` after each to commit it.\n\nThe committed finding STAYS when it is closed: the ledger is append-only and\nthe pair IS the history. Every reader folds by natural key to the latest\nversion, so the finding is still counted once.\n\nRECORDED OBSERVATION, NOT PROOF — canon never verifies that the defect\nexisted, that it was fixed, or that --introduced-by is the true cause.\nSee `canon finding add --help` for exactly what it does and does not establish.")]
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
    Retrieve(RetrieveArgs),
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
    RegimeKey(RegimeKeyArgs),
    // ── Reading & reporting ──
    /// Read stored records across every storage tier
    Query(QueryArgs),
    /// Export a redacted retention manifest without deleting records
    Export(ExportArgs),
    /// Remove allowlisted records older than a cutoff: `--plan` shows the removal set and its planDigest, `--confirm <planDigest>` deletes it
    Purge(PurgeArgs),
    /// Generate the status report (write, --check, or --snapshot)
    Report(ReportArgs),
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
    #[command(after_help = "Examples:\n  canon ledger compact --dry-run\n  canon ledger compact\n\nMoves every superseded version of a record (an evidence record a newer one replaced, a\nscenario or finding rewritten since) from .canon/ledger to .canon/archive, once. The\nledger then holds the current decision per key, and every default read (status, gate\ncheck, guards, query) reads only that; `canon query --kind <kind> --history` reads the\narchive too. Appending a new version moves the versions it supersedes on its own, so\nthis is for a ledger written before 0.23, or after an interrupted move. Idempotent;\n`canon migrate` runs it. A record that carries a standing claim (a flagged overlay, a\nbrief override, an approval at the current evidence) stays in the ledger.")]
    Ledger {
        #[command(subcommand)]
        action: LedgerCommand,
    },
    /// Run canon's built-in fixture self-tests
    Selftest(SelftestArgs),
    /// Describe canon's own commands: list them, or show one command's arguments, options, effect and exit codes
    #[command(after_help = "Examples:\n  canon schema\n  canon schema --all --json\n  canon schema evidence add --json\n  CANON_ROLE=reviewer canon schema --json\n  canon schema --errors --json\n\nGenerated from the command tree itself, so it never drifts from what the parser accepts.\nTouches no repo and no network: it works from any directory. An alias (`fmt`) resolves to its\ncanonical command and the output names the canonical one. An unknown path exits 2 and names the\nnearest matches.\n\nWith a declared role (CANON_ROLE or --as) the bare listing shows only that role's commands; --all\nlists every command. A command whose effect depends on its arguments carries `effectNote`.")]
    Schema(SchemaArgs),
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
