//! `PolicyResolution` (design decisions 3, 7, 10) — THE single
//! resolver S12's `canon context` and S5's own gate both call (S12
//! design D2: "`canon context` and `canon fmt`/`canon gate` both call
//! one `SchemaRegistry`/`PolicyResolution` API — no second,
//! independently-loaded copy of schema or policy data"). Generalizes
//! `tools/parity.py`'s `_load_policy`/`_required_platforms`/
//! `_allowed_lanes`/`_required_lanes`
//! (the donor parity-harness audit's policy-derivation notes): a
//! pure `(artifact, policy) -> derived-value` contract, policy loaded
//! once per gate run, fixture-rebindable.
//!
//! # `policy.yaml`'s on-disk shape
//! Fixed at `<repo>/.canon/policy.yaml`
//! ([`POLICY_YAML_RELATIVE_PATH`] —
//! `docs/superpowers/specs/2026-07-10-canon-design.md`'s infra layout
//! table). Every field — `trust_required`/`trust_sample`'s
//! per-key entries, `staleness.max_commits_behind`,
//! `staleness.surface_scoped`, and every `risk_routing` entry — is
//! independently EITHER a flat value OR a single-key `{cel: "<expression>"}`
//! mapping (design decision 10 / D7: "Whether a given ... field is a flat
//! value or a CEL predicate over scenario facts ... D7 discipline kept:
//! facts on artifacts, routing in policy"). The `{cel: ...}` wrapper — not
//! string-shape sniffing — disambiguates a CEL predicate from a flat value
//! that happens to also be a string (`trust_required`'s flat values,
//! `"human"`/`"agent"`, are themselves valid strings). The separate
//! `risk_tiers` section is declarative and never CEL-compiled.
//!
//! # CEL predicates evaluate against one `EvidenceRecord`
//! Every predicate's `record` variable is one
//! [`canon_model::EvidenceRecord`]'s own `serde_json::to_value(...)`
//! ([`POLICY_BINDING_KIND`]) — matching this change's Goals:
//! "a domain-agnostic static + dynamic gate over any
//! `EvidenceRecord`-shaped corpus". `resolve()` COMPILES (write-time
//! validates, `canon_policy::compile`) every predicate against
//! [`canon_policy::SchemaRegistry`]-derived bindings up front — an
//! unparseable/type-invalid predicate can never reach evaluation
//! (canon-policy's own S13 invariant); [`PolicyField::resolve`] is
//! where a compiled predicate is actually EVALUATED, later, against a
//! specific record (S5 wave-2's coverage/ledger checks call this per
//! artifact).
//!
//! # Fail-soft load, fail-loud diagnostics
//! `resolve()` is infallible (frozen signature, S12 design D2 — no
//! `Result`), mirroring `_load_policy`'s own tolerant behavior
//! (policy-derivation.md §"Entry points": "a missing or malformed
//! policy file degrades every downstream `.get(key, {})` call to empty
//! routing rather than crashing the gate"). Every load/compile problem
//! along the way is still recorded, loudly, in
//! [`PolicyResolution::diagnostics`] — never silently swallowed, just
//! never a panic or an `Err` that would block `canon context`/`canon
//! gate` from resolving SOME policy (possibly the all-defaults one).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use canon_model::{RecordKind, SubjectStatus};
use canon_policy::{bindings_for, compile, evaluate, BindingSet, CompiledPolicy, EvalBudget, PolicyValue, SchemaRegistry};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::trust_ladder::TrustLevel;

/// `policy.yaml`'s fixed on-disk location relative to a repo root
/// (infra layout doc: `<repo>/.canon/policy.yaml # required-cell
/// derivation (D7 pattern)`) — unlike `canon.yaml`'s own
/// `tiers.git.root`, this path is NOT per-repo-configurable.
pub const POLICY_YAML_RELATIVE_PATH: &str = canon_model::paths::POLICY_FILE;

/// The [`RecordKind`] every `policy.yaml` CEL predicate binds against
/// (module doc) — the one `bindings_for` call [`PolicyResolution::resolve`]
/// makes.
pub const POLICY_BINDING_KIND: RecordKind = RecordKind::EvidenceRecord;

/// `staleness.max_commits_behind`'s default when `policy.yaml`
/// declares no value — parity.py's own audited default
/// (the donor parity-harness audit's staleness notes §3.1:
/// "`max_commits_behind` (default `50`, `policy.yaml:34`)").
pub const DEFAULT_MAX_COMMITS_BEHIND: u32 = 50;

/// `staleness.surface_scoped`'s default when `policy.yaml` declares no
/// value — parity.py's own audited default (staleness.md §6 citations:
/// `spec/policy.yaml:30-36 — surface_scoped: true`).
pub const DEFAULT_SURFACE_SCOPED: bool = true;

/// Converts a scalar [`PolicyValue`] (what evaluating a compiled CEL
/// predicate produces) into `Self`. A local trait, not
/// `TryFrom<PolicyValue>`, because Rust's orphan rule forbids `impl
/// TryFrom<PolicyValue> for bool` from this crate (both types
/// foreign); implemented here for every concrete type a
/// [`PolicyField`] in this crate uses.
pub trait FromPolicyValue: Sized {
    fn from_policy_value(value: &PolicyValue) -> Result<Self, PolicyResolveError>;
}

impl FromPolicyValue for bool {
    fn from_policy_value(value: &PolicyValue) -> Result<Self, PolicyResolveError> {
        value.as_bool().ok_or_else(|| PolicyResolveError::TypeMismatch { expected: "bool", got: format!("{value:?}") })
    }
}

impl FromPolicyValue for u32 {
    fn from_policy_value(value: &PolicyValue) -> Result<Self, PolicyResolveError> {
        let mismatch = || PolicyResolveError::TypeMismatch { expected: "u32", got: format!("{value:?}") };
        match value {
            PolicyValue::UInt(n) => u32::try_from(*n).map_err(|_| mismatch()),
            PolicyValue::Int(n) if *n >= 0 => u32::try_from(*n).map_err(|_| mismatch()),
            _ => Err(mismatch()),
        }
    }
}

