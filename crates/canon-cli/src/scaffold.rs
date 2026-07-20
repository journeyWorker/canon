//! `canon scenario new <area>.<surface>.<nn> --title <label> [--feature
//! <path>]` + `canon feature new <area>.<surface> --title <label>` (s16
//! `s16-plugin-extensibility`, P5 `corpus-authoring-scaffold` —
//! INDEPENDENT of s16 P1-P4: a `.feature`-authoring convenience, never
//! a plugin concern, tasks.md task group 5): the two scaffold commands
//! that write S11-conformant `.feature` corpus content directly,
//! matching the EXACT tag-then-header shape `canon_fmt::gherkin::scan`
//! already reads (s15 D4) — never a new parser, never a new
//! `RecordKind`, and NO ledger record of any kind (spec.md's own
//! requirement text: "writes NO ledger record of any kind; its only
//! output is the `.feature` file").
//!
//! # Byte shape (grounded in `tests/plugin_sync.rs::write_repo`'s own
//! `feature_text` fixture — the one hand-authored `.feature` sample in
//! this workspace, and `canon_fmt::gherkin::scan`'s own scan rules)
//! ```text
//! Feature: <label>
//!   # canon: {"schema":1,"at":"...","actor":{"agent_id":"..."}}
//!
//!   @<area>.<surface>.<nn>
//!   Scenario: <label>
//!   # canon: {"schema":1,"at":"...","actor":{"agent_id":"..."}}
//!     Given a step
//! ```
//! A 2-space-indented provenance comment immediately follows EVERY
//! header (`gherkin::scan`'s `has_provenance` requires the FIRST
//! non-blank line after a header to parse as one); a blank line
//! separates the `Feature:` block from the first `Scenario:` block,
//! and every subsequent scenario block from its predecessor.
//! [`run_scenario_new`] produces this via [`append_scenario_block`]'s
//! trim-and-rejoin — the SAME helper whether the file is brand new
//! (created by [`run_feature_new`], or minted fresh by
//! [`run_scenario_new`] itself) or already carries scenarios, so a
//! file assembled purely from repeated `canon scenario new` calls is
//! byte-identical in shape to the hand-authored fixture above.
//!
//! # Deterministic provenance, never a bare `Utc::now()`
//! [`run_scenario_new`]/[`run_feature_new`] take `at: DateTime<Utc>` as
//! an EXPLICIT parameter — `main.rs`'s dispatch match arms are the ONE
//! place `Utc::now()` is ever called for these commands (mirroring
//! `canon context`'s own "resolve once" discipline), so a file this
//! module writes in ONE invocation never straddles two different
//! timestamps even when it stamps two provenance comments at once (a
//! brand-new `.feature` file's `Feature:` + first `Scenario:` header,
//! both under `run_scenario_new`), and a test driving these library
//! functions directly gets fully reproducible bytes. The actor is
//! [`scaffold_actor`] — a FIXED, unattributed agent id, mirroring
//! `canon inventory sync`'s own
//! `Actor::new_unattributed("canon-inventory-sync")`
//! (`crate::inventory::run_sync_with_ctx`): never an agent-authored
//! attestation, this is deterministic tooling output.
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
use canon_model::{Actor, ScenarioId};
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

/// Both commands' fixed, unattributed provenance actor (module doc,
/// "Deterministic provenance").
fn scaffold_actor() -> Actor {
    Actor::new_unattributed("canon-scaffold")
}

