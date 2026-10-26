//! `canon context [--repo <dir>] [--json]` (S12 `context-authoring-surface`):
//! the authoring surface an agent needs BEFORE writing any canon artifact —
//! record kinds + envelope fields, enum domains, join-key grammars,
//! partition layout, policy-derived requirements, and a capability version.
//!
//! # Ported from the donor authoring-surface implementation (operator-attributed port)
//! Generalizes the donor authoring-surface tool's two-phase "resolve
//! once, render twice" flow and its compact-outline shape from the
//! donor's single DSL document to canon's whole artifact family
//! (design D1/D3/D5):
//! - `resolve_surface` mirrors `run_context`'s own structure — ported
//!   from the donor's `run_context`:
//!   resolve once (there, `build_input`/`fold_env`; here,
//!   `SchemaRegistry::load`/`PolicyResolution::resolve`), then hand the ONE
//!   resolved value to either renderer — never a second resolution path per
//!   output mode.
//! - The `ContextReport` BTreeMap-everywhere/sorted-array determinism
//!   contract (design D3) is `authoring_surface`'s own doc comment, ported
//!   verbatim in spirit — ported from the donor's `authoring_surface`.
//! - `render_outline` mirrors `context_outline`'s compact, per-section
//!   "name list, then one indented line per entry" shape — ported from
//!   the donor's `context_outline`.
//!
//! # The three S12 invariants (design, proposal.md)
//! 1. **Capability query, not validation** — [`resolve_surface`] never runs
//!    `canon fmt`'s corpus walk or `canon gate`'s evidence-corpus read
//!    (`canon_gate::GateContext::load` is never called here); a repo whose
//!    corpus fails `canon fmt --check` still gets a full, unchanged surface.
//! 2. **Same registry as the validator** — [`resolve_surface`] calls exactly
//!    `canon_policy::SchemaRegistry::load()` and
//!    `canon_gate::PolicyResolution::resolve(repo, &registry)`, the SAME two
//!    calls `canon fmt`/`canon gate` make (`canon_gate::context::GateContext::load`
//!    calls the identical `PolicyResolution::resolve(&ctx.repo, registry)`) —
//!    no second, independently-loaded copy of schema or policy data. S10
//!    part2 extends this invariant to the typed authoring vocabulary
//!    (design.md D3): [`resolve_surface`] also calls exactly
//!    `canon_vocab::resolve_snapshot(repo, None)` — the SAME function
//!    `canon-vocab`'s own checker calls to validate a typed task-atom/
//!    handoff-body file (`crate::gate`'s typed evidence path, task 4.4,
//!    calls it again fresh at gate time for the identical reason) — so
//!    [`ContextReport::vocab`] can never diverge from what a typed
//!    atom is actually checked against.
//! 3. **Deterministic output** — every map below is a `BTreeMap` (key-sorted
//!    by construction) and every array is sorted or built from an already-
//!    sorted source; `render_json`/`render_outline` both read the one
//!    resolved [`ContextReport`] value, never re-resolving anything.
//!    [`canon_vocab::CapabilitySnapshot`] holds the same discipline
//!    end-to-end (every map a `BTreeMap`), so embedding it verbatim in
//!    [`ContextReport`] introduces no new non-determinism.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use canon_gate::{PolicyField, PolicyResolution, RiskTierRule, SpecCoverage};
use canon_model::SubjectStatus;
use canon_policy::SchemaRegistry;
use canon_vocab::CapabilitySnapshot;
use schemars::JsonSchema;
use serde::Serialize;

use crate::json_out::Report;
use crate::cli_error::CliError;

/// The authoring-surface CAPABILITY version — `canon context`'s own
/// outline/JSON contract version, deliberately NOT any record kind's
/// `Envelope.schema`. Those became genuinely per-kind in
/// `s38-evidence-bearing-memory` (`Task` is generation `2`, every other
/// kind `1`), which is exactly the day this doc anticipated: each
/// [`KindSurface::schema_version`] is now the lookup
/// [`canon_model::RecordKind::schema_version`] answers, so the surface
/// can never advertise a generation nothing writes, while this constant
/// keeps describing the CLI surface itself (design D1's "resolve, then
/// render" split — the registry call site stays singular either way).
/// Bumped to `3` for the effect-aware `risk_tiers` policy projection,
/// to `4` for the `review` section (issue #2), and to `5` when
/// `policy.spec_coverage`, `policy.evidence_binding`, and
/// `review.requireReview` became typed JSON objects instead of one-line
/// summaries.
///
/// Bumped to `6` (0.28): `commands` (the command index, from the generated
/// spec) and `families` joined the surface, the evidence kind is named
/// `evidence` (what `--kind` takes), and each kind carries its schema link.
const CURRENT_CAPABILITY_VERSION: u32 = 6;

/// Resolution-time options beyond the repo root itself. Empty today —
/// `canon context` takes only `--repo`/`--json`, and `--json` selects a
/// RENDERER, never a resolution input (design D5). `role` is `--as`: it
/// narrows `commands` to that role's leaves and nothing else — `CANON_ROLE`
/// never reaches the context.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextOptions {
    pub role: Option<crate::role::Role>,
}

/// The Hive-style partition layout one record kind's git-tier files follow —
/// design D3's `partition: LayoutDescriptor`, built from
/// [`RecordKind::partition_template`]/[`RecordKind::is_area_scoped`] (S1's
/// own registry — never a second, hand-written layout table).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartitionLayout {
    /// The path template, e.g. `kind={kind}/{id}.json` or
    /// `kind={kind}/area={area}/{id}.json`.
    pub template: String,
    /// Whether `template` requires the `area={area}/` segment.
    pub area_scoped: bool,
}

/// One record kind's authoring shape: the fields an agent must supply, the
/// partition it lands in, and its current schema version.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KindSurface {
    /// This kind's CURRENT format generation, read straight from
    /// [`canon_model::RecordKind::schema_version`] — never a
    /// hand-maintained copy, so `canon context` can never advertise a
    /// generation no writer stamps (`s38-evidence-bearing-memory`).
    pub schema_version: u32,
    /// Every top-level field name this kind's record carries (envelope
    /// fields `schema`/`kind`/`at`/`actor` flattened alongside the kind's
    /// own business fields — `#[serde(flatten)]` puts them all in the same
    /// JSON object, so "envelope fields" for authoring purposes means the
    /// whole top-level shape) mapped to whether it is required.
    pub envelope_fields: BTreeMap<String, bool>,
    pub partition: PartitionLayout,
    /// The kind's JSON Schema, relative to the repository root of canon itself.
    pub schema: String,
}

/// One artifact family (the files under `spec/`: ledger, divergences,
/// inventory, feature provenance, policy) and the record kinds that share its
/// name.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FamilySurface {
    /// `FamilyKind::as_str()`.
    pub family: String,
    /// The family's JSON Schema, next to the record kinds' schemas.
    pub schema: String,
    /// The record kinds (as `--kind` names them) whose wire name is this family's.
    pub record_kinds: Vec<String>,
}

/// One command a caller may run: a leaf of the generated spec, compact.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandSurface {
    /// The canonical space-separated path, e.g. `evidence add`.
    pub command: String,
    /// `read`, `write` or `destructive`.
    pub effect: &'static str,
    /// The roles whose steps use the command; empty means humans only.
    pub roles: Vec<crate::role::Role>,
    pub about: Option<String>,
    /// One runnable invocation.
    pub example: Option<&'static str>,
}

/// One `policy.yaml` field's authoring-time shape: either a fixed value or
/// a CEL predicate's source (never its evaluated result — [`resolve_surface`]
/// has no specific artifact to evaluate against, only the repo's resolved
/// policy shape, matching invariant 1's "describes what CAN be authored").
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyFieldSurface {
    Flat {
        /// The fixed value (the value itself is a [`serde_json::Value`] in memory; its schema is [`PolicyFlatValue`]).
        #[schemars(with = "PolicyFlatValue")]
        value: serde_json::Value,
    },
    Cel { expression: String },
}

/// What a fixed `policy.yaml` field can be: a flag, a count, a ratio or a name (a trust level).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum PolicyFlatValue {
    Flag(bool),
    Count(u64),
    Ratio(f64),
    Name(String),
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StalenessSurface {
    pub max_commits_behind: PolicyFieldSurface,
    pub surface_scoped: PolicyFieldSurface,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RiskTierSurface {
    pub rank: u32,
    pub paths: Vec<String>,
    pub effects: Vec<String>,
    pub min_human_approvals: u32,
}

/// `spec_coverage.require_review` in the `policy` section, keyed like
/// `policy.yaml`. `scope` is the resolved status list (the default
/// `[verifying, shipped]` when the key is omitted); empty means every
/// scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RequireReviewSurface {
    pub scope: Vec<String>,
    pub distinct_actor: bool,
    pub block_on_findings: bool,
}

/// `spec_coverage.require_audit` in the `policy` section, keyed like
/// `policy.yaml` (0.15 D3). `scope` is the resolved status list (the
/// default `[verifying, shipped]` when the key is omitted); empty means
/// every scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RequireAuditSurface {
    pub rate: serde_json::Number,
    pub seed: String,
    pub scope: Vec<String>,
}