impl FromPolicyValue for f64 {
    fn from_policy_value(value: &PolicyValue) -> Result<Self, PolicyResolveError> {
        match value {
            PolicyValue::Double(d) => Ok(*d),
            PolicyValue::Int(n) => Ok(*n as f64),
            PolicyValue::UInt(n) => Ok(*n as f64),
            _ => Err(PolicyResolveError::TypeMismatch { expected: "f64", got: format!("{value:?}") }),
        }
    }
}

impl FromPolicyValue for TrustLevel {
    fn from_policy_value(value: &PolicyValue) -> Result<Self, PolicyResolveError> {
        let mismatch = || PolicyResolveError::TypeMismatch { expected: "\"agent\" | \"human\"", got: format!("{value:?}") };
        match value {
            PolicyValue::String(s) => TrustLevel::from_str_exact(s).ok_or_else(mismatch),
            _ => Err(mismatch()),
        }
    }
}

/// One `policy.yaml` field's RESOLVED (write-time-validated) form: a
/// flat value, unconditionally, OR a compiled CEL predicate ready to
/// evaluate against a specific record (module doc). `Flat`'s value is
/// returned as-is by [`PolicyField::resolve`] regardless of `record`;
/// `Cel` evaluates.
#[derive(Debug, Clone)]
pub enum PolicyField<T> {
    Flat(T),
    Cel(CompiledPolicy),
}

