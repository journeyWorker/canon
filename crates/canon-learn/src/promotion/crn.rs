//! `CrnPromotionGate` (design D3, task group 2) — a clean-room Rust port
//! of MaTTS's pure statistics core (per the donor's MaTTS
//! statistical-promotion audit), the paired
//! common-random-number (CRN) promotion gate for roles whose domain
//! supports deterministic replay.
//!
//! Two layers, same split MaTTS itself documents (§3.1
//! "pure-statistics-core / sampling-integration-layer split"):
//!
//! 1. **Pure statistics core** — [`seed_panels`], [`decompose_band_variance`],
//!    [`paired_contrast`], [`should_stop_scaling`], [`corroborated_effect`].
//!    Every function here is a deterministic, no-I/O function of `f64`/
//!    `u64` sample arrays — a fixture test feeds synthetic arrays
//!    directly, no store/simulator involved, ported FIXED-versions
//!    verbatim (the F1/F2/F3 review fixes MaTTS's own doc comments
//!    name: df-aware `F(1,df)`/`t(df)` critical-value tables instead of
//!    a fixed threshold, sample — not population — paired variance, the
//!    `MIN_DF_RESIDUAL`/`MIN_PANELS_FOR_SIGNIFICANCE` floors).
//! 2. **`CrnPromotionGate`** — the [`super::PromotionGate`] impl that
//!    turns a regime's already-resolved [`Trajectory`] slice into a
//!    [`super::PromotionDecision`] by parsing CRN panel/config identity
//!    back out of [`Trajectory::tags`] (see [`CRN_CONFIG_TAG_PREFIX`]/
//!    [`CRN_PANEL_TAG_PREFIX`]) and running it through the pure core above.
//!
//! **Explicitly out of scope** (per the vendor audit's own "SKIP"
//! recommendation, §5.5): MaTTS's `FrozenField`/
//! `assertFrozenFieldFresh` drift guard and the `contrastConfigSets`/
//! `contrastAndSynthesize` sim-integration layer (arena roundRobin, D-
//! metric diversity gates, frozen-field snapshot hashing) are sim-
//! domain-specific I/O this crate's insulated surface never touches —
//! `CrnPromotionGate::evaluate` is a pure function of already-collected
//! [`Trajectory`] rows, mirroring [`super::PromotionGate::evaluate`]'s
//! own "no I/O" contract.

use std::collections::BTreeMap;

use canon_model::ids::RegimeKey;
use chrono::{DateTime, Utc};

use super::{PromotionDecision, PromotionGate};
use crate::error::LearnError;
use crate::trajectory::Trajectory;

// =============================================================================
// 1. PURE STATISTICS CORE — deterministic, no I/O. Operates on f64/u64 sample
//    arrays, ported verbatim from MaTTS's own pure core.
// =============================================================================

/// `k` DISJOINT, reproducible base-seed panels: panel `p` is the
/// `panel_size`-length run of integers starting at `start + p *
/// panel_size` (`start` defaults to `1`, mirroring `makeSeedPanels`'s
/// own `opts?.start ?? 1`). Disjoint by construction — passing the SAME
/// `k` panels to every compared config-set is the whole CRN contract
/// (no config ever reuses another config's seeds, and no panel overlaps
/// another panel within one config). `k = 1` yields exactly one panel
/// (`[start..start+panel_size)`, i.e. a plain single-sweep seed list —
/// "k=1 reduces to a single sweep"). `k = 0` yields an empty `Vec`.
///
/// # Errors
/// `panel_size == 0` (`makeSeedPanels`'s own `panelSize must be a
/// positive integer` guard — a caller contract violation, not a
/// gracefully-degradable input).
pub fn seed_panels(k: usize, panel_size: usize, start: Option<u64>) -> Result<Vec<Vec<u64>>, LearnError> {
    if panel_size == 0 {
        return Err(LearnError::InvalidCrnInput(format!("seed_panels: panel_size must be a positive integer (got {panel_size})")));
    }
    let start = start.unwrap_or(1);
    Ok((0..k as u64)
        .map(|p| {
            let base = start + p * panel_size as u64;
            (0..panel_size as u64).map(|i| base + i).collect()
        })
        .collect())
}

/// LEGACY fixed F-ratio constant — superseded by the df-aware
/// `F_CRIT_1_TABLE`/[`f_critical_1`] below for the actual
/// `config_effect_real` decision (F3 fix, MaTTS review: a fixed
/// threshold false-positives on small-df batches — e.g. a 2-config k=2
/// batch, `df_residual = 1`). Kept as a conservative reference ceiling:
/// every table entry below df=30 is strictly above `4` (F(1,1)≈161.4
/// down to F(1,30)≈4.17), so this no longer gates anything itself but a
/// reader who wants a single "never below the real per-df bar" number
/// still has one.
pub const F_THRESHOLD: f64 = 4.0;

/// F(1, df) critical value at α≈0.05, df 1..30 — standard published
/// values (NIST/SEMATECH e-Handbook of Statistical Methods §1.3.6.7.2
/// "F Distribution"), indexed `[df - 1]`. Cross-checked against
/// `T_CRIT_2SIDED_TABLE` via the identity F(1,df) = t(df)² (locked by
/// `f_and_t_tables_satisfy_the_f_equals_t_squared_identity` below).
const F_CRIT_1_TABLE: [f64; 30] = [
    161.448, 18.513, 10.128, 7.709, 6.608, 5.987, 5.591, 5.318, 5.117, 4.965, 4.844, 4.747, 4.667, 4.6, 4.543, 4.494, 4.451, 4.414,
    4.381, 4.351, 4.325, 4.301, 4.279, 4.26, 4.242, 4.225, 4.21, 4.196, 4.183, 4.171,
];
const F_CRIT_1_ASYMPTOTE: f64 = 3.8415;

