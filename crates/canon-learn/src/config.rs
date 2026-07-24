//! `canon.yaml`'s `learn:` section (this crate's own narrow slice of
//! the shared config file, mirroring `canon-model::handoff`'s
//! `TemplateRegistry::from_manifest` and `canon-store::policy`'s
//! `TierPolicy::parse` — every crate parses only the top-level key(s)
//! it owns and ignores every other section via `#[serde(default)]`,
//! never `deny_unknown_fields`).
//!
//! Wiring this to a REAL repo's `canon.yaml` (resolving the repo root,
//! reading the file, handing its text to [`LearnConfig::from_manifest`])
//! is `canon-cli`'s job (mirrors `canon-cli::tiers`'s "builds the live
//! tier handles a repo's `canon.yaml` configures" role for
//! `canon-store`) — out of this change's scope; this module owns only
//! the parse + the typed, validated result.
//!
//! **S7 task 3.2 reconciliation**: the design doc's own text
//! (`openspec/changes/s7-reward-statistical-promotion/design.md`
//! decision 3) says `promotion.<role>.mode: crn | occurrence` lives in
//! `policy.yaml` — this module puts it in `canon.yaml`'s `learn:`
//! section instead ([`LearnConfig::promotion`]). `policy.yaml` is
//! `canon-policy`'s own crate (`crates/canon-policy`), read by
//! `canon-gate`'s trust ladder for risk-routing/trust-required/
//! staleness CEL predicates — a promotion-gate mode selection has no
//! `canon-gate`/`canon-policy` reader anywhere, so routing it through
//! `policy.yaml` would make THIS crate depend on `canon-policy` (or
//! `canon-gate`) just to read its own config, the exact cross-crate
//! coupling this doc's own "every crate parses only the top-level
//! key(s) it owns" convention (first paragraph above) exists to avoid.
//! `demote_strategy`'s git-tier policy (soft-flag vs hard-delete,
//! [`LearnConfig::demotion`]) is reconciled the same way.

use std::collections::BTreeMap;
use std::path::PathBuf;

use canon_model::ids::RoleId;
use serde::Deserialize;

use crate::error::LearnError;

/// Operator-local files land under `<repo_root>/.canon/learn` by
/// default when `canon.yaml` carries no `learn:` section at all
/// (local-first: `canon-learn` works with zero config, matching
/// `canon-store`'s "an unconfigured tier is never attempted" ethos
/// applied here to "a default IS attempted, just at a sane default
/// path" — trajectory/strategy storage is this crate's core function,
/// not an opt-in tier).
pub const DEFAULT_LEARN_ROOT: &str = canon_model::paths::LEARN_DIR;

/// The git tier a promoted [`crate::strategy::StrategyItem`] lands
/// under (S6 design decision 4: `.canon/strategies/<role>/<id>.md`) —
/// also where [`crate::promotion::demote_strategy`] soft-flags/
/// hard-deletes a demoted strategy's file. Distinct from
/// [`DEFAULT_LEARN_ROOT`]: that is the operator-local PARQUET warm/cold
/// tier; this is the git-tracked, PR-reviewed tier.
pub const DEFAULT_STRATEGIES_ROOT: &str = canon_model::paths::STRATEGIES_DIR;

/// [`PromotionRoleConfig::n_min`]'s conservative default (S7 design D3
/// risk section: "ship conservative defaults... documented as
/// provisional").
pub const DEFAULT_N_MIN: u32 = 5;

/// [`PromotionRoleConfig::window_days`]'s conservative default (S7
/// design D3 risk section: "window: 30 days").
pub const DEFAULT_WINDOW_DAYS: i64 = 30;