impl<T: FromPolicyValue + Clone> PolicyField<T> {
    /// Resolve this field's value for `record` — an `EvidenceRecord`'s
    /// own JSON ([`POLICY_BINDING_KIND`]). `now` is the "current time"
    /// `age_days(...)` (if the predicate uses it) resolves against
    /// (`canon_policy::evaluate`'s own contract: never read from the
    /// wall clock internally).
    pub fn resolve(&self, record: &serde_json::Value, now: DateTime<Utc>) -> Result<T, PolicyResolveError> {
        match self {
            PolicyField::Flat(value) => Ok(value.clone()),
            PolicyField::Cel(compiled) => {
                let value = evaluate(compiled, record, now, EvalBudget::default())?;
                T::from_policy_value(&value)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyResolveError {
    #[error("policy CEL evaluation failed: {0}")]
    Eval(#[from] canon_policy::PolicyError),
    #[error("policy predicate resolved to a value not matching the field's expected type `{expected}` (got {got})")]
    TypeMismatch { expected: &'static str, got: String },
}

/// `staleness.max_commits_behind`/`staleness.surface_scoped`, each
/// independently flat-or-CEL (module doc).
#[derive(Debug, Clone)]
pub struct StalenessPolicy {
    pub max_commits_behind: PolicyField<u32>,
    pub surface_scoped: PolicyField<bool>,
}

/// One problem `resolve()` encountered while loading/compiling
/// `policy.yaml` — recorded, never silently dropped, never a panic or
/// an `Err` (module doc's "fail-soft load, fail-loud diagnostics").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDiagnostic {
    /// `policy.yaml` does not exist at [`POLICY_YAML_RELATIVE_PATH`] —
    /// every field resolves to its documented default (parity.py's own
    /// `_load_policy` precedent, module doc).
    Missing { path: PathBuf },
    /// `policy.yaml` exists but failed to parse as YAML matching this
    /// crate's schema — every field resolves to its documented
    /// default, identical to `Missing`.
    Malformed { path: PathBuf, detail: String },
    /// One field's `{cel: ...}` predicate failed write-time
    /// compilation (`canon_policy::compile`) — that ONE field is
    /// dropped from the resolved policy (a map entry is omitted; a
    /// singular field like `staleness.max_commits_behind` falls back
    /// to its documented default), every OTHER field still resolves
    /// normally.
    InvalidPredicate { field: String, source: String, errors: Vec<String> },
    /// `registry` (the [`SchemaRegistry`] `resolve()` was called with)
    /// has no schema for [`POLICY_BINDING_KIND`] — e.g. a fixture
    /// registry built with [`SchemaRegistry::single`] and keyed to a
    /// different [`RecordKind`] (`canon_policy::bindings_for`'s own
    /// documented panic condition, which this diagnostic exists to
    /// intercept BEFORE that call — `resolve()`'s frozen signature is
    /// infallible, module doc). No `{cel: ...}` predicate anywhere in
    /// `policy.yaml` can compile without bindings, so EVERY such
    /// predicate degrades to its documented default (each one also
    /// recorded individually as an [`Self::InvalidPredicate`]); every
    /// flat (non-CEL) value is unaffected — it never touches the
    /// registry.
    SchemaUnavailable { kind: RecordKind },
    /// One whole SECTION was present but unusable (s44
    /// `spec-derived-worklist`). Unlike [`Self::InvalidPredicate`],
    /// which drops one field and lets the rest resolve, a section this
    /// diagnostic names resolves to a POISONED value: `resolve()`'s
    /// signature is frozen infallible so it cannot refuse here, and
    /// degrading a present-but-broken section to "absent" would make a
    /// typo silently equivalent to not opting in. The owning check
    /// turns the poison into a violation at CHECK time instead.
    InvalidSection { section: &'static str, detail: String },
}

impl std::fmt::Display for PolicyDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyDiagnostic::Missing { path } => write!(f, "{}: not found (using defaults)", path.display()),
            PolicyDiagnostic::Malformed { path, detail } => write!(f, "{}: {detail} (using defaults)", path.display()),
            PolicyDiagnostic::InvalidPredicate { field, source, errors } => {
                write!(f, "{field}: CEL predicate `{source}` rejected at write-time validation: {}", errors.join("; "))
            }
            PolicyDiagnostic::SchemaUnavailable { kind } => {
                write!(f, "SchemaRegistry has no schema for {kind:?} — every {{cel: ...}} predicate falls back to its documented default")
            }
            PolicyDiagnostic::InvalidSection { section, detail } => {
                write!(f, "{section}: section is present but unusable ({detail}) — the owning check refuses rather than treating it as absent")
            }
        }
    }
}

/// A `policy.yaml` field written as a raw CEL predicate — a single-key
/// `{cel: "<expression>"}` mapping (module doc), the ONLY on-disk
/// shape that means "this is a predicate, not a flat value".
#[derive(Debug, Clone, Deserialize)]
struct CelSpec {
    cel: String,
}

/// One `policy.yaml` field's RAW (pre-compile) on-disk shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawField<T> {
    Flat(T),
    Cel(CelSpec),
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawStaleness {
    #[serde(default)]
    max_commits_behind: Option<RawField<u32>>,
    #[serde(default)]
    surface_scoped: Option<RawField<bool>>,
}

/// `policy.yaml`'s raw on-disk schema (design decisions 3/7). Every
/// top-level key is `#[serde(default)]` — an absent section resolves to
/// "no entries"/documented defaults, never a parse failure (module
/// doc).
#[derive(Debug, Clone, Default, Deserialize)]
struct RawPolicy {
    /// Declared but not (yet) enforced — parity.py's own
    /// `spec/policy.yaml:schema: 1` precedent (policy-derivation.md
    /// §3.2: "a version key with nothing reading it ... the placement
    /// convention is worth keeping; the enforcement has to be written
    /// new"). Kept here as the same forward-declared convention;
    /// `resolve()` does not yet reject a schema mismatch (no S5
    /// wave-2 consumer of a second schema version exists to validate
    /// against).
    #[serde(default)]
    #[allow(dead_code)]
    schema: Option<u32>,
    #[serde(default)]
    trust_required: BTreeMap<String, RawField<TrustLevel>>,
    #[serde(default)]
    trust_sample: BTreeMap<String, RawField<f64>>,
    #[serde(default)]
    staleness: RawStaleness,
    #[serde(default)]
    risk_routing: BTreeMap<String, RawField<bool>>,
    /// Declarative effect-aware approval rules. Kept untyped at the
    /// section boundary so one malformed tier cannot discard sibling
    /// policy sections.
    #[serde(default)]
    risk_tiers: Option<serde_yaml::Value>,
    /// Deliberately an untyped [`serde_yaml::Value`], not a typed
    /// struct: a typed field would make a malformed `spec_coverage:`
    /// fail the WHOLE `RawPolicy` deserialize, and
    /// [`load_raw_policy`] degrades that to `RawPolicy::default()` —
    /// so one typo in this section would silently discard every
    /// `trust_required`/`risk_routing` entry in the file. Parsing it
    /// separately in [`resolve_spec_coverage`] keeps the blast radius
    /// to this section.
    #[serde(default)]
    spec_coverage: Option<serde_yaml::Value>,
}

/// One effect-aware approval tier in `policy.yaml`. The simple wildcard
/// matcher used by the gate treats `*` as spanning any characters,
/// including `/`; these are not CEL expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskTierRule {
    pub rank: u32,
    pub paths: Vec<String>,
    pub effects: Vec<String>,
    pub min_human_approvals: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRiskTierRule {
    rank: u32,
    paths: Vec<String>,
    effects: Vec<String>,
    min_human_approvals: u32,
}
/// Resolve the declarative `risk_tiers:` section. A malformed section or
/// entry is diagnosed and omitted from the usable map; the owning risk gate
/// consumes that diagnostic and fails closed rather than treating it as absent.
fn resolve_risk_tiers(raw: Option<serde_yaml::Value>, diagnostics: &mut Vec<PolicyDiagnostic>) -> BTreeMap<String, RiskTierRule> {
    const SECTION: &str = "risk_tiers";
    let Some(raw) = raw else {
        return BTreeMap::new();
    };
    let serde_yaml::Value::Mapping(entries) = raw else {
        diagnostics.push(PolicyDiagnostic::InvalidSection {
            section: SECTION,
            detail: "expected a mapping of tier names to {rank, paths, effects, min_human_approvals}".to_string(),
        });
        return BTreeMap::new();
    };

    let mut resolved = BTreeMap::new();
    for (raw_name, raw_rule) in entries {
        let Some(name) = raw_name.as_str() else {
            diagnostics.push(PolicyDiagnostic::InvalidSection {
                section: SECTION,
                detail: format!("tier name {raw_name:?} is not a string"),
            });
            continue;
        };
        let parsed: RawRiskTierRule = match serde_yaml::from_value(raw_rule) {
            Ok(rule) => rule,
            Err(error) => {
                diagnostics.push(PolicyDiagnostic::InvalidSection {
                    section: SECTION,
                    detail: format!("tier `{name}` is invalid: {error}"),
                });
                continue;
            }
        };
        let detail = if parsed.rank == 0 {
            Some("rank must be greater than zero".to_string())
        } else if parsed.paths.is_empty() && parsed.effects.is_empty() {
            Some("paths or effects must contain at least one matcher".to_string())
        } else if parsed.paths.iter().any(|path| path.is_empty()) {
            Some("paths must not contain empty patterns".to_string())
        } else if parsed.effects.iter().any(|effect| !is_kebab_slug(effect)) {
            Some("effects must be non-empty kebab-case slugs".to_string())
        } else {
            None
        };
        if let Some(detail) = detail {
            diagnostics.push(PolicyDiagnostic::InvalidSection { section: SECTION, detail: format!("tier `{name}` is invalid: {detail}") });
            continue;
        }
        resolved.insert(
            name.to_string(),
            RiskTierRule { rank: parsed.rank, paths: parsed.paths, effects: parsed.effects, min_human_approvals: parsed.min_human_approvals },
        );
    }
    resolved
}


/// `policy.yaml`'s optional `spec_coverage:` section (s44
/// `spec-derived-worklist`), resolved. `None` on
/// [`PolicyResolution::spec_coverage`] means the section is ABSENT and
/// `crate::spec_coverage` derives zero violations — the same
/// opt-in-by-omission default an empty `risk_routing` already gives
/// `crate::coverage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecCoverage {
    /// A well-formed section. `require_evidence: false` is a real,
    /// distinct state from an absent section: the repo opted in and
    /// then turned the requirement off, which stays visible in
    /// `canon context` where an absent section does not.
    Active {
        require_evidence: bool,
        /// Restrict blocking to scenarios linked to a Subject in one of
        /// these statuses. EMPTY means every scenario is in scope.
        scope: Vec<SubjectStatus>,
        /// Scenario lanes excluded after the subject-status scope filter.
        exclude_lanes: Vec<String>,
    },
    /// Present but unusable — see [`PolicyDiagnostic::InvalidSection`].
    /// `crate::spec_coverage` emits a violation carrying `detail`
    /// rather than silently behaving like the absent case.
    Invalid { detail: String },
}

/// The typed shape [`resolve_spec_coverage`] parses the untyped section
/// into. `deny_unknown_fields` on purpose: a misspelled key in an
/// opt-in security-adjacent section must be loud, not ignored.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct RawSpecCoverage {
    #[serde(default)]
    require_evidence: bool,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    exclude_lanes: Vec<String>,
}

/// Parse the `spec_coverage:` section, poisoning it (never dropping it)
/// when it is present and broken.
///
/// Absent → `None`, the opt-out default. Present and well-formed →
/// `Some(Active)`. Present and broken → a
/// [`PolicyDiagnostic::InvalidSection`] AND `Some(Invalid)`, because
/// `resolve()` is frozen infallible and cannot refuse here: returning
/// `None` would make a typo indistinguishable from never opting in,
/// which is the one outcome an evidence gate must not produce.
fn resolve_spec_coverage(raw: Option<serde_yaml::Value>, diagnostics: &mut Vec<PolicyDiagnostic>) -> Option<SpecCoverage> {
    const SECTION: &str = "spec_coverage";
    let raw = raw?;

    let parsed: RawSpecCoverage = match serde_yaml::from_value(raw) {
        Ok(p) => p,
        Err(e) => {
            let detail = e.to_string();
            diagnostics.push(PolicyDiagnostic::InvalidSection { section: SECTION, detail: detail.clone() });
            return Some(SpecCoverage::Invalid { detail });
        }
    };

    let mut scope = Vec::with_capacity(parsed.scope.len());
    for name in &parsed.scope {
        match subject_status_from_str(name) {
            Some(status) => scope.push(status),
            None => {
                let detail = format!("`{name}` is not a valid value for `scope` (expected one of: {})", SUBJECT_STATUS_NAMES.join(", "));
                diagnostics.push(PolicyDiagnostic::InvalidSection { section: SECTION, detail: detail.clone() });
                return Some(SpecCoverage::Invalid { detail });
            }
        }
    }

    for lane in &parsed.exclude_lanes {
        if !is_kebab_slug(lane) {
            let detail = format!("`{lane}` is not a valid value for `exclude_lanes` (expected a kebab-case slug)");
            diagnostics.push(PolicyDiagnostic::InvalidSection { section: SECTION, detail: detail.clone() });
            return Some(SpecCoverage::Invalid { detail });
        }
    }

    Some(SpecCoverage::Active { require_evidence: parsed.require_evidence, scope, exclude_lanes: parsed.exclude_lanes })
}

/// Validate a lane's shape before policy resolution accepts it.
fn is_kebab_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The closed [`SubjectStatus`] wire vocabulary, in lifecycle order —
/// named here so a `scope` rejection can print the legal set in the
/// same "expected one of: …" grammar the typed-atom checker uses.
const SUBJECT_STATUS_NAMES: [&str; 6] = ["proposed", "specced", "building", "verifying", "shipped", "retired"];

fn subject_status_from_str(s: &str) -> Option<SubjectStatus> {
    match s {
        "proposed" => Some(SubjectStatus::Proposed),
        "specced" => Some(SubjectStatus::Specced),
        "building" => Some(SubjectStatus::Building),
        "verifying" => Some(SubjectStatus::Verifying),
        "shipped" => Some(SubjectStatus::Shipped),
        "retired" => Some(SubjectStatus::Retired),
        _ => None,
    }
}

/// The resolved `policy.yaml`, S12 design D2's single shared object:
/// `canon context`, `canon fmt`, and `canon gate` all call
/// [`PolicyResolution::resolve`] and nothing else to obtain policy
/// data (design decision 3/7's D7 discipline: facts on artifacts,
/// routing in `policy.yaml`).
#[derive(Debug, Clone)]
pub struct PolicyResolution {
    /// Per-severity (or whatever key vocabulary a repo's `policy.yaml`
    /// declares — canon does not hardcode a severity enum) required
    /// [`TrustLevel`]. An absent key means "no declared requirement",
    /// not "no trust required" — S5 wave-2's release check (D7,
    /// `trust-below-required`) decides what absence means for its own
    /// gate.
    pub trust_required: BTreeMap<String, PolicyField<TrustLevel>>,
    /// Per-key spot-check sampling fraction (report-only, D21 pattern
    /// 3.4 — never a gate input; S9's dashboard consumes it).
    pub trust_sample: BTreeMap<String, PolicyField<f64>>,
    pub staleness: StalenessPolicy,
    /// Open, repo-declared routing keys (D7's "risk→platform fan-out"
    /// generalized past the donor consumer repo's specific platform vocabulary,
    /// static-gate.md recommendation 7's explicit SKIP on porting
    /// the donor's platform-specific routing semantics verbatim) — each key
    /// resolves to a boolean ("does this routing rule apply"),
    /// matching `canon_policy::PolicyValue`'s scalar-only evaluation
    /// contract.
    pub risk_routing: BTreeMap<String, PolicyField<bool>>,
    /// Declarative effect-aware approval tiers. An empty map is a strict
    /// no-op only when the section is absent and has no `risk_tiers`
    /// diagnostic; the owning gate fails closed for a present malformed
    /// section. These rules are intentionally separate from risk routing.
    pub risk_tiers: BTreeMap<String, RiskTierRule>,
    /// s44's opt-in spec-corpus coverage requirement. `None` = the
    /// section is absent and `crate::spec_coverage` is silent; see
    /// [`SpecCoverage`] for why a BROKEN section resolves to
    /// `Some(Invalid)` rather than `None`.
    pub spec_coverage: Option<SpecCoverage>,
    /// Every load/compile problem `resolve()` encountered — see
    /// [`PolicyDiagnostic`].
    pub diagnostics: Vec<PolicyDiagnostic>,
}

impl PolicyResolution {
    /// THE single resolver S12's `canon context` and S5's own gate
    /// both call (module doc). Loads `<repo>/.canon/policy.yaml`,
    /// compiles every `{cel: ...}` field against `registry`-derived
    /// bindings (write-time validation, `canon_policy::compile`), and
    /// returns a resolution that is ALWAYS usable — a missing file, a
    /// YAML parse error, or one invalid predicate degrades gracefully
    /// to documented defaults / entry omission rather than failing the
    /// whole call (module doc's fail-soft-load discipline).
    pub fn resolve(repo: &Path, registry: &SchemaRegistry) -> Self {
        let mut diagnostics = Vec::new();
        let policy_path = repo.join(POLICY_YAML_RELATIVE_PATH);
        let raw = load_raw_policy(&policy_path, &mut diagnostics);

        // `canon_policy::bindings_for` PANICS when `registry` has no
        // schema for `POLICY_BINDING_KIND` (its own doc: "every
        // RecordKind::ALL member is exported by
        // canon_model::schema_export::record_schemas()" — an
        // assumption a fixture `SchemaRegistry::single(...)` keyed to
        // a different kind violates). `resolve()`'s signature is
        // FROZEN infallible (module doc, S12 design D2) — that panic
        // must never reach a caller, so the registry is checked FIRST
        // and `bindings_for` is only ever called once we know it
        // cannot panic.
        let bindings = match registry.get(POLICY_BINDING_KIND) {
            Some(_) => Some(bindings_for(POLICY_BINDING_KIND, registry)),
            None => {
                diagnostics.push(PolicyDiagnostic::SchemaUnavailable { kind: POLICY_BINDING_KIND });
                None
            }
        };
        let bindings = bindings.as_ref();

        let trust_required = compile_map(raw.trust_required, "trust_required", bindings, &mut diagnostics);
        let trust_sample = compile_map(raw.trust_sample, "trust_sample", bindings, &mut diagnostics);
        let staleness = StalenessPolicy {
            max_commits_behind: compile_single(
                raw.staleness.max_commits_behind,
                "staleness.max_commits_behind",
                DEFAULT_MAX_COMMITS_BEHIND,
                bindings,
                &mut diagnostics,
            ),
            surface_scoped: compile_single(raw.staleness.surface_scoped, "staleness.surface_scoped", DEFAULT_SURFACE_SCOPED, bindings, &mut diagnostics),
        };
        let risk_routing = compile_map(raw.risk_routing, "risk_routing", bindings, &mut diagnostics);
        let risk_tiers = resolve_risk_tiers(raw.risk_tiers, &mut diagnostics);
        // Parsed, never CEL-compiled: `spec_coverage` carries no
        // per-record predicate, so it needs no bindings and is
        // unaffected by `SchemaUnavailable`.
        let spec_coverage = resolve_spec_coverage(raw.spec_coverage, &mut diagnostics);

        Self { trust_required, trust_sample, staleness, risk_routing, risk_tiers, spec_coverage, diagnostics }
    }