/// `F_CRIT_1_TABLE`'s df=1..30 entries; `F_CRIT_1_ASYMPTOTE` beyond
/// df=30; `+Infinity` at `df == 0` (no residual estimate at all — never
/// a passable bar, mirrors [`decompose_band_variance`]'s own "no data to
/// test" contract). Feeds [`decompose_band_variance`]'s
/// `config_effect_real` gate.
pub fn f_critical_1(df: usize) -> f64 {
    if df == 0 {
        return f64::INFINITY;
    }
    if df > 30 {
        return F_CRIT_1_ASYMPTOTE;
    }
    F_CRIT_1_TABLE[df - 1]
}

/// Minimum residual df [`decompose_band_variance`] requires before
/// `config_effect_real` can EVER read `true` — below this, even a
/// per-df table critical value is too noisy an estimate to trust. `2`
/// rejects the smallest batch shape that would otherwise false-positive:
/// 2 config-sets × k=2 panels has `df_residual = 1*1 = 1` — the MaTTS
/// review's own motivating counter-example (a 2-config k=2 batch with
/// per-panel diffs like `[0.1, 0.3]`, task 2.3's golden fixture).
pub const MIN_DF_RESIDUAL: usize = 2;

/// One two-way (config × panel), no-replication variance decomposition
/// — see [`decompose_band_variance`].
#[derive(Debug, Clone, PartialEq)]
pub struct VarianceDecomposition {
    pub n_configs: usize,
    pub n_panels: usize,
    pub grand_mean: f64,
    /// Per-config mean of its panel-mean samples, in `samples_by_config`
    /// row order.
    pub config_means: Vec<f64>,
    /// Per-panel mean across configs, in panel-index order.
    pub panel_means: Vec<f64>,
    pub ss_config: f64,
    pub ss_panel: f64,
    pub ss_residual: f64,
    pub df_config: usize,
    pub df_panel: usize,
    pub df_residual: usize,
    pub ms_config: f64,
    pub ms_residual: f64,
    /// `ms_config / ms_residual`. `+Infinity` when the residual is
    /// exactly `0` and `ss_config > 0` (a real config spread with
    /// literally zero measured panel-noise); `0` whenever no
    /// config-effect can even be tested. Never `NaN`.
    pub f_ratio: f64,
    /// The pooled within-config, between-panel noise variance
    /// (`ms_residual`) — the "noise" half of the between-config-vs-
    /// between-panel split this function performs.
    pub between_panel_variance: f64,
    /// `f_ratio >= f_critical_1(df_residual)` AND `df_residual >=
    /// MIN_DF_RESIDUAL` (df-aware — F3 fix, MaTTS review). `n_configs <
    /// 2` (nothing to contrast), `n_panels < 2` (no panel-noise
    /// estimate — includes `k=1`), or `df_residual < MIN_DF_RESIDUAL`
    /// all degrade to `false` rather than a spurious pass — "no effect"
    /// is the only honest read when there isn't enough data to
    /// attribute one either way.
    pub config_effect_real: bool,
}

/// Deterministic two-way (config × panel) ANOVA-style decomposition
/// WITHOUT replication (each `(config, panel)` cell is exactly one
/// panel-mean sample — CRN blocks, not repeated trials): attributes the
/// total spread across `samples_by_config[config][panel]` into a
/// between-CONFIG sum of squares (`ss_config` — the candidate "real
/// signal"), a between-PANEL sum of squares (`ss_panel` — CRN block
/// effects, e.g. a panel that happens to draw harder seeds for
/// everyone), and a residual (`ss_residual` — config×panel interaction /
/// unexplained noise). `config_effect_real = df_residual >=
/// MIN_DF_RESIDUAL && f_ratio >= f_critical_1(df_residual)` is the
/// df-aware real-vs-noise call (F3 fix, MaTTS review — supersedes a
/// fixed `f_ratio >= F_THRESHOLD` rule). Every compared config-set MUST
/// have been evaluated on the SAME panels in the SAME order (CRN) —
/// this function does not (and cannot) verify that; a caller building
/// `samples_by_config` (here, [`CrnPromotionGate::evaluate`]) guarantees
/// it by construction.
///
/// Pure: a function of its argument alone — same samples twice ⇒
/// identical attribution.
///
/// # Errors
/// Ragged input (rows of differing length) — CRN requires identical
/// panel counts per config, a caller contract violation, never a
/// gracefully-degrade case. `n_configs < 2`, `n_panels < 2` (including
/// the empty/`k=1` inputs), or `df_residual < MIN_DF_RESIDUAL` ARE
/// handled gracefully: every field is populated (no `NaN`/`Infinity`
/// leaks besides the documented `f_ratio` case above) and
/// `config_effect_real` is `false`.
pub fn decompose_band_variance(samples_by_config: &[Vec<f64>]) -> Result<VarianceDecomposition, LearnError> {
    let n_configs = samples_by_config.len();
    let n_panels = samples_by_config.first().map_or(0, Vec::len);
    for row in samples_by_config {
        if row.len() != n_panels {
            return Err(LearnError::InvalidCrnInput(format!(
                "decompose_band_variance: ragged samples_by_config — every config's panel row must have the \
                 same length (expected {n_panels}, got {}). CRN requires identical seed panels across every \
                 compared config-set.",
                row.len()
            )));
        }
    }

    let flat: Vec<f64> = samples_by_config.iter().flatten().copied().collect();
    let grand_mean = if flat.is_empty() { 0.0 } else { flat.iter().sum::<f64>() / flat.len() as f64 };

    let config_means: Vec<f64> =
        samples_by_config.iter().map(|row| if row.is_empty() { 0.0 } else { row.iter().sum::<f64>() / row.len() as f64 }).collect();

    let panel_means: Vec<f64> = (0..n_panels)
        .map(|p| {
            let sum: f64 = samples_by_config.iter().map(|row| row[p]).sum();
            if n_configs > 0 { sum / n_configs as f64 } else { 0.0 }
        })
        .collect();

    let ss_config = n_panels as f64 * config_means.iter().map(|m| (m - grand_mean).powi(2)).sum::<f64>();
    let ss_panel = n_configs as f64 * panel_means.iter().map(|m| (m - grand_mean).powi(2)).sum::<f64>();

    let mut ss_residual = 0.0;
    for (c, row) in samples_by_config.iter().enumerate() {
        for (p, &sample) in row.iter().enumerate() {
            let resid = sample - config_means[c] - panel_means[p] + grand_mean;
            ss_residual += resid * resid;
        }
    }

    let df_config = n_configs.saturating_sub(1);
    let df_panel = n_panels.saturating_sub(1);
    let df_residual = df_config * df_panel;

    let ms_config = if df_config > 0 { ss_config / df_config as f64 } else { 0.0 };
    let ms_residual = if df_residual > 0 { ss_residual / df_residual as f64 } else { 0.0 };

    let mut f_ratio = if df_residual == 0 {
        // n_configs<2 or n_panels<2 — no residual (noise) estimate to test against; graceful "no effect".
        0.0
    } else if ms_residual == 0.0 {
        // Zero measured noise: any real spread is unambiguous.
        if ss_config > 0.0 { f64::INFINITY } else { 0.0 }
    } else {
        ms_config / ms_residual
    };
    if f_ratio.is_nan() {
        f_ratio = 0.0; // Defensive guard; unreachable given the branches above.
    }

    let config_effect_real = df_residual >= MIN_DF_RESIDUAL && f_ratio >= f_critical_1(df_residual);

    Ok(VarianceDecomposition {
        n_configs,
        n_panels,
        grand_mean,
        config_means,
        panel_means,
        ss_config,
        ss_panel,
        ss_residual,
        df_config,
        df_panel,
        df_residual,
        ms_config,
        ms_residual,
        f_ratio,
        between_panel_variance: ms_residual,
        config_effect_real,
    })
}

