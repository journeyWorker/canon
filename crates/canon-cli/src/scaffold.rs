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

use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use canon_model::family::feature::FeatureProvenance;
use canon_model::{Actor, ProjectId, ScenarioId};
use chrono::{DateTime, Utc};

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
/// `features/` corpus (`gherkin::scan`'s own `scenario_ids` — every
/// tag found, whether or not it paired with a following `Scenario:`
/// header, unlike `crate::inventory::scan_feature_corpus`'s paired-only
/// `scenarios`) — used ONLY for duplicate-tag existence checking here
/// (never a second, title/digest-resolving copy of that scan, which
/// duplicate detection doesn't need).
fn corpus_tags(root: &Path) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    for path in canon_fmt::util::walk_files(root, "features") {
        if path.extension().and_then(|e| e.to_str()) != Some("feature") {
            continue;
        }
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
}

/// Append one scenario block. Provenance leads the block, followed by
/// the optional subject/lane/case tags in that order, then the id tag
/// and header.
fn append_scenario_block(existing: &str, tag: &str, title: &str, prov_line: &str, axes: ScenarioAxes<'_>) -> String {
    let mut block = String::new();
    block.push_str(prov_line);
    block.push('\n');
    for (axis, value) in [("subject", axes.subject), ("lane", axes.lane), ("case", axes.case)] {
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
pub fn resolve_feature_path(root: &SpecRoot, area: &str, surface: &str) -> PathBuf {
    root.root.join("features").join("kind=feature").join(format!("area={area}")).join(format!("{surface}.feature"))
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
            format!("refused — `--project {}` names no configured `specs.roots[]` entry (configured ids: {})", id.as_str(), configured())
        }),
        (None, [one]) => Ok(one),
        (None, many) => Err(format!(
            "refused — {} configured `specs.roots[]` entries (ids: {}); pass `--project <id>` to select which one `{command}` writes under",
            many.len(),
            configured()
        )),
    }
}