/// The same requirement in the camelCase `review` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequirementSurface {
    pub scope: Vec<String>,
    pub distinct_actor: bool,
    pub block_on_findings: bool,
}

/// `spec_coverage`, keyed like `policy.yaml`. Serialized untagged: an
/// active section is its fields; a present-but-unusable one is
/// `{"invalid": "<detail>"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum SpecCoverageSurface {
    Active {
        require_evidence: bool,
        /// Empty means every scenario.
        scope: Vec<String>,
        exclude_lanes: Vec<String>,
        require_cases: Vec<String>,
        /// `null` when the sub-section is absent.
        require_review: Option<RequireReviewSurface>,
        /// Omitted when the sub-section is absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        require_audit: Option<RequireAuditSurface>,
    },
    Invalid { invalid: String },
}

/// `experimental.evidence_binding`, keyed like `policy.yaml` (`case`,
/// `lane`, `scope`; each empty means unfiltered). Untagged like
/// [`SpecCoverageSurface`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum EvidenceBindingSurface {
    Active { mode: &'static str, strength: &'static str, case: Vec<String>, lane: Vec<String>, scope: Vec<String> },
    Invalid { invalid: String },
}


/// `canon_gate::PolicyResolution`'s own fields, summarized field-by-field
/// via [`PolicyFieldSurface`] — one Rust field per `PolicyResolution` field,
/// so the mapping between "what the gate resolved" and "what context shows"
/// stays traceable by name, never a re-derived shape.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PolicySurface {
    pub trust_required: BTreeMap<String, PolicyFieldSurface>,
    pub trust_sample: BTreeMap<String, PolicyFieldSurface>,
    pub staleness: StalenessSurface,
    pub risk_routing: BTreeMap<String, PolicyFieldSurface>,
    pub risk_tiers: BTreeMap<String, RiskTierSurface>,
    /// s44's opt-in spec-corpus requirement. `None` (JSON `null`) = the
    /// section is absent, which is a DIFFERENT fact from
    /// `require_evidence: false` and stays distinguishable here: an
    /// author needs to see whether the repo opted in at all.
    pub spec_coverage: Option<SpecCoverageSurface>,
    /// `experimental.evidence_binding`. Skipped when absent (off by
    /// default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_binding: Option<EvidenceBindingSurface>,
    /// `PolicyResolution::is_clean()` — whether `policy.yaml` loaded with
    /// zero problems for this repo.
    pub clean: bool,
    /// `PolicyDiagnostic::to_string()` per problem `resolve()` recorded,
    /// in the order `resolve()` encountered them (deterministic per repo
    /// state, matching invariant 3 — this is never sorted, since it is
    /// already reproducible byte-for-byte on an unchanged repo).
    pub diagnostics: Vec<String>,
}

/// canon-vocab's own resolved capability snapshot (design.md D3, S10 part2
/// task 6.1): folded in verbatim from the SAME `canon_vocab::resolve_snapshot`
/// call the checker itself resolves against — no second, independently
/// re-derived projection of the typed authoring vocabulary (invariant 2).
/// `profile` is always `None` (the project's own `defaultProfile`, or
/// `"default"` with no `canon.project.yaml` at all) — `canon context` has
/// no `--profile` flag of its own yet; a future one would thread straight
/// through `resolve_surface`'s existing `(repo, opts)` shape without
/// touching this struct.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VocabularySurface {
    /// The resolved directive/enum/evidence-kind index a typed task-atom
    /// or handoff-body file is checked against right now — the exact
    /// value `crate::gate`'s typed evidence path (task 4.4) re-resolves
    /// fresh at gate time for the identical reason (Risks section: policy
    /// is the live source of truth, never an authoring-time snapshot).
    pub snapshot: CapabilitySnapshot,
    /// `canon_vocab::checker::Diagnostic` (plugin load/activation
    /// problems the resolution itself hit — a malformed
    /// `canon.project.yaml`, an unresolvable `depends` range),
    /// stringified one-per-entry in resolution order — mirrors
    /// `PolicySurface::diagnostics`'s own "already reproducible
    /// byte-for-byte on an unchanged repo" reasoning, never sorted.
    pub diagnostics: Vec<String>,
}

/// One record kind's CEL binding surface (S13 5.1/5.2/5.3, design D6):
/// every field a `policy.yaml` CEL expression bound to this kind may
/// reference off the `record` variable, plus the fixed pure-function
/// allowlist — folded in verbatim from the SAME `canon_policy::bindings_for`
/// call `canon-policy`'s own write-time validator uses (`canon-policy`
/// lib.rs's own forward reference: "S12's `resolve_surface`/
/// `ContextReport` gain a `policy` section populated by
/// `bindings::bindings_for`"), so this can never disagree with what a
/// `policy.yaml` CEL expression is actually checked against — invariant 2,
/// extended to the CEL surface.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CelSurface {
    /// `record.<field>` → that field's statically-resolved `CelType`,
    /// rendered via `CelType`'s own `Display` (`BindingSet::field_names()`
    /// over `BindingSet::record_fields`, never a hand-typed field list).
    pub fields: BTreeMap<String, String>,
    /// The allowlisted, non-macro callable functions
    /// (`BindingSet::callable_function_names()`), each rendered via
    /// `Display for FunctionSig` (`name(args) -> returns`) — excludes
    /// CEL-native macros like `has` (`callable_function_names()`'s own
    /// doc: the parser rewrites a well-formed `has(...)` call away before
    /// it ever reaches the validator as a `Call`).
    pub functions: Vec<String>,
}

/// The review vocabulary (issue #2): what `canon finding` and `canon
/// review` accept, and whether this repo requires review before a
/// subject is claimed done. Static vocabulary plus the resolved policy;
/// never a corpus read (invariant 1).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSurface {
    /// `canon finding add --severity` values, in blocking order.
    pub finding_severities: Vec<String>,
    /// `canon finding add/close --disposition` values.
    pub finding_dispositions: Vec<String>,
    /// The `review` kind's fields → required, the SAME map
    /// `kinds.review.envelopeFields` carries.
    pub review_fields: BTreeMap<String, bool>,
    /// `spec_coverage.require_review`. `None` = not required here
    /// (absent, or `spec_coverage` itself is absent or invalid).
    pub require_review: Option<ReviewRequirementSurface>,
}

/// The full authoring surface (design D3): everything an agent needs to
/// know BEFORE writing a canon artifact against this repo. Every map is a
/// `BTreeMap`; every array is sorted or built from an already-sorted
/// source (invariant 3).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextReport {
    pub capability_version: u32,
    /// By the name `--kind` takes ([`crate::query::kind_cli_name`]): every
    /// `RecordKind::ALL` member. The wire string (`evidence_record`) stays
    /// storage.
    pub kinds: BTreeMap<String, KindSurface>,
    /// Every artifact family, in `FamilyKind::ALL` order.
    pub families: Vec<FamilySurface>,
    /// Every leaf of the command spec (filtered to the `--as` role's leaves
    /// when one is given), sorted by command.
    pub commands: Vec<CommandSurface>,
    /// By enum type name (the schema registry's own `$defs` — never a
    /// hand-maintained second list); each value is that enum's sorted
    /// member set.
    pub enums: BTreeMap<String, Vec<String>>,
    /// By join-spine key name (S1's `join_spine_doc::rows()`) → that key's
    /// grammar string.
    pub join_keys: BTreeMap<String, String>,
    pub policy: PolicySurface,
    /// S10 part2's typed authoring vocabulary (design.md D3, task 6.1) —
    /// canon-vocab's own resolved directive/enum/evidence-kind index,
    /// never a second hand-projected view of it (invariant 2).
    pub vocab: VocabularySurface,
    /// Issue #2's review vocabulary and requirement ([`ReviewSurface`]).
    pub review: ReviewSurface,
    /// S13 5.1/5.2/5.3 (design D6): by the same kind names, the SAME
    /// key set `kinds` uses — the CEL binding surface (`record.<field>`
    /// types + allowlisted functions) `policy.yaml`'s own write-time
    /// validator checks a CEL expression against, never a second
    /// independently-derived projection (invariant 2).
    pub cel: BTreeMap<String, CelSurface>,
    /// The outline header's policy warning ([`crate::status::policy_warning`]),
    /// resolved from the same [`PolicyResolution`] as `policy`. Outline
    /// only: the JSON surface is unchanged, and `policy.spec_coverage` /
    /// `policy.diagnostics` already carry the same fact there.
    #[serde(skip)]
    pub policy_warning: Option<String>,
}

impl Report for ContextReport {}