/// LEGACY fixed z-critical constant — superseded by the df-aware
/// `T_CRIT_2SIDED_TABLE`/[`t_critical_2sided`] below for the actual
/// `significant` decision (F3 fix, MaTTS review). Kept as a conservative
/// reference ceiling: every table entry below df=30 is at or above `2`
/// (t(1)≈12.71 down to t(30)≈2.04).
pub const Z_PAIRED: f64 = 2.0;

/// Two-sided Student's t critical value at α≈0.05, df 1..30 — standard
/// published values (NIST/SEMATECH e-Handbook of Statistical Methods
/// §1.3.6.7.1 "Critical Values of the Student's t Distribution"), indexed
/// `[df - 1]`. See `F_CRIT_1_TABLE`'s doc for the t(df)² = F(1,df)
/// cross-check between the two tables.
const T_CRIT_2SIDED_TABLE: [f64; 30] = [
    12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.16, 2.145, 2.131, 2.12, 2.11, 2.101,
    2.093, 2.086, 2.08, 2.074, 2.069, 2.064, 2.06, 2.056, 2.052, 2.048, 2.045, 2.042,
];
const T_CRIT_2SIDED_ASYMPTOTE: f64 = 1.96;

/// `T_CRIT_2SIDED_TABLE`'s df=1..30 entries; `T_CRIT_2SIDED_ASYMPTOTE`
/// beyond df=30; `+Infinity` at `df == 0` (no paired-variance estimate at
/// all). Feeds [`paired_contrast`]'s `significant` gate.
pub fn t_critical_2sided(df: usize) -> f64 {
    if df == 0 {
        return f64::INFINITY;
    }
    if df > 30 {
        return T_CRIT_2SIDED_ASYMPTOTE;
    }
    T_CRIT_2SIDED_TABLE[df - 1]
}

/// Minimum panel count [`paired_contrast`] requires before `significant`
/// can EVER read `true` — below this (`n_panels < 3`, i.e. `df_paired <
/// 2`), a per-df t-critical is too noisy an estimate to trust (mirrors
/// [`MIN_DF_RESIDUAL`]'s rationale). `3` is the smallest panel count
/// that clears it.
pub const MIN_PANELS_FOR_SIGNIFICANCE: usize = 3;

/// Per-panel paired difference between two config-sets' panel-mean
/// samples — see [`paired_contrast`].
#[derive(Debug, Clone, PartialEq)]
pub struct PairedContrast {
    pub n_panels: usize,
    /// Per-panel paired difference `a[p] - b[p]`, in panel-index order.
    pub diffs: Vec<f64>,
    pub mean_diff: f64,
    pub var_diff: f64,
    pub se_paired_mean: f64,
    /// `n_panels >= MIN_PANELS_FOR_SIGNIFICANCE` AND `|mean_diff| >=
    /// t_critical_2sided(df_paired) * se_paired_mean` (`df_paired =
    /// n_panels - 1`), with one refinement at the zero-variance boundary:
    /// when `se_paired_mean == 0` (every panel agrees exactly) the
    /// literal inequality collapses to `|mean_diff| >= 0`, trivially true
    /// even for a genuinely zero `mean_diff` (byte-identical `a`/`b`) —
    /// that degenerate case reads as `false` here, while any nonzero,
    /// zero-noise `mean_diff` still reads `true` (deterministically
    /// real, no ambiguity possible). `n_panels < MIN_PANELS_FOR_
    /// SIGNIFICANCE` (too few panels for the per-df critical value to
    /// mean anything — F3 fix, MaTTS review) always reads `false`,
    /// regardless of how separated `a`/`b` look.
    pub significant: bool,
}