    /// The required [`TrustLevel`] for `key` (whatever vocabulary this
    /// repo's `policy.yaml` declares — `"p1"`, `"p2"`, …), resolved
    /// against `record`. `None` when `key` has no `trust_required`
    /// entry at all.
    pub fn trust_required_for(&self, key: &str, record: &serde_json::Value, now: DateTime<Utc>) -> Result<Option<TrustLevel>, PolicyResolveError> {
        self.trust_required.get(key).map(|field| field.resolve(record, now)).transpose()
    }

    /// The spot-check sampling fraction for `key`, resolved against
    /// `record`. `None` when `key` has no `trust_sample` entry.
    pub fn trust_sample_for(&self, key: &str, record: &serde_json::Value, now: DateTime<Utc>) -> Result<Option<f64>, PolicyResolveError> {
        self.trust_sample.get(key).map(|field| field.resolve(record, now)).transpose()
    }

    pub fn max_commits_behind(&self, record: &serde_json::Value, now: DateTime<Utc>) -> Result<u32, PolicyResolveError> {
        self.staleness.max_commits_behind.resolve(record, now)
    }

    pub fn surface_scoped(&self, record: &serde_json::Value, now: DateTime<Utc>) -> Result<bool, PolicyResolveError> {
        self.staleness.surface_scoped.resolve(record, now)
    }