/// `canon scenario new <tag> --title <label> [--feature <path>]
/// [--project <id>]` (task 5.1; s19 `derived-validated-scenario-feature`
/// makes `--feature` optional, design D1-D3). Returns the process exit
/// code: `0` on a successful append/create, `2` on a refused
/// invocation — a `specs.roots[]` config fault, a spec root
/// [`resolve_spec_root`] cannot pin down when `--feature` is omitted
/// (design D2), an explicit `--feature` path resolving outside every
/// configured root (design D3), or `tag` already existing somewhere in
/// the target feature corpus — with ZERO bytes written either way,
/// mirroring `canon review add`'s own
/// refusal-exits-`2`/nothing-written convention
/// (`crate::review::run_add`).
///
/// `project` is `--project <id>`: which configured `specs.roots[]`
/// entry the DERIVED path lands under, selected by the root's `id`
/// exactly as [`run_feature_new`] selects it ([`resolve_spec_root`]).
pub fn run_scenario_new(
    repo: &Path,
    tag: &ScenarioId,
    title: &str,
    feature: Option<&Path>,
    project: Option<&ProjectId>,
    axes: ScenarioAxes<'_>,
    actor: &Actor,
    at: DateTime<Utc>,
) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let roots = match SyncCtx::from_repo(&repo_root).and_then(|ctx| ctx.spec_roots(None)) {
        Ok(roots) => roots,
        Err(e) => {
            eprintln!("canon scenario new: {e}");
            return 2;
        }
    };

    // Resolve the target `.feature` path FIRST — spec.md's own
    // ordering requirement (design D3): root-membership validation for
    // an explicit `--feature` runs BEFORE the duplicate-tag/target-file
    // checks below.
    let feature_path: PathBuf = match feature {
        None => {
            // D2: the tag-derived default mirrors `run_feature_new`'s
            // own root selection — the SAME [`resolve_spec_root`], so
            // `--project` means one thing across both commands and an
            // unresolvable root is never guessed at.
            let root = match resolve_spec_root("canon scenario new", &roots, project) {
                Ok(root) => root,
                Err(msg) => {
                    eprintln!("canon scenario new: {msg}");
                    return 2;
                }
            };
            resolve_feature_path(root, tag.area(), tag.surface())
        }
        Some(feature) => {
            let resolved: PathBuf = if feature.is_absolute() { feature.to_path_buf() } else { repo_root.join(feature) };
            if !path_under_any_root(&resolved, &roots) {
                eprintln!(
                    "canon scenario new: refused — `{}` does not resolve under any configured `specs.roots[]` entry ({}); never a silent orphan write outside the validated corpus",
                    resolved.display(),
                    roots.iter().map(|r| r.root.display().to_string()).collect::<Vec<_>>().join(", ")
                );
                return 2;
            }
            resolved
        }
    };

    for root in &roots {
        if corpus_tags(&root.root).contains(tag.as_str()) {
            eprintln!(
                "canon scenario new: refused — `@{}` already exists under spec root `{}` ({}); never a silent duplicate",
                tag.as_str(),
                root.id.as_str(),
                root.root.display()
            );
            return 2;
        }
    }

    let existing = if feature_path.exists() {
        match fs::read_to_string(&feature_path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("canon scenario new: failed to read `{}`: {e}", feature_path.display());
                return 2;
            }
        }
    } else {
        String::new()
    };
    // Belt-and-suspenders: guard the target file directly too, even
    // when it sits outside every configured spec root — spec.md's own
    // literal duplicate-tag scenario is "runs a second time against a
    // `.feature` file that already carries" the tag, the SAME target
    // file, regardless of corpus-root config.
    if canon_fmt::gherkin::scan(&existing).scenario_ids.iter().any(|t| t == tag.as_str()) {
        eprintln!("canon scenario new: refused — `@{}` already exists in `{}`; never a silent duplicate", tag.as_str(), feature_path.display());
        return 2;
    }

    let prov_line = provenance_line(at, actor);
    let content = if existing.trim().is_empty() {
        // New file: emit `Feature:` + provenance first (task 5.1). No
        // `--feature-title` flag exists on this command — `<area>
        // <surface>` space-joined mirrors the one hand-authored fixture
        // in this workspace.
        format!("Feature: {} {}\n{prov_line}\n", tag.area(), tag.surface())
    } else {
        existing
    };
    let out = append_scenario_block(&content, tag.as_str(), title, &prov_line, axes);

    if let Some(parent) = feature_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("canon scenario new: failed to create `{}`: {e}", parent.display());
            return 2;
        }
    }
    if let Err(e) = fs::write(&feature_path, &out) {
        eprintln!("canon scenario new: failed to write `{}`: {e}", feature_path.display());
        return 2;
    }
    println!("canon scenario new: wrote `@{}` to {} — {}", tag.as_str(), feature_path.display(), crate::write_mode::DIRECT);
    0
}