/// `--repo <dir>` root resolution (design D7, task 1.4): a bare `canon
/// context` (`--repo` omitted — clap's `default_value = "."`) or an
/// explicit `--repo .` resolves the PROJECT root — the nearest ancestor of
/// cwd carrying a `canon.yaml`, the identical repo-root marker `canon
/// fmt`/`canon gate` key off — so running `canon context` from any
/// subdirectory still reads the repo ROOT's `<repo>/.canon/policy.yaml`,
/// never a subdirectory's absence of one. Any OTHER explicit `--repo
/// <dir>` is used as-is (no walk), matching every other canon-cli root
/// argument (`canon fmt`'s `root`, `canon gate`'s `GateCtx::from_repo`).
/// No ancestor of cwd carries a `canon.yaml`? Falls back to cwd itself —
/// the exact prior "repo used as-is" behavior for a directory with no
/// canon state at all
/// (`resolve_surface_never_fails_on_a_repo_with_no_canon_state_at_all`).
/// A command that needs canon state does not run on that fallback: `main`
/// refuses it first ([`require_config`]).
///
/// Called by `main.rs`'s `run_context` BEFORE [`resolve_surface`] —
/// `resolve_surface` itself stays a pure "(already-resolved repo, opts) ->
/// surface" fold (its own doc below), so this is the ONE place the
/// ancestor walk happens, never duplicated per call site.
pub fn resolve_repo_root(repo: &Path) -> PathBuf {
    if repo != Path::new(".") {
        return repo.to_path_buf();
    }
    discover_repo_root()
}

/// The nearest ancestor of the current directory holding a `canon.yaml`,
/// else the current directory: what `--repo` means when it is absent (D5).
pub fn discover_repo_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.ancestors().find(|dir| dir.join("canon.yaml").is_file()).map(Path::to_path_buf).unwrap_or(cwd)
}

/// The commands with a `--repo` option that run without a `canon.yaml`
/// there, and why. Every other command that takes `--repo` needs canon
/// state and is refused by [`require_config`] when it finds none, instead of
/// grading an empty repo (`gate check` used to print `clean` for a
/// directory canon was never set up in).
pub const CONFIG_EXEMPT: &[(&str, &str)] = &[
    ("init", "creates canon.yaml"),
    ("migrate", "carries its own CONFIG_MISSING refusal, worded for the layout it moves"),
    ("demo init", "scaffolds a new repo, canon.yaml included"),
    ("demo review", "refuses with DEMO_NOT_INITIALIZED when `demo init` has not run"),
    ("context", "a capability query that never fails: without a canon.yaml it describes the built-in surface"),
    ("regime show", "reads no repo: it only builds a key from its arguments"),
    ("adapter validate", "reads one response file; the context-pack join runs only when a canon.yaml is found"),
    ("adapter authorize", "reads .canon/policy.yaml only; an absent policy is default denial"),
    ("context-pack add", "a context pack is made from the git tree and never reads canon.yaml"),
    ("context-pack show", "a context pack is made from the git tree and never reads canon.yaml"),
    ("context-pack verify", "a context pack is made from the git tree and never reads canon.yaml"),
    ("prompt add", "a prompt bundle is stored beside the git tree and never reads canon.yaml"),
    ("prompt show", "a prompt bundle is stored beside the git tree and never reads canon.yaml"),
    ("retrieve", "run by agent hooks in any directory: with no repo state it finds no guidance and prints none"),
    ("dispatch begin", "run by agent hooks in any directory: the run manifest and the learn store default to `.canon/`"),
    ("dispatch end", "closes a run `dispatch begin` opened, found by id under `.canon/`"),
    ("strategy approval-payload", "the strategy store defaults to `.canon/learn`; canon.yaml only tunes it"),
    ("strategy promote", "the strategy store defaults to `.canon/learn`; canon.yaml only tunes it"),
    ("strategy rollback", "the strategy store defaults to `.canon/learn`; canon.yaml only tunes it"),
    ("skills install", "projects the guides into the directory; canon.yaml only records the provider choice"),
    ("skills check", "inspects the directory's install; canon.yaml only supplies the provider choice"),
    ("skills doctor", "inspects the directory's install; canon.yaml only supplies the provider choice"),
    ("task done", "run by agent hooks and pre-commit scripts: it flips a plan row on evidence it finds and refuses otherwise"),
    ("gate promote", "run by agent hooks and pre-commit scripts: with nothing staged it reports so"),
    ("gate install-hooks", "installs hook seams into a repo that may not be set up yet"),
    ("completion", "writes a shell script from the command tree: no repo state"),
    ("dashboard", "serves a built app and a snapshot; its canon.yaml section is optional and a missing repo or app is refused with its own code"),
];

/// Whether `command` (a canonical leaf path such as `gate check`) needs a
/// `canon.yaml` at its `--repo`.
pub fn needs_config(command: &str) -> bool {
    !CONFIG_EXEMPT.iter().any(|(exempt, _)| *exempt == command)
}

/// The repo root a command that needs canon state runs against, or the
/// `CONFIG_MISSING` refusal when no `canon.yaml` is there. `repo` is `--repo`
/// as given (absent: the nearest ancestor of the current directory holding a
/// `canon.yaml`; present: that directory, no search).
pub fn require_config(repo: Option<&Path>) -> Result<PathBuf, CliError> {
    let root = match repo {
        Some(dir) if dir == Path::new(".") => std::env::current_dir().unwrap_or_else(|_| dir.to_path_buf()),
        Some(dir) => dir.to_path_buf(),
        None => discover_repo_root(),
    };
    if root.join("canon.yaml").is_file() {
        return Ok(root);
    }
    let searched = if repo.is_none() { format!("`{}` and its parent directories", root.display()) } else { format!("`{}`", root.display()) };
    Err(CliError::refused("CONFIG_MISSING", format!("no canon.yaml in {searched}"))
        .field("--repo")
        .hint(format!("run `canon init` in the repository root to set canon up, or pass --repo <dir> naming a directory that holds canon.yaml (searched {searched})")))
}

/// The `canon.yaml` of `repo`, resolved through [`resolve_repo_root`].
pub fn resolve_canon_yaml(repo: &Path) -> PathBuf {
    resolve_repo_root(repo).join("canon.yaml")
}

/// Phase 1 (design D1): resolve the project's schema registry + policy
/// exactly as `canon fmt`/`canon gate` do — [`SchemaRegistry::load`] +
/// [`PolicyResolution::resolve`] and NOTHING else (invariant 2) — then fold
/// the result into one [`ContextReport`] value. Never touches a corpus
/// (no `canon fmt` walk, no `canon gate` evidence read — invariant 1);
/// infallible, matching `run_context`'s own "parse + fold exactly as
/// `compile` does ... `fold_env` is pure/total" contract (ported from
/// the donor's `run_context`).
///
/// `repo` is used exactly as given — no ancestor walk happens HERE. The
/// nearest-`canon.yaml` discovery (design D7, task 1.4) is
/// [`resolve_repo_root`], one layer up, called by `main.rs`'s
/// `run_context` BEFORE this function ever runs; every other canon-cli
/// root argument (`canon fmt`'s `root`, `canon gate`'s
/// `GateCtx::from_repo`) is likewise handed an already-resolved path, not
/// a bare CLI flag.
pub fn resolve_surface(repo: &Path, opts: ContextOptions) -> ContextReport {
    // Invariant 2: the one shared registry call every command uses.
    let registry = SchemaRegistry::load();
    // Invariant 2: the one shared policy-resolution call every command
    // uses — never `canon_gate::GateContext::load` (invariant 1: that
    // additionally reads the evidence corpus, which `canon context` must
    // never do).
    let policy = PolicyResolution::resolve(repo, &registry);
    // Invariant 2 (S10 part2, design.md D3): the one shared vocabulary-
    // resolution call `canon-vocab`'s own checker uses — see module doc.
    let (vocab_snapshot, vocab_diagnostics) = canon_vocab::resolve_snapshot(repo, None);

    let kinds = collect_kinds(&registry);
    let review = summarize_review(&policy, &kinds);

    ContextReport {
        capability_version: CURRENT_CAPABILITY_VERSION,
        kinds,
        families: collect_families(),
        commands: collect_commands(opts.role),
        enums: collect_enums(&registry),
        join_keys: collect_join_keys(),
        policy: summarize_policy(&policy),
        vocab: VocabularySurface {
            snapshot: vocab_snapshot,
            diagnostics: vocab_diagnostics.into_iter().map(|d| format!("{}: {} ({})", d.code, d.message, d.subject)).collect(),
        },
        review,
        cel: collect_cel(&registry),
        policy_warning: crate::status::policy_warning(&policy),
    }
}

/// `review` (issue #2): the finding vocabulary from the parsers that
/// accept it, the review kind's fields from the `kinds` projection, and
/// the resolved `require_review` settings.
fn summarize_review(policy: &PolicyResolution, kinds: &BTreeMap<String, KindSurface>) -> ReviewSurface {
    let require_review = match &policy.spec_coverage {
        Some(SpecCoverage::Active { require_review: Some(rr), .. }) => Some(ReviewRequirementSurface {
            scope: status_slugs(&rr.scope),
            distinct_actor: rr.distinct_actor,
            block_on_findings: rr.block_on_findings,
        }),
        _ => None,
    };
    ReviewSurface {
        finding_severities: crate::finding::SEVERITY_VALUES.iter().map(std::string::ToString::to_string).collect(),
        finding_dispositions: crate::finding::DISPOSITION_VALUES.iter().map(std::string::ToString::to_string).collect(),
        review_fields: kinds.get(crate::query::kind_cli_name(canon_model::RecordKind::Review)).map(|k| k.envelope_fields.clone()).unwrap_or_default(),
        require_review,
    }
}