    /// Whether routing rule `key` applies to `record`. `None` when
    /// `key` has no `risk_routing` entry.
    pub fn risk_routing_for(&self, key: &str, record: &serde_json::Value, now: DateTime<Utc>) -> Result<Option<bool>, PolicyResolveError> {
        self.risk_routing.get(key).map(|field| field.resolve(record, now)).transpose()
    }

    /// Whether `resolve()` encountered zero problems — a repo whose
    /// `policy.yaml` is missing/malformed/carries an invalid predicate
    /// still returns a USABLE `PolicyResolution` (module doc), but a
    /// caller that wants to surface load health (e.g. `canon context`)
    /// checks this.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

fn load_raw_policy(path: &Path, diagnostics: &mut Vec<PolicyDiagnostic>) -> RawPolicy {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_yaml::from_str(&content).unwrap_or_else(|e| {
            diagnostics.push(PolicyDiagnostic::Malformed { path: path.to_path_buf(), detail: e.to_string() });
            RawPolicy::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            diagnostics.push(PolicyDiagnostic::Missing { path: path.to_path_buf() });
            RawPolicy::default()
        }
        Err(e) => {
            diagnostics.push(PolicyDiagnostic::Malformed { path: path.to_path_buf(), detail: e.to_string() });
            RawPolicy::default()
        }
    }
}

/// Compiles one raw field, dropping (with a diagnostic) a `{cel: ...}`
/// entry that fails write-time validation OR — [`resolve()`]'s own
/// infallibility guard — that has no `bindings` to compile against at
/// all (`bindings: None` means `registry` had no schema for
/// [`POLICY_BINDING_KIND`]; [`PolicyDiagnostic::SchemaUnavailable`] is
/// already recorded once by the caller, this pushes the PER-FIELD
/// [`PolicyDiagnostic::InvalidPredicate`] naming which predicate
/// degraded). A `Flat` value never touches `bindings` at all — it
/// resolves regardless. `None` return means "no resolved value for
/// this entry" — the caller decides the fallback (map entries are
/// simply omitted; [`compile_single`] substitutes a documented
/// default).
fn compile_field<T>(raw: RawField<T>, field: String, bindings: Option<&BindingSet>, diagnostics: &mut Vec<PolicyDiagnostic>) -> Option<PolicyField<T>> {
    match raw {
        RawField::Flat(value) => Some(PolicyField::Flat(value)),
        RawField::Cel(spec) => match bindings {
            None => {
                diagnostics.push(PolicyDiagnostic::InvalidPredicate {
                    field,
                    source: spec.cel,
                    errors: vec![format!("SchemaRegistry has no schema for {POLICY_BINDING_KIND:?} — CEL bindings unavailable, predicate not compiled")],
                });
                None
            }
            Some(bindings) => match compile(&spec.cel, bindings) {
                Ok(compiled) => Some(PolicyField::Cel(compiled)),
                Err(errors) => {
                    diagnostics.push(PolicyDiagnostic::InvalidPredicate {
                        field,
                        source: spec.cel,
                        errors: errors.iter().map(ToString::to_string).collect(),
                    });
                    None
                }
            },
        },
    }
}

fn compile_map<T>(raw: BTreeMap<String, RawField<T>>, field_prefix: &str, bindings: Option<&BindingSet>, diagnostics: &mut Vec<PolicyDiagnostic>) -> BTreeMap<String, PolicyField<T>> {
    raw.into_iter().filter_map(|(key, raw_field)| compile_field(raw_field, format!("{field_prefix}.{key}"), bindings, diagnostics).map(|field| (key, field))).collect()
}

fn compile_single<T>(raw: Option<RawField<T>>, field: &str, default: T, bindings: Option<&BindingSet>, diagnostics: &mut Vec<PolicyDiagnostic>) -> PolicyField<T> {
    match raw {
        None => PolicyField::Flat(default),
        Some(raw_field) => compile_field(raw_field, field.to_string(), bindings, diagnostics).unwrap_or(PolicyField::Flat(default)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn fixture_record(verdict: &str, at: DateTime<Utc>) -> serde_json::Value {
        json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": at.to_rfc3339(),
            "actor": {"agent_id": "test-agent"},
            "verdict": verdict,
        })
    }

    fn write_policy(dir: &TempDir, contents: &str) -> PathBuf {
        let canon_dir = dir.path().join(".canon");
        std::fs::create_dir_all(&canon_dir).unwrap();
        let path = canon_dir.join("policy.yaml");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn missing_policy_yaml_resolves_to_documented_defaults_with_a_diagnostic() {
        let dir = TempDir::new().unwrap();
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);

        assert!(!resolution.is_clean());
        assert!(matches!(resolution.diagnostics[0], PolicyDiagnostic::Missing { .. }));
        assert!(resolution.trust_required.is_empty());

        let record = fixture_record("faithful", Utc::now());
        assert_eq!(resolution.max_commits_behind(&record, Utc::now()).unwrap(), DEFAULT_MAX_COMMITS_BEHIND);
        assert!(resolution.surface_scoped(&record, Utc::now()).unwrap());
    }

    #[test]
    fn malformed_policy_yaml_resolves_to_defaults_with_a_diagnostic() {
        let dir = TempDir::new().unwrap();
        write_policy(&dir, "trust_required: [this is not a mapping ::");
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);

        assert!(!resolution.is_clean());
        assert!(matches!(resolution.diagnostics[0], PolicyDiagnostic::Malformed { .. }));
        assert!(resolution.trust_required.is_empty());
    }