/// Per-panel PAIRED contrast between two config-sets' panel-mean samples
/// (CRN: `a`/`b` MUST be the SAME length — the same panel count, in the
/// same panel order). `var_diff` is the paired differences' SAMPLE
/// variance (÷ `df_paired = n_panels - 1`, NOT population ÷ `n_panels` —
/// F3 fix, MaTTS review: the standard unbiased estimator a t-test's SE
/// requires). Pure, deterministic: calling `paired_contrast(a, b)` twice
/// on the same inputs yields identical output.
///
/// # Errors
/// `a.len() != b.len()` — CRN requires paired panels; a caller contract
/// violation.
pub fn paired_contrast(a: &[f64], b: &[f64]) -> Result<PairedContrast, LearnError> {
    if a.len() != b.len() {
        return Err(LearnError::InvalidCrnInput(format!(
            "paired_contrast: a/b must be paired (equal length) panels — got {} vs {}",
            a.len(),
            b.len()
        )));
    }
    let n_panels = a.len();
    let diffs: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let mean_diff = if n_panels > 0 { diffs.iter().sum::<f64>() / n_panels as f64 } else { 0.0 };
    let df_paired = n_panels.saturating_sub(1);
    let var_diff =
        if df_paired > 0 { diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f64>() / df_paired as f64 } else { 0.0 };
    let se_paired_mean = if n_panels > 0 { (var_diff / n_panels as f64).sqrt() } else { 0.0 };
    let significant = if n_panels < MIN_PANELS_FOR_SIGNIFICANCE {
        false
    } else if se_paired_mean > 0.0 {
        mean_diff.abs() >= t_critical_2sided(df_paired) * se_paired_mean
    } else {
        mean_diff != 0.0
    };
    Ok(PairedContrast { n_panels, diffs, mean_diff, var_diff, se_paired_mean, significant })
}

/// Fixed variance-floor tolerance for [`should_stop_scaling`], in the
/// sample's own `[0,1]`-ish units. `0.02` = the panel-mean's standard
/// error must fall within 2% of the term's full range before scaling
/// stops — small enough that it can't itself hide an
/// `F_THRESHOLD`/`Z_PAIRED`-level real effect, so halting at this floor
/// never masks a signal the decomposition would otherwise have caught.
pub const DEFAULT_STOP_TOLERANCE: f64 = 0.02;

/// [`should_stop_scaling`]'s result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopScaling {
    pub stop: bool,
    pub between_panel_variance: f64,
    pub se_mean: f64,
}

/// The `k` scaling STOP criterion: halt adding panels once the
/// panel-mean samples' own standard error (`se_mean = sqrt(var(panel_
/// means) / n)`) falls at/under `tolerance` (default
/// [`DEFAULT_STOP_TOLERANCE`]) — i.e. one more panel can no longer
/// plausibly move the attribution. `between_panel_variance` here is the
/// raw (population) variance of the SUPPLIED `panel_means` array itself
/// — for a single config's own panel-mean history, this is that
/// config's between-panel noise estimate; a caller checking the FULL
/// batch's noise floor instead passes a [`VarianceDecomposition::panel_means`]
/// (or any config's row of `samples_by_config`). `n < 2` (no variance can
/// be estimated yet) always returns `stop: false` — more panels are
/// needed before a floor call can be made at all. Pure, deterministic.
pub fn should_stop_scaling(panel_means: &[f64], tolerance: Option<f64>) -> StopScaling {
    let tolerance = tolerance.unwrap_or(DEFAULT_STOP_TOLERANCE);
    let n = panel_means.len();
    if n < 2 {
        return StopScaling { stop: false, between_panel_variance: 0.0, se_mean: 0.0 };
    }
    let mean = panel_means.iter().sum::<f64>() / n as f64;
    let between_panel_variance = panel_means.iter().map(|b| (b - mean).powi(2)).sum::<f64>() / n as f64;
    let se_mean = (between_panel_variance / n as f64).sqrt();
    StopScaling { stop: se_mean <= tolerance, between_panel_variance, se_mean }
}

/// One batch's `[config][panel]` panel-mean samples + its decomposition
/// — the pure core's own MaTTS `ContrastBatch` analog, narrowed to
/// the fields [`corroborated_effect`] and a [`CrnPromotionGate`] caller
/// need (the D-metric diversity gates and frozen-field snapshot hash are
/// sim-domain-specific integration-layer concerns, out of this crate's
/// insulated surface — see this module's doc).
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastBatch {
    /// Config-set labels, in the same order as `samples_by_config` rows.
    pub labels: Vec<String>,
    /// `[config][panel]` panel-mean samples — the pure core's input
    /// shape.
    pub samples_by_config: Vec<Vec<f64>>,
    pub decomposition: VarianceDecomposition,
}

impl ContrastBatch {
    /// Builds a batch and runs its decomposition eagerly (so
    /// [`corroborated_effect`] and any caller-side diagnostic never
    /// re-derive it).
    ///
    /// # Errors
    /// Propagates [`decompose_band_variance`]'s ragged-input error.
    pub fn new(labels: Vec<String>, samples_by_config: Vec<Vec<f64>>) -> Result<Self, LearnError> {
        let decomposition = decompose_band_variance(&samples_by_config)?;
        Ok(Self { labels, samples_by_config, decomposition })
    }
}