fn status_slugs(statuses: &[SubjectStatus]) -> Vec<String> {
    statuses.iter().map(|s| subject_status_slug(s).to_string()).collect()
}

/// The outline's one-line `require_review` summary.
fn require_review_line(scope: &[String], distinct_actor: bool, block_on_findings: bool) -> String {
    format!("scope={} distinct_actor={distinct_actor} block_on_findings={block_on_findings}", scope_text(scope))
}

/// An outline status list; empty reads as every scenario.
fn scope_text(scope: &[String]) -> String {
    if scope.is_empty() {
        "<every scenario>".to_string()
    } else {
        scope.join(", ")
    }
}

/// The one-line `require_audit` summary `canon context` and `canon
/// status` print.
pub fn require_audit_line(ra: &RequireAuditSurface) -> String {
    format!("rate={} seed=`{}` scope={}", ra.rate, ra.seed, scope_text(&ra.scope))
}

impl SpecCoverageSurface {
    /// The outline's one-line summary.
    fn outline(&self) -> String {
        match self {
            SpecCoverageSurface::Active { require_evidence, scope, exclude_lanes, require_cases, require_review, require_audit } => {
                let mut summary = format!("require_evidence={require_evidence} scope={}", scope_text(scope));
                if !exclude_lanes.is_empty() {
                    summary.push_str(&format!(" exclude_lanes={}", exclude_lanes.join(", ")));
                }
                if !require_cases.is_empty() {
                    summary.push_str(&format!(" require_cases={}", require_cases.join(", ")));
                }
                if let Some(rr) = require_review {
                    summary.push_str(&format!(" require_review=({})", require_review_line(&rr.scope, rr.distinct_actor, rr.block_on_findings)));
                }
                if let Some(ra) = require_audit {
                    summary.push_str(&format!(" require_audit=({})", require_audit_line(ra)));
                }
                summary
            }
            SpecCoverageSurface::Invalid { invalid } => format!("INVALID — {invalid}"),
        }
    }
}

impl EvidenceBindingSurface {
    /// The outline's one-line summary.
    fn outline(&self) -> String {
        match self {
            EvidenceBindingSurface::Active { mode, strength, case, lane, scope } => {
                let mut summary = format!("mode={mode} strength={strength}");
                for (field, values) in [("case", case), ("lane", lane), ("scope", scope)] {
                    if !values.is_empty() {
                        summary.push_str(&format!(" {field}={}", values.join(", ")));
                    }
                }
                summary
            }
            EvidenceBindingSurface::Invalid { invalid } => format!("INVALID — {invalid}"),
        }
    }
}

/// `kinds` (design D3/task 2.2): walks whatever kinds `registry` actually
/// holds (`registry.kinds()`, never the closed `RecordKind::ALL` directly)
/// so a fixture registry built with fewer kinds — or a future registry with
/// more — is reflected exactly, matching the design's Non-Goals: "a kind
/// with no registered schema simply does not appear".
fn collect_kinds(registry: &SchemaRegistry) -> BTreeMap<String, KindSurface> {
    let mut kinds = BTreeMap::new();
    for kind in registry.kinds() {
        let Some(schema) = registry.get(kind) else { continue };
        let value = schema.as_value();
        let required: std::collections::BTreeSet<&str> =
            value.get("required").and_then(|r| r.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
        let envelope_fields: BTreeMap<String, bool> = value
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|props| props.keys().map(|name| (name.clone(), required.contains(name.as_str()))).collect())
            .unwrap_or_default();
        let partition = PartitionLayout { template: kind.partition_template().to_string(), area_scoped: kind.is_area_scoped() };
        let schema = format!("{SCHEMA_DIR}/{}.schema.json", kind.as_str());
        kinds.insert(crate::query::kind_cli_name(kind).to_string(), KindSurface { schema_version: kind.schema_version(), envelope_fields, partition, schema });
    }
    kinds
}

/// Where canon's own JSON Schemas live, for the `schema` links.
const SCHEMA_DIR: &str = "crates/canon-model/schemas";

/// `families`: every [`canon_model::family::FamilyKind`], with the record
/// kinds that carry its name (`run`, `review` and `divergence` are both).
fn collect_families() -> Vec<FamilySurface> {
    use canon_model::family::FamilyKind;
    FamilyKind::ALL
        .iter()
        .map(|family| FamilySurface {
            family: family.as_str().to_string(),
            schema: format!("{SCHEMA_DIR}/family-{}.schema.json", family.as_str()),
            record_kinds: canon_model::RecordKind::ALL.iter().filter(|k| k.as_str() == family.as_str()).map(|k| crate::query::kind_cli_name(*k).to_string()).collect(),
        })
        .collect()
}

/// `commands`: the leaves of the generated spec ([`crate::spec::nodes`]), so
/// the index can never name a command the parser lacks. `role` keeps only the
/// leaves that role's steps use.
fn collect_commands(role: Option<crate::role::Role>) -> Vec<CommandSurface> {
    let mut root = crate::cli::command();
    root.build();
    crate::spec::nodes(&root)
        .into_iter()
        .filter_map(|node| {
            let leaf = node.leaf?;
            if role.is_some_and(|r| !leaf.roles.contains(&r)) {
                return None;
            }
            Some(CommandSurface { command: node.command, effect: leaf.effect.as_str(), roles: leaf.roles, about: node.about, example: leaf.examples.first().copied() })
        })
        .collect()
}

/// `enums` (design D3/task 2.3): every enum-shaped `$defs` entry across
/// every registered kind's own `schemars`-generated schema — the exact JSON
/// Schema fragments the schema registry already carries, never a hand-typed
/// domain list. `RecordKind`'s own `$defs` entry is excluded: it is already
/// the `kinds` map's own key set, and listing the same closed domain twice
/// under two names is exactly the "second list" invariant 2 rules out.
fn collect_enums(registry: &SchemaRegistry) -> BTreeMap<String, Vec<String>> {
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for kind in registry.kinds() {
        let Some(schema) = registry.get(kind) else { continue };
        let Some(defs) = schema.as_value().get("$defs").and_then(|d| d.as_object()) else { continue };
        for (name, def) in defs {
            if name == "RecordKind" {
                continue;
            }
            let Some(members) = def.get("enum").and_then(|e| e.as_array()) else { continue };
            let mut values: Vec<String> = members.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
            values.sort();
            enums.entry(name.clone()).or_insert(values);
        }
    }
    enums
}

/// `cel` (S13 5.1/5.2/5.3, design D6): walks the SAME `registry.kinds()`
/// set `collect_kinds`/`collect_enums` do, deriving each kind's
/// [`CelSurface`] from exactly one `canon_policy::bindings_for(kind,
/// registry)` call — never a second, hand-typed field/function list.
fn collect_cel(registry: &SchemaRegistry) -> BTreeMap<String, CelSurface> {
    let mut cel = BTreeMap::new();
    for kind in registry.kinds() {
        let bindings = canon_policy::bindings_for(kind, registry);
        let fields: BTreeMap<String, String> = bindings
            .field_names()
            .into_iter()
            .map(|name| {
                let ty = bindings.record_fields.get(&name).expect("field_names() is derived from record_fields' own keys");
                (format!("record.{name}"), ty.to_string())
            })
            .collect();
        let functions: Vec<String> =
            bindings.callable_function_names().into_iter().filter_map(|name| bindings.function(&name).map(ToString::to_string)).collect();
        cel.insert(crate::query::kind_cli_name(kind).to_string(), CelSurface { fields, functions });
    }
    cel
}

/// `joinKeys` (design D3/task 2.4): `canon_model::join_spine_doc::rows()` —
/// the exact same `GRAMMAR`/`JOINS` constants that generate the committed
/// `JOIN_SPINE.md`, never a second copy. The combined `sha` / `pr` row's key
/// carries literal backticks (markdown-table styling, `join_spine_doc.rs`'s
/// own `"sha\` / \`pr"` literal) — stripped here so the JSON key is a plain
/// string, not markdown.
fn collect_join_keys() -> BTreeMap<String, String> {
    canon_model::join_spine_doc::rows().into_iter().map(|row| (row.key.replace('`', "").trim().to_string(), row.grammar.to_string())).collect()
}