    #[test]
    fn flat_trust_required_resolves_regardless_of_record() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
trust_required:
  p1: human
  p2: agent
"#,
        );
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);
        assert!(resolution.is_clean(), "diagnostics: {:?}", resolution.diagnostics);

        let now = Utc::now();
        let record = fixture_record("divergent", now);
        assert_eq!(resolution.trust_required_for("p1", &record, now).unwrap(), Some(TrustLevel::Human));
        assert_eq!(resolution.trust_required_for("p2", &record, now).unwrap(), Some(TrustLevel::Agent));
        assert_eq!(resolution.trust_required_for("p3", &record, now).unwrap(), None);
    }

    /// The acceptance criterion: a flat-value `trust_required.p1` and
    /// an equivalent CEL-predicate `trust_required.p1` agree on the
    /// resolved trust level across a small fixture corpus of records
    /// (mirroring `canon-policy`'s own S13 equivalence fixture,
    /// `crates/canon-policy/tests/equivalence.rs`, applied here through
    /// `PolicyResolution::resolve` itself rather than calling
    /// `canon-policy` directly).
    #[test]
    fn flat_and_cel_trust_required_agree_on_the_resolved_required_cell_set() {
        let flat_dir = TempDir::new().unwrap();
        write_policy(
            &flat_dir,
            r#"
trust_required:
  p1: human
"#,
        );
        let cel_dir = TempDir::new().unwrap();
        write_policy(
            &cel_dir,
            r#"
trust_required:
  p1:
    cel: "'human'"
"#,
        );

        let registry = SchemaRegistry::load();
        let flat_resolution = PolicyResolution::resolve(flat_dir.path(), &registry);
        let cel_resolution = PolicyResolution::resolve(cel_dir.path(), &registry);
        assert!(flat_resolution.is_clean());
        assert!(cel_resolution.is_clean(), "diagnostics: {:?}", cel_resolution.diagnostics);

        // The flat form is a policy CONSTANT (D7's floor case: no
        // per-artifact judgement at all) — an EQUIVALENT CEL
        // predicate must agree for every record in the corpus, not
        // just one verdict, since the flat policy makes no
        // distinction by verdict at all.
        let now = Utc::now();
        for verdict in ["faithful", "not_applicable", "divergent"] {
            let record = fixture_record(verdict, now);
            let from_flat = flat_resolution.trust_required_for("p1", &record, now).unwrap();
            let from_cel = cel_resolution.trust_required_for("p1", &record, now).unwrap();
            assert_eq!(from_flat, Some(TrustLevel::Human), "flat form, verdict={verdict}");
            assert_eq!(from_cel, Some(TrustLevel::Human), "cel form, verdict={verdict}");
            assert_eq!(from_flat, from_cel, "flat/cel disagreement at verdict={verdict}");
        }
    }

    /// A second equivalence fixture where the CEL form is genuinely
    /// conditional (D7's actual "tightening coverage is a policy diff"
    /// example: p3 usually needs only agent trust, but a CEL predicate
    /// can route stale-looking evidence to human review without any
    /// change to the artifact corpus) — verified against a static Rust
    /// re-implementation of the identical rule, matching
    /// `canon-policy`'s own equivalence-test shape
    /// (`crates/canon-policy/tests/equivalence.rs`).
    #[test]
    fn conditional_cel_trust_required_agrees_with_an_equivalent_static_rule() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