#[derive(Debug, Clone, Default, Deserialize)]
struct LearnManifest {
    #[serde(default)]
    learn: Option<LearnSectionRaw>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LearnSectionRaw {
    #[serde(default)]
    root: Option<PathBuf>,
    #[serde(default)]
    roles: Vec<String>,
    /// `promotion.<role>.mode: crn | occurrence` (+ `n_min`/
    /// `window_days` for `occurrence`) — S7 task 3.2, reconciled into
    /// `learn:` (see module doc).
    #[serde(default)]
    promotion: BTreeMap<String, PromotionRoleConfigRaw>,
    /// `demotion.hard_delete` / `demotion.strategies_root` — S7 task
    /// 4.1's `demote_strategy` git-tier policy, same reconciliation.
    #[serde(default)]
    demotion: DemotionConfigRaw,
}

/// `promotion.<role>.mode` — S7 design D3: `crn` for roles whose
/// domain supports deterministic CRN replay
/// ([`crate::promotion::CrnPromotionGate`]), `occurrence` for every
/// other role ([`crate::promotion::OccurrencePromotionGate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromotionMode {
    Crn,
    Occurrence,
}

#[derive(Debug, Clone, Deserialize)]
struct PromotionRoleConfigRaw {
    mode: PromotionMode,
    #[serde(default = "default_n_min")]
    n_min: u32,
    #[serde(default = "default_window_days")]
    window_days: i64,
}

fn default_n_min() -> u32 {
    DEFAULT_N_MIN
}

fn default_window_days() -> i64 {
    DEFAULT_WINDOW_DAYS
}

/// A role's parsed `promotion.<role>` entry (or the conservative
/// occurrence-gate default when a role has no explicit entry —
/// [`LearnConfig::promotion_config_for`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromotionRoleConfig {
    pub mode: PromotionMode,
    /// `OccurrencePromotionGate::n_min` — ignored for `mode: crn` roles
    /// (the CRN gate has no `n_min` knob of its own; see `crn.rs`'s
    /// `MIN_PANELS_FOR_SIGNIFICANCE`/`MIN_DF_RESIDUAL` for its own,
    /// separately-fixed floors).
    pub n_min: u32,
    /// `OccurrencePromotionGate::window`, in days.
    pub window_days: i64,
}

impl PromotionRoleConfig {
    /// The conservative default EVERY unconfigured role gets: occurrence
    /// mode, `n_min: 5`, a 30-day window (S7 design D3 risk section) —
    /// occurrence, not CRN, because CRN requires a deterministic
    /// simulator most roles don't have (design D3: "the n-occurrence
    /// fallback is the permanent answer for non-replayable domains").
    pub fn default_occurrence() -> Self {
        Self { mode: PromotionMode::Occurrence, n_min: DEFAULT_N_MIN, window_days: DEFAULT_WINDOW_DAYS }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DemotionConfigRaw {
    #[serde(default)]
    hard_delete: bool,
    #[serde(default)]
    strategies_root: Option<PathBuf>,
}

/// `demote_strategy`'s git-tier file policy (S7 design D4): soft-flag
/// (`status: demoted` front-matter, the default) or hard-delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DemotionConfig {
    pub hard_delete: bool,
}

/// A parsed, validated `canon.yaml` `learn:` section.
#[derive(Debug, Clone)]
pub struct LearnConfig {
    /// The operator-local directory the parquet stores read/write
    /// under (relative to the repo root; resolving that root is the
    /// caller's job, same convention as `GitTierConfig::root`).
    pub root: PathBuf,
    /// Additional roles this repo registers beyond
    /// [`crate::role::BUILTIN_ROLES`] (design decision 1's
    /// `canon.yaml`-extensible role set).
    pub extra_roles: Vec<RoleId>,
    /// The git tier a promoted strategy's `<role>/<id>.md` file lands
    /// under, and where [`crate::promotion::demote_strategy`]
    /// soft-flags/hard-deletes it (S6 design decision 4, S7 design D4).
    pub strategies_root: PathBuf,
    /// `promotion.<role>.mode`, per role (S7 design D3 / task 3.2 —
    /// reconciled into `learn:`, see module doc). A role with no entry
    /// here gets [`PromotionRoleConfig::default_occurrence`], never a
    /// missing-config error — [`LearnConfig::promotion_config_for`].
    pub promotion: BTreeMap<RoleId, PromotionRoleConfig>,
    /// `demote_strategy`'s git-tier policy (S7 design D4 / task 4.1).
    pub demotion: DemotionConfig,
}

impl Default for LearnConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from(DEFAULT_LEARN_ROOT),
            extra_roles: Vec::new(),
            strategies_root: PathBuf::from(DEFAULT_STRATEGIES_ROOT),
            promotion: BTreeMap::new(),
            demotion: DemotionConfig::default(),
        }
    }
}

