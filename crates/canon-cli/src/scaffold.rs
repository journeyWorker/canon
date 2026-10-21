//! `canon scenario new <area>.<surface>.<nn> --title <label> [--feature
//! <path>]` + `canon feature new <area>.<surface> --title <label>` (s16
//! `s16-plugin-extensibility`, P5 `corpus-authoring-scaffold` —
//! INDEPENDENT of s16 P1-P4: a `.feature`-authoring convenience, never
//! a plugin concern, tasks.md task group 5): the two scaffold commands
//! that write S11-conformant `.feature` corpus content directly,
//! matching the leading provenance/tag/header shape `canon_fmt::gherkin::scan`
//! now accepts (while retaining trailing-form compatibility) — never a new
//! `RecordKind`, and NO ledger record of any kind (spec.md's own
//! requirement text: "writes NO ledger record of any kind; its only
//! output is the `.feature` file").
//!
//! # Byte shape
//! ```text
//! Feature: <label>
//!   # canon: {"schema":1,"at":"...","actor":{"agent_id":"..."}}
//!
//!   # canon: {"schema":1,"at":"...","actor":{"agent_id":"..."}}
//!   [@subject:<id>]
//!   [@lane:<value>]
//!   [@case:<value>]
//!   [@flow:<change>.<F-id>]
//!   @<area>.<surface>.<nn>
//!   Scenario: <label>
//!     Given a step
//! ```
//! Scenario provenance leads its tag/header block. The scanner accepts
//! this leading form and the existing first-non-blank-line-after-header
//! form, while Feature provenance stays directly after `Feature:`.
//! A blank line separates the `Feature:` block from the first
//! `Scenario:` block, and every subsequent scenario block from its
//! predecessor.
//! [`run_scenario_new`] produces this via [`append_scenario_block`]'s
//! trim-and-rejoin — the SAME helper whether the file is brand new
//! (created by [`run_feature_new`], or minted fresh by
//! [`run_scenario_new`] itself) or already carries scenarios.
//!
//! # Deterministic provenance, never a bare `Utc::now()`
//! [`run_scenario_new`]/[`run_feature_new`] take `at: DateTime<Utc>` as an
//! EXPLICIT parameter — `main.rs`'s dispatch match arms are the ONE
//! place `Utc::now()` is ever called for these commands, truncated to
//! whole seconds there, so a file this module writes in ONE invocation
//! never straddles two different timestamps even when it stamps two
//! provenance comments at once. The actor is supplied by the caller,
//! with the CLI defaulting to `CANON_ACTOR` then `canon-scaffold`.
//!
//! # Writes NO ledger record
//! Neither function touches `canon-store`/`GitTier` at all — the ONLY
//! side effect either has is the one `.feature` file write (module doc
//! above).

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use canon_model::family::feature::FeatureProvenance;
use canon_model::family::FamilyKind;
use canon_model::{Actor, ChangeId, ProjectId, ScenarioId, SubjectId};
use chrono::{DateTime, Utc};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::inventory::{SpecRoot, SyncCtx};

/// `<area>.<surface>` — `canon feature new`'s own tag shape. No
/// [`ScenarioId`]-like newtype exists for this bare 2-segment grammar
/// (`canon_model::ids`'s own module doc: eight join-spine keys, no
/// bare-surface ninth), so this mirrors `ScenarioId`'s own per-segment
/// grammar (`[a-z0-9-]+`, `canon_model::ids::is_scenario_id`'s
/// `ok_segment`) exactly, rather than inventing a looser one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaSurface {
    pub area: String,
    pub surface: String,
}

fn is_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl AreaSurface {
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('.').collect();
        let [area, surface] = parts.as_slice() else {
            return Err(format!(
                "invalid <area>.<surface> `{s}`: expected exactly one `.` separator (grammar `[a-z0-9-]+\\.[a-z0-9-]+`, matching `ScenarioId`'s own first two segments)"
            ));
        };
        if !is_segment(area) || !is_segment(surface) {
            return Err(format!(
                "invalid <area>.<surface> `{s}`: both segments must match `[a-z0-9-]+` (matching `ScenarioId`'s own first two segments)"
            ));
        }
        Ok(Self { area: (*area).to_string(), surface: (*surface).to_string() })
    }
}

/// `<tag>`'s `clap` value parser (`canon scenario new`) — reuses
/// [`ScenarioId::parse`] verbatim, never a second `<area>.<surface>.<nn>`
/// grammar, save for stripping AT MOST one leading `@` first (s26 D3):
/// `@story.x.01` and `story.x.01` are equivalent INPUT spellings for this
/// one `clap` boundary (the `@`-prefixed form is what scenario bodies
/// themselves use, e.g. `Scenario: @story.x.01`) — [`ScenarioId::parse`]
/// itself is called with the (possibly-stripped) rest verbatim, so its
/// grammar and every other call site (gate evidence matching, inventory
/// sync, query scope filters) stays untouched and `@`-free.
pub fn parse_scenario_tag(s: &str) -> Result<ScenarioId, String> {
    ScenarioId::parse(s.strip_prefix('@').unwrap_or(s)).map_err(|e| e.to_string())
}


/// `<surface>`'s `clap` value parser (`canon feature new`).
pub fn parse_area_surface(s: &str) -> Result<AreaSurface, String> {
    AreaSurface::parse(s)
}

/// `<lane>`'s `clap` value parser (`canon scenario new`) — the same
/// kebab-slug grammar used by model `domain` values.
pub fn parse_lane_slug(s: &str) -> Result<String, String> {
    let valid = !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.contains("--");
    if valid {
        Ok(s.to_string())
    } else {
        Err(format!("invalid lane `{s}`: expected a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`)"))
    }
}