trust_required:
  p3:
    cel: "record.verdict == 'divergent' ? 'human' : 'agent'"
"#,
        );
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);
        assert!(resolution.is_clean(), "diagnostics: {:?}", resolution.diagnostics);

        fn static_rule(verdict: &str) -> TrustLevel {
            if verdict == "divergent" {
                TrustLevel::Human
            } else {
                TrustLevel::Agent
            }
        }

        let now = Utc::now();
        for verdict in ["faithful", "not_applicable", "divergent"] {
            let record = fixture_record(verdict, now);
            let from_cel = resolution.trust_required_for("p3", &record, now).unwrap();
            assert_eq!(from_cel, Some(static_rule(verdict)), "verdict={verdict}");
        }
    }

    #[test]
    fn cel_staleness_fields_resolve_against_a_record() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
staleness:
  max_commits_behind: 10
  surface_scoped:
    cel: "record.verdict != 'divergent'"
"#,
        );
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);
        assert!(resolution.is_clean(), "diagnostics: {:?}", resolution.diagnostics);

        let now = Utc::now();
        assert_eq!(resolution.max_commits_behind(&fixture_record("faithful", now), now).unwrap(), 10);
        assert!(resolution.surface_scoped(&fixture_record("faithful", now), now).unwrap());
        assert!(!resolution.surface_scoped(&fixture_record("divergent", now), now).unwrap());
    }

    #[test]
    fn an_invalid_predicate_is_dropped_with_a_diagnostic_others_still_resolve() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
trust_required:
  p1: human
  p2:
    cel: "record.nonexistent_field == 'x'"
"#,
        );
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);

        assert!(!resolution.is_clean());
        assert!(matches!(&resolution.diagnostics[0], PolicyDiagnostic::InvalidPredicate { field, .. } if field == "trust_required.p2"));

        let now = Utc::now();
        let record = fixture_record("faithful", now);
        assert_eq!(resolution.trust_required_for("p1", &record, now).unwrap(), Some(TrustLevel::Human));
        assert_eq!(resolution.trust_required_for("p2", &record, now).unwrap(), None);
    }

    #[test]
    fn risk_routing_resolves_flat_and_cel() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
risk_routing:
  always_on: true
  divergent_only:
    cel: "record.verdict == 'divergent'"