/// The 2-space-indented `# canon: {...}` comment line every
/// `Feature:`/`Scenario:` header in this module's output carries
/// (module doc's byte shape) — built fresh per call so a caller
/// stamping two headers in one write (a brand-new file's `Feature:` +
/// first `Scenario:`) can still reuse the SAME `at`/actor for both
/// without a second [`FeatureProvenance`] construction drifting from
/// the first.
fn provenance_line(at: DateTime<Utc>) -> String {
    let prov = FeatureProvenance::new(1, at, scaffold_actor());
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

/// Append one `@<tag>` scenario block to `existing` (the target
/// `.feature` file's current content, `""` when the file doesn't exist
/// yet) — trims `existing`'s trailing newline(s), then rejoins with
/// exactly one blank line before the new block, so the separator is
/// correct whether `existing` already ends with a trailing newline, a
/// trailing blank line, or neither (module doc's byte shape; matches
/// `tests/plugin_sync.rs::write_repo`'s hand-authored fixture's own
/// inter-scenario blank line). `existing` MUST already carry a
/// `Feature:` header (the caller synthesizes one first when the file
/// is new — see [`run_scenario_new`]).
fn append_scenario_block(existing: &str, tag: &str, title: &str, prov_line: &str) -> String {
    let block = format!("  @{tag}\n  Scenario: {title}\n{prov_line}\n    Given a step\n");
    let trimmed = existing.trim_end_matches('\n');
    let mut out = String::with_capacity(trimmed.len() + block.len() + 2);
    out.push_str(trimmed);
    out.push_str("\n\n");
    out.push_str(&block);
    out
}

/// The `features/kind=feature/area=<area>/<surface>.feature` layout
/// `canon_model::family::FamilyKind::Feature::layout_descriptor`
/// declares (design D1) — the ONE authoritative constructor for this
/// shape. [`run_feature_new`] and [`run_scenario_new`]'s tag-derived
/// default both call this, never a second, independently hand-typed
/// copy of the same join (the exact bug class s19
/// `derived-validated-scenario-feature` closes).
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

/// `canon scenario new <tag> --title <label> [--feature <path>]` (task
/// 5.1; s19 `derived-validated-scenario-feature` makes `--feature`
/// optional, design D1-D3). Returns the process exit code: `0` on a
/// successful append/create, `2` on a refused invocation — a
/// `specs.roots[]` config fault, an ambiguous multi-root config when
/// `--feature` is omitted (design D2), an explicit `--feature` path
/// resolving outside every configured root (design D3), or `tag`
/// already existing somewhere in the target feature corpus — with
/// ZERO bytes written either way, mirroring `canon review add`'s own
/// refusal-exits-`2`/nothing-written convention
/// (`crate::review::run_add`).
pub fn run_scenario_new(repo: &Path, tag: &ScenarioId, title: &str, feature: Option<&Path>, at: DateTime<Utc>) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let ctx = SyncCtx::from_repo(&repo_root);
    let roots = match ctx.spec_roots(None) {
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
            // own ambiguity refusal — never guess which root among
            // several a derived file belongs under.
            let root = match roots.as_slice() {
                [one] => one,
                many => {
                    eprintln!(
                        "canon scenario new: refused — {} configured `specs.roots[]` entries; omitting `--feature` requires exactly one configured root (pass `--feature <path>` explicitly to disambiguate which root `{}` belongs under)",
                        many.len(),
                        tag.as_str()
                    );
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

    let prov_line = provenance_line(at);
    let content = if existing.trim().is_empty() {
        // New file: emit `Feature:` + provenance first (task 5.1). No
        // `--feature-title` flag exists on this command — `<area>
        // <surface>` space-joined mirrors the one hand-authored
        // fixture in this workspace (`tests/plugin_sync.rs`'s
        // `"Feature: idolive hub"` for tags under `idolive.hub`).
        format!("Feature: {} {}\n{prov_line}\n", tag.area(), tag.surface())
    } else {
        existing
    };
    let out = append_scenario_block(&content, tag.as_str(), title, &prov_line);

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
    println!("canon scenario new: wrote `@{}` to {}", tag.as_str(), feature_path.display());
    0
}

/// `canon feature new <area>.<surface> --title <label>` (task 5.2).
/// Returns the process exit code: `0` on a fresh file written, `2` on
/// a refused invocation (a `specs.roots[]` config fault, an ambiguous
/// multi-root config — this command has no `--spec-root` override,
/// unlike `canon inventory sync`/`canon plugin sync`, to disambiguate
/// which root to scaffold under — or the target file already
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
pub fn run_feature_new(repo: &Path, area_surface: &AreaSurface, title: &str, at: DateTime<Utc>) -> i32 {
    let repo_root = resolve_repo_root(repo);
    let ctx = SyncCtx::from_repo(&repo_root);
    let roots = match ctx.spec_roots(None) {
        Ok(roots) => roots,
        Err(e) => {
            eprintln!("canon feature new: {e}");
            return 2;
        }
    };
    let root = match roots.as_slice() {
        [one] => one,
        many => {
            eprintln!(
                "canon feature new: refused — {} configured `specs.roots[]` entries; this command has no `--spec-root` override to disambiguate which root `{}.{}` belongs under",
                many.len(),
                area_surface.area,
                area_surface.surface
            );
            return 2;
        }
    };

    let feature_path = resolve_feature_path(root, &area_surface.area, &area_surface.surface);

    if let Some(parent) = feature_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("canon feature new: failed to create `{}`: {e}", parent.display());
            return 2;
        }
    }

    let content = format!("Feature: {title}\n{}\n", provenance_line(at));
    match fs::OpenOptions::new().write(true).create_new(true).open(&feature_path) {
        Ok(mut file) => match file.write_all(content.as_bytes()) {
            Ok(()) => {
                println!("canon feature new: wrote {}", feature_path.display());
                println!(
                    "canon feature new: next: `canon scenario new {}.{}.01 --title '<label>' [--feature <path>]` to make it fmt-clean",
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
        // Only one leading `@` is stripped -- `@@story.x.01` strips to
        // `@story.x.01`, which `ScenarioId::parse`'s `@`-free grammar
        // still refuses (design R3).
        assert!(parse_scenario_tag("@@story.x.01").is_err());
    }

    #[test]
    fn append_scenario_block_inserts_exactly_one_blank_line_regardless_of_existing_trailing_newlines() {
        let prov = "  # canon: {}";
        let no_trailing = "Feature: x";
        let one_trailing = "Feature: x\n";
        let blank_trailing = "Feature: x\n\n";
        let expected = "Feature: x\n\n  @a.b.01\n  Scenario: t\n  # canon: {}\n    Given a step\n";
        assert_eq!(append_scenario_block(no_trailing, "a.b.01", "t", prov), expected);
        assert_eq!(append_scenario_block(one_trailing, "a.b.01", "t", prov), expected);
        assert_eq!(append_scenario_block(blank_trailing, "a.b.01", "t", prov), expected);
    }
}