/// The 2-space-indented `# canon: {...}` comment line every
/// `Feature:`/`Scenario:` header in this module's output carries,
/// built fresh per call so a caller stamping two headers in one write
/// can still reuse the SAME `at`/actor for both.
fn provenance_line(at: DateTime<Utc>, actor: &Actor) -> String {
    let prov = FeatureProvenance::new(1, at, actor.clone());
    format!("  {}", prov.render_comment_line())
}

/// Every `@<area>.<surface>.<nn>`-shaped tag anywhere under `root`'s
/// `kind=feature/` corpus (`gherkin::scan`'s own `scenario_ids` — every
/// tag found, whether or not it paired with a following `Scenario:`
/// header, unlike `crate::inventory::scan_feature_corpus`'s paired-only
/// `scenarios`) — used ONLY for duplicate-tag existence checking here
/// (never a second, title/digest-resolving copy of that scan, which
/// duplicate detection doesn't need).
fn corpus_tags(root: &Path) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    for path in canon_fmt::util::feature_files(root) {
        let Ok(text) = fs::read_to_string(&path) else { continue };
        tags.extend(canon_fmt::gherkin::scan(&text).scenario_ids);
    }
    tags
}

/// The optional classification tags a scaffolded scenario carries, in
/// the order they are written above its id tag.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScenarioAxes<'a> {
    pub subject: Option<&'a str>,
    pub lane: Option<&'a str>,
    pub case: Option<&'a str>,
    /// `<change>.<F-id>` of the brief flow step this scenario covers
    /// (0.19 D4), written as `@flow:<value>`.
    pub flow: Option<&'a str>,
}

/// Append one scenario block. Provenance leads the block, followed by
/// the optional subject/lane/case/flow tags in that order, then the id tag
/// and header.
fn append_scenario_block(existing: &str, tag: &str, title: &str, prov_line: &str, axes: ScenarioAxes<'_>) -> String {
    let mut block = String::new();
    block.push_str(prov_line);
    block.push('\n');
    for (axis, value) in [("subject", axes.subject), ("lane", axes.lane), ("case", axes.case), ("flow", axes.flow)] {
        if let Some(value) = value {
            block.push_str(&format!("  @{axis}:{value}\n"));
        }
    }
    block.push_str(&format!("  @{tag}\n  Scenario: {title}\n    Given a step\n"));
    let trimmed = existing.trim_end_matches('\n');
    let mut out = String::with_capacity(trimmed.len() + block.len() + 2);
    out.push_str(trimmed);
    out.push_str("\n\n");
    out.push_str(&block);
    out
}
/// Where `<area>.<surface>`'s `.feature` file lives under `root`:
/// `<root>/kind=feature/area=<area>/<surface>.feature` (0.17 D1), the
/// directory [`FamilyKind::Feature`]'s descriptor declares.
pub fn resolve_feature_path(root: &SpecRoot, area: &str, surface: &str) -> PathBuf {
    root.root.join(FamilyKind::Feature.layout_descriptor().corpus_dir()).join(format!("area={area}")).join(format!("{surface}.feature"))
}

/// Refuse a scaffold write under a root still on the pre-0.17 layout
/// (0.17 D1): writing `kind=feature/` beside `features/kind=feature/`
/// would leave a root no reader accepts. `Err` is the message without
/// the command prefix, naming the root and `canon migrate`.
fn refuse_pre_0_17(repo: &Path, root: &SpecRoot) -> Result<(), String> {
    match crate::inventory::pre_0_17_detail(repo, root) {
        Some(detail) => Err(format!("{detail}; nothing written")),
        None => Ok(()),
    }
}

/// D3: whether `path` (already resolved to an absolute path) falls
/// under `root`'s directory — a canonicalized, path-component-wise
/// prefix check (`Path::starts_with` compares whole components, so a
/// root named `specs` never falsely accepts a sibling `specs2`), never
/// a naive string-prefix compare.
fn path_under_root(path: &Path, root: &Path) -> bool {
    canonicalize_best_effort(path).starts_with(canonicalize_best_effort(root))
}

fn path_under_any_root(path: &Path, roots: &[SpecRoot]) -> bool {
    roots.iter().any(|r| path_under_root(path, &r.root))
}