/// `canon feature new <area>.<surface> --title <label> [--project
/// <id>]` (task 5.2). Returns the process exit code: `0` on a fresh
/// file written, `2` on a refused invocation (a `specs.roots[]` config
/// fault, a spec root [`resolve_spec_root`] cannot pin down — a
/// multi-root config with no `--project <id>`, or a `--project` naming
/// no configured entry — or the target file already
/// existing). Uses `create_new` (atomic create-fails-if-exists), never
/// a check-then-write race, so the existing file's bytes are UNTOUCHED
/// in every refusal case. The path is derived via
/// [`resolve_feature_path`] (s19 design D1) — the SAME
/// `features/kind=feature/area=<area>/<surface>.feature` layout
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
/// `project` is `--project <id>`, naming which configured
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
) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let roots = match SyncCtx::from_repo(&repo_root).and_then(|ctx| ctx.spec_roots(None)) {
        Ok(roots) => roots,
        Err(e) => {
            eprintln!("canon feature new: {e}");
            return 2;
        }
    };
    let root = match resolve_spec_root("canon feature new", &roots, project) {
        Ok(root) => root,
        Err(msg) => {
            eprintln!("canon feature new: {msg}");
            return 2;
        }
    };

    let feature_path = resolve_feature_path(root, &area_surface.area, &area_surface.surface);
    // The next-step hint below must be an invocation that actually
    // RUNS (design D4 calls it "the exact invocation that closes that
    // gap"), so it carries `--project` exactly when a bare `canon
    // scenario new` would refuse for ambiguity — echoing the root just
    // resolved rather than making the operator re-derive it.
    let project_hint = if roots.len() > 1 { format!(" --project {}", root.id.as_str()) } else { String::new() };

    if let Some(parent) = feature_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("canon feature new: failed to create `{}`: {e}", parent.display());
            return 2;
        }
    }

    let content = format!("Feature: {title}\n{}\n", provenance_line(at, actor));
    match fs::OpenOptions::new().write(true).create_new(true).open(&feature_path) {
        Ok(mut file) => match file.write_all(content.as_bytes()) {
            Ok(()) => {
                println!("canon feature new: wrote {} — {}", feature_path.display(), crate::write_mode::DIRECT);
                println!(
                    "canon feature new: next: `canon scenario new {}.{}.01 --title '<label>'{project_hint} [--feature <path>]` to make it fmt-clean",
                    area_surface.area, area_surface.surface
                );
                0
            }
            Err(e) => {
                eprintln!("canon feature new: failed to write `{}`: {e}", feature_path.display());
                2
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("canon feature new: refused — `{}` already exists; never overwriting an existing feature file", feature_path.display());
            2
        }
        Err(e) => {
            eprintln!("canon feature new: failed to create `{}`: {e}", feature_path.display());
            2
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
    fn axis_tags_are_written_subject_lane_case_above_the_id_tag() {
        let axes = ScenarioAxes { subject: Some("cart"), lane: Some("behavior"), case: Some("failure") };
        let out = append_scenario_block("Feature: x\n", "a.b.01", "t", "  # canon: {}", axes);
        assert!(out.ends_with("  @subject:cart\n  @lane:behavior\n  @case:failure\n  @a.b.01\n  Scenario: t\n    Given a step\n"), "{out}");
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
        // script can always pass `--project` regardless of how many
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
        assert!(err.contains("`--project nope`"), "{err}");
        assert!(err.contains("configured ids: p0, p1"), "{err}");
    }

    #[test]
    fn resolve_spec_root_refuses_an_ambiguous_multi_root_config_and_names_the_flag_that_fixes_it() {
        let dir = repo_with_spec_roots(2);
        let roots = spec_roots_of(&dir);
        let err = resolve_spec_root("canon feature new", &roots, None).unwrap_err();
        assert!(err.contains("2 configured `specs.roots[]` entries"), "{err}");
        assert!(err.contains("ids: p0, p1"), "{err}");
        assert!(err.contains("pass `--project <id>`"), "{err}");
        assert!(err.contains("`canon feature new`"), "{err}");
    }

    #[test]
    fn feature_new_still_writes_under_the_lone_configured_root_with_no_project_flag() {
        // The single-root repo never has to learn the flag exists.
        let dir = repo_with_spec_roots(1);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", None, &actor(), at()), 0);
        assert!(dir.path().join("specs-p0/features/kind=feature/area=world/hotdeal.feature").is_file());
    }

    #[test]
    fn feature_new_writes_under_the_project_named_root_when_several_are_configured() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", Some(&project("p1")), &actor(), at()), 0);
        assert!(dir.path().join("specs-p1/features/kind=feature/area=world/hotdeal.feature").is_file());
        assert!(!dir.path().join("specs-p0").exists(), "the unnamed root must be left entirely alone");
    }

    #[test]
    fn feature_new_refuses_an_unknown_project_id_with_zero_bytes_written() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", Some(&project("p9")), &actor(), at()), 2);
        assert!(!dir.path().join("specs-p0").exists());
        assert!(!dir.path().join("specs-p1").exists());
    }

    #[test]
    fn feature_new_refuses_several_configured_roots_with_no_project_flag() {
        let dir = repo_with_spec_roots(2);
        let surface = AreaSurface::parse("world.hotdeal").unwrap();
        assert_eq!(run_feature_new(dir.path(), &surface, "Hot deals", None, &actor(), at()), 2);
        assert!(!dir.path().join("specs-p0").exists());
        assert!(!dir.path().join("specs-p1").exists());
    }
}
