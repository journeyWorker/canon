//! `demote_strategy`'s real persistence body (S7 design D4, task group
//! 4) — WIDENED from S7Core's original frozen 2-arg stub
//! (`demote_strategy(strategy_id, contradicting_trajectory_id) ->
//! Result<DemotionRecord, LearnError>`) per a ReviewS7Core finding: the
//! original signature had no way to reach the persistence context (a
//! `StrategyStore`, the git-tier root, the demotion policy) its own doc
//! comment already described as its job. The stub had zero real
//! callers (S7Core's own doc: "nothing in this crate calls it"), so
//! widening it is a strict extension of an unused seam, never a
//! breaking change to a real caller — [`tests`]'s own updated
//! placeholder test exercises the new shape.
//!
//! [`demote_strategy`] does two independent things, matching design
//! D4's own two-part contract:
//! 1. **Durable evidence** — looks the strategy up via
//!    [`StrategyStore::find_by_id`], builds a
//!    [`crate::strategy::DemotionEvidence`] (S1-envelope-shaped), and
//!    persists it via [`StrategyStore::mark_demoted`] — the operator-
//!    local parquet warm tier, durable the moment the write returns
//!    (`store/mod.rs`'s own "no separate Layer-composition step to
//!    forget" property applied here).
//! 2. **Git-tier file update** — soft-flags (`status: demoted`
//!    front-matter, the default) or hard-deletes
//!    (`<git_tier_root>/<role>/<strategy_id>.md`) per [`DemotionPolicy`],
//!    ONLY when that file actually exists. `canon learn promote`, the
//!    writer that would have created this file in the first place, is
//!    still unbuilt (`canon-cli` territory — `lib.rs` module doc); a
//!    strategy demoted before ever being promoted to the git tier has
//!    nothing to soft-flag, which is not an error.

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde_yaml::Value;

use crate::error::LearnError;
use crate::ids::{StrategyId, TrajectoryId};
use crate::store::StrategyStore;
use crate::strategy::DemotionEvidence;

use super::{git_tier_path, DemotionRecord};

/// `demote_strategy`'s git-tier file policy (S7 design D4): soft-flag
/// (the default — `hard_delete: false`) leaves the file in place with
/// `status: demoted` + `reason` front-matter merged in; hard-delete
/// removes it outright. Mirrors [`crate::config::DemotionConfig`]
/// field-for-field — a caller resolving a real repo's `canon.yaml`
/// passes `config.demotion.hard_delete` straight through
/// ([`DemotionPolicy::from_hard_delete`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DemotionPolicy {
    pub hard_delete: bool,
}

impl DemotionPolicy {
    pub const SOFT_FLAG: Self = Self { hard_delete: false };
    pub const HARD_DELETE: Self = Self { hard_delete: true };

    pub fn from_hard_delete(hard_delete: bool) -> Self {
        Self { hard_delete }
    }
}

/// Demotes a promoted strategy that later collected a contradicting
/// trajectory (design D4). WIDENED signature (S7 wave-2, task 4.1 —
/// see module doc): `strategy_store`/`git_tier_root`/`policy` supply
/// the persistence context S7Core's original 2-arg stub had no way to
/// reach.
///
/// # Errors
/// [`LearnError::UnknownStrategyId`] when `strategy_id` matches no
/// stored [`crate::strategy::StrategyItem`] — never a silent no-op,
/// the same "fail loud on an unmatched id" discipline
/// `mark_trajectory_verdict` already established for trajectories.
pub fn demote_strategy(
    strategy_store: &dyn StrategyStore,
    strategy_id: StrategyId,
    contradicting_trajectory_id: TrajectoryId,
    git_tier_root: &Path,
    policy: DemotionPolicy,
) -> Result<DemotionRecord, LearnError> {
    let item = strategy_store.find_by_id(&strategy_id)?.ok_or_else(|| LearnError::UnknownStrategyId(strategy_id.to_string()))?;
    let demoted_at = Utc::now();
    let reason = format!("contradicting trajectory {contradicting_trajectory_id} arrived for regime {}", item.regime_key.as_str());

    let evidence = DemotionEvidence::new(contradicting_trajectory_id, reason.clone(), demoted_at);
    strategy_store.mark_demoted(&strategy_id, evidence)?;

    apply_git_tier_policy(git_tier_root, item.role.as_str(), strategy_id, &reason, policy)?;

    Ok(DemotionRecord { strategy_id, contradicting_trajectory_id, demoted_at })
}

/// Applies `policy` to the git-tier file at `<git_tier_root>/<role>/
/// <strategy_id>.md`, IF it exists — "for git-tier-promoted
/// strategies" is design D4's own scoping (see module doc).
fn apply_git_tier_policy(git_tier_root: &Path, role: &str, strategy_id: StrategyId, reason: &str, policy: DemotionPolicy) -> Result<(), LearnError> {
    let path = git_tier_path(git_tier_root, role, strategy_id);
    if !path.exists() {
        return Ok(());
    }
    if policy.hard_delete {
        fs::remove_file(&path)?;
        return Ok(());
    }
    soft_flag_front_matter(&path, reason)
}

