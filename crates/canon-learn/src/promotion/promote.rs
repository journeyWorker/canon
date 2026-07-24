//! `promote_strategy`'s git-tier file WRITER (S6 design decision 4,
//! task group 4) — the counterpart `demote::demote_strategy`'s own
//! module doc named as "still unbuilt": the writer that creates the
//! `<git_tier_root>/<role>/<strategy_id>.md` file a later demotion
//! soft-flags or hard-deletes. Promotion moves a distilled
//! [`StrategyItem`] from the operator-local parquet warm tier up into
//! the git-tracked, PR-reviewed tier (`.canon/strategies/<role>/<id>.md`
//! by default, [`crate::config::DEFAULT_STRATEGIES_ROOT`]).
//!
//! The file is written as a `---`-delimited YAML front-matter block +
//! the strategy's `content` as the body — the EXACT shape
//! [`demote::split_front_matter`] parses and [`demote::
//! soft_flag_front_matter`] merges `status: demoted` into, so a
//! promote-then-demote round-trip is closed (a unit test here proves
//! it). The front matter opens with `status: active`; demotion flips
//! only that key, leaving every provenance field byte-unchanged
//! (append-only §7 discipline).
//!
//! An ADVISORY lint (S6 task 4.2, [`lint_strategy`]) runs at promote
//! time — a content-length ceiling and a literal absolute-path check —
//! surfaced through the crate's established `eprintln!` diagnostic
//! convention by the CLI, NEVER failing the promote (a strategy that
//! trips the lint is still promoted; the operator decides).

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::LearnError;
use crate::ids::StrategyId;
use crate::store::StrategyStore;
use crate::strategy::StrategyItem;

use super::git_tier_path;

/// A content longer than this (chars) trips [`lint_strategy`]'s
/// advisory length warning — a distilled strategy is meant to be a
/// compact insight, not a transcript dump (reasoning-bank distillation
/// abstracts the low-level execution detail away, `strategy.rs`'s own
/// `content` doc).
pub const CONTENT_ADVISORY_CEILING: usize = 2_000;

/// The outcome of a promotion (or a dry-run plan of one): where the
/// git-tier file lands, the advisory lint warnings, and the full
/// rendered file content (so a `--dry-run` caller can show exactly
/// what WOULD be written without touching the filesystem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promotion {
    pub path: PathBuf,
    pub warnings: Vec<String>,
    pub rendered: String,
}

/// Advisory-only lint (S6 task 4.2): a content-length ceiling
/// ([`CONTENT_ADVISORY_CEILING`]), a literal machine-specific
/// absolute-path check (a strategy that hard-codes `/Users/…` or
/// `/home/…` will not generalize across operators), and a
/// promoting-an-already-demoted-item note. Returns human-readable
/// warnings; NEVER blocks a promotion — the caller surfaces these via
/// `eprintln!` and proceeds (mirrors `guidance.rs`'s own fail-soft
/// diagnostic convention).
pub fn lint_strategy(item: &StrategyItem) -> Vec<String> {
    let mut warnings = Vec::new();
    if item.content.chars().count() > CONTENT_ADVISORY_CEILING {
        warnings.push(format!(
            "content is {} chars, above the {CONTENT_ADVISORY_CEILING}-char advisory ceiling — consider distilling it further",
            item.content.chars().count()
        ));
    }
    if let Some(line) = item.content.lines().find(|l| l.contains("/Users/") || l.contains("/home/")) {
        warnings.push(format!(
            "content carries a literal machine-specific absolute path ({:?}) — a promoted strategy should be operator-agnostic",
            line.trim()
        ));
    }
    if item.demotion.is_some() {
        warnings.push("this strategy is already demoted — promoting it re-writes its git-tier file with `status: active`".to_string());
    }
    warnings
}

/// The `---`-delimited front-matter block for `item`, opening with
/// `status: active` — a YAML MAPPING (the shape [`demote::
/// soft_flag_front_matter`] requires), carrying every provenance field
/// so the git-tier file is self-describing and a demotion can flip
/// `status` without losing context.
#[derive(Serialize)]
struct FrontMatter {
    status: &'static str,
    id: String,
    regime_key: String,
    role: String,
    title: String,
    description: String,
    source_trajectory_ids: Vec<String>,
    recorded_at: String,
}

/// Renders `item` into its full git-tier file content: `---\n<yaml>---\n
/// <content>\n`, byte-compatible with [`demote::split_front_matter`].
fn render_strategy_file(item: &StrategyItem) -> Result<String, LearnError> {
    let front = FrontMatter {
        status: "active",
        id: item.id.to_string(),
        regime_key: item.regime_key.as_str().to_string(),
        role: item.role.as_str().to_string(),
        title: item.title.clone(),
        description: item.description.clone(),
        source_trajectory_ids: item.source_trajectory_ids.iter().map(ToString::to_string).collect(),
        recorded_at: item.recorded_at.to_rfc3339(),
    };
    let yaml = serde_yaml::to_string(&front).map_err(|e| LearnError::MalformedRow(e.to_string()))?;
    Ok(format!("---\n{yaml}---\n{}\n", item.content))
}