/// `policy` (design D3/task 2.5): `PolicyResolution`'s own fields,
/// summarized one-for-one via [`summarize_field`] — never a re-derived
/// requirement shape.
fn summarize_policy(policy: &PolicyResolution) -> PolicySurface {
    PolicySurface {
        trust_required: policy.trust_required.iter().map(|(k, v)| (k.clone(), summarize_field(v))).collect(),
        trust_sample: policy.trust_sample.iter().map(|(k, v)| (k.clone(), summarize_field(v))).collect(),
        staleness: StalenessSurface {
            max_commits_behind: summarize_field(&policy.staleness.max_commits_behind),
            surface_scoped: summarize_field(&policy.staleness.surface_scoped),
        },
        risk_routing: policy.risk_routing.iter().map(|(k, v)| (k.clone(), summarize_field(v))).collect(),
        risk_tiers: policy.risk_tiers.iter().map(|(name, rule)| (name.clone(), summarize_risk_tier(rule))).collect(),
        spec_coverage: spec_coverage_surface(policy),
        evidence_binding: policy.evidence_binding.as_ref().map(|binding| match binding {
            canon_gate::EvidenceBinding::Active { mode, strength, cases, lanes, scope } => EvidenceBindingSurface::Active {
                mode: match mode {
                    canon_gate::BindingMode::Off => "off",
                    canon_gate::BindingMode::Warn => "warn",
                    canon_gate::BindingMode::Require => "require",
                },
                strength: strength.as_str(),
                case: cases.clone(),
                lane: lanes.clone(),
                scope: status_slugs(scope),
            },
            canon_gate::EvidenceBinding::Invalid { detail } => EvidenceBindingSurface::Invalid { invalid: detail.clone() },
        }),
        clean: policy.is_clean(),
        diagnostics: policy.diagnostics.iter().map(ToString::to_string).collect(),
    }
}

/// `policy.spec_coverage`'s surface: the shape `canon context` and
/// `canon status` both print, keyed like `policy.yaml`.
pub fn spec_coverage_surface(policy: &PolicyResolution) -> Option<SpecCoverageSurface> {
    policy.spec_coverage.as_ref().map(|sc| match sc {
        SpecCoverage::Active { require_evidence, scope, exclude_lanes, require_cases, require_review, require_audit } => SpecCoverageSurface::Active {
            require_evidence: *require_evidence,
            scope: status_slugs(scope),
            exclude_lanes: exclude_lanes.clone(),
            require_cases: require_cases.clone(),
            require_review: require_review.as_ref().map(|rr| RequireReviewSurface {
                scope: status_slugs(&rr.scope),
                distinct_actor: rr.distinct_actor,
                block_on_findings: rr.block_on_findings,
            }),
            require_audit: require_audit.as_ref().map(|ra| RequireAuditSurface {
                rate: serde_json::Number::from_f64(ra.rate.get()).expect("a resolved audit rate is finite"),
                seed: ra.seed.clone(),
                scope: status_slugs(&ra.scope),
            }),
        },
        SpecCoverage::Invalid { detail } => SpecCoverageSurface::Invalid { invalid: detail.clone() },
    })
}

fn summarize_risk_tier(rule: &RiskTierRule) -> RiskTierSurface {
    RiskTierSurface { rank: rule.rank, paths: rule.paths.clone(), effects: rule.effects.clone(), min_human_approvals: rule.min_human_approvals }
}

/// A [`SubjectStatus`]'s wire spelling — the same vocabulary
/// `canon subject move` and `canon query --status` accept, so the
/// surface prints values an operator can paste straight back into
/// `policy.yaml`.
fn subject_status_slug(status: &SubjectStatus) -> &'static str {
    match status {
        SubjectStatus::Proposed => "proposed",
        SubjectStatus::Specced => "specced",
        SubjectStatus::Building => "building",
        SubjectStatus::Verifying => "verifying",
        SubjectStatus::Shipped => "shipped",
        SubjectStatus::Retired => "retired",
    }
}

/// One `PolicyField<T>` → its authoring-surface shape. `Flat`'s value is
/// serialized as-is; a serialization failure (only reachable for a
/// malformed `trust_sample` fraction — a YAML `.nan`/`.inf` literal, which
/// `serde_json` refuses to represent) degrades to `Null` rather than
/// failing the whole surface, mirroring `authoring_surface`'s own
/// "infallible for these concrete shapes; a defensive ... fallback keeps
/// the surface total" discipline (ported from the donor's
/// `authoring_surface`).
fn summarize_field<T: Serialize>(field: &PolicyField<T>) -> PolicyFieldSurface {
    match field {
        PolicyField::Flat(value) => PolicyFieldSurface::Flat { value: serde_json::to_value(value).unwrap_or(serde_json::Value::Null) },
        PolicyField::Cel(compiled) => PolicyFieldSurface::Cel { expression: compiled.source().to_string() },
    }
}

/// Phase 2, JSON mode (design D5): `serde_json::to_string_pretty`, no
/// additional resolution step — matching `run_context`'s own `--json`
/// branch (ported from the donor's `run_context`). Every leaf
/// value that could plausibly fail to serialize is already normalized by
/// [`summarize_field`], so the fallback below is defensive-only.
pub fn render_json(surface: &ContextReport) -> String {
    serde_json::to_string_pretty(surface).unwrap_or_else(|e| format!("{{\"error\": \"failed to serialize authoring surface: {e}\"}}"))
}

/// [`render_json`] keeping only the named top-level sections (`--fields`).
pub fn render_json_fields(surface: &ContextReport, fields: &[String]) -> String {
    let shown = serde_json::to_value(surface).and_then(|value| serde_json::to_string_pretty(&crate::paging::project(&value, fields)));
    shown.unwrap_or_else(|e| format!("{{\"error\": \"failed to serialize authoring surface: {e}\"}}"))
}

/// The top-level sections of the `--json` surface, which `--fields` may name.
pub const SURFACE_FIELDS: [&str; 10] = ["capabilityVersion", "kinds", "families", "commands", "enums", "joinKeys", "policy", "vocab", "review", "cel"];

/// Phase 2, default mode (design D5): a compact, per-section outline — name
/// counts + name lists, then one indented summary line per entry — for
/// prompt injection, never a full schema dump (ported from the donor's
/// `context_outline`). It opens with a short header: a pointer to `canon
/// status` (this outline says what CAN be authored, never where the repo
/// stands) and, when no `spec_coverage` section is in force, the policy
/// warning, then a blank line before `capabilityVersion:`.
pub fn render_outline(surface: &ContextReport, fields: Option<&[String]>) -> String {
    let want = |name: &str| fields.is_none_or(|f| f.iter().any(|x| x == name));
    let mut out = String::new();
    let _ = writeln!(out, "status: run `canon status` for this repo's subjects, gaps and next commands");
    if let Some(warning) = &surface.policy_warning {
        let _ = writeln!(out, "warning: {warning}");
    }
    let _ = writeln!(out);
    if want("capabilityVersion") {
        let _ = writeln!(out, "capabilityVersion: {}", surface.capability_version);
    }

    if want("kinds") {
        let _ = writeln!(out, "kinds ({}): {}", surface.kinds.len(), surface.kinds.keys().cloned().collect::<Vec<_>>().join(", "));
        for (name, kind) in &surface.kinds {
            let _ = writeln!(
                out,
                "  {name}: schema={} fields={} partition={}{}",
                kind.schema_version,
                kind.envelope_fields.len(),
                kind.partition.template,
                if kind.partition.area_scoped { " [area-scoped]" } else { "" },
            );
        }
    }

    if want("families") {
        let _ = writeln!(out, "families ({}):", surface.families.len());
        for family in &surface.families {
            let _ = writeln!(out, "  {}: schema={} recordKinds={}", family.family, family.schema, family.record_kinds.join(","));
        }
    }
    if want("commands") {
        let _ = writeln!(out, "commands ({}):", surface.commands.len());
        let width = surface.commands.iter().map(|c| c.command.len()).max().unwrap_or(0);
        for c in &surface.commands {
            let _ = writeln!(out, "  {:<width$}  {:<11} {}", c.command, c.effect, c.about.as_deref().unwrap_or(""));
        }
    }
    if want("enums") {
        let _ = writeln!(out, "enums ({}): {}", surface.enums.len(), surface.enums.keys().cloned().collect::<Vec<_>>().join(", "));
        for (name, members) in &surface.enums {
            let _ = writeln!(out, "  {name}: {}", members.join(", "));
        }
    }

    if want("joinKeys") {
        let _ = writeln!(out, "joinKeys ({}): {}", surface.join_keys.len(), surface.join_keys.keys().cloned().collect::<Vec<_>>().join(", "));
        for (key, grammar) in &surface.join_keys {
            let _ = writeln!(out, "  {key}: {grammar}");
        }
    }

    if want("policy") {
        let _ = writeln!(out, "policy:");
        write_field_map(&mut out, "trust_required", &surface.policy.trust_required);
        write_field_map(&mut out, "trust_sample", &surface.policy.trust_sample);
        let _ = writeln!(
            out,
            "  staleness: max_commits_behind={} surface_scoped={}",
            render_field_compact(&surface.policy.staleness.max_commits_behind),
            render_field_compact(&surface.policy.staleness.surface_scoped),
        );
        write_field_map(&mut out, "risk_routing", &surface.policy.risk_routing);
        let _ = writeln!(out, "  risk_tiers ({}):", surface.policy.risk_tiers.len());
        for (name, tier) in &surface.policy.risk_tiers {
            let _ = writeln!(
                out,
                "    {name}: rank={} paths={} effects={} min_human_approvals={}",
                tier.rank,
                tier.paths.join(","),
                tier.effects.join(","),
                tier.min_human_approvals
            );
        }
        let _ = match &surface.policy.spec_coverage {
            Some(section) => writeln!(out, "  spec_coverage: {}", section.outline()),
            // Printed even when absent: "this repo has not opted in" is the
            // fact an author most needs before writing a `.feature` file.
            None => writeln!(out, "  spec_coverage: <absent — spec-corpus coverage is not enforced here>"),
        };
        if let Some(section) = &surface.policy.evidence_binding {
            let _ = writeln!(out, "  experimental.evidence_binding: {}", section.outline());
        }
        let _ = writeln!(out, "  clean: {}", surface.policy.clean);
        let _ = writeln!(out, "  diagnostics ({}):", surface.policy.diagnostics.len());
        for diag in &surface.policy.diagnostics {
            let _ = writeln!(out, "    {diag}");
        }
    }

    if want("review") {
        let _ = writeln!(out, "review:");
        let _ = writeln!(out, "  findingSeverities: {}", surface.review.finding_severities.join(", "));
        let _ = writeln!(out, "  findingDispositions: {}", surface.review.finding_dispositions.join(", "));
        let fields: Vec<String> = surface.review.review_fields.iter().map(|(name, required)| format!("{name}{}", if *required { "*" } else { "" })).collect();
        let _ = writeln!(out, "  reviewFields: {}", fields.join(", "));
        let _ = match &surface.review.require_review {
            Some(rr) => writeln!(out, "  require_review: {}", require_review_line(&rr.scope, rr.distinct_actor, rr.block_on_findings)),
            None => writeln!(out, "  require_review: <absent — review is not required before a subject is claimed done>"),
        };
    }

    if want("vocab") {
        let _ = writeln!(out, "vocab:");
        let _ = writeln!(
            out,
            "  directives ({}): {}",
            surface.vocab.snapshot.directives.len(),
            surface.vocab.snapshot.directives.keys().cloned().collect::<Vec<_>>().join(", ")
        );
        for (tag, decl) in &surface.vocab.snapshot.directives {
            let attrs: Vec<String> = decl.attrs.iter().map(|a| format!("{}{}", a.name, if a.required { "*" } else { "" })).collect();
            let _ = writeln!(out, "    ::{tag}: {}", attrs.join(", "));
        }
        let _ = writeln!(
            out,
            "  enums ({}): {}",
            surface.vocab.snapshot.enums.len(),
            surface.vocab.snapshot.enums.keys().cloned().collect::<Vec<_>>().join(", ")
        );
        for (name, members) in &surface.vocab.snapshot.enums {
            let _ = writeln!(out, "    {name}: {}", members.join(", "));
        }
        let _ = writeln!(
            out,
            "  evidenceKinds ({}): {}",
            surface.vocab.snapshot.evidence_kinds.len(),
            surface.vocab.snapshot.evidence_kinds.join(", ")
        );
        let _ = writeln!(out, "  diagnostics ({}):", surface.vocab.diagnostics.len());
        for diag in &surface.vocab.diagnostics {
            let _ = writeln!(out, "    {diag}");
        }
    }

    if want("cel") {
        let _ = writeln!(out, "cel:");
        for (kind, cel) in &surface.cel {
            let _ = writeln!(out, "  {kind}:");
            let _ = writeln!(out, "    fields ({}): {}", cel.fields.len(), cel.fields.keys().cloned().collect::<Vec<_>>().join(", "));
            for (field, ty) in &cel.fields {
                let _ = writeln!(out, "      {field}: {ty}");
            }
            let _ = writeln!(out, "    functions ({}): {}", cel.functions.len(), cel.functions.join(", "));
        }
    }

    out
}