/// Splits `content` into its `---`-delimited YAML front matter and the
/// remaining body. Errors (never panics or silently truncates) when
/// the file does not open with a well-formed front-matter block — a
/// git-tier strategy file with no front matter is malformed, the same
/// "malformed evidence is no evidence" posture the rest of this crate
/// holds.
fn split_front_matter(content: &str) -> Result<(&str, &str), LearnError> {
    let rest = content
        .strip_prefix("---\n")
        .ok_or_else(|| LearnError::MalformedRow("git-tier strategy file missing `---` front-matter opener".to_string()))?;
    rest.split_once("\n---\n").ok_or_else(|| LearnError::MalformedRow("git-tier strategy file missing `---` front-matter closer".to_string()))
}

/// Rewrites the git-tier file's front matter with `status: demoted` +
/// `reason: <reason>` merged in — every OTHER front-matter key/value
/// (title, description, provenance, …) and the body content are left
/// byte-unchanged, matching §7's "corrections are new records; nothing
/// force-rewritten" append-only discipline applied to the git tier (a
/// NEW commit demotes; the file's own history stays intact through
/// normal git blame/log on the surviving lines).
fn soft_flag_front_matter(path: &Path, reason: &str) -> Result<(), LearnError> {
    let content = fs::read_to_string(path)?;
    let (front_matter, body) = split_front_matter(&content)?;
    let mut doc: Value = serde_yaml::from_str(front_matter).map_err(|e| LearnError::MalformedRow(e.to_string()))?;
    let Value::Mapping(map) = &mut doc else {
        return Err(LearnError::MalformedRow("git-tier strategy file front-matter is not a YAML mapping".to_string()));
    };
    map.insert(Value::String("status".to_string()), Value::String("demoted".to_string()));
    map.insert(Value::String("reason".to_string()), Value::String(reason.to_string()));
    let new_front = serde_yaml::to_string(&doc).map_err(|e| LearnError::MalformedRow(e.to_string()))?;
    fs::write(path, format!("---\n{new_front}---\n{body}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use canon_model::ids::{RegimeKey, RoleId, regime_key};
    use chrono::Utc;

    use super::*;
    use crate::store::ParquetStrategyStore;
    use crate::strategy::StrategyItem;

    fn regime() -> RegimeKey {
        RegimeKey::parse(regime_key("dev", "repo", "auth-flow", "deadbeef")).unwrap()
    }

    fn strategy_item() -> StrategyItem {
        StrategyItem::new(
            StrategyId::new(),
            regime(),
            RoleId::parse("dev").unwrap(),
            "title",
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        )
    }

    #[test]
    fn demote_strategy_constructs_a_record_carrying_both_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy_item();
        store.append(&item).unwrap();
        let trajectory_id = TrajectoryId::new();

        let record = demote_strategy(&store, item.id, trajectory_id, dir.path(), DemotionPolicy::default()).unwrap();
        assert_eq!(record.strategy_id, item.id);
        assert_eq!(record.contradicting_trajectory_id, trajectory_id);
    }

    #[test]
    fn demote_strategy_persists_demotion_evidence_durably() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy_item();
        store.append(&item).unwrap();
        let trajectory_id = TrajectoryId::new();

        demote_strategy(&store, item.id, trajectory_id, dir.path(), DemotionPolicy::default()).unwrap();

        let reloaded = store.find_by_id(&item.id).unwrap().unwrap();
        let evidence = reloaded.demotion.unwrap();
        assert_eq!(evidence.contradicting_trajectory_id, trajectory_id);
    }

    #[test]
    fn demote_strategy_on_an_unknown_id_is_an_error_not_a_silent_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let err = demote_strategy(&store, StrategyId::new(), TrajectoryId::new(), dir.path(), DemotionPolicy::default()).unwrap_err();
        assert!(matches!(err, LearnError::UnknownStrategyId(_)));
    }

    #[test]
    fn a_strategy_never_promoted_to_the_git_tier_has_nothing_to_soft_flag() {
        // No file exists under git_tier_root — demote_strategy still
        // succeeds (durable evidence lands regardless).
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy_item();
        store.append(&item).unwrap();

        let record =
            demote_strategy(&store, item.id, TrajectoryId::new(), &dir.path().join("strategies-git-tier"), DemotionPolicy::default()).unwrap();
        assert_eq!(record.strategy_id, item.id);
    }

    #[test]
    fn default_policy_soft_flags_an_existing_git_tier_file_leaving_other_front_matter_intact() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy_item();
        store.append(&item).unwrap();

        let git_tier_root = dir.path().join("git-strategies");
        let role_dir = git_tier_root.join("dev");
        fs::create_dir_all(&role_dir).unwrap();
        let file_path = role_dir.join(format!("{}.md", item.id));
        fs::write(&file_path, "---\ntitle: title\ndescription: description\n---\ncontent body\n").unwrap();

        demote_strategy(&store, item.id, TrajectoryId::new(), &git_tier_root, DemotionPolicy::default()).unwrap();

        let updated = fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("status: demoted"));
        assert!(updated.contains("title: title"));
        assert!(updated.contains("content body"));
    }

    #[test]
    fn hard_delete_policy_removes_the_git_tier_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let item = strategy_item();
        store.append(&item).unwrap();

        let git_tier_root = dir.path().join("git-strategies");
        let role_dir = git_tier_root.join("dev");
        fs::create_dir_all(&role_dir).unwrap();
        let file_path = role_dir.join(format!("{}.md", item.id));
        fs::write(&file_path, "---\ntitle: title\n---\ncontent\n").unwrap();

        demote_strategy(&store, item.id, TrajectoryId::new(), &git_tier_root, DemotionPolicy::HARD_DELETE).unwrap();

        assert!(!file_path.exists());
    }
}