/// Canonicalize as much of `path` as already exists on disk, then
/// append whatever tail doesn't (a target `.feature` file, or even its
/// whole `specs/` root, may not exist yet at validation time — plain
/// `fs::canonicalize` would hard-fail on that). Resolving the deepest
/// EXISTING ancestor (symlinks included) keeps the component-wise
/// prefix check in [`path_under_root`] meaningful before any directory
/// is ever created, since both the root and the candidate path walk up
/// to and resolve through the same real ancestor.
fn canonicalize_best_effort(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(canon) = existing.canonicalize() {
            let mut out = canon;
            for component in tail.iter().rev() {
                out.push(component);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Pick the ONE configured `specs.roots[]` entry a scaffold write
/// lands under — the single owner of that choice for BOTH
/// [`run_feature_new`] and [`run_scenario_new`]'s tag-derived path, so
/// the two commands can never drift into two different disambiguation
/// rules or two differently-worded refusals. `Err` carries the
/// operator-facing message WITHOUT a leading `canon <cmd>: ` prefix;
/// each caller prints it behind its own prefix exactly as it prints
/// every other refusal it owns.
///
/// Resolution, in order: an explicit `project` selects by id at ANY
/// root count; absent it, a lone configured root is unambiguous and is
/// taken as-is (the single-root repo never has to learn the flag
/// exists); anything else refuses loud, never a guess.
///
/// Selection is by CONFIGURED ID, never by directory, for three
/// reasons that all point the same way: the id is what the operator
/// already wrote in `canon.yaml` and the only handle they have on a
/// root, it is literally what becomes the `project_id` of every record
/// `canon inventory sync` later materializes from this corpus (so
/// naming it here is naming the key), and a directory-shaped bypass
/// would resolve to a synthetic root carrying
/// `crate::inventory::default_root_id()` instead of the configured id
/// — silently mis-keying the very corpus the write is joining. That
/// directory bypass is `canon inventory sync --spec-root`'s
/// deliberately DIFFERENT job (an ad hoc root OUTSIDE config, whose id
/// is immaterial to a read-and-validate pass), never this one's.
fn resolve_spec_root<'a>(command: &str, roots: &'a [SpecRoot], project: Option<&ProjectId>) -> Result<&'a SpecRoot, String> {
    // Every refusal below names the full configured set: the operator
    // who got the id wrong, or didn't know one was needed, learns the
    // exact accepted values from the refusal itself.
    let configured = || roots.iter().map(|r| r.id.as_str()).collect::<Vec<_>>().join(", ");
    match (project, roots) {
        (Some(id), _) => roots.iter().find(|r| &r.id == id).ok_or_else(|| {
            format!("`--project-id {}` names no configured `specs.roots[]` entry (configured ids: {})", id.as_str(), configured())
        }),
        (None, [one]) => Ok(one),
        (None, many) => Err(format!(
            "{} configured `specs.roots[]` entries (ids: {}); pass `--project-id <id>` to select which one `{command}` writes under",
            many.len(),
            configured()
        )),
    }
}

/// One scenario `canon scenario new` scaffolds: the positional tag and
/// every flag of the single form, or one line of its `--from` NDJSON.
#[derive(Debug, Clone)]
pub struct ScenarioSpec {
    pub tag: ScenarioId,
    pub title: String,
    pub subject: Option<SubjectId>,
    pub lane: Option<String>,
    pub case: Option<String>,
    pub flow: Option<String>,
    pub change: Option<ChangeId>,
    pub feature: Option<PathBuf>,
    pub project: Option<ProjectId>,
}

/// A validated scenario: where it lands and the `@flow:` value it carries.
struct Planned {
    feature_path: PathBuf,
    flow_tag: Option<String>,
    /// The target file's text now (empty when it does not exist yet).
    existing: String,
}

/// A `.feature` file with the scenarios this run appends, not yet written.
struct Pending {
    path: PathBuf,
    content: String,
}

type FlowKey = (Option<String>, Option<String>, String);

/// The validation every `scenario new` form shares: loaded once per run, so
/// a batch scans the corpus and resolves each flow only once.
struct Planner {
    repo_root: PathBuf,
    roots: Vec<SpecRoot>,
    /// Every `@<id>` tag under each of `roots`, in the same order.
    corpus: Vec<BTreeSet<String>>,
    flows: HashMap<FlowKey, Result<String, String>>,
}

impl Planner {
    fn new(repo_root: &Path) -> Result<Self, CliError> {
        let roots = SyncCtx::from_repo(repo_root).and_then(|ctx| ctx.spec_roots(None)).map_err(|e| CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again"))?;
        let corpus = roots.iter().map(|root| corpus_tags(&root.root)).collect();
        Ok(Self { repo_root: repo_root.to_path_buf(), roots, corpus, flows: HashMap::new() })
    }

    /// Every check a single `scenario new` makes, in its order; nothing is written.
    fn plan(&mut self, spec: &ScenarioSpec) -> Result<Planned, CliError> {
        let flow_tag = match &spec.flow {
            None => None,
            Some(flow) => {
                let key = (spec.subject.as_ref().map(|s| s.as_str().to_string()), spec.change.as_ref().map(|c| c.as_str().to_string()), flow.clone());
                let repo_root = &self.repo_root;
                let found = self.flows.entry(key).or_insert_with(|| crate::brief_check::flow_tag_value(repo_root, spec.subject.as_ref(), spec.change.as_ref(), flow)).clone();
                Some(found.map_err(|e| CliError::refused("SCAFFOLD_INVALID_ARGUMENT", e).hint("`canon schema scenario new` lists the accepted arguments and their forms").field("flow"))?)
            }
        };
        for (axis, value) in [("lane", spec.lane.as_deref()), ("case", spec.case.as_deref())] {
            if let Some(value) = value {
                if let Some(violation) = crate::subject::enum_membership_violation(&self.repo_root, axis, value) {
                    return Err(CliError::refused("SCAFFOLD_INVALID_ARGUMENT", violation).hint("`canon schema scenario new` lists the accepted arguments and their forms").field(axis));
                }
            }
        }
        let tag = &spec.tag;

        // Resolve the target `.feature` path FIRST — spec.md's own
        // ordering requirement (design D3): root-membership validation for
        // an explicit `--feature` runs BEFORE the duplicate-tag/target-file
        // checks below.
        let feature_path: PathBuf = match spec.feature.as_deref() {
            None => {
                // D2: the tag-derived default mirrors `run_feature_new`'s
                // own root selection — the SAME [`resolve_spec_root`], so
                // `--project-id` means one thing across both commands and an
                // unresolvable root is never guessed at.
                let root = resolve_spec_root("canon scenario new", &self.roots, spec.project.as_ref()).map_err(|msg| CliError::refused("SPEC_ROOT_UNRESOLVED", msg).hint("pass --project-id <id> naming a configured spec root (`specs.roots[]` in canon.yaml)").field("project-id"))?;
                resolve_feature_path(root, tag.area(), tag.surface())
            }
            Some(feature) => {
                if let Some(project) = spec.project.as_ref() {
                    crate::inventory::check_spec_root(&self.roots, project)?;
                }
                let resolved: PathBuf = if feature.is_absolute() { feature.to_path_buf() } else { self.repo_root.join(feature) };
                if !path_under_any_root(&resolved, &self.roots) {
                    return Err(CliError::refused("FEATURE_OUTSIDE_SPEC_ROOTS", format!("`{}` does not resolve under any configured `specs.roots[]` entry ({}); never a silent orphan write outside the validated corpus",
                        resolved.display(),
                        self.roots.iter().map(|r| r.root.display().to_string()).collect::<Vec<_>>().join(", ")
                    )).hint("put the file under a configured `specs.roots[]` entry, or add its root to canon.yaml").field("feature"));
                }
                resolved
            }
        };
        for root in self.roots.iter().filter(|root| path_under_root(&feature_path, &root.root)) {
            if let Err(msg) = refuse_pre_0_17(&self.repo_root, root) {
                return Err(CliError::refused("LAYOUT_MIGRATION_REQUIRED", msg).hint("run `canon migrate` to move the repository to the current layout"));
            }
        }

        for (root, tags) in self.roots.iter().zip(&self.corpus) {
            if tags.contains(tag.as_str()) {
                return Err(CliError::refused("SCENARIO_TAG_EXISTS", format!("`@{}` already exists under spec root `{}` ({}); never a silent duplicate",
                    tag.as_str(),
                    root.id.as_str(),
                    root.root.display()
                )).hint("choose an unused scenario id; `canon inventory sync` refreshes the index of the existing ones").field("id"));
            }
        }

        let existing = if feature_path.exists() {
            fs::read_to_string(&feature_path).map_err(|e| CliError::refused("REPO_UNREADABLE", format!("failed to read `{}`: {e}", feature_path.display())).hint("check that the repository is readable from here, or pass --repo <path> for its root"))?
        } else {
            String::new()
        };
        // Belt-and-suspenders: guard the target file directly too, even
        // when it sits outside every configured spec root — spec.md's own
        // literal duplicate-tag scenario is "runs a second time against a
        // `.feature` file that already carries" the tag, the SAME target
        // file, regardless of corpus-root config.
        if canon_fmt::gherkin::scan(&existing).scenario_ids.iter().any(|t| t == tag.as_str()) {
            return Err(CliError::refused("SCENARIO_TAG_EXISTS", format!("`@{}` already exists in `{}`; never a silent duplicate", tag.as_str(), feature_path.display())).hint("choose an unused scenario id; `canon inventory sync` refreshes the index of the existing ones").field("id"));
        }
        Ok(Planned { feature_path, flow_tag, existing })
    }
}

/// Append `spec`'s block to its file's pending content, starting the
/// file's `Feature:` header when it is new (task 5.1). No
/// `--feature-title` flag exists on this command — `<area> <surface>`
/// space-joined mirrors the one hand-authored fixture in this workspace.
fn stage(pending: &mut Vec<Pending>, planned: Planned, spec: &ScenarioSpec, prov_line: &str) {
    let axes = ScenarioAxes { subject: spec.subject.as_ref().map(SubjectId::as_str), lane: spec.lane.as_deref(), case: spec.case.as_deref(), flow: planned.flow_tag.as_deref() };
    let at = match pending.iter().position(|p| p.path == planned.feature_path) {
        Some(at) => at,
        None => {
            let content = if planned.existing.trim().is_empty() { format!("Feature: {} {}\n{prov_line}\n", spec.tag.area(), spec.tag.surface()) } else { planned.existing };
            pending.push(Pending { path: planned.feature_path, content });
            pending.len() - 1
        }
    };
    pending[at].content = append_scenario_block(&pending[at].content, spec.tag.as_str(), &spec.title, prov_line, axes);
}

/// Write every pending file, creating its directory.
fn write_pending(pending: &[Pending]) -> Result<(), CliError> {
    for file in pending {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent).map_err(|e| CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", parent.display())).hint("check that the path is writable and the disk has room, then run the command again"))?;
        }
        fs::write(&file.path, &file.content).map_err(|e| CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", file.path.display())).hint("check that the path is writable and the disk has room, then run the command again"))?;
    }
    Ok(())
}

/// What `canon scenario new` says about the `# canon: {...}` lines it
/// writes (#64): filling in the scenario must not drop them, or
/// `canon inventory sync` aborts on `missing-provenance`. Human output
/// prints it; `--json` carries it as `meta.note`.
const PROVENANCE_NOTE: &str = "keep the `# canon: {...}` provenance lines when you fill in the scenario: one right after `Feature:` and one above the scenario's tags; without them `canon inventory sync` aborts on missing-provenance";

fn provenance_meta() -> serde_json::Value {
    serde_json::json!({ "note": PROVENANCE_NOTE })
}

/// `canon scenario new <tag> --title <label> [--feature <path>]
/// [--project-id <id>]` (task 5.1; s19 `derived-validated-scenario-feature`
/// makes `--feature` optional, design D1-D3). Returns the process exit
/// code: `0` on a successful append/create, `2` on a refused
/// invocation — a `specs.roots[]` config fault, a spec root
/// [`resolve_spec_root`] cannot pin down when `--feature` is omitted
/// (design D2), an explicit `--feature` path resolving outside every
/// configured root (design D3), a target root still on the pre-0.17
/// layout (0.17 D1), or `tag` already existing somewhere in
/// the target feature corpus — with ZERO bytes written either way,
/// mirroring `canon review add`'s own
/// refusal-exits-`2`/nothing-written convention
/// (`crate::review::run_add`).
///
/// `spec.project` is `--project-id <id>`: which configured `specs.roots[]`
/// entry the DERIVED path lands under, selected by the root's `id`
/// exactly as [`run_feature_new`] selects it ([`resolve_spec_root`]).
pub fn run_scenario_new(repo: &Path, spec: &ScenarioSpec, actor: &Actor, at: DateTime<Utc>, json: bool) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let planned = match Planner::new(&repo_root).and_then(|mut planner| planner.plan(spec)) {
        Ok(planned) => planned,
        Err(e) => return e.emit("canon scenario new", json),
    };
    let feature_path = planned.feature_path.clone();
    let mut pending = Vec::new();
    stage(&mut pending, planned, spec, &provenance_line(at, actor));
    if let Err(e) = write_pending(&pending) {
        return e.emit("canon scenario new", json);
    }
    let tag = &spec.tag;
    crate::say!(json, "canon scenario new: wrote `@{}` to {} — {}", tag.as_str(), feature_path.display(), crate::write_mode::DIRECT);
    crate::say!(json, "canon scenario new: {PROVENANCE_NOTE}");
    let scope = crate::next::Scope::Scenarios(vec![tag.as_str().to_string()]);
    crate::next::finish_with_meta(&repo_root, "scenario new", &scope, false, json, serde_json::json!({ "scenarioId": tag.as_str(), "feature": feature_path.display().to_string() }), provenance_meta());
    0
}

/// One `--from` line: the single form's fields by their long flag names
/// (`id` for the positional tag). The removed names of the renamed flags
/// (`subject`, `change`, `project`) are refused with the name to use
/// ([`removed_key_refusal`]).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchLine {
    id: String,
    title: String,
    #[serde(rename = "subject-id")]
    subject_id: Option<String>,
    lane: Option<String>,
    case: Option<String>,
    flow: Option<String>,
    #[serde(rename = "change-id")]
    change_id: Option<String>,
    feature: Option<PathBuf>,
    #[serde(rename = "project-id")]
    project_id: Option<String>,
}