"#,
        );
        let registry = SchemaRegistry::load();
        let resolution = PolicyResolution::resolve(dir.path(), &registry);
        assert!(resolution.is_clean(), "diagnostics: {:?}", resolution.diagnostics);

        let now = Utc::now();
        let faithful = fixture_record("faithful", now);
        let divergent = fixture_record("divergent", now);
        assert_eq!(resolution.risk_routing_for("always_on", &faithful, now).unwrap(), Some(true));
        assert_eq!(resolution.risk_routing_for("divergent_only", &faithful, now).unwrap(), Some(false));
        assert_eq!(resolution.risk_routing_for("divergent_only", &divergent, now).unwrap(), Some(true));
        assert_eq!(resolution.risk_routing_for("unknown_key", &faithful, now).unwrap(), None);
    }
    #[test]
    fn risk_tiers_resolve_rank_paths_effects_and_minimum() {
        let (resolution, _dir) = resolve_with(
            "risk_tiers:\n  high:\n    rank: 3\n    paths: [\"src/auth/**\"]\n    effects: [secret-access]\n    min_human_approvals: 1\n",
        );
        assert_eq!(
            resolution.risk_tiers.get("high"),
            Some(&RiskTierRule {
                rank: 3,
                paths: vec!["src/auth/**".to_string()],
                effects: vec!["secret-access".to_string()],
                min_human_approvals: 1,
            })
        );
    }

    #[test]
    fn malformed_risk_tier_is_diagnosed_and_not_enabled() {
        let (resolution, _dir) = resolve_with(
            "risk_tiers:\n  broken:\n    rank: 0\n    paths: []\n    effects: []\n    min_human_approvals: 1\n",
        );
        assert!(resolution.risk_tiers.is_empty());
        assert!(resolution.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            PolicyDiagnostic::InvalidSection { section: "risk_tiers", detail } if detail.contains("broken")
        )));
    }


    /// The second Important review finding this fix closes:
    /// `canon_policy::bindings_for` PANICS when the registry has no
    /// schema for `POLICY_BINDING_KIND` (`EvidenceRecord`) — e.g. a
    /// fixture `SchemaRegistry::single(...)` keyed to a different
    /// kind. `resolve()`'s signature is FROZEN infallible (no
    /// `Result`); reaching every assertion below (rather than the
    /// test process aborting on an unwind-across-FFI-unsafe panic) is
    /// itself part of the proof this never panics.
    #[test]
    fn resolve_is_infallible_when_registry_lacks_evidence_record_schema() {
        let dir = TempDir::new().unwrap();
        write_policy(
            &dir,
            r#"
trust_required:
  p1: human
  p2:
    cel: "record.verdict == 'divergent' ? 'human' : 'agent'"
"#,
        );

        // A registry keyed to a DIFFERENT `RecordKind` than
        // `POLICY_BINDING_KIND` — exactly `bindings_for`'s documented
        // panic condition.
        let handoff_schema = canon_model::schema_export::record_schemas()
            .into_iter()
            .find(|(kind, _)| *kind == RecordKind::Handoff)
            .expect("canon-model exports a Handoff schema")
            .1;
        let registry = SchemaRegistry::single(RecordKind::Handoff, handoff_schema);

        let resolution = PolicyResolution::resolve(dir.path(), &registry);

        assert!(!resolution.is_clean());
        assert!(resolution.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::SchemaUnavailable { kind } if *kind == RecordKind::EvidenceRecord)));
        // p2's `{cel: ...}` predicate can never compile without
        // bindings — dropped with its OWN diagnostic naming it, same
        // as any other invalid predicate.
        assert!(resolution.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::InvalidPredicate { field, .. } if field == "trust_required.p2")));

        let now = Utc::now();
        let record = fixture_record("faithful", now);
        // p1 is a FLAT value — never touches the registry, resolves
        // regardless of the missing schema.
        assert_eq!(resolution.trust_required_for("p1", &record, now).unwrap(), Some(TrustLevel::Human));
        // p2 never compiled — no entry (`None`), never a crash.
        assert_eq!(resolution.trust_required_for("p2", &record, now).unwrap(), None);
        // Every OTHER field still resolves to its documented default.
        assert_eq!(resolution.max_commits_behind(&record, now).unwrap(), DEFAULT_MAX_COMMITS_BEHIND);
        assert!(resolution.surface_scoped(&record, now).unwrap());
    }

    // ── s44: the `spec_coverage:` section ──

    fn resolve_with(contents: &str) -> (PolicyResolution, TempDir) {
        let dir = TempDir::new().unwrap();
        write_policy(&dir, contents);
        let resolution = PolicyResolution::resolve(dir.path(), &SchemaRegistry::load());
        (resolution, dir)
    }

    /// Opt-in by omission: this is the default every existing consumer
    /// upgrades into, and it must derive nothing.
    #[test]
    fn an_absent_spec_coverage_section_resolves_to_none() {
        let (resolution, _dir) = resolve_with("trust_required:\n  test-run: agent\n");
        assert_eq!(resolution.spec_coverage, None);
        assert!(
            !resolution.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::InvalidSection { .. })),
            "an absent section is not a problem: {:?}",
            resolution.diagnostics
        );
    }

    #[test]
    fn a_well_formed_section_resolves_its_scope_to_typed_statuses() {
        let (resolution, _dir) = resolve_with("spec_coverage:\n  require_evidence: true\n  scope: [building, verifying]\n");
        assert_eq!(
            resolution.spec_coverage,
            Some(SpecCoverage::Active { require_evidence: true, scope: vec![SubjectStatus::Building, SubjectStatus::Verifying], exclude_lanes: Vec::new() })
        );
    }

    /// An omitted `scope` means corpus-wide, not "nothing in scope" —
    /// the opposite reading would make the section silently inert.
    #[test]
    fn an_omitted_scope_means_every_scenario() {
        let (resolution, _dir) = resolve_with("spec_coverage:\n  require_evidence: true\n");
        assert_eq!(resolution.spec_coverage, Some(SpecCoverage::Active { require_evidence: true, scope: Vec::new(), exclude_lanes: Vec::new() }));
    }

    /// The frozen-infallible resolver cannot refuse, so a broken
    /// section must POISON rather than vanish. If it resolved to `None`
    /// a typo would be silently identical to never opting in.
    #[test]
    fn an_unknown_scope_status_poisons_the_section_and_names_the_legal_set() {
        let (resolution, _dir) = resolve_with("spec_coverage:\n  require_evidence: true\n  scope: [buidling]\n");
        let Some(SpecCoverage::Invalid { detail }) = &resolution.spec_coverage else {
            panic!("expected a poisoned section, got {:?}", resolution.spec_coverage);
        };
        assert!(detail.contains("buidling"), "must name the offender: {detail}");
        assert!(detail.contains("expected one of: proposed, specced, building, verifying, shipped, retired"), "must name the legal set: {detail}");
        assert!(resolution.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::InvalidSection { section: "spec_coverage", .. })));
    }

    #[test]
    fn an_invalid_exclude_lane_poisons_the_section() {
        let (resolution, _dir) = resolve_with("spec_coverage:\n  require_evidence: true\n  exclude_lanes: [Not-a-lane]\n");
        let Some(SpecCoverage::Invalid { detail }) = &resolution.spec_coverage else {
            panic!("expected a poisoned section, got {:?}", resolution.spec_coverage);
        };
        assert!(detail.contains("Not-a-lane"), "must name the offender: {detail}");
        assert!(detail.contains("exclude_lanes"), "must name the field: {detail}");
    }

    #[test]
    fn a_misspelled_key_poisons_the_section_rather_than_being_ignored() {
        let (resolution, _dir) = resolve_with("spec_coverage:\n  require_evidenc: true\n");
        assert!(matches!(resolution.spec_coverage, Some(SpecCoverage::Invalid { .. })), "got {:?}", resolution.spec_coverage);
    }

    /// The reason `RawPolicy::spec_coverage` is an untyped
    /// `serde_yaml::Value`: a typed field would fail the whole-file
    /// deserialize, and `load_raw_policy` degrades that to
    /// `RawPolicy::default()` — silently discarding every OTHER section.
    #[test]
    fn a_broken_spec_coverage_section_does_not_discard_the_rest_of_the_file() {
        let (resolution, _dir) = resolve_with("trust_required:\n  test-run: agent\nspec_coverage:\n  scope: [nonsense]\n");
        assert!(matches!(resolution.spec_coverage, Some(SpecCoverage::Invalid { .. })));
        assert!(resolution.trust_required.contains_key("test-run"), "the sibling section must survive: {:?}", resolution.trust_required.keys());
    }
}