/// Resolves + renders the promotion for `strategy_id` WITHOUT touching
/// the filesystem — the `--dry-run` path (and the shared prefix of the
/// real write). Fails loud with [`LearnError::UnknownStrategyId`] when
/// nothing matches (never a silent no-op, the same discipline
/// [`demote::demote_strategy`] holds).
pub fn plan_promotion(strategy_store: &dyn StrategyStore, strategy_id: &StrategyId, git_tier_root: &Path) -> Result<Promotion, LearnError> {
    let item = strategy_store.find_by_id(strategy_id)?.ok_or_else(|| LearnError::UnknownStrategyId(strategy_id.to_string()))?;
    let path = git_tier_path(git_tier_root, item.role.as_str(), item.id);
    let rendered = render_strategy_file(&item)?;
    Ok(Promotion { path, warnings: lint_strategy(&item), rendered })
}

/// Promotes a distilled strategy into the git tier: writes
/// `<git_tier_root>/<role>/<strategy_id>.md` (creating the `<role>`
/// directory), returning the [`Promotion`] (path + advisory warnings +
/// rendered content). Idempotent by content — re-promoting an
/// unchanged strategy rewrites the byte-identical file.
pub fn promote_strategy(strategy_store: &dyn StrategyStore, strategy_id: &StrategyId, git_tier_root: &Path) -> Result<Promotion, LearnError> {
    let promotion = plan_promotion(strategy_store, strategy_id, git_tier_root)?;
    if let Some(parent) = promotion.path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&promotion.path, &promotion.rendered)?;
    Ok(promotion)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::tempdir;

    use super::*;
    use crate::ids::TrajectoryId;
    use crate::promotion::{demote_strategy, DemotionPolicy};
    use crate::store::ParquetStrategyStore;
    use canon_model::ids::{regime_key, RegimeKey, RoleId};

    fn seed(store: &ParquetStrategyStore, content: &str) -> StrategyId {
        let id = StrategyId::new();
        let rk = RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
        let item = StrategyItem::new(id, rk, RoleId::parse("dev").unwrap(), "title", "description", content, vec![TrajectoryId::new()], Utc::now());
        store.append(&item).unwrap();
        id
    }

    #[test]
    fn promote_writes_a_role_scoped_git_tier_file_with_active_front_matter() {
        let learn = tempdir().unwrap();
        let git = tempdir().unwrap();
        let store = ParquetStrategyStore::open(learn.path().join("strategies"));
        let id = seed(&store, "always check Option before unwrap");

        let promotion = promote_strategy(&store, &id, git.path()).unwrap();
        assert_eq!(promotion.path, git.path().join("dev").join(format!("{id}.md")));
        assert!(promotion.path.exists(), "promote must write the git-tier file");
        let written = std::fs::read_to_string(&promotion.path).unwrap();
        assert!(written.starts_with("---\n"), "front-matter opener");
        assert!(written.contains("status: active"), "opens active");
        assert!(written.contains("always check Option before unwrap"), "body carries content");
        assert!(promotion.warnings.is_empty(), "a small clean strategy trips no advisory lint");
    }

    #[test]
    fn a_promoted_file_round_trips_through_demote_soft_flag() {
        // The whole point of matching demote's front-matter shape: the
        // file promote writes is exactly the file demote can soft-flag.
        let learn = tempdir().unwrap();
        let git = tempdir().unwrap();
        let store = ParquetStrategyStore::open(learn.path().join("strategies"));
        let id = seed(&store, "prefer the boring option");

        let promotion = promote_strategy(&store, &id, git.path()).unwrap();
        demote_strategy(&store, id, TrajectoryId::new(), git.path(), DemotionPolicy::SOFT_FLAG).unwrap();

        let flagged = std::fs::read_to_string(&promotion.path).unwrap();
        assert!(flagged.contains("status: demoted"), "demote soft-flags the file promote wrote: {flagged}");
        assert!(flagged.contains("prefer the boring option"), "body survives the soft-flag");
    }

    #[test]
    fn lint_flags_a_literal_absolute_path_and_an_oversized_content_without_blocking() {
        let learn = tempdir().unwrap();
        let git = tempdir().unwrap();
        let store = ParquetStrategyStore::open(learn.path().join("strategies"));
        let big = "x".repeat(CONTENT_ADVISORY_CEILING + 1);
        let content = format!("see /Users/someone/notes for context\n{big}");
        let id = seed(&store, &content);

        let promotion = promote_strategy(&store, &id, git.path()).unwrap();
        assert!(promotion.path.exists(), "lint is advisory — the file is still written");
        assert!(promotion.warnings.iter().any(|w| w.contains("absolute path")), "flags the literal path: {:?}", promotion.warnings);
        assert!(promotion.warnings.iter().any(|w| w.contains("advisory ceiling")), "flags the oversized content: {:?}", promotion.warnings);
    }

    #[test]
    fn promote_fails_loud_on_an_unknown_strategy_id() {
        let learn = tempdir().unwrap();
        let git = tempdir().unwrap();
        let store = ParquetStrategyStore::open(learn.path().join("strategies"));
        let err = promote_strategy(&store, &StrategyId::new(), git.path()).unwrap_err();
        assert!(matches!(err, LearnError::UnknownStrategyId(_)), "unknown id must fail loud, got {err:?}");
    }
}