/// The refusal for a `--from` line that still carries a removed key.
fn removed_key_refusal(line: &serde_json::Value) -> Option<CliError> {
    ["subject", "change", "project"].into_iter().find_map(|old| {
        line.get(old)?;
        let new = crate::removed::batch_key_replacement(old)?;
        Some(CliError::refused("SCAFFOLD_INVALID_ARGUMENT", format!("`{old}` was removed in 0.25.0")).hint(crate::removed::batch_key_hint(old, new)).field(old).expected([new]))
    })
}

impl BatchLine {
    /// The same grammar checks the single form's clap parsers make.
    fn into_spec(self) -> Result<ScenarioSpec, CliError> {
        fn field<T>(name: &'static str, parsed: Result<T, String>) -> Result<T, CliError> {
            parsed.map_err(|e| CliError::refused("SCAFFOLD_INVALID_ARGUMENT", e).hint("`canon schema scenario new` lists the accepted arguments and their forms").field(name))
        }
        if self.change_id.is_some() && self.flow.is_none() {
            return Err(CliError::refused("SCAFFOLD_INVALID_ARGUMENT", "`change-id` needs `flow`").hint("`canon schema scenario new` lists the accepted arguments and their forms").field("change-id"));
        }
        Ok(ScenarioSpec {
            tag: field("id", parse_scenario_tag(&self.id))?,
            title: self.title,
            subject: self.subject_id.as_deref().map(|s| field("subject-id", crate::subject::parse_subject_id(s))).transpose()?,
            lane: self.lane.as_deref().map(|s| field("lane", parse_lane_slug(s))).transpose()?,
            case: self.case.as_deref().map(|s| field("case", parse_lane_slug(s))).transpose()?,
            flow: self.flow,
            change: self.change_id.as_deref().map(|s| field("change-id", crate::subject::parse_change_id(s))).transpose()?,
            feature: self.feature,
            project: self.project_id.as_deref().map(|s| field("project-id", crate::review::parse_project_id(s))).transpose()?,
        })
    }
}