impl LearnConfig {
    /// Parse `canon.yaml`'s content, narrowed to the `learn:` key this
    /// crate owns. A missing `learn:` section (or an empty `canon.yaml`)
    /// is not an error — it resolves to [`LearnConfig::default`].
    /// A `roles:` entry, or a `promotion:` key, that is not a valid
    /// kebab-slug `RoleId` fails loud (never silently dropped).
    pub fn from_manifest(canon_yaml: &str) -> Result<Self, LearnError> {
        let manifest: LearnManifest = serde_yaml::from_str(canon_yaml).map_err(|e| LearnError::Config(e.to_string()))?;
        let Some(section) = manifest.learn else {
            return Ok(Self::default());
        };
        let root = section.root.unwrap_or_else(|| PathBuf::from(DEFAULT_LEARN_ROOT));
        let extra_roles =
            section.roles.into_iter().map(|s| RoleId::parse(s).map_err(LearnError::from)).collect::<Result<Vec<_>, _>>()?;
        let promotion = section
            .promotion
            .into_iter()
            .map(|(role, raw)| {
                let role = RoleId::parse(role).map_err(LearnError::from)?;
                if raw.n_min == 0 {
                    return Err(LearnError::Config(format!("promotion.{role}.n_min must be positive, got 0")));
                }
                if raw.window_days <= 0 {
                    return Err(LearnError::Config(format!(
                        "promotion.{role}.window_days must be positive, got {}",
                        raw.window_days
                    )));
                }
                Ok((role, PromotionRoleConfig { mode: raw.mode, n_min: raw.n_min, window_days: raw.window_days }))
            })
            .collect::<Result<BTreeMap<_, _>, LearnError>>()?;
        let strategies_root = section.demotion.strategies_root.unwrap_or_else(|| PathBuf::from(DEFAULT_STRATEGIES_ROOT));
        let demotion = DemotionConfig { hard_delete: section.demotion.hard_delete };
        Ok(Self { root, extra_roles, strategies_root, promotion, demotion })
    }

