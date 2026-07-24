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
//! - The `AuthoringSurface` BTreeMap-everywhere/sorted-array determinism
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
//!    [`AuthoringSurface::vocab`] can never diverge from what a typed
//!    atom is actually checked against.
//! 3. **Deterministic output** — every map below is a `BTreeMap` (key-sorted
//!    by construction) and every array is sorted or built from an already-
//!    sorted source; `render_json`/`render_outline` both read the one
//!    resolved [`AuthoringSurface`] value, never re-resolving anything.
//!    [`canon_vocab::CapabilitySnapshot`] holds the same discipline
//!    end-to-end (every map a `BTreeMap`), so embedding it verbatim in
//!    [`AuthoringSurface`] introduces no new non-determinism.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use canon_gate::{PolicyField, PolicyResolution};
use canon_policy::SchemaRegistry;
use canon_vocab::CapabilitySnapshot;
use serde::Serialize;

/// canon-model has not yet introduced per-kind schema-version bumps: every
/// record constructed anywhere in this workspace still passes
/// `Envelope::new(1, ...)` (`canon_model::envelope::Envelope.schema`'s own
/// doc: "bumped on any breaking field change to that kind" — none has
/// happened yet). This is the single current baseline both
/// `capabilityVersion` and every kind's `schema_version` read; the day
/// canon-model versions a kind independently, this becomes a lookup into
/// that registry instead of a constant — a one-line change here, matching
/// design D1's "resolve, then render" split (the registry call site stays
/// singular either way).
const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Resolution-time options beyond the repo root itself. Empty today —
/// `canon context` takes only `--repo`/`--json`, and `--json` selects a
/// RENDERER, never a resolution input (design D5) — kept as [`resolve_surface`]'s
/// own parameter (mirroring `run_context`'s `providers`/`project` opts) so a
/// future resolution-scoped flag (e.g. a `--kind` filter) never has to
/// change `resolve_surface`'s call shape, only this struct.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextOptions {}

/// The Hive-style partition layout one record kind's git-tier files follow —
/// design D3's `partition: LayoutDescriptor`, built from
/// [`RecordKind::partition_template`]/[`RecordKind::is_area_scoped`] (S1's
/// own registry — never a second, hand-written layout table).
#[derive(Debug, Clone, Serialize)]
pub struct PartitionLayout {
    /// The path template, e.g. `kind={kind}/{id}.json` or
    /// `kind={kind}/area={area}/{id}.json`.
    pub template: String,
    /// Whether `template` requires the `area={area}/` segment.
    pub area_scoped: bool,
}

/// One record kind's authoring shape: the fields an agent must supply, the
/// partition it lands in, and its current schema version.
#[derive(Debug, Clone, Serialize)]
pub struct KindSurface {
    pub schema_version: u32,
    /// Every top-level field name this kind's record carries (envelope
    /// fields `schema`/`kind`/`at`/`actor` flattened alongside the kind's
    /// own business fields — `#[serde(flatten)]` puts them all in the same
    /// JSON object, so "envelope fields" for authoring purposes means the
    /// whole top-level shape) mapped to whether it is required.
    pub envelope_fields: BTreeMap<String, bool>,
    pub partition: PartitionLayout,
}