/// A `--from` line that failed validation.
struct BadLine {
    line: usize,
    id: Option<String>,
    error: CliError,
}

impl BadLine {
    fn to_json(&self) -> serde_json::Value {
        let mut error = serde_json::json!({ "code": self.error.code, "message": self.error.message });
        if let Some(hint) = &self.error.hint {
            error["hint"] = serde_json::json!(hint);
        }
        if let Some(field) = &self.error.field {
            error["field"] = serde_json::json!(field);
        }
        serde_json::json!({ "line": self.line, "id": self.id, "error": error })
    }
}

/// `canon scenario new --from <file|->`: one scenario per NDJSON line
/// (blank lines and `#` comments skipped), each with the single form's
/// fields by long flag name. Every line is validated first, with the checks
/// of the single form plus a repeated id within the batch; any bad line
/// refuses the whole batch (`2`, nothing written) and `data.items` names
/// each one. Otherwise every scenario is written as N single calls would
/// write them, under one provenance timestamp, and `data.items` lists
/// `{line, id, path}` for each.
pub fn run_scenario_batch(repo: &Path, source: &str, actor: &Actor, at: DateTime<Utc>, json: bool) -> i32 {
    const COMMAND: &str = "canon scenario new";
    let text = if source == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).map(|_| text)
    } else {
        fs::read_to_string(source)
    };
    let text = match text {
        Ok(text) => text,
        Err(e) => return CliError::refused("REPO_UNREADABLE", format!("--from {source}: {e}")).hint("check that the repository is readable from here, or pass --repo <path> for its root").field("from").emit(COMMAND, json),
    };
    let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty() && !l.starts_with('#')).collect();
    if lines.is_empty() {
        return CliError::refused("SCAFFOLD_INVALID_ARGUMENT", format!("--from {source} holds no scenario line (one JSON object per line)")).hint("`canon schema scenario new` lists the accepted arguments and their forms").field("from").emit(COMMAND, json);
    }

    let repo_root = resolve_repo_root(repo);
    let mut planner = match Planner::new(&repo_root) {
        Ok(planner) => planner,
        Err(e) => return e.emit(COMMAND, json),
    };
    let prov_line = provenance_line(at, actor);
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut bad: Vec<BadLine> = Vec::new();
    let mut good: Vec<(usize, ScenarioSpec, Planned)> = Vec::new();
    for (line, raw) in &lines {
        let value: serde_json::Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(e) => {
                bad.push(BadLine { line: *line, id: None, error: CliError::refused("SCAFFOLD_INVALID_ARGUMENT", format!("not a JSON object: {e}")).hint("`canon schema scenario new` lists the accepted arguments and their forms") });
                continue;
            }
        };
        let id = value.get("id").and_then(|v| v.as_str()).map(str::to_string);
        let removed = removed_key_refusal(&value);
        let planned = removed
            .map_or(Ok(()), Err)
            .and_then(|()| serde_json::from_value::<BatchLine>(value).map_err(|e| CliError::refused("SCAFFOLD_INVALID_ARGUMENT", e.to_string()).hint("`canon schema scenario new` lists the accepted arguments and their forms")))
            .and_then(BatchLine::into_spec)
            .and_then(|spec| {
                let planned = planner.plan(&spec)?;
                match seen.get(spec.tag.as_str()) {
                    Some(first) => Err(CliError::refused("SCENARIO_TAG_EXISTS", format!("`@{}` repeats line {first} of this batch; never a silent duplicate", spec.tag.as_str())).hint("choose an unused scenario id; `canon inventory sync` refreshes the index of the existing ones").field("id")),
                    None => Ok((spec, planned)),
                }
            });
        // The first line to use an id owns it, valid or not.
        if let Some(id) = id.as_deref().and_then(|id| parse_scenario_tag(id).ok()) {
            seen.entry(id.as_str().to_string()).or_insert(*line);
        }
        match planned {
            Ok((spec, planned)) => good.push((*line, spec, planned)),
            Err(error) => bad.push(BadLine { line: *line, id, error }),
        }
    }

    if !bad.is_empty() {
        for item in &bad {
            eprintln!("{COMMAND}: line {}{}: {} — {}", item.line, item.id.as_deref().map(|id| format!(" (`{id}`)")).unwrap_or_default(), item.error.code, item.error.message);
        }
        let refusal = CliError::refused("SCAFFOLD_INVALID_ARGUMENT", format!("{} of {} line(s) invalid; nothing written", bad.len(), lines.len())).hint("fix the listed lines and run the batch again");
        eprintln!("{}", refusal.human(COMMAND));
        return refusal.emit_with_data(COMMAND, json, serde_json::json!({ "items": bad.iter().map(BadLine::to_json).collect::<Vec<_>>() }));
    }

    let mut pending = Vec::new();
    let mut items = Vec::new();
    let mut ids = Vec::new();
    for (line, spec, planned) in good {
        items.push(serde_json::json!({ "line": line, "id": spec.tag.as_str(), "path": planned.feature_path.display().to_string() }));
        ids.push(spec.tag.as_str().to_string());
        stage(&mut pending, planned, &spec, &prov_line);
    }
    if let Err(e) = write_pending(&pending) {
        return e.emit(COMMAND, json);
    }
    for item in &items {
        crate::say!(json, "{COMMAND}: wrote `@{}` to {}", item["id"].as_str().unwrap_or_default(), item["path"].as_str().unwrap_or_default());
    }
    crate::say!(json, "{COMMAND}: wrote {} scenario(s) — {}", items.len(), crate::write_mode::DIRECT);
    crate::say!(json, "{COMMAND}: {PROVENANCE_NOTE}");
    crate::next::finish_with_meta(&repo_root, "scenario new", &crate::next::Scope::Scenarios(ids), false, json, serde_json::json!({ "items": items }), provenance_meta());
    0
}