fn write_field_map(out: &mut String, label: &str, fields: &BTreeMap<String, PolicyFieldSurface>) {
    let _ = writeln!(out, "  {label} ({}):", fields.len());
    for (key, field) in fields {
        let _ = writeln!(out, "    {key}={}", render_field_compact(field));
    }
}

fn render_field_compact(field: &PolicyFieldSurface) -> String {
    match field {
        PolicyFieldSurface::Flat { value } => value.to_string(),
        PolicyFieldSurface::Cel { expression } => format!("cel({expression})"),
    }
}

/// canon-context's shared-contract selftest entry point (Wave-3 `canon
/// selftest` aggregator, per-crate registration — unblocks S12 7.4).
/// Resolves the authoring surface against a repo path with NO canon
/// state: `SchemaRegistry::load()` is compiled-in, and policy/vocab
/// degrade to empty (`resolve_surface`'s own "never fails on a repo with
/// no canon state" contract), so this needs no on-disk fixture and is
/// side-effect-free against the real repo. Checks the byte-stability +
/// JSON/outline agreement + cel-covers-kinds invariants S12/S13's own
/// unit tests assert.
///
/// `Ok(n)` = checks passed; `Err(_)` = one line per failure, never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let repo = Path::new("__canon_context_selftest_no_such_repo__");
    let mut passed = 0;
    let mut failures = Vec::new();

    // 1. `--json` output is byte-identical across repeated resolution.
    let a = render_json(&resolve_surface(repo, ContextOptions::default()));
    let b = render_json(&resolve_surface(repo, ContextOptions::default()));
    if a == b {
        passed += 1;
    } else {
        failures.push("render-json-byte-stable: two resolutions of the same repo produced different JSON".to_string());
    }

    let surface = resolve_surface(repo, ContextOptions::default());

    // 2. Every kind/enum/join-key name in the JSON also appears in the
    //    outline (and vice-versa by construction).
    match check_json_outline_agreement(&surface) {
        Ok(()) => passed += 1,
        Err(e) => failures.push(format!("json-outline-agreement: {e}")),
    }

    // 3. The CEL section covers exactly the `kinds` set (S13 5.1/D6).
    if surface.cel.keys().collect::<Vec<_>>() == surface.kinds.keys().collect::<Vec<_>>() {
        passed += 1;
    } else {
        failures.push("cel-covers-kinds: the cel section does not cover the same kind set as `kinds`".to_string());
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

fn check_json_outline_agreement(surface: &ContextReport) -> Result<(), String> {
    let json: serde_json::Value = serde_json::from_str(&render_json(surface)).map_err(|e| e.to_string())?;
    let outline = render_outline(surface, None);
    for name in surface.kinds.keys() {
        if json["kinds"].get(name).is_none() {
            return Err(format!("JSON missing kind `{name}`"));
        }
        if !outline.contains(name.as_str()) {
            return Err(format!("outline missing kind `{name}`"));
        }
    }
    for name in surface.enums.keys() {
        if json["enums"].get(name).is_none() {
            return Err(format!("JSON missing enum `{name}`"));
        }
        if !outline.contains(name.as_str()) {
            return Err(format!("outline missing enum `{name}`"));
        }
    }
    for key in surface.join_keys.keys() {
        if json["joinKeys"].get(key).is_none() {
            return Err(format!("JSON missing join key `{key}`"));
        }
        if !outline.contains(key.as_str()) {
            return Err(format!("outline missing join key `{key}`"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_repo(policy_yaml: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        if let Some(yaml) = policy_yaml {
            std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
            std::fs::write(dir.path().join(".canon/policy.yaml"), yaml).unwrap();
        }
        dir
    }

    /// Invariant 3: two resolutions of the identical, unchanged repo
    /// produce byte-identical `render_json` output.
    #[test]
    fn render_json_is_byte_stable_across_repeated_resolution() {
        let dir = fixture_repo(Some("trust_required:\n  p1: human\n  p2: agent\nrisk_routing:\n  needs-design-review:\n    cel: \"record.verdict == 'divergent'\"\n"));
        let first = render_json(&resolve_surface(dir.path(), ContextOptions::default()));
        let second = render_json(&resolve_surface(dir.path(), ContextOptions::default()));
        assert_eq!(first, second, "canon context --json must be byte-identical across repeated runs over an unchanged repo");
    }

    /// Invariant 1: `resolve_surface` never touches a corpus at all — a
    /// repo with no `.canon/policy.yaml`, no ledger, and no `canon.yaml`
    /// still resolves a full surface (every kind, every enum, every join
    /// key), only `policy` degrading to documented defaults + a `Missing`
    /// diagnostic.
    #[test]
    fn resolve_surface_never_fails_on_a_repo_with_no_canon_state_at_all() {
        let dir = fixture_repo(None);
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        assert_eq!(surface.kinds.len(), canon_model::envelope::RecordKind::ALL.len(), "every registered kind must still appear");
        assert!(!surface.enums.is_empty(), "enum domains must still be populated with no corpus present");
        assert!(!surface.join_keys.is_empty(), "join-spine keys must still be populated with no corpus present");
        assert!(!surface.policy.clean, "a missing policy.yaml is recorded as a diagnostic, never a panic/failure");
        assert!(!surface.policy.diagnostics.is_empty());
    }

    /// Design D5: `--json` and the outline render from the identical
    /// resolved value — every kind/enum/join-key name in the JSON output
    /// must also appear (by name) in the outline.
    #[test]
    fn json_and_outline_agree_on_every_kind_enum_and_join_key_name() {
        let dir = fixture_repo(Some("trust_sample:\n  p1: 0.2\n"));
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        let outline = render_outline(&surface, None);

        for name in surface.kinds.keys() {
            assert!(json["kinds"].get(name).is_some(), "JSON missing kind `{name}`");
            assert!(outline.contains(name.as_str()), "outline missing kind `{name}`");
        }
        for name in surface.enums.keys() {
            assert!(json["enums"].get(name).is_some(), "JSON missing enum `{name}`");
            assert!(outline.contains(name.as_str()), "outline missing enum `{name}`");
        }
        for key in surface.join_keys.keys() {
            assert!(json["joinKeys"].get(key).is_some(), "JSON missing join key `{key}`");
            assert!(outline.contains(key.as_str()), "outline missing join key `{key}`");
        }
    }

    /// Invariant 2, verified at the canon-cli boundary: `resolve_surface`'s
    /// `kinds`/`enums` are byte-identical to a FRESH, independent
    /// introspection of `SchemaRegistry::load()` done here in the test —
    /// proving `resolve_surface` derives everything from that one call
    /// rather than hand-maintaining a second copy. (`resolve_surface`'s own
    /// signature — `(repo, opts)`, no injectable registry — is itself part
    /// of invariant 2: there is no seam through which a caller could hand
    /// it a different registry than `SchemaRegistry::load()` produces.)
    #[test]
    fn kinds_and_enums_match_a_fresh_independent_schema_registry_walk() {
        let dir = fixture_repo(None);
        let surface = resolve_surface(dir.path(), ContextOptions::default());

        let registry = SchemaRegistry::load();
        let expected_kinds = collect_kinds(&registry);
        let expected_enums = collect_enums(&registry);

        assert_eq!(surface.kinds.keys().collect::<Vec<_>>(), expected_kinds.keys().collect::<Vec<_>>());
        for (name, kind) in &surface.kinds {
            let expected = &expected_kinds[name];
            assert_eq!(kind.envelope_fields, expected.envelope_fields, "kind `{name}` fields diverged from a fresh registry walk");
            assert_eq!(kind.partition.template, expected.partition.template);
        }
        assert_eq!(surface.enums, expected_enums, "enums must be byte-identical to a fresh SchemaRegistry::load() walk");
        assert!(surface.enums.len() >= 2, "fixture expectation: at least two enum domains registered");
        assert!(surface.kinds.len() >= 2, "fixture expectation: at least two kinds registered");
    }

    /// S13 5.3 (design D6): `resolve_surface`'s `cel[kind]` is
    /// byte-identical to a FRESH, independent `canon_policy::bindings_for`
    /// call over the same repo's `SchemaRegistry::load()` — proving
    /// `canon context`'s CEL section can never diverge from what
    /// `canon-policy`'s own write-time validator checks a `policy.yaml`
    /// CEL expression against (invariant 2, extended to the CEL surface;
    /// mirrors `kinds_and_enums_match_a_fresh_independent_schema_registry_walk`).
    #[test]
    fn cel_surface_matches_a_fresh_independent_bindings_for_walk() {
        let dir = fixture_repo(None);
        let surface = resolve_surface(dir.path(), ContextOptions::default());

        let registry = SchemaRegistry::load();
        assert_eq!(surface.cel.keys().collect::<Vec<_>>(), surface.kinds.keys().collect::<Vec<_>>(), "cel must cover the SAME kind set as `kinds`");
        for kind in registry.kinds() {
            let expected = canon_policy::bindings_for(kind, &registry);
            let actual = &surface.cel[crate::query::kind_cli_name(kind)];

            let expected_fields: BTreeMap<String, String> = expected
                .field_names()
                .into_iter()
                .map(|name| {
                    let ty = expected.record_fields.get(&name).unwrap();
                    (format!("record.{name}"), ty.to_string())
                })
                .collect();
            assert_eq!(actual.fields, expected_fields, "cel fields for kind `{}` diverged from a fresh bindings_for walk", kind.as_str());

            let expected_functions: Vec<String> = expected
                .callable_function_names()
                .into_iter()
                .filter_map(|name| expected.function(&name).map(ToString::to_string))
                .collect();
            assert_eq!(actual.functions, expected_functions, "cel functions for kind `{}` diverged from a fresh bindings_for walk", kind.as_str());
        }
        assert!(!surface.cel.is_empty(), "fixture expectation: at least one kind's cel surface resolved");
    }

    /// Design D5, extended to the CEL section: `--json` and the outline
    /// render from the identical resolved value — every cel kind/field/
    /// function name in the JSON output must also appear in the outline.
    #[test]
    fn json_and_outline_agree_on_the_cel_section() {
        let dir = fixture_repo(None);
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        let outline = render_outline(&surface, None);

        assert!(!surface.cel.is_empty(), "fixture expectation: at least one kind's cel surface resolved");
        for (kind, cel) in &surface.cel {
            assert!(json["cel"].get(kind).is_some(), "JSON missing cel section for kind `{kind}`");
            assert!(outline.contains(kind.as_str()), "outline missing cel kind `{kind}`");
            for (field, ty) in &cel.fields {
                assert!(json["cel"][kind]["fields"][field] == *ty, "JSON cel field `{field}` for kind `{kind}` missing or wrong type");
                assert!(outline.contains(field.as_str()), "outline missing cel field `{field}` for kind `{kind}`");
            }
            for function in &cel.functions {
                assert!(
                    json["cel"][kind]["functions"].as_array().unwrap().iter().any(|v| v == function),
                    "JSON cel functions for kind `{kind}` missing `{function}`"
                );
                assert!(outline.contains(function.as_str()), "outline missing cel function `{function}` for kind `{kind}`");
            }
        }
    }

    #[test]
    fn flat_and_cel_policy_fields_render_distinctly() {
        let dir = fixture_repo(Some(
            "trust_required:\n  p1: human\n  p2:\n    cel: \"record.verdict == 'divergent' ? 'human' : 'agent'\"\n",
        ));
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        match &surface.policy.trust_required["p1"] {
            PolicyFieldSurface::Flat { value } => assert_eq!(value, "human"),
            other => panic!("expected a flat value, got {other:?}"),
        }
        match &surface.policy.trust_required["p2"] {
            PolicyFieldSurface::Cel { expression } => assert_eq!(expression, "record.verdict == 'divergent' ? 'human' : 'agent'"),
            other => panic!("expected a cel expression, got {other:?}"),
        }
    }

    /// A minimal, self-contained vocabulary plugin tree scoped to one
    /// directive + one enum — S10 part2's OWN fixture, independent of this
    /// repo's real checked-in `.canon/vocab/canon.core/`, so this test suite
    /// never depends on that content changing (mirrors `canon-vocab`'s own
    /// `canon_core_selftest.rs` "seed a tempdir" pattern, just smaller).
    fn seed_vocab(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join(".canon/vocab/canon.core/directives")).unwrap();
        std::fs::write(
            dir.join(".canon/vocab/canon.core/plugin.yaml"),
            "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(".canon/vocab/canon.core/directives/task.yaml"),
            "directives:\n  - name: task\n    attrs:\n      - name: desc\n        type: string\n        required: true\n      - name: status\n        type: { domain: task-status }\n        required: true\n      - name: evidence\n        type: evidence\n        required: true\n",
        )
        .unwrap();
        std::fs::write(dir.join(".canon/vocab/canon.core/enums.yaml"), "enums:\n  task-status:\n    - open\n    - done\n").unwrap();
    }

    /// Design.md D3, S10 part2 task 6.1: `resolve_surface`'s `vocab.snapshot`
    /// is byte-identical to a FRESH, independent `canon_vocab::resolve_snapshot`
    /// call over the same repo — proving `canon context` never computes its
    /// own partial vocabulary view (invariant 2, extended).
    #[test]
    fn vocab_surface_matches_a_fresh_independent_resolve_snapshot_call() {
        let dir = fixture_repo(Some("trust_required:\n  test-run: agent\n"));
        seed_vocab(dir.path());

        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let (expected_snapshot, expected_diags) = canon_vocab::resolve_snapshot(dir.path(), None);

        assert_eq!(surface.vocab.snapshot.directives.keys().collect::<Vec<_>>(), expected_snapshot.directives.keys().collect::<Vec<_>>());
        assert!(surface.vocab.snapshot.directives.contains_key("task"), "the seeded `task` directive must appear");
        assert_eq!(surface.vocab.snapshot.enums, expected_snapshot.enums, "vocab enums must be byte-identical to a fresh resolve_snapshot walk");
        assert_eq!(surface.vocab.snapshot.evidence_kinds, expected_snapshot.evidence_kinds);
        assert_eq!(surface.vocab.snapshot.evidence_kinds, vec!["test-run".to_string()]);
        assert_eq!(surface.vocab.diagnostics.len(), expected_diags.len());
    }

    /// Invariant 3, extended: two resolutions of the identical repo — now
    /// including a seeded vocabulary — still produce byte-identical
    /// `render_json` output.
    #[test]
    fn render_json_is_byte_stable_with_a_vocabulary_present() {
        let dir = fixture_repo(Some("trust_required:\n  test-run: agent\n"));
        seed_vocab(dir.path());
        let first = render_json(&resolve_surface(dir.path(), ContextOptions::default()));
        let second = render_json(&resolve_surface(dir.path(), ContextOptions::default()));
        assert_eq!(first, second, "canon context --json must stay byte-identical with a vocabulary present");
    }

    /// Design D5: the vocab section's directive tags, enum names, and
    /// evidence kinds all appear in both JSON and outline renders of the
    /// SAME resolved surface.
    #[test]
    fn json_and_outline_agree_on_the_vocab_section() {
        let dir = fixture_repo(Some("trust_required:\n  test-run: agent\n"));
        seed_vocab(dir.path());
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        let outline = render_outline(&surface, None);

        for tag in surface.vocab.snapshot.directives.keys() {
            assert!(json["vocab"]["snapshot"]["directives"].get(tag).is_some(), "JSON missing vocab directive `{tag}`");
            assert!(outline.contains(&format!("::{tag}")), "outline missing vocab directive `::{tag}`");
        }
        for name in surface.vocab.snapshot.enums.keys() {
            assert!(json["vocab"]["snapshot"]["enums"].get(name).is_some(), "JSON missing vocab enum `{name}`");
            assert!(outline.contains(name.as_str()), "outline missing vocab enum `{name}`");
        }
        for kind in &surface.vocab.snapshot.evidence_kinds {
            assert!(outline.contains(kind.as_str()), "outline missing evidence kind `{kind}`");
        }
    }

    /// Invariant 1, extended: a repo with NO `.canon/vocab/` directory at
    /// all still resolves a full surface — `vocab.snapshot` degrades to the
    /// empty-but-valid snapshot `canon_vocab::resolve_snapshot` itself
    /// documents for a missing vocab dir, never a panic.
    #[test]
    fn resolve_surface_never_fails_with_no_vocab_directory_at_all() {
        let dir = fixture_repo(None);
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        assert!(surface.vocab.snapshot.directives.is_empty());
        assert!(surface.vocab.snapshot.evidence_kinds.is_empty());
    }
    #[test]
    fn risk_tiers_are_projected_to_json_and_outline() {
        let dir = fixture_repo(Some(
            "risk_tiers:\n  high:\n    rank: 3\n    paths: [\"src/auth/**\"]\n    effects: [secret-access]\n    min_human_approvals: 1\n",
        ));
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        assert_eq!(json["policy"]["risk_tiers"]["high"]["rank"], 3);
        assert_eq!(json["policy"]["risk_tiers"]["high"]["paths"][0], "src/auth/**");
        assert_eq!(json["policy"]["risk_tiers"]["high"]["effects"][0], "secret-access");
        assert_eq!(json["policy"]["risk_tiers"]["high"]["min_human_approvals"], 1);
        let outline = render_outline(&surface, None);
        assert!(outline.contains("risk_tiers (1):"));
        assert!(outline.contains("high: rank=3"));

        let absent = fixture_repo(None);
        let absent_surface = resolve_surface(absent.path(), ContextOptions::default());
        let absent_json: serde_json::Value = serde_json::from_str(&render_json(&absent_surface)).unwrap();
        assert_eq!(absent_json["policy"]["risk_tiers"], serde_json::json!({}));
        assert!(render_outline(&absent_surface, None).contains("risk_tiers (0):"));
    }

    /// A broken `spec_coverage` section must read as INVALID in the
    /// surface an author consults, never as the "not enforced here"
    /// absent state — and a well-formed one names every cut it applies,
    /// so `require_cases` is visible before writing a `.feature` file.
    #[test]
    fn a_malformed_spec_coverage_surfaces_as_invalid_and_a_valid_one_names_its_cases() {
        let broken = fixture_repo(Some("spec_coverage:\n  require_evidence: true\n  require_cases: [Not-A-Case]\n"));
        let surface = resolve_surface(broken.path(), ContextOptions::default());
        let section = surface.policy.spec_coverage.as_ref().expect("a present section is never reported absent");
        assert!(matches!(section, SpecCoverageSurface::Invalid { invalid } if invalid.contains("Not-A-Case")), "{section:?}");
        assert!(!surface.policy.clean, "a poisoned section makes the policy unclean");
        assert!(render_outline(&surface, None).contains("spec_coverage: INVALID — "));
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        let invalid = json["policy"]["spec_coverage"].as_object().expect("an invalid section is an object");
        assert_eq!(invalid.keys().collect::<Vec<_>>(), ["invalid"]);
        assert!(invalid["invalid"].as_str().unwrap().contains("Not-A-Case"));
        assert!(json["review"]["requireReview"].is_null(), "an invalid spec_coverage requires no review");

        let valid = fixture_repo(Some("spec_coverage:\n  require_evidence: true\n  exclude_lanes: [process]\n  require_cases: [failure]\n"));
        let surface = resolve_surface(valid.path(), ContextOptions::default());
        assert!(render_outline(&surface, None).contains("\n  spec_coverage: require_evidence=true scope=<every scenario> exclude_lanes=process require_cases=failure\n"));
    }

    /// D4: `spec_coverage`, `evidence_binding`, and `review.requireReview`
    /// are typed JSON objects (booleans, arrays), keyed like their
    /// surrounding section, while the outline lines stay the 0.12 ones.
    #[test]
    fn policy_sections_are_typed_json_and_the_outline_is_unchanged() {
        let dir = fixture_repo(Some(
            "spec_coverage:\n  require_evidence: true\n  scope: [building, verifying]\n  exclude_lanes: [process]\n  require_cases: [failure]\n  require_review:\n    distinct_actor: true\n    block_on_findings: true\nexperimental:\n  evidence_binding:\n    mode: warn\n    strength: report\n    case: [failure]\n    lane: [e2e, unit]\n    scope: [verifying]\n",
        ));
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        assert!(surface.policy.clean, "{:?}", surface.policy.diagnostics);
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        assert_eq!(json["capabilityVersion"], 6);
        assert_eq!(
            json["policy"]["spec_coverage"],
            serde_json::json!({
                "require_evidence": true,
                "scope": ["building", "verifying"],
                "exclude_lanes": ["process"],
                "require_cases": ["failure"],
                "require_review": {"scope": ["verifying", "shipped"], "distinct_actor": true, "block_on_findings": true},
            })
        );
        assert_eq!(
            json["policy"]["evidence_binding"],
            serde_json::json!({"mode": "warn", "strength": "report", "case": ["failure"], "lane": ["e2e", "unit"], "scope": ["verifying"]})
        );
        assert_eq!(json["review"]["requireReview"], serde_json::json!({"scope": ["verifying", "shipped"], "distinctActor": true, "blockOnFindings": true}));

        let outline = render_outline(&surface, None);
        for line in [
            "  spec_coverage: require_evidence=true scope=building, verifying exclude_lanes=process require_cases=failure require_review=(scope=verifying, shipped distinct_actor=true block_on_findings=true)\n",
            "  experimental.evidence_binding: mode=warn strength=report case=failure lane=e2e, unit scope=verifying\n",
            "  require_review: scope=verifying, shipped distinct_actor=true block_on_findings=true\n",
        ] {
            assert!(outline.contains(line), "missing {line:?} in:\n{outline}");
        }

        let defaults = fixture_repo(Some("spec_coverage:\n  require_evidence: false\n  require_review:\n    scope: []\nexperimental:\n  evidence_binding: {}\n"));
        let surface = resolve_surface(defaults.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        assert_eq!(
            json["policy"]["spec_coverage"],
            serde_json::json!({
                "require_evidence": false,
                "scope": [],
                "exclude_lanes": [],
                "require_cases": [],
                "require_review": {"scope": [], "distinct_actor": true, "block_on_findings": true},
            })
        );
        assert_eq!(json["policy"]["evidence_binding"], serde_json::json!({"mode": "off", "strength": "artifact", "case": [], "lane": [], "scope": []}));
        let outline = render_outline(&surface, None);
        assert!(outline.contains("\n  spec_coverage: require_evidence=false scope=<every scenario> require_review=(scope=<every scenario> distinct_actor=true block_on_findings=true)\n"), "{outline}");
        assert!(outline.contains("\n  experimental.evidence_binding: mode=off strength=artifact\n"), "{outline}");
    }

    #[test]
    fn an_invalid_evidence_binding_is_an_invalid_object_and_absent_sections_stay_null_or_omitted() {
        let dir = fixture_repo(Some("experimental:\n  evidence_binding:\n    mode: strict\n"));
        let surface = resolve_surface(dir.path(), ContextOptions::default());
        let json: serde_json::Value = serde_json::from_str(&render_json(&surface)).unwrap();
        let invalid = json["policy"]["evidence_binding"]["invalid"].as_str().expect("an invalid section is {\"invalid\": ...}");
        assert!(invalid.contains("`strict`"), "{invalid}");
        assert_eq!(json["policy"]["evidence_binding"].as_object().unwrap().len(), 1);
        assert!(render_outline(&surface, None).contains(&format!("\n  experimental.evidence_binding: INVALID — {invalid}\n")));
        assert!(json["policy"]["spec_coverage"].is_null());

        let absent = fixture_repo(Some("trust_required:\n  p1: human\n"));
        let json: serde_json::Value = serde_json::from_str(&render_json(&resolve_surface(absent.path(), ContextOptions::default()))).unwrap();
        assert!(json["policy"]["spec_coverage"].is_null());
        assert!(json["policy"].get("evidence_binding").is_none(), "an absent evidence_binding is omitted");
        assert!(json["review"]["requireReview"].is_null());
    }
}