/// One `policy.yaml` field's authoring-time shape: either a fixed value or
/// a CEL predicate's source (never its evaluated result — [`resolve_surface`]
/// has no specific artifact to evaluate against, only the repo's resolved
/// policy shape, matching invariant 1's "describes what CAN be authored").
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyFieldSurface {
    Flat { value: serde_json::Value },
    Cel { expression: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct StalenessSurface {
    pub max_commits_behind: PolicyFieldSurface,
    pub surface_scoped: PolicyFieldSurface,
}

/// `canon_gate::PolicyResolution`'s own fields, summarized field-by-field
/// via [`PolicyFieldSurface`] — one Rust field per `PolicyResolution` field,
/// so the mapping between "what the gate resolved" and "what context shows"
/// stays traceable by name, never a re-derived shape.
#[derive(Debug, Clone, Serialize)]
pub struct PolicySurface {
    pub trust_required: BTreeMap<String, PolicyFieldSurface>,
    pub trust_sample: BTreeMap<String, PolicyFieldSurface>,
    pub staleness: StalenessSurface,
    pub risk_routing: BTreeMap<String, PolicyFieldSurface>,
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
#[derive(Debug, Clone, Serialize)]
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
/// `AuthoringSurface` gain a `policy` section populated by
/// `bindings::bindings_for`"), so this can never disagree with what a
/// `policy.yaml` CEL expression is actually checked against — invariant 2,
/// extended to the CEL surface.
#[derive(Debug, Clone, Serialize)]
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

/// The full authoring surface (design D3): everything an agent needs to
/// know BEFORE writing a canon artifact against this repo. Every map is a
/// `BTreeMap`; every array is sorted or built from an already-sorted
/// source (invariant 3).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringSurface {
    pub capability_version: u32,
    /// By `RecordKind::as_str()` (S1's twelve closed kinds — S11's
    /// registry, D1 of the S11 design).
    pub kinds: BTreeMap<String, KindSurface>,
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
    /// S13 5.1/5.2/5.3 (design D6): by `RecordKind::as_str()`, the SAME
    /// key set `kinds` uses — the CEL binding surface (`record.<field>`
    /// types + allowlisted functions) `policy.yaml`'s own write-time
    /// validator checks a CEL expression against, never a second
    /// independently-derived projection (invariant 2).
    pub cel: BTreeMap<String, CelSurface>,
}

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
///
/// Called by `main.rs`'s `run_context` BEFORE [`resolve_surface`] —
/// `resolve_surface` itself stays a pure "(already-resolved repo, opts) ->
/// surface" fold (its own doc below), so this is the ONE place the
/// ancestor walk happens, never duplicated per call site.
pub fn resolve_repo_root(repo: &Path) -> PathBuf {
    if repo != Path::new(".") {
        return repo.to_path_buf();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| repo.to_path_buf());
    cwd.ancestors().find(|dir| dir.join("canon.yaml").is_file()).map(Path::to_path_buf).unwrap_or(cwd)
}

/// Resolve the `canon.yaml` path a `--repo`/`--canon-yaml`-accepting verb
/// reads from (s26 D2, relocated from `query.rs` so `canon tier age` can
/// share it verbatim rather than inventing a third resolution shape): an
/// explicit `--canon-yaml` (`Some`) wins, used AS-IS -- no ancestor walk,
/// no dependency on `repo` at all (back-compat, spec "--canon-yaml
/// remains an explicit override that bypasses the ancestor walk"). `None`
/// resolves `repo` through [`resolve_repo_root`], then joins `canon.yaml`
/// onto the resolved root.
pub fn resolve_canon_yaml(repo: &Path, canon_yaml: Option<&Path>) -> PathBuf {
    match canon_yaml {
        Some(path) => path.to_path_buf(),
        None => resolve_repo_root(repo).join("canon.yaml"),
    }
}

/// Phase 1 (design D1): resolve the project's schema registry + policy
/// exactly as `canon fmt`/`canon gate` do — [`SchemaRegistry::load`] +
/// [`PolicyResolution::resolve`] and NOTHING else (invariant 2) — then fold
/// the result into one [`AuthoringSurface`] value. Never touches a corpus
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
pub fn resolve_surface(repo: &Path, _opts: ContextOptions) -> AuthoringSurface {
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

    AuthoringSurface {
        capability_version: CURRENT_SCHEMA_VERSION,
        kinds: collect_kinds(&registry),
        enums: collect_enums(&registry),
        join_keys: collect_join_keys(),
        policy: summarize_policy(&policy),
        vocab: VocabularySurface {
            snapshot: vocab_snapshot,
            diagnostics: vocab_diagnostics.into_iter().map(|d| format!("{}: {} ({})", d.code, d.message, d.subject)).collect(),
        },
        cel: collect_cel(&registry),
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
        kinds.insert(kind.as_str().to_string(), KindSurface { schema_version: CURRENT_SCHEMA_VERSION, envelope_fields, partition });
    }
    kinds
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
        cel.insert(kind.as_str().to_string(), CelSurface { fields, functions });
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
        clean: policy.is_clean(),
        diagnostics: policy.diagnostics.iter().map(ToString::to_string).collect(),
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
pub fn render_json(surface: &AuthoringSurface) -> String {
    serde_json::to_string_pretty(surface).unwrap_or_else(|e| format!("{{\"error\": \"failed to serialize authoring surface: {e}\"}}"))
}

/// Phase 2, default mode (design D5): a compact, per-section outline — name
/// counts + name lists, then one indented summary line per entry — for
/// prompt injection, never a full schema dump (ported from the donor's
/// `context_outline`).
pub fn render_outline(surface: &AuthoringSurface) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "capabilityVersion: {}", surface.capability_version);

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

    let _ = writeln!(out, "enums ({}): {}", surface.enums.len(), surface.enums.keys().cloned().collect::<Vec<_>>().join(", "));
    for (name, members) in &surface.enums {
        let _ = writeln!(out, "  {name}: {}", members.join(", "));
    }

    let _ = writeln!(out, "joinKeys ({}): {}", surface.join_keys.len(), surface.join_keys.keys().cloned().collect::<Vec<_>>().join(", "));
    for (key, grammar) in &surface.join_keys {
        let _ = writeln!(out, "  {key}: {grammar}");
    }

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
    let _ = writeln!(out, "  clean: {}", surface.policy.clean);
    let _ = writeln!(out, "  diagnostics ({}):", surface.policy.diagnostics.len());
    for diag in &surface.policy.diagnostics {
        let _ = writeln!(out, "    {diag}");
    }
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
    let _ = writeln!(out, "cel:");
    for (kind, cel) in &surface.cel {
        let _ = writeln!(out, "  {kind}:");
        let _ = writeln!(out, "    fields ({}): {}", cel.fields.len(), cel.fields.keys().cloned().collect::<Vec<_>>().join(", "));
        for (field, ty) in &cel.fields {
            let _ = writeln!(out, "      {field}: {ty}");
        }
        let _ = writeln!(out, "    functions ({}): {}", cel.functions.len(), cel.functions.join(", "));
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

fn check_json_outline_agreement(surface: &AuthoringSurface) -> Result<(), String> {
    let json: serde_json::Value = serde_json::from_str(&render_json(surface)).map_err(|e| e.to_string())?;
    let outline = render_outline(surface);
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
        let outline = render_outline(&surface);

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
            let actual = &surface.cel[kind.as_str()];

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
        let outline = render_outline(&surface);

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
        let outline = render_outline(&surface);

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
}
