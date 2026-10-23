//! `canon demo` (first-win onboarding): a self-contained, throwaway
//! evidence-loop demonstration. `init` scaffolds a real demo repo (the
//! same `canon.yaml` skeleton `canon init` writes) plus a `policy.yaml`
//! requiring an independent `reviewer`, and seeds ONE dev-authored
//! `EvidenceRecord` for scenario `auth.login.01` — so `canon gate check`
//! is RED (`uncovered-cell auth.login.01`, the reviewer cell has no
//! matching-role record). `attest` records the reviewer's evidence,
//! turning the SAME gate GREEN.
//!
//! The point is the flip, not the config: a newcomer sees a red gate go
//! green after evidence lands, before learning what a scenario, review
//! record, or regime key is. Everything it writes is ordinary canon —
//! the real `canon init` config, real `EvidenceRecord`s in the git-backed
//! `local` ledger, the real `canon gate check` reading them.

use std::path::{Path, PathBuf};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, RecordKind, RoleId, ScenarioId};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;

/// The one scenario the demo's evidence loop is keyed to — `<area>.
/// <surface>.<nn>`, the same shape `canon scenario new` accepts.
const DEMO_SCENARIO: &str = "auth.login.01";

/// The demo's `policy.yaml`: every cell that carries evidence requires an
/// independent `reviewer`-role record. `demo init` seeds only the `dev`
/// record, so the reviewer cell is uncovered until `demo review`.
const DEMO_POLICY_YAML: &str = "\
# canon demo policy (.canon/policy.yaml): every evidenced cell needs an
# independent `reviewer`-role evidence record. `canon demo init` seeds
# only the dev record, so `canon gate check` is RED until `canon demo
# attest` records the reviewer's evidence.
risk_routing:
  reviewer: true
";

/// The git-backed `local` ledger root the demo writes evidence to and
/// `canon gate check` reads it from — the `tiers.local.root` the scaffolded
/// `canon.yaml` declares (`.canon/ledger`), which is also `canon gate`'s
/// own default when no override is present.
fn ledger_root(repo: &Path) -> PathBuf {
    repo.join(".canon").join("ledger")
}

/// Write one `EvidenceRecord` for [`DEMO_SCENARIO`], authored by `agent`
/// in role `role`, directly through the committed git tier — the same
/// path `canon review add` uses for a simple attestation.
fn write_evidence(repo: &Path, agent: &str, role: &str) -> Result<(), String> {
    let scenario = ScenarioId::parse(DEMO_SCENARIO).map_err(|e| e.to_string())?;
    let role_id = RoleId::parse(role).map_err(|e| e.to_string())?;
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new(agent, role_id));
    let record = EvidenceRecord::new(envelope, None, Some(scenario), None, EvidenceVerdict::Faithful);
    GitTier::new(ledger_root(repo)).write(&record).map_err(|e| e.to_string())?;
    Ok(())
}

/// `canon demo init [--repo <dir>]`: scaffold the throwaway demo repo. Reuses
/// `canon init` for the real `canon.yaml` (so it refuses to overwrite an
/// existing one — exit `2`), then writes the reviewer-requiring
/// `policy.yaml` and seeds the dev evidence. Exit `0` on success, `2` on
/// any refusal/IO failure.
pub fn run_demo_init(repo: &Path, json: bool) -> i32 {
    // Real `canon init`: creates the dir, writes `canon.yaml`, scaffolds
    // `.gitignore`. Refuses (non-zero) if a `canon.yaml` already exists:
    // with no AGENTS.md block to refresh, a rerun has nothing to do. The
    // demo writes its own policy below, so init's starter policy is off,
    // and a sandbox gets no canon skill, so the harness choice is skipped.
    let code = crate::init::run_init_in(repo, crate::init::InitOptions { agents_md: false, policy: None, skills: crate::init::SkillsChoice::Skip, language: None, dry_run: false }, json, false);
    if code != 0 {
        return code;
    }

    let policy_path = repo.join(".canon").join("policy.yaml");
    if let Some(parent) = policy_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", parent.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon demo init", json);
        }
    }
    if let Err(e) = std::fs::write(&policy_path, DEMO_POLICY_YAML) {
        return CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", policy_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon demo init", json);
    }

    if let Err(e) = write_evidence(repo, "dev-agent", "dev") {
        return CliError::refused("WRITE_FAILED", format!("failed to seed dev evidence: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon demo init", json);
    }

    crate::say!(json, "canon demo: scaffolded a throwaway evidence loop in `{}`.", repo.display());
    crate::say!(json, "");
    crate::say!(json, "  The dev voice recorded evidence for scenario `{DEMO_SCENARIO}`,");
    crate::say!(json, "  but policy requires an independent reviewer — so the gate is red.");
    crate::say!(json, "");
    crate::say!(json, "  Watch it flip:");
    crate::say!(json, "");
    crate::say!(json, "      canon gate check      # RED:   uncovered-cell {DEMO_SCENARIO}");
    crate::say!(json, "      canon demo review     # record the reviewer's evidence");
    crate::say!(json, "      canon gate check      # GREEN: clean");
    if json {
        crate::json_out::data(
            "demo init",
            &crate::json_out::untyped(serde_json::json!({ "repo": repo.display().to_string(), "scenario": DEMO_SCENARIO, "next": ["canon gate check", "canon demo review", "canon gate check"] })));
    }
    0
}

/// `canon demo review [--repo <dir>]`: record the missing reviewer evidence
/// for [`DEMO_SCENARIO`], clearing the `uncovered-cell` the seeded dev
/// evidence left. Resolves the repo root the same nearest-ancestor way
/// every other subcommand does. Exit `0` on success, `2` when no demo
/// scaffold is present.
pub fn run_demo_review(repo: &Path, json: bool) -> i32 {
    let root = resolve_repo_root(repo);
    if !root.join("canon.yaml").exists() {
        return CliError::refused("DEMO_NOT_INITIALIZED", format!("no `canon.yaml` found under `{}` — run `canon demo init` first", root.display())).hint("run `canon demo init` first, or pass --repo <the demo directory>").emit("canon demo review", json);
    }

    if let Err(e) = write_evidence(&root, "reviewer-agent", "reviewer") {
        return CliError::refused("WRITE_FAILED", format!("failed to record reviewer evidence: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon demo review", json);
    }

    crate::say!(json, "canon demo: recorded the reviewer's evidence for scenario `{DEMO_SCENARIO}`.");
    crate::say!(json, "");
    crate::say!(json, "  Re-run the gate — the uncovered cell is now covered:");
    crate::say!(json, "");
    crate::say!(json, "      canon gate check      # GREEN: clean");
    if json {
        crate::json_out::data("demo review", &crate::json_out::untyped(serde_json::json!({ "scenario": DEMO_SCENARIO, "next": ["canon gate check"] })));
    }
    0
}