/// The CORROBORATION gate a strategy-synthesis caller MUST check before
/// promoting a `sim`-like strategy item from a CRN batch (design D3,
/// task 2.2): `true` iff the variance decomposition attributes the
/// spread to a real config-effect (`batch.decomposition.
/// config_effect_real`), AND — for the common two-config-set A/B
/// contrast — the paired per-panel difference independently clears
/// [`paired_contrast`]'s significance bar too (belt-and-suspenders: the
/// omnibus F-test and the paired-difference test must agree, MaTTS
/// §3.3). A batch with more than 2 config-sets has no single
/// well-defined "A vs B" pair, so only the omnibus decomposition gates
/// it there.
pub fn corroborated_effect(batch: &ContrastBatch) -> bool {
    if !batch.decomposition.config_effect_real {
        return false;
    }
    if batch.samples_by_config.len() == 2 {
        let a = &batch.samples_by_config[0];
        let b = &batch.samples_by_config[1];
        return paired_contrast(a, b).map(|pc| pc.significant).unwrap_or(false);
    }
    true
}

// =============================================================================
// 2. CrnPromotionGate — the PromotionGate impl. Parses CRN panel/config
//    identity out of Trajectory::tags, runs it through the pure core above.
// =============================================================================

/// `Trajectory::tags` prefix a CRN-capable role's own trajectory-
/// recording caller uses to stamp which compared config-set produced one
/// trajectory (e.g. `"crn:config=A"`, `"crn:config=candidate-42"`) — the
/// tag/config identity contract [`CrnPromotionGate::evaluate`] parses
/// back out (`super` module doc: "a CRN-capable role's own trajectory-
/// recording caller encodes panel/config identity in `Trajectory::tags`").
pub const CRN_CONFIG_TAG_PREFIX: &str = "crn:config=";

/// `Trajectory::tags` prefix stamping WHICH CRN panel (a
/// [`seed_panels`] index, `0`-based) produced one trajectory (e.g.
/// `"crn:panel=0"`). Paired with [`CRN_CONFIG_TAG_PREFIX`], one
/// `(config, panel)` pair identifies exactly one cell of a
/// [`ContrastBatch`]'s `[config][panel]` sample grid — mirroring
/// MaTTS's own "exactly once per (config, panel) pair" contract.
pub const CRN_PANEL_TAG_PREFIX: &str = "crn:panel=";

/// Extracts `(config_label, panel_index, reward)` from one trajectory,
/// or `None` if it does not carry both CRN tags or is still `Pending`
/// (a pending trajectory has no resolved reward yet — its default `0.5`
/// placeholder would pollute the decomposition as a fake sample, so
/// `CrnPromotionGate::evaluate` only ever contrasts RESOLVED
/// trajectories, mirroring MaTTS's own samples — always a fully
/// computed `bandTerm`, never a placeholder).
fn crn_sample(t: &Trajectory) -> Option<(String, usize, f64)> {
    if t.verdict_record.is_pending() {
        return None;
    }
    let config = t.tags.iter().find_map(|tag| tag.strip_prefix(CRN_CONFIG_TAG_PREFIX))?.to_string();
    let panel = t.tags.iter().find_map(|tag| tag.strip_prefix(CRN_PANEL_TAG_PREFIX))?.parse::<usize>().ok()?;
    Some((config, panel, t.verdict_record.reward))
}

/// Groups `samples` into a [`ContrastBatch`] by CRN config/panel tag
/// identity (last-recorded reward wins per `(config, panel)` cell —
/// this crate's own store is append-only, so a re-recorded cell is a
/// correction, not ambiguous duplication). Config labels sort
/// lexicographically (deterministic batch/label order); each config's
/// panels sort by panel index (`BTreeMap` iteration order) — the "same
/// panels in the same order" CRN pairing [`decompose_band_variance`]
/// itself assumes but cannot verify.
///
/// # Errors
/// No CRN-tagged, resolved trajectory found at all, or the compared
/// configs disagree on WHICH panel indices they cover (CRN requires
/// identical panel sets across every compared config-set — a caller
/// contract violation, surfaced as a `Reject` reason rather than a
/// panic).
fn batch_from_trajectories(samples: &[Trajectory]) -> Result<ContrastBatch, String> {
    let mut by_config: BTreeMap<String, BTreeMap<usize, f64>> = BTreeMap::new();
    for t in samples {
        if let Some((config, panel, reward)) = crn_sample(t) {
            by_config.entry(config).or_default().insert(panel, reward);
        }
    }
    if by_config.is_empty() {
        return Err(format!(
            "no CRN-tagged, resolved trajectories found — every sample must carry both {CRN_CONFIG_TAG_PREFIX:?} and \
             {CRN_PANEL_TAG_PREFIX:?} tags and a non-pending verdict"
        ));
    }

    let labels: Vec<String> = by_config.keys().cloned().collect();
    let reference_panels: Vec<usize> = by_config[&labels[0]].keys().copied().collect();
    for label in &labels[1..] {
        let panels: Vec<usize> = by_config[label].keys().copied().collect();
        if panels != reference_panels {
            return Err(format!(
                "CRN panel identity mismatch: config {:?} covers panels {reference_panels:?} but config {label:?} covers \
                 {panels:?} — CRN requires every compared config-set to share IDENTICAL panel indices",
                labels[0]
            ));
        }
    }

    let samples_by_config: Vec<Vec<f64>> = by_config.values().map(|panels| panels.values().copied().collect()).collect();
    ContrastBatch::new(labels, samples_by_config).map_err(|e| e.to_string())
}