    /// A role's effective promotion-gate config: its explicit
    /// `promotion.<role>` entry, or the conservative occurrence default
    /// ([`PromotionRoleConfig::default_occurrence`]) when unset — never
    /// a missing-config error, mirroring [`crate::role::RoleRegistry`]'s
    /// own "a repo works with zero config" ethos applied to promotion.
    pub fn promotion_config_for(&self, role: &RoleId) -> PromotionRoleConfig {
        self.promotion.get(role).copied().unwrap_or_else(PromotionRoleConfig::default_occurrence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_falls_back_to_defaults() {
        let config = LearnConfig::from_manifest("").unwrap();
        assert_eq!(config.root, PathBuf::from(DEFAULT_LEARN_ROOT));
        assert!(config.extra_roles.is_empty());
        assert_eq!(config.strategies_root, PathBuf::from(DEFAULT_STRATEGIES_ROOT));
        assert!(config.promotion.is_empty());
        assert_eq!(config.demotion, DemotionConfig::default());
        assert!(!config.demotion.hard_delete);
    }

    #[test]
    fn manifest_with_other_sections_only_reads_learn() {
        let yaml = "tiers:\n  git: { root: .canon/ledger }\n";
        let config = LearnConfig::from_manifest(yaml).unwrap();
        assert_eq!(config.root, PathBuf::from(DEFAULT_LEARN_ROOT));
    }

    #[test]
    fn explicit_root_and_roles_are_parsed() {
        let yaml = "learn:\n  root: .canon/learn-custom\n  roles:\n    - triage\n    - ops\n";
        let config = LearnConfig::from_manifest(yaml).unwrap();
        assert_eq!(config.root, PathBuf::from(".canon/learn-custom"));
        assert_eq!(config.extra_roles, vec![RoleId::parse("triage").unwrap(), RoleId::parse("ops").unwrap()]);
    }

    #[test]
    fn a_malformed_role_slug_fails_loud() {
        let yaml = "learn:\n  roles:\n    - \"Not Valid!\"\n";
        assert!(LearnConfig::from_manifest(yaml).is_err());
    }

    #[test]
    fn a_role_with_no_promotion_entry_gets_the_conservative_occurrence_default() {
        let config = LearnConfig::from_manifest("").unwrap();
        let dev = RoleId::parse("dev").unwrap();
        assert_eq!(config.promotion_config_for(&dev), PromotionRoleConfig::default_occurrence());
        assert_eq!(config.promotion_config_for(&dev).mode, PromotionMode::Occurrence);
        assert_eq!(config.promotion_config_for(&dev).n_min, DEFAULT_N_MIN);
        assert_eq!(config.promotion_config_for(&dev).window_days, DEFAULT_WINDOW_DAYS);
    }

    #[test]
    fn explicit_promotion_entries_parse_mode_and_occurrence_fields() {
        let yaml = "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      n_min: 8\n      window_days: 14\n    sim:\n      mode: crn\n";
        let config = LearnConfig::from_manifest(yaml).unwrap();
        let dev = RoleId::parse("dev").unwrap();
        let sim = RoleId::parse("sim").unwrap();
        assert_eq!(config.promotion_config_for(&dev), PromotionRoleConfig { mode: PromotionMode::Occurrence, n_min: 8, window_days: 14 });
        // A `crn` role's `n_min`/`window_days` fall back to the
        // conservative defaults when omitted (unused by the CRN gate,
        // but still a well-formed value, never left uninitialized).
        assert_eq!(config.promotion_config_for(&sim).mode, PromotionMode::Crn);
        assert_eq!(config.promotion_config_for(&sim).n_min, DEFAULT_N_MIN);
    }

    #[test]
    fn a_malformed_promotion_role_slug_fails_loud() {
        let yaml = "learn:\n  promotion:\n    \"Not Valid!\":\n      mode: occurrence\n";
        assert!(LearnConfig::from_manifest(yaml).is_err());
    }

    #[test]
    fn promotion_n_min_zero_is_rejected_at_parse() {
        // n_min: 0 would let `OccurrencePromotionGate` promote with
        // zero corroborating successes (a streak count is always
        // `>= 0`), defeating the n-occurrence gate entirely.
        let yaml = "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      n_min: 0\n";
        let err = LearnConfig::from_manifest(yaml).unwrap_err();
        assert!(matches!(err, LearnError::Config(_)));
    }

    #[test]
    fn promotion_window_days_nonpositive_is_rejected_at_parse() {
        // A nonpositive window_days is accepted here today even though
        // the webhook timer's own config already rejects a nonpositive
        // window — this closes that inconsistency.
        let yaml = "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      window_days: 0\n";
        assert!(matches!(LearnConfig::from_manifest(yaml).unwrap_err(), LearnError::Config(_)));
        let negative = "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      window_days: -1\n";
        assert!(matches!(LearnConfig::from_manifest(negative).unwrap_err(), LearnError::Config(_)));
    }

    #[test]
    fn a_valid_positive_occurrence_config_still_parses() {
        let yaml = "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      n_min: 3\n      window_days: 7\n";
        let config = LearnConfig::from_manifest(yaml).unwrap();
        let dev = RoleId::parse("dev").unwrap();
        assert_eq!(config.promotion_config_for(&dev), PromotionRoleConfig { mode: PromotionMode::Occurrence, n_min: 3, window_days: 7 });
    }

    #[test]
    fn demotion_policy_and_strategies_root_are_parsed() {
        let yaml = "learn:\n  demotion:\n    hard_delete: true\n    strategies_root: .canon/strategies-custom\n";
        let config = LearnConfig::from_manifest(yaml).unwrap();
        assert!(config.demotion.hard_delete);
        assert_eq!(config.strategies_root, PathBuf::from(".canon/strategies-custom"));
    }
}