/// `canon feature new <area>.<surface> --title <label> [--project-id
/// <id>]` (task 5.2). Returns the process exit code: `0` on a fresh
/// file written, `2` on a refused invocation (a `specs.roots[]` config
/// fault, a spec root [`resolve_spec_root`] cannot pin down — a
/// multi-root config with no `--project-id <id>`, or a `--project-id` naming
/// no configured entry — a root still on the pre-0.17 layout (0.17
/// D1), or the target file already existing). Uses `create_new` (atomic create-fails-if-exists), never
/// a check-then-write race, so the existing file's bytes are UNTOUCHED
/// in every refusal case. The path is derived via
/// [`resolve_feature_path`] (s19 design D1) — the SAME
/// `kind=feature/area=<area>/<surface>.feature` layout
/// `canon_model::family::FamilyKind::Feature::layout_descriptor`
/// declares and `canon-fmt`/`canon inventory sync` already validate
/// against.
///
/// The written stub is a `Feature:` header + `# canon:` provenance with
/// ZERO scenarios (spec: "a starting point for subsequent `canon
/// scenario new` calls"). An empty feature is not yet a valid corpus
/// entry, so `canon fmt --check`'s feature resolver flags it (no
/// `@<area>.<surface>.<nn>` tag to derive `area` from) until the first
/// `canon scenario new` against this file adds one — success prints a
/// next-step hint naming the exact invocation that closes that gap
/// (s19 `wip-feature-stub-class`, design D4); the
/// `corpus-authoring-scaffold` spec deliberately ties the fmt-clean
/// round-trip to `scenario new`'s output, never this bare stub.
///
/// `project` is `--project-id <id>`, naming which configured
/// `specs.roots[]` entry to scaffold under by that entry's `id`. By
/// id, not by directory: the id is the operator's own handle on the
/// root and is what becomes the `project_id` key of every record
/// `canon inventory sync` later derives from this file, whereas a
/// directory-shaped override would stamp
/// `crate::inventory::default_root_id()` over the configured id and
/// mis-key the corpus — that override is `canon inventory sync
/// --spec-root`'s separate contract, not this command's
/// ([`resolve_spec_root`]).
pub fn run_feature_new(
    repo: &Path,
    area_surface: &AreaSurface,
    title: &str,
    project: Option<&ProjectId>,
    actor: &Actor,
    at: DateTime<Utc>,
    json: bool,
) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let roots = match SyncCtx::from_repo(&repo_root).and_then(|ctx| ctx.spec_roots(None)) {
        Ok(roots) => roots,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again").emit("canon feature new", json);
        }
    };
    let root = match resolve_spec_root("canon feature new", &roots, project) {
        Ok(root) => root,
        Err(msg) => {
            return CliError::refused("SPEC_ROOT_UNRESOLVED", msg.to_string()).hint("pass --project-id <id> naming a configured spec root (`specs.roots[]` in canon.yaml)").emit("canon feature new", json);
        }
    };

    if let Err(msg) = refuse_pre_0_17(&repo_root, root) {
        return CliError::refused("LAYOUT_MIGRATION_REQUIRED", msg.to_string()).hint("run `canon migrate` to move the repository to the current layout").emit("canon feature new", json);
    }

    let feature_path = resolve_feature_path(root, &area_surface.area, &area_surface.surface);
    // The next-step hint below must be an invocation that actually
    // RUNS (design D4 calls it "the exact invocation that closes that
    // gap"), so it carries `--project-id` exactly when a bare `canon
    // scenario new` would refuse for ambiguity — echoing the root just
    // resolved rather than making the operator re-derive it.
    let project_hint = if roots.len() > 1 { format!(" --project-id {}", root.id.as_str()) } else { String::new() };

    if let Some(parent) = feature_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", parent.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon feature new", json);
        }
    }

    let content = format!("Feature: {title}\n{}\n", provenance_line(at, actor));
    match fs::OpenOptions::new().write(true).create_new(true).open(&feature_path) {
        Ok(mut file) => match file.write_all(content.as_bytes()) {
            Ok(()) => {
                let next = format!("canon scenario new {}.{}.01 --title '<label>'{project_hint} [--feature <path>]", area_surface.area, area_surface.surface);
                crate::say!(json, "canon feature new: wrote {} — {}", feature_path.display(), crate::write_mode::DIRECT);
                crate::say!(json, "canon feature new: next: `{next}` to make it fmt-clean");
                if json {
                    let step = crate::status::NextStep {
                        command: next.clone(),
                        why: "the stub holds no scenario yet, so it is not fmt-clean; add the first".into(),
                        actor: crate::status::StepActor::Agent,
                        role: Some(crate::role::Role::Planner),
                    };
                    crate::json_out::data_with_next("feature new", serde_json::json!({ "feature": feature_path.display().to_string(), "next": next }), &crate::next::After::fixed(vec![step]));
                }
                0
            }
            Err(e) => {
                CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", feature_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon feature new", json)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            CliError::refused("FEATURE_EXISTS", format!("`{}` already exists; never overwriting an existing feature file", feature_path.display())).hint("choose another area.surface, or edit the existing feature file").emit("canon feature new", json)
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", feature_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon feature new", json)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_surface_parses_a_well_formed_two_segment_tag() {
        let parsed = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(parsed, AreaSurface { area: "world".to_string(), surface: "hotdeal".to_string() });
    }

    #[test]
    fn area_surface_rejects_a_three_segment_scenario_id_shaped_string() {
        assert!(AreaSurface::parse("world.hotdeal.42").is_err());
    }

    #[test]
    fn area_surface_rejects_an_empty_segment() {
        assert!(AreaSurface::parse("world.").is_err());
        assert!(AreaSurface::parse(".hotdeal").is_err());
    }

    fn actor() -> Actor {
        Actor::new_unattributed("test-actor")
    }

    #[test]
    fn area_surface_rejects_uppercase_or_underscore() {
        assert!(AreaSurface::parse("World.hotdeal").is_err());
        assert!(AreaSurface::parse("world.hot_deal").is_err());
    }

    #[test]
    fn parse_scenario_tag_strips_a_single_leading_at_and_matches_the_bare_form() {
        let at_prefixed = parse_scenario_tag("@story.x.01").unwrap();
        let bare = parse_scenario_tag("story.x.01").unwrap();
        assert_eq!(at_prefixed, bare);
    }

    #[test]
    fn parse_scenario_tag_rejects_a_malformed_tag_with_or_without_the_at_prefix() {
        assert!(parse_scenario_tag("@Story.X.01").is_err());
        assert!(parse_scenario_tag("Story.X.01").is_err());
    }

    #[test]
    fn parse_scenario_tag_rejects_a_double_at_prefix() {
        assert!(parse_scenario_tag("@@story.x.01").is_err());
    }

    #[test]
    fn append_scenario_block_inserts_exactly_one_blank_line_regardless_of_existing_trailing_newlines() {
        let prov = "  # canon: {}";
        let no_trailing = "Feature: x";
        let one_trailing = "Feature: x\n";
        let blank_trailing = "Feature: x\n\n";
        let expected = "Feature: x\n\n  # canon: {}\n  @a.b.01\n  Scenario: t\n    Given a step\n";
        assert_eq!(append_scenario_block(no_trailing, "a.b.01", "t", prov, ScenarioAxes::default()), expected);
        assert_eq!(append_scenario_block(one_trailing, "a.b.01", "t", prov, ScenarioAxes::default()), expected);
        assert_eq!(append_scenario_block(blank_trailing, "a.b.01", "t", prov, ScenarioAxes::default()), expected);
    }

    #[test]
    fn axis_tags_are_written_subject_lane_case_flow_above_the_id_tag() {
        let axes = ScenarioAxes { subject: Some("cart"), lane: Some("behavior"), case: Some("failure"), flow: Some("add-cart.F1") };
        let out = append_scenario_block("Feature: x\n", "a.b.01", "t", "  # canon: {}", axes);
        assert!(out.ends_with("  @subject:cart\n  @lane:behavior\n  @case:failure\n  @flow:add-cart.F1\n  @a.b.01\n  Scenario: t\n    Given a step\n"), "{out}");
    }

    /// `<n>` configured roots, ids `p0..p<n-1>`, each under its own
    /// `specs-p<i>/` directory — the multi-root config these commands
    /// used to refuse outright, built here rather than read off the
    /// workspace's own `canon.yaml` so the tests pin the RULE, not this
    /// repo's current root count.
    fn repo_with_spec_roots(n: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut yaml = String::from("specs:\n  roots:\n");
        for i in 0..n {
            yaml.push_str(&format!("    - id: p{i}\n      root: specs-p{i}\n"));
        }
        fs::write(dir.path().join("canon.yaml"), yaml).unwrap();
        dir
    }

    fn spec_roots_of(dir: &tempfile::TempDir) -> Vec<SpecRoot> {
        SyncCtx::from_repo(dir.path()).unwrap().spec_roots(None).unwrap()
    }

    fn project(id: &str) -> ProjectId {
        ProjectId::parse(id).unwrap()
    }

    const AT: &str = "2026-01-02T03:04:05Z";

    fn at() -> DateTime<Utc> {
        AT.parse().unwrap()
    }

    #[test]
    fn resolve_spec_root_takes_the_lone_configured_root_when_no_project_is_given() {
        let dir = repo_with_spec_roots(1);
        let roots = spec_roots_of(&dir);
        let picked = resolve_spec_root("canon feature new", &roots, None).unwrap();
        assert_eq!(picked.id.as_str(), "p0");
    }

    #[test]
    fn resolve_spec_root_selects_a_named_project_at_any_root_count() {
        // Rule 2 is NOT conditioned on ambiguity: naming the only root
        // explicitly is as valid as naming one among several, so a
        // script can always pass `--project-id` regardless of how many
        // roots the repo it runs against happens to configure.
        let one = repo_with_spec_roots(1);
        let one_roots = spec_roots_of(&one);
        assert_eq!(resolve_spec_root("canon feature new", &one_roots, Some(&project("p0"))).unwrap().id.as_str(), "p0");

        let three = repo_with_spec_roots(3);
        let three_roots = spec_roots_of(&three);
        assert_eq!(resolve_spec_root("canon feature new", &three_roots, Some(&project("p1"))).unwrap().id.as_str(), "p1");
    }

    #[test]
    fn resolve_spec_root_refuses_a_project_id_no_configured_root_carries_and_lists_the_ones_that_exist() {
        let dir = repo_with_spec_roots(2);
        let roots = spec_roots_of(&dir);
        let err = resolve_spec_root("canon feature new", &roots, Some(&project("nope"))).unwrap_err();
        assert!(err.contains("`--project-id nope`"), "{err}");
        assert!(err.contains("configured ids: p0, p1"), "{err}");
    }

    #[test]
    fn resolve_spec_root_refuses_an_ambiguous_multi_root_config_and_names_the_flag_that_fixes_it() {
        let dir = repo_with_spec_roots(2);
        let roots = spec_roots_of(&dir);
        let err = resolve_spec_root("canon feature new", &roots, None).unwrap_err();
        assert!(err.contains("2 configured `specs.roots[]` entries"), "{err}");
        assert!(err.contains("ids: p0, p1"), "{err}");
        assert!(err.contains("pass `--project-id <id>`"), "{err}");
        assert!(err.contains("`canon feature new`"), "{err}");
    }

    #[test]
    fn feature_new_still_writes_under_the_lone_configured_root_with_no_project_flag() {
        // The single-root repo never has to learn the flag exists.
        let dir = repo_with_spec_roots(1);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", None, &actor(), at(), false), 0);
        assert!(dir.path().join("specs-p0/kind=feature/area=world/hotdeal.feature").is_file());
    }

    #[test]
    fn feature_new_writes_under_the_project_named_root_when_several_are_configured() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", Some(&project("p1")), &actor(), at(), false), 0);
        assert!(dir.path().join("specs-p1/kind=feature/area=world/hotdeal.feature").is_file());
        assert!(!dir.path().join("specs-p0").exists(), "the unnamed root must be left entirely alone");
    }

    #[test]
    fn feature_new_refuses_an_unknown_project_id_with_zero_bytes_written() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", Some(&project("p9")), &actor(), at(), false), 2);
        assert!(!dir.path().join("specs-p0").exists());
        assert!(!dir.path().join("specs-p1").exists());
    }

    #[test]
    fn feature_new_refuses_several_configured_roots_with_no_project_flag() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", None, &actor(), at(), false), 2);
        assert!(!dir.path().join("specs-p0").exists());
        assert!(!dir.path().join("specs-p1").exists());
    }
}