fn promotion_reason(regime_key: &RegimeKey, batch: &ContrastBatch, promoted: bool) -> String {
    let d = &batch.decomposition;
    if !d.config_effect_real {
        return if d.df_residual < MIN_DF_RESIDUAL {
            format!(
                "{regime_key}: omnibus residual df {} below MIN_DF_RESIDUAL {MIN_DF_RESIDUAL} ({} configs x {} panels) \
                 — too few residual degrees of freedom for a per-df critical value to mean anything",
                d.df_residual, d.n_configs, d.n_panels
            )
        } else {
            format!(
                "{regime_key}: omnibus config effect not real (F={:.4} < F_crit({})={:.4})",
                d.f_ratio,
                d.df_residual,
                f_critical_1(d.df_residual)
            )
        };
    }
    if batch.samples_by_config.len() == 2 {
        // `batch_from_trajectories` already validated identical panel counts per config.
        let pc = paired_contrast(&batch.samples_by_config[0], &batch.samples_by_config[1])
            .expect("ContrastBatch construction already validated equal-length config rows");
        return if promoted {
            format!(
                "{regime_key}: corroborated CRN effect — omnibus F={:.4} >= F_crit({})={:.4} AND paired contrast \
                 between {:?}/{:?} significant (mean_diff={:.4}, n_panels={})",
                d.f_ratio,
                d.df_residual,
                f_critical_1(d.df_residual),
                batch.labels[0],
                batch.labels[1],
                pc.mean_diff,
                pc.n_panels
            )
        } else {
            format!(
                "{regime_key}: omnibus effect real (F={:.4}, df_residual={}) but the independent paired contrast \
                 between {:?} and {:?} is not significant (mean_diff={:.4}, n_panels={})",
                d.f_ratio, d.df_residual, batch.labels[0], batch.labels[1], pc.mean_diff, pc.n_panels
            )
        };
    }
    format!(
        "{regime_key}: omnibus CRN effect real across {} configs (F={:.4} >= F_crit({})={:.4}, df_residual={})",
        d.n_configs,
        d.f_ratio,
        d.df_residual,
        f_critical_1(d.df_residual),
        d.df_residual
    )
}

/// Paired-CRN promotion gate (design D3) — [`corroborated_effect`] IS
/// the promotion decision for CRN-capable roles. `samples` (every
/// [`Trajectory`] collected for `regime_key` so far, per
/// [`super::PromotionGate::evaluate`]'s own contract) is grouped into a
/// [`ContrastBatch`] by CRN config/panel tag identity
/// (`batch_from_trajectories`), then gated by [`corroborated_effect`].
///
/// `evaluate`'s `as_of` parameter (required by
/// [`super::PromotionGate`]'s uniform, trait-wide signature) is
/// accepted and DELIBERATELY ignored here: unlike
/// `OccurrencePromotionGate`'s trailing wall-clock window, CRN's
/// decision is already time-independent — a pure function of the
/// paired `samples`' panel/config identity and resolved rewards alone
/// (this module's own doc, "no I/O"). Passing a different `as_of` for
/// the SAME `samples` can never change the outcome.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrnPromotionGate;

impl PromotionGate for CrnPromotionGate {
    fn evaluate(&self, regime_key: &RegimeKey, samples: &[Trajectory], _as_of: DateTime<Utc>) -> PromotionDecision {
        let batch = match batch_from_trajectories(samples) {
            Ok(batch) => batch,
            Err(reason) => return PromotionDecision::Reject { reason: format!("{regime_key}: {reason}") },
        };
        let promoted = corroborated_effect(&batch);
        let reason = promotion_reason(regime_key, &batch, promoted);
        if promoted { PromotionDecision::Promote { reason } } else { PromotionDecision::Reject { reason } }
    }
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_model::ids::RoleId;
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::ids::TrajectoryId;
    use crate::verdict_outcome::{TrajectoryVerdict, VerdictOutcome};

    // ---------------------------------------------------------------
    // Pure statistics core
    // ---------------------------------------------------------------

    #[test]
    fn seed_panels_yields_k_disjoint_panel_size_length_runs() {
        let panels = seed_panels(3, 4, None).unwrap();
        assert_eq!(panels, vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8], vec![9, 10, 11, 12]]);
    }

    #[test]
    fn seed_panels_k_one_reduces_to_a_single_sweep() {
        let panels = seed_panels(1, 5, Some(10)).unwrap();
        assert_eq!(panels, vec![vec![10, 11, 12, 13, 14]]);
    }

    #[test]
    fn seed_panels_k_zero_yields_empty() {
        assert_eq!(seed_panels(0, 4, None).unwrap(), Vec::<Vec<u64>>::new());
    }

    #[test]
    fn seed_panels_rejects_zero_panel_size() {
        let err = seed_panels(2, 0, None).unwrap_err();
        assert!(matches!(err, LearnError::InvalidCrnInput(_)));
    }

    #[test]
    fn f_and_t_tables_satisfy_the_f_equals_t_squared_identity() {
        // MaTTS's own cross-check: an F-test with ONE numerator df against a
        // single contrast is mathematically the square of the matching two-sided
        // t-test — every table entry must satisfy this to the tables' own precision.
        for df in 1..=30 {
            let f = f_critical_1(df);
            let t = t_critical_2sided(df);
            assert!((f - t * t).abs() < 0.01, "df={df}: F={f} != t^2={}", t * t);
        }
    }

    #[test]
    fn decompose_band_variance_rejects_ragged_rows() {
        let err = decompose_band_variance(&[vec![1.0, 2.0], vec![1.0]]).unwrap_err();
        assert!(matches!(err, LearnError::InvalidCrnInput(_)));
    }

    #[test]
    fn decompose_band_variance_the_matts_counter_example_never_reads_config_effect_real() {
        // The documented MaTTS counter-example (task 2.3): a 2-config k=2 batch
        // with per-panel diffs [0.1, 0.3] — df_residual = 1*1 = 1, below
        // MIN_DF_RESIDUAL (2), so config_effect_real MUST read false regardless
        // of how large the observed gap looks.
        let a = vec![0.50, 0.50];
        let b = vec![0.60, 0.80];
        let d = decompose_band_variance(&[a, b]).unwrap();
        assert_eq!(d.df_residual, 1);
        assert!(!d.config_effect_real, "df_residual=1 < MIN_DF_RESIDUAL=2 must never read config_effect_real: true");
    }

    #[test]
    fn decompose_band_variance_below_min_df_residual_floor_stays_false_even_at_k3() {
        // 2 configs x 3 panels clears df_residual=2 (== MIN_DF_RESIDUAL) but a
        // small, noisy gap still doesn't clear the per-df F-critical bar.
        let a = vec![0.50, 0.55, 0.45];
        let b = vec![0.52, 0.50, 0.50];
        let d = decompose_band_variance(&[a, b]).unwrap();
        assert_eq!(d.df_residual, 2);
        assert!(d.f_ratio < f_critical_1(2));
        assert!(!d.config_effect_real);
    }

    #[test]
    fn decompose_band_variance_single_config_or_single_panel_degrades_gracefully() {
        let single_config = decompose_band_variance(&[vec![0.5, 0.6, 0.7]]).unwrap();
        assert!(!single_config.config_effect_real);
        assert_eq!(single_config.df_residual, 0);

        let single_panel = decompose_band_variance(&[vec![0.3], vec![0.9]]).unwrap();
        assert!(!single_panel.config_effect_real);
        assert_eq!(single_panel.df_residual, 0);
    }

    #[test]
    fn paired_contrast_rejects_unequal_length_panels() {
        let err = paired_contrast(&[1.0, 2.0], &[1.0]).unwrap_err();
        assert!(matches!(err, LearnError::InvalidCrnInput(_)));
    }

    #[test]
    fn paired_contrast_below_min_panels_for_significance_never_significant() {
        // n_panels=2 < MIN_PANELS_FOR_SIGNIFICANCE=3 — reads false even for a
        // huge, zero-noise difference.
        let pc = paired_contrast(&[1.0, 1.0], &[0.0, 0.0]).unwrap();
        assert_eq!(pc.n_panels, 2);
        assert!(!pc.significant);
    }

    #[test]
    fn paired_contrast_zero_variance_nonzero_mean_reads_significant() {
        let pc = paired_contrast(&[1.0, 1.0, 1.0], &[0.0, 0.0, 0.0]).unwrap();
        assert_eq!(pc.se_paired_mean, 0.0);
        assert!(pc.significant);
    }

    #[test]
    fn paired_contrast_zero_variance_zero_mean_reads_not_significant() {
        let pc = paired_contrast(&[1.0, 1.0, 1.0], &[1.0, 1.0, 1.0]).unwrap();
        assert_eq!(pc.mean_diff, 0.0);
        assert_eq!(pc.se_paired_mean, 0.0);
        assert!(!pc.significant);
    }

    #[test]
    fn should_stop_scaling_needs_at_least_two_panels() {
        let r = should_stop_scaling(&[0.5], None);
        assert!(!r.stop);
    }

    #[test]
    fn should_stop_scaling_stops_once_se_mean_clears_the_tolerance() {
        let tight = should_stop_scaling(&[0.500, 0.501, 0.499, 0.500], None);
        assert!(tight.stop);
        let loose = should_stop_scaling(&[0.1, 0.9, 0.2, 0.8], None);
        assert!(!loose.stop);
    }

    #[test]
    fn corroborated_effect_the_matts_counter_example_stays_rejected() {
        let batch = ContrastBatch::new(vec!["A".into(), "B".into()], vec![vec![0.50, 0.50], vec![0.60, 0.80]]).unwrap();
        assert!(!corroborated_effect(&batch));
    }

    #[test]
    fn corroborated_effect_requires_paired_agreement_for_two_configs() {
        // Enough df (k=5, df_residual=4) but the omnibus F doesn't clear its bar.
        let a = vec![0.40, 0.60, 0.50, 0.45, 0.55];
        let b = vec![0.45, 0.55, 0.55, 0.40, 0.50];
        let batch = ContrastBatch::new(vec!["A".into(), "B".into()], vec![a, b]).unwrap();
        assert!(!batch.decomposition.config_effect_real);
        assert!(!corroborated_effect(&batch));
    }

    #[test]
    fn corroborated_effect_a_clearly_significant_effect_promotes() {
        let a = vec![0.30, 0.35, 0.25, 0.33, 0.27];
        let b = vec![0.65, 0.72, 0.60, 0.68, 0.65];
        let batch = ContrastBatch::new(vec!["A".into(), "B".into()], vec![a, b]).unwrap();
        assert!(batch.decomposition.config_effect_real);
        assert!(corroborated_effect(&batch));
    }

    #[test]
    fn corroborated_effect_three_configs_skips_the_paired_agreement_requirement() {
        let a = vec![0.30, 0.35, 0.25, 0.33, 0.27];
        let b = vec![0.65, 0.72, 0.60, 0.68, 0.65];
        let c = vec![0.45, 0.50, 0.40, 0.48, 0.42];
        let batch = ContrastBatch::new(vec!["A".into(), "B".into(), "C".into()], vec![a, b, c]).unwrap();
        assert!(batch.decomposition.config_effect_real);
        assert!(corroborated_effect(&batch));
    }

    // ---------------------------------------------------------------
    // CrnPromotionGate — fixture Trajectory rows through the real trait impl
    // ---------------------------------------------------------------

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "tuning", "abc123def456")).unwrap()
    }

    fn verdict(role: &str) -> VerdictRow {
        VerdictRow { role: RoleId::parse(role).unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }
    }

    /// One resolved (non-pending) CRN-tagged fixture trajectory.
    fn crn_trajectory(role: &str, config: &str, panel: usize, reward: f64) -> Trajectory {
        Trajectory::new(
            TrajectoryId::new(),
            regime(role),
            "sim tuning sweep",
            "panel evaluation",
            vec![verdict(role)],
            fixed_as_of(),
            vec![format!("{CRN_CONFIG_TAG_PREFIX}{config}"), format!("{CRN_PANEL_TAG_PREFIX}{panel}")],
        )
        .unwrap()
        .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, reward))
    }

    /// A fixed evaluation instant — `CrnPromotionGate::evaluate`
    /// ignores `as_of` entirely (its own doc above), but the trait
    /// signature still requires one; every fixture below passes this
    /// rather than reading `Utc::now()`, keeping this test module's
    /// own promotion-decision inputs fully deterministic too.
    fn fixed_as_of() -> DateTime<Utc> {
        "2025-06-15T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn crn_gate_golden_fixture_matts_counter_example_rejects() {
        // task 2.3: the documented MaTTS counter-example — 2-config k=2 batch,
        // per-panel diffs [0.1, 0.3] — must NOT promote.
        let samples = vec![
            crn_trajectory("sim", "A", 0, 0.50),
            crn_trajectory("sim", "A", 1, 0.50),
            crn_trajectory("sim", "B", 0, 0.60),
            crn_trajectory("sim", "B", 1, 0.80),
        ];
        let decision = CrnPromotionGate.evaluate(&regime("sim"), &samples, fixed_as_of());
        assert!(!decision.is_promote(), "counter-example must reject, got: {}", decision.reason());
    }

    #[test]
    fn crn_gate_rejects_a_non_significant_contrast() {
        let samples = vec![
            crn_trajectory("sim", "A", 0, 0.40),
            crn_trajectory("sim", "A", 1, 0.60),
            crn_trajectory("sim", "A", 2, 0.50),
            crn_trajectory("sim", "A", 3, 0.45),
            crn_trajectory("sim", "A", 4, 0.55),
            crn_trajectory("sim", "B", 0, 0.45),
            crn_trajectory("sim", "B", 1, 0.55),
            crn_trajectory("sim", "B", 2, 0.55),
            crn_trajectory("sim", "B", 3, 0.40),
            crn_trajectory("sim", "B", 4, 0.50),
        ];
        let decision = CrnPromotionGate.evaluate(&regime("sim"), &samples, fixed_as_of());
        assert!(!decision.is_promote(), "non-significant contrast must reject, got: {}", decision.reason());
    }

    #[test]
    fn crn_gate_accepts_a_clearly_significant_contrast() {
        let samples = vec![
            crn_trajectory("sim", "A", 0, 0.30),
            crn_trajectory("sim", "A", 1, 0.35),
            crn_trajectory("sim", "A", 2, 0.25),
            crn_trajectory("sim", "A", 3, 0.33),
            crn_trajectory("sim", "A", 4, 0.27),
            crn_trajectory("sim", "B", 0, 0.65),
            crn_trajectory("sim", "B", 1, 0.72),
            crn_trajectory("sim", "B", 2, 0.60),
            crn_trajectory("sim", "B", 3, 0.68),
            crn_trajectory("sim", "B", 4, 0.65),
        ];
        let decision = CrnPromotionGate.evaluate(&regime("sim"), &samples, fixed_as_of());
        assert!(decision.is_promote(), "clearly significant contrast must promote, got: {}", decision.reason());
    }

    #[test]
    fn crn_gate_rejects_when_no_crn_tags_present() {
        let samples = vec![Trajectory::new(
            TrajectoryId::new(),
            regime("sim"),
            "untagged",
            "no crn identity",
            vec![verdict("sim")],
            fixed_as_of(),
            vec![],
        )
        .unwrap()
        .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.9))];
        let decision = CrnPromotionGate.evaluate(&regime("sim"), &samples, fixed_as_of());
        assert!(!decision.is_promote());
    }

    #[test]
    fn crn_gate_ignores_still_pending_trajectories() {
        // A pending trajectory has no resolved reward yet — it must not silently
        // contribute its 0.5 placeholder as a fake sample.
        let mut samples = vec![
            crn_trajectory("sim", "A", 0, 0.30),
            crn_trajectory("sim", "A", 1, 0.35),
            crn_trajectory("sim", "A", 2, 0.25),
            crn_trajectory("sim", "B", 0, 0.65),
            crn_trajectory("sim", "B", 1, 0.72),
            crn_trajectory("sim", "B", 2, 0.60),
        ];
        // Pending: carries CRN tags but must be excluded from the decomposition.
        samples.push(
            Trajectory::new(
                TrajectoryId::new(),
                regime("sim"),
                "still running",
                "panel not yet resolved",
                vec![verdict("sim")],
                fixed_as_of(),
                vec![format!("{CRN_CONFIG_TAG_PREFIX}A"), format!("{CRN_PANEL_TAG_PREFIX}3")],
            )
            .unwrap(),
        );
        let decision = CrnPromotionGate.evaluate(&regime("sim"), &samples, fixed_as_of());
        assert!(decision.is_promote(), "pending sample must not block an otherwise-significant contrast, got: {}", decision.reason());
    }
}
