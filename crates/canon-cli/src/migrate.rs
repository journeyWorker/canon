//! `canon migrate [--dry-run] [--repo <dir>]` (0.17 D6): moves a repo set
//! up by canon 0.16 or earlier onto the 0.17 layout, in one idempotent
//! command. It prints the plan, then applies it; `--dry-run` stops after
//! the plan. A second run finds nothing to move.
//!
//! # What it moves and writes
//! - **Spec roots.** A root still holding `<root>/features/kind=feature/`
//!   has it moved to `<root>/kind=feature/` (`git mv` inside a git work
//!   tree, a rename otherwise). A root at the repo-level `specs` (the
//!   0.16 default, also what an absent `specs:` key or an entry without
//!   `root:` meant then) moves to `.canon/specs`, from either layout, and
//!   `canon.yaml` records where it was as `specs.roots[].former_roots`, so
//!   the legacy-pin lookup ([`canon_gate::legacy_pins`]) still finds a
//!   scenario at a commit from before the move. Any other root stays where
//!   it is and only drops its `features/` level; no former root is needed
//!   there, because the lookup lists every `.feature` file under the
//!   root's directory at the pin.
//! - **Changes `canon change new` wrote under openspec** move to
//!   `.canon/changes/<slug>/`, `proposal.md` renamed `brief.md` and
//!   `tasks.md` alongside ([`canon_written`]). The native source then
//!   re-imports each under the same change id, so its tasks
//!   (`<slug>#<n>`) and its adoption keep joining. Every other openspec
//!   change stays where it is and is named. The `{ dialect: openspec,
//!   root: . }` plans source `canon init` 0.16 wrote is removed when no
//!   change is left under it, with the `openspec/changes/.gitkeep` init
//!   created for it.
//! - **Approvals.** An external change whose `proposal.md` carries an
//!   `Approved:` line (0.16 recorded it there) gets a canon pointer brief,
//!   `.canon/changes/<change>/brief.md`, holding `Source:` and that line.
//!   The external file is not edited; the leftover line is named.
//! - **Skills.** `skills.providers` is written from the install lock when
//!   `canon.yaml` has no `skills:` key.
//! - **Gitignore.** The canon `.gitignore` lines the repo lacks are added
//!   ([`crate::init::scaffold_gitignore`]).
//! - **Index.** The scenario index is rebuilt last with a full
//!   `canon inventory sync`, which rewrites any generation a reader would
//!   not (a pre-0.15 one without `scenario_digest`).
//!
//! # canon.yaml stays byte-identical elsewhere
//! Every edit is textual, the way `canon skills install` edits `skills:`:
//! comments and line endings are kept, and the result is refused unless it
//! parses to exactly the intended change with every other key unchanged.
//!
//! # Refusals (exit `2`, nothing written)
//! A `canon.yaml` canon cannot read; uncommitted changes (untracked files
//! included) under a path it moves; a destination that already exists; a
//! `canon.yaml` edit it cannot make safely.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use canon_ingest::markdown::field;
use canon_ingest::plan_adapters::openspec::proposal_path;
use canon_ingest::{find_plan_adapter, PlanSourceHandle};
use canon_model::{paths, ChangeId, RecordKind};
use canon_store::tier::TierQuery;
use serde_yaml::Value;

use crate::context::resolve_repo_root;
use crate::skills::{self, Provider, ProviderOrigin};

const EXIT_REFUSED: i32 = 2;

/// The spec root 0.16 defaulted to, and the one that moves into `.canon/`.
const OLD_DEFAULT_ROOT: &str = "specs";

/// `proposal.md` as `canon change new` wrote it from 0.14 through 0.16,
/// up to and including the `## What Changes` comment.
fn scaffold_proposal(title: &str) -> String {
    format!(
        "# {title}\n\n## Why\n\n{title}\n\n## What Changes\n\n<!-- What this change adds or changes, and the scenarios it covers (`@subject` tag, failure paths included). -->\n"
    )
}

/// `tasks.md` as `canon change new` wrote it from 0.14 through 0.16.
fn scaffold_tasks(title: &str) -> String {
    format!(
        "# Tasks: {title}\n\n<!-- One checkbox row per task, numbered: `- [ ] 1 Write the failure-path scenarios`. Each row imports as task `<slug>#<n>`; flip it with `canon gate task` once evidence exists, never by hand. -->\n"
    )
}

/// One spec root to move.
#[derive(Debug, Clone)]
struct RootMove {
    id: String,
    /// The directory moved: `<root>/features/kind=feature` or, for a root
    /// at `specs` already on the new layout, `specs/kind=feature`.
    from: PathBuf,
    /// `<new root>/kind=feature`.
    to: PathBuf,
    /// The `canon.yaml` edit it needs, if any.
    config: Option<RootConfigEdit>,
    /// Directories left behind that are removed when empty, innermost
    /// first (`<root>/features`, then `specs` when the root moved).
    emptied: Vec<PathBuf>,
}

/// How [`RootMove`] changes `specs.roots`.
#[derive(Debug, Clone)]
enum RootConfigEdit {
    /// Entry `index` names `root:` explicitly: set it to `new_root` and
    /// add `former` to its `former_roots`.
    Entry { index: usize, new_root: String, former: String, explicit_root: bool },
    /// No `specs:` key: append one root, id `root`, at `new_root`.
    Section { new_root: String, former: String },
}

/// One openspec change `canon change new` wrote, to move.
#[derive(Debug, Clone)]
struct ChangeMove {
    change: ChangeId,
    from: PathBuf,
    to: PathBuf,
}

/// One canon pointer brief to write for an external change.
#[derive(Debug, Clone)]
struct PointerBrief {
    path: PathBuf,
    source: String,
    approval: String,
}

/// Everything one run would do ([`plan`]).
#[derive(Debug, Default)]
struct Plan {
    roots: Vec<RootMove>,
    changes: Vec<ChangeMove>,
    briefs: Vec<PointerBrief>,
    /// The `{ dialect: openspec, root: . }` source entry to remove, by
    /// its index in `plans.sources`.
    drop_source: Option<usize>,
    /// `openspec/changes/.gitkeep`, removed with that source.
    gitkeep: Option<PathBuf>,
    providers: Option<Vec<Provider>>,
    gitignore: Vec<&'static str>,
    /// What stays, said once each: changes left in place and why.
    left: Vec<String>,
    /// Paths it moves, checked for uncommitted changes.
    moved_paths: Vec<PathBuf>,
}

impl Plan {
    fn changes_files(&self) -> bool {
        !self.roots.is_empty() || !self.changes.is_empty() || !self.briefs.is_empty() || self.drop_source.is_some() || self.providers.is_some() || !self.gitignore.is_empty()
    }
}

/// `canon migrate` (module doc). Exit `0` migrated or nothing to do, `1`
/// migrated but the index rebuild found the corpus unclean, `2` refused
/// or failed.
pub fn run(repo: &Path, dry_run: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let canon_yaml = repo.join("canon.yaml");
    let Ok(text) = fs::read_to_string(&canon_yaml) else {
        eprintln!("canon migrate: refused — `{}` does not exist; run it in a repo canon manages (`canon init` sets one up)", canon_yaml.display());
        return EXIT_REFUSED;
    };
    let git = in_git_work_tree(&repo);
    let plan = match plan(&repo, &text) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("canon migrate: refused — {e}");
            return EXIT_REFUSED;
        }
    };
    let config = match render_config(&text, &plan, plan.roots.len(), plan.drop_source.is_some()) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("canon migrate: refused — {e}");
            return EXIT_REFUSED;
        }
    };
    if git {
        if let Some(dirty) = uncommitted(&repo, &plan.moved_paths) {
            eprintln!("canon migrate: refused — uncommitted changes under the paths it moves; commit or stash them, then run it again:\n{dirty}");
            return EXIT_REFUSED;
        }
    }

    print_plan(&repo, &plan, config != text);
    if dry_run {
        println!("canon migrate: --dry-run, nothing written");
        return 0;
    }

    let applied = apply(&repo, &plan, git);
    let written = render_config(&text, &plan, applied.roots, applied.changes_done)
        .map_err(|e| format!("could not rewrite canon.yaml: {e}"))
        .and_then(|config| if config == text { Ok(()) } else { fs::write(&canon_yaml, config).map_err(|e| format!("failed to write canon.yaml: {e}")) });
    if let Err(e) = written {
        eprintln!("canon migrate: {e}");
        return EXIT_REFUSED;
    }
    if config != text && applied.error.is_none() {
        println!("canon migrate: rewrote canon.yaml");
    }
    if let Some(e) = applied.error {
        eprintln!("canon migrate: stopped — {e}; what moved is recorded in canon.yaml, so running `canon migrate` again finishes the rest");
        return EXIT_REFUSED;
    }

    for change in &plan.changes {
        match crate::plans::refresh_change(&repo, &change.change) {
            Ok(_) => println!("canon migrate: re-imported change `{}` from {} (same change id; its tasks and adoption keep joining)", change.change.as_str(), rel(&repo, &change.to)),
            Err(e) => {
                eprintln!("canon migrate: moved change `{}` but could not re-import it: {e}; run `canon ingest plans`", change.change.as_str());
                return EXIT_REFUSED;
            }
        }
    }
    match crate::init::scaffold_gitignore(&repo) {
        Ok(added) if !added.is_empty() => println!("canon migrate: added to .gitignore: {}", added.join(", ")),
        Ok(_) => {}
        Err(e) => {
            eprintln!("canon migrate: failed to update .gitignore: {e}");
            return EXIT_REFUSED;
        }
    }
    if git {
        let tracked = tracked_index_files(&repo);
        if tracked > 0 {
            println!(
                "canon migrate: note — {tracked} scenario index file(s) under {} are tracked in git; the index is derived and now ignored, so `git rm -r --cached {}` stops tracking them",
                paths::SCENARIO_INDEX_GITIGNORE,
                paths::SCENARIO_INDEX_GITIGNORE
            );
        }
    }
    rebuild_index(&repo)
}

/// The plan for `repo` with `canon.yaml` `text`, or why it is refused.
fn plan(repo: &Path, text: &str) -> Result<Plan, String> {
    let doc: Value = serde_yaml::from_str(text).map_err(|e| format!("canon.yaml is not valid YAML: {e}"))?;
    let mut plan = Plan::default();
    plan_roots(repo, &doc, &mut plan)?;
    plan_changes(repo, &doc, &mut plan)?;
    let selection = skills::select_providers(repo, None).map_err(|e| e.to_string())?;
    if selection.origin == ProviderOrigin::Lock {
        plan.providers = Some(selection.providers);
    }
    plan.gitignore = crate::init::missing_gitignore_lines(repo).into_iter().map(|(_, line)| line).collect();
    Ok(plan)
}

/// `path` relative to `repo`, `/`-separated, without `./` or a trailing
/// `/`; `None` when it leaves the repo.
fn normalized(repo: &Path, path: &Path) -> Option<String> {
    let path = if path.is_absolute() { path.strip_prefix(repo).ok()? } else { path };
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// [`RootMove`]s for every configured spec root (module doc).
fn plan_roots(repo: &Path, doc: &Value, plan: &mut Plan) -> Result<(), String> {
    // `(entry index, id, root as written)`; `None` index: no `specs:` key.
    let mut entries: Vec<(Option<usize>, String, Option<String>)> = Vec::new();
    match doc.get("specs") {
        None => entries.push((None, "root".to_string(), None)),
        Some(specs) => {
            let roots = specs.get("roots").and_then(Value::as_sequence).ok_or("canon.yaml `specs:` has no `roots:` list")?;
            for (index, entry) in roots.iter().enumerate() {
                let id = entry.get("id").and_then(Value::as_str).ok_or("a canon.yaml `specs.roots[]` entry has no `id`")?;
                let root = entry.get("root").and_then(Value::as_str).map(str::to_string);
                entries.push((Some(index), id.to_string(), root));
            }
        }
    }
    for (index, id, written) in entries {
        // What 0.16 resolved this entry to.
        let old = written.clone().unwrap_or_else(|| OLD_DEFAULT_ROOT.to_string());
        let old_path = repo.join(&old);
        let Some(old_rel) = normalized(repo, &old_path) else { continue };
        let moves_home = old_rel == OLD_DEFAULT_ROOT;
        let new_rel = if moves_home { paths::SPECS_DIR.to_string() } else { old_rel.clone() };
        let new_path = repo.join(&new_rel);
        let nested = old_path.join("features").join("kind=feature");
        let from = if nested.is_dir() {
            nested
        } else if moves_home && old_path.join("kind=feature").is_dir() {
            old_path.join("kind=feature")
        } else {
            continue;
        };
        let to = new_path.join("kind=feature");
        if to.exists() {
            return Err(format!(
                "spec root `{id}`: {} would move to {}, which already exists; move what is there out of the way first",
                rel(repo, &from),
                rel(repo, &to)
            ));
        }
        let config = if !moves_home {
            None
        } else {
            match (index, written.is_some()) {
                (Some(index), explicit_root) => Some(RootConfigEdit::Entry { index, new_root: new_rel.clone(), former: old_rel.clone(), explicit_root }),
                (None, _) => Some(RootConfigEdit::Section { new_root: new_rel.clone(), former: old_rel.clone() }),
            }
        };
        let mut emptied = Vec::new();
        if from.parent() != Some(old_path.as_path()) {
            emptied.push(old_path.join("features"));
        }
        if moves_home {
            emptied.push(old_path.clone());
        }
        plan.moved_paths.push(from.clone());
        plan.roots.push(RootMove { id, from, to, config, emptied });
    }
    Ok(())
}

/// The plan sources `canon.yaml` lists, as `(dialect, root)` written.
fn plan_sources(doc: &Value) -> Vec<(String, String)> {
    doc.get("plans")
        .and_then(|plans| plans.get("sources"))
        .and_then(Value::as_sequence)
        .map(|sources| {
            sources
                .iter()
                .map(|source| {
                    let text = |key: &str| source.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
                    (text("dialect"), text("root"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// [`ChangeMove`]s, [`PointerBrief`]s and the openspec source to drop
/// (module doc).
fn plan_changes(repo: &Path, doc: &Value, plan: &mut Plan) -> Result<(), String> {
    let sources = crate::plans::load_plan_sources_for_gate(repo).map_err(|e| e.to_string())?;
    let Some(entry) = find_plan_adapter("openspec") else { return Ok(()) };
    let first_versions = first_change_versions(repo);
    let canon_changes = repo.join(paths::CHANGES_DIR);
    // Per openspec source root, repo-relative: how many changes stay under it.
    let mut staying: BTreeMap<String, usize> = BTreeMap::new();
    let mut external = Vec::new();
    for source in sources.iter().filter(|source| source.dialect() == "openspec") {
        let root = source.root().to_path_buf();
        let staying = staying.entry(normalized(repo, &root).unwrap_or_default()).or_default();
        let outcome = entry.adapter.parse(&PlanSourceHandle::Path(root.clone()));
        for change in outcome.changes {
            let Some(proposal) = proposal_path(&root, &change.change_id) else { continue };
            let Some(dir) = proposal.parent().map(Path::to_path_buf) else { continue };
            let archived = dir.parent().and_then(Path::file_name).is_some_and(|name| name == "archive");
            let id = change.change_id.as_str().to_string();
            match canon_written(&dir, first_versions.get(&id).copied()) {
                Ok(()) => {
                    let to = if archived { canon_changes.join("archive").join(&id) } else { canon_changes.join(&id) };
                    if to.exists() {
                        return Err(format!("change `{id}`: {} would move to {}, which already exists", rel(repo, &dir), rel(repo, &to)));
                    }
                    plan.moved_paths.push(dir.clone());
                    plan.changes.push(ChangeMove { change: change.change_id.clone(), from: dir, to });
                }
                Err(why) => {
                    *staying += 1;
                    if let Some(why) = why {
                        plan.left.push(format!("{} stays where it is: {why}", rel(repo, &dir)));
                    } else {
                        external.push(id.clone());
                    }
                    let text = fs::read_to_string(&proposal).unwrap_or_default();
                    if let Some(approval) = field(&text, "Approved:") {
                        let brief = canon_changes.join(&id).join("brief.md");
                        let recorded = fs::read_to_string(&brief).ok().is_some_and(|brief| field(&brief, "Approved:").is_some());
                        if recorded {
                            continue;
                        }
                        if canon_changes.join(&id).exists() {
                            return Err(format!(
                                "change `{id}`: its approval in {} would be copied to {}, but {} already exists without an `Approved:` line; record the approval there by hand",
                                rel(repo, &proposal),
                                rel(repo, &brief),
                                rel(repo, &canon_changes.join(&id))
                            ));
                        }
                        plan.briefs.push(PointerBrief { path: brief, source: rel(repo, &proposal), approval: approval.to_string() });
                    }
                }
            }
        }
    }
    if !external.is_empty() {
        plan.left.push(format!("{} openspec change(s) not written by `canon change new` stay where they are, read as an external plan: {}", external.len(), external.join(", ")));
    }
    // The 0.16 init's `{ dialect: openspec, root: . }`, once empty.
    for (index, (dialect, root)) in plan_sources(doc).into_iter().enumerate() {
        if dialect != "openspec" || normalized(repo, Path::new(&root)).as_deref() != Some("") {
            continue;
        }
        if staying.get("").copied().unwrap_or_default() == 0 {
            plan.drop_source = Some(index);
            let gitkeep = repo.join("openspec/changes/.gitkeep");
            let only_gitkeep = fs::read_dir(repo.join("openspec/changes"))
                .map(|entries| entries.filter_map(Result::ok).all(|entry| entry.file_name() == ".gitkeep" || plan.changes.iter().any(|c| c.from == entry.path())))
                .unwrap_or(false);
            if gitkeep.is_file() && only_gitkeep {
                plan.gitkeep = Some(gitkeep);
            }
        }
        break;
    }
    Ok(())
}

/// The `at` and `subject_id` presence of the first stored version of
/// every change, by change id. A change tier this run cannot read gives
/// no versions, so no change counts as canon-written.
fn first_change_versions(repo: &Path) -> BTreeMap<String, bool> {
    let Ok(registry) = crate::subject::registry_for(&repo.join("canon.yaml"), &[RecordKind::Change]) else { return BTreeMap::new() };
    let Ok(result) = registry.query(&TierQuery::kind(RecordKind::Change)) else { return BTreeMap::new() };
    let mut first: BTreeMap<String, (chrono::DateTime<chrono::Utc>, bool)> = BTreeMap::new();
    for raw in result.records {
        let Some(id) = raw.0.get("change_id").and_then(serde_json::Value::as_str) else { continue };
        let Some(at) = raw.0.get("at").and_then(serde_json::Value::as_str).and_then(|at| at.parse().ok()) else { continue };
        let adopted = raw.0.get("subject_id").and_then(serde_json::Value::as_str).is_some();
        let slot = first.entry(id.to_string()).or_insert((at, adopted));
        if at < slot.0 {
            *slot = (at, adopted);
        }
    }
    first.into_iter().map(|(id, (_, adopted))| (id, adopted)).collect()
}

/// `Ok` when the change dir `dir` was written by `canon change new`
/// (0.14–0.16), which takes both signals: its files still open with that
/// command's scaffold (`proposal.md` through the `## What Changes`
/// comment, `tasks.md` through the row-grammar comment; what follows is
/// free), and the change's first stored version is already adopted
/// (`adopted`), because `change new` adopts before any ingest. `Err(None)`
/// when neither holds (an external change); `Err(Some(why))` when only
/// one does, or the change has no stored version, naming why it stays.
fn canon_written(dir: &Path, adopted: Option<bool>) -> Result<(), Option<String>> {
    let read = |name: &str| fs::read_to_string(dir.join(name)).map(|text| text.replace("\r\n", "\n")).unwrap_or_default();
    let proposal = read("proposal.md");
    let tasks = read("tasks.md");
    let title = proposal.lines().next().and_then(|line| line.strip_prefix("# ")).unwrap_or_default();
    let scaffold = !title.is_empty() && proposal.starts_with(&scaffold_proposal(title)) && tasks.starts_with(&scaffold_tasks(title));
    match (scaffold, adopted) {
        (true, Some(true)) => Ok(()),
        (false, None | Some(false)) => Err(None),
        (true, None) => Err(Some("its files are `canon change new`'s scaffold, but canon has no stored record of the change to confirm it".to_string())),
        (true, Some(false)) => Err(Some("its files are `canon change new`'s scaffold, but its first stored version was not adopted into a subject, as `canon change new` adopts".to_string())),
        (false, Some(true)) => Err(Some("its first stored version was adopted at once, as `canon change new` adopts, but its files are not that command's scaffold".to_string())),
    }
}

/// `text` with the plan's `canon.yaml` edits: the first `roots_done`
/// root moves', the openspec source removal when `drop_source`, and
/// `skills.providers`. Each edit checks its own result.
fn render_config(text: &str, plan: &Plan, roots_done: usize, drop_source: bool) -> Result<String, String> {
    let mut out = text.to_string();
    for root in plan.roots.iter().take(roots_done) {
        if let Some(edit) = &root.config {
            out = edit_spec_root(&out, edit)?;
        }
    }
    if drop_source {
        if let Some(index) = plan.drop_source {
            out = drop_plan_source(&out, index)?;
        }
    }
    if let Some(providers) = &plan.providers {
        out = skills::rewrite_skills_section(&out, providers)?;
    }
    Ok(out)
}

/// The file's newline: `\r\n` when it uses it, else `\n`.
fn newline_of(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The line span `[start, end)` of the top-level `key:` section: the key
/// line and the indented lines after it (a blank line belongs to it only
/// when an indented line follows), as `canon skills install` reads
/// `skills:`.
fn section(lines: &[&str], key: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| line.strip_prefix(key).is_some_and(|rest| rest.starts_with(':')))?;
    let mut end = start + 1;
    let mut cursor = start + 1;
    while let Some(line) = lines.get(cursor) {
        if line.trim().is_empty() {
            cursor += 1;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            cursor += 1;
            end = cursor;
        } else {
            break;
        }
    }
    Some((start, end))
}

/// The list items of the section span `[start, end)`: the line index each
/// one starts at, at the indent of the first `- ` line after `list_key:`.
fn list_items(lines: &[&str], (start, end): (usize, usize), list_key: &str) -> Vec<usize> {
    let Some(key_line) = (start..end).find(|&i| lines[i].trim_start().strip_prefix(list_key).is_some_and(|rest| rest.starts_with(':'))) else { return Vec::new() };
    let key_indent = indent(lines[key_line]);
    let mut items = Vec::new();
    let mut item_indent = None;
    for (i, line) in lines.iter().enumerate().take(end).skip(key_line + 1) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let at = indent(line);
        if at <= key_indent && !(at == key_indent && trimmed.starts_with("- ")) {
            break;
        }
        if trimmed.starts_with("- ") && item_indent.is_none_or(|indent| indent == at) {
            item_indent = Some(at);
            items.push(i);
        }
    }
    items
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The YAML scalar for a path: plain when it is plain-safe, else quoted.
fn scalar(path: &str) -> String {
    if !path.is_empty() && !path.starts_with('-') && path.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c)) {
        path.to_string()
    } else {
        serde_json::to_string(path).unwrap_or_default()
    }
}

/// `text` with one `specs.roots` edit applied textually, then checked:
/// the result must parse to `text`'s document with exactly that edit.
fn edit_spec_root(text: &str, edit: &RootConfigEdit) -> Result<String, String> {
    let nl = newline_of(text);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let refuse = |what: &str| format!("could not rewrite canon.yaml `specs.roots` safely ({what}); set it by hand: {}", describe_root_edit(edit));
    let rewritten = match edit {
        RootConfigEdit::Section { new_root, former } => {
            let mut out = text.to_string();
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push_str(nl);
                }
                out.push_str(nl);
            }
            for line in [
                "# The .feature corpus: <root>/kind=feature/area=<area>/<surface>.feature.".to_string(),
                "# `canon migrate` moved it from `former_roots`; reviews pinned before".to_string(),
                "# the move still find their scenarios there.".to_string(),
                "specs:".to_string(),
                "  roots:".to_string(),
                "    - id: root".to_string(),
                format!("      root: {}", scalar(new_root)),
                format!("      former_roots: [{}]", scalar(former)),
            ] {
                out.push_str(&line);
                out.push_str(nl);
            }
            out
        }
        RootConfigEdit::Entry { index, new_root, former, explicit_root } => {
            let span = section(&lines, "specs").ok_or_else(|| refuse("no top-level `specs:` block"))?;
            let items = list_items(&lines, span, "roots");
            let first = *items.get(*index).ok_or_else(|| refuse("its `roots:` entries are not a block list"))?;
            let last = items.get(index + 1).copied().unwrap_or(span.1);
            let key_of = |line: &str, key: &str| -> Option<usize> {
                let trimmed = line.trim_start();
                let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
                body.strip_prefix(key).is_some_and(|rest| rest.starts_with(':')).then(|| line.len() - body.len())
            };
            let mut out: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
            let ending = |line: &str| if line.ends_with("\r\n") { "\r\n" } else if line.ends_with('\n') { "\n" } else { nl };
            if *explicit_root {
                let at = (first..last).find(|&i| key_of(lines[i], "root").is_some()).ok_or_else(|| refuse("no `root:` line in the entry"))?;
                let line = lines[at];
                let col = key_of(line, "root").unwrap_or_default();
                let value = line[col + "root:".len()..].trim_end_matches(['\r', '\n']);
                let comment = value.find(" #").map(|i| &value[value[..=i].trim_end().len()..]).unwrap_or_default();
                out[at] = format!("{}root: {}{comment}{}", &line[..col], scalar(new_root), ending(line));
                out.insert(at + 1, format!("{}former_roots: [{}]{}", " ".repeat(col), scalar(former), ending(line)));
            } else {
                let at = (first..last).find(|&i| key_of(lines[i], "id").is_some()).ok_or_else(|| refuse("no `id:` line in the entry"))?;
                let col = key_of(lines[at], "id").unwrap_or_default();
                let end = (first..last).rev().find(|&i| !lines[i].trim().is_empty()).unwrap_or(at);
                let line = lines[end];
                if !line.ends_with('\n') {
                    out[end].push_str(nl);
                }
                out.insert(end + 1, format!("{}former_roots: [{}]{}", " ".repeat(col), scalar(former), ending(line)));
            }
            out.concat()
        }
    };
    let expected = |text: &str| -> Option<Value> {
        let mut doc: Value = serde_yaml::from_str(text).ok()?;
        let map = doc.as_mapping_mut()?;
        let entry = match edit {
            RootConfigEdit::Section { .. } => {
                let mut roots = serde_yaml::Mapping::new();
                roots.insert("roots".into(), Value::Sequence(vec![Value::Mapping(serde_yaml::Mapping::from_iter([("id".into(), "root".into())]))]));
                map.insert("specs".into(), Value::Mapping(roots));
                map.get_mut("specs")?.get_mut("roots")?.get_mut(0)?
            }
            RootConfigEdit::Entry { index, .. } => map.get_mut("specs")?.get_mut("roots")?.get_mut(*index)?,
        };
        let entry = entry.as_mapping_mut()?;
        let (new_root, former, explicit) = match edit {
            RootConfigEdit::Section { new_root, former } => (new_root, former, true),
            RootConfigEdit::Entry { new_root, former, explicit_root, .. } => (new_root, former, *explicit_root),
        };
        if explicit {
            entry.insert("root".into(), new_root.as_str().into());
        }
        let mut formers = entry.get("former_roots").and_then(Value::as_sequence).cloned().unwrap_or_default();
        formers.push(former.as_str().into());
        entry.insert("former_roots".into(), Value::Sequence(formers));
        Some(doc)
    };
    let parsed: Option<Value> = serde_yaml::from_str(&rewritten).ok();
    if parsed.is_none() || parsed != expected(text) {
        return Err(refuse("the edited file would not parse to exactly that change"));
    }
    Ok(rewritten)
}

fn describe_root_edit(edit: &RootConfigEdit) -> String {
    match edit {
        RootConfigEdit::Section { new_root, former } => format!("`specs: {{ roots: [{{ id: root, root: {new_root}, former_roots: [{former}] }}] }}`"),
        RootConfigEdit::Entry { index, new_root, former, .. } => format!("`specs.roots[{index}]` with `root: {new_root}` and `former_roots: [{former}]`"),
    }
}

/// `text` without `plans.sources[index]`, an empty list written `[]`,
/// checked like [`edit_spec_root`].
fn drop_plan_source(text: &str, index: usize) -> Result<String, String> {
    let refuse = || "could not remove the `{ dialect: openspec, root: . }` plans source from canon.yaml safely; remove it by hand".to_string();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let span = section(&lines, "plans").ok_or_else(refuse)?;
    let items = list_items(&lines, span, "sources");
    let first = *items.get(index).ok_or_else(refuse)?;
    let item_indent = indent(lines[first]);
    let mut last = items.get(index + 1).copied().unwrap_or(span.1);
    // Comments and blank lines after the entry belong to what follows.
    while last > first + 1 && {
        let line = lines[last - 1];
        line.trim().is_empty() || (line.trim_start().starts_with('#') && indent(line) <= item_indent)
    } {
        last -= 1;
    }
    let mut out: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
    out.drain(first..last);
    if items.len() == 1 {
        let key = (span.0..first).rev().find(|&i| lines[i].trim_start().starts_with("sources:")).ok_or_else(refuse)?;
        let line = lines[key];
        let ending = if line.ends_with("\r\n") { "\r\n" } else if line.ends_with('\n') { "\n" } else { "" };
        out[key] = format!("{}sources: []{ending}", &line[..indent(line)]);
    }
    let rewritten = out.concat();
    let expected = (|| -> Option<Value> {
        let mut doc: Value = serde_yaml::from_str(text).ok()?;
        let sources = doc.get_mut("plans")?.get_mut("sources")?.as_sequence_mut()?;
        sources.remove(index);
        Some(doc)
    })();
    let parsed: Option<Value> = serde_yaml::from_str(&rewritten).ok();
    if parsed.is_none() || parsed != expected {
        return Err(refuse());
    }
    Ok(rewritten)
}

fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).display().to_string()
}

fn print_plan(repo: &Path, plan: &Plan, config_changes: bool) {
    if !plan.changes_files() && plan.left.is_empty() {
        println!("canon migrate: nothing to migrate; the repo is on the 0.17 layout");
        println!("canon migrate: plan: rebuild the scenario index (full sync)");
        return;
    }
    println!("canon migrate: plan:");
    for root in &plan.roots {
        let config = match &root.config {
            Some(RootConfigEdit::Entry { new_root, former, explicit_root: true, .. }) => format!("; canon.yaml root: {new_root}, former_roots: [{former}]"),
            Some(RootConfigEdit::Entry { former, explicit_root: false, .. }) => format!("; canon.yaml former_roots: [{former}]"),
            Some(RootConfigEdit::Section { new_root, former }) => format!("; canon.yaml gains specs.roots: id root, root: {new_root}, former_roots: [{former}]"),
            None => String::new(),
        };
        println!("  move {} -> {} (spec root `{}`{config})", rel(repo, &root.from), rel(repo, &root.to), root.id);
    }
    for change in &plan.changes {
        println!("  move {} -> {} (proposal.md -> brief.md; written by `canon change new`)", rel(repo, &change.from), rel(repo, &change.to));
    }
    for brief in &plan.briefs {
        println!(
            "  write {} (Source: {}; Approved: {}); the Approved: line stays in {}, which canon does not edit",
            rel(repo, &brief.path),
            brief.source,
            brief.approval,
            brief.source
        );
    }
    if plan.drop_source.is_some() {
        println!("  canon.yaml: remove the plans source `{{ dialect: openspec, root: . }}` canon init wrote; no change is left under it");
    }
    if let Some(gitkeep) = &plan.gitkeep {
        println!("  remove {} (canon init's placeholder for that source)", rel(repo, gitkeep));
    }
    if let Some(providers) = &plan.providers {
        println!("  canon.yaml: skills.providers: [{}] (from the install lock)", providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "));
    }
    if !plan.gitignore.is_empty() {
        println!("  .gitignore: add {}", plan.gitignore.join(", "));
    }
    if config_changes {
        println!("  canon.yaml: every other byte kept");
    }
    println!("  rebuild the scenario index (full sync)");
    for left in &plan.left {
        println!("  left: {left}");
    }
}

/// What [`apply`] got through.
struct Applied {
    roots: usize,
    changes_done: bool,
    error: Option<String>,
}

/// Perform the plan's file moves and writes, stopping at the first
/// failure ([`Applied`]).
fn apply(repo: &Path, plan: &Plan, git: bool) -> Applied {
    let mut applied = Applied { roots: 0, changes_done: false, error: None };
    for root in &plan.roots {
        if let Err(e) = move_dir(repo, &root.from, &root.to, git) {
            applied.error = Some(e);
            return applied;
        }
        applied.roots += 1;
        println!("canon migrate: moved {} -> {}", rel(repo, &root.from), rel(repo, &root.to));
        for dir in &root.emptied {
            if fs::remove_dir(dir).is_err() && dir.is_dir() {
                println!("canon migrate: left {} in place; it still holds files canon does not move", rel(repo, dir));
            }
        }
    }
    for change in &plan.changes {
        let renamed = move_dir(repo, &change.from, &change.to, git).and_then(|()| move_file(repo, &change.to.join("proposal.md"), &change.to.join("brief.md"), git));
        if let Err(e) = renamed {
            applied.error = Some(e);
            return applied;
        }
        println!("canon migrate: moved {} -> {} (proposal.md -> brief.md)", rel(repo, &change.from), rel(repo, &change.to));
    }
    applied.changes_done = true;
    for brief in &plan.briefs {
        let written = fs::create_dir_all(brief.path.parent().unwrap_or(repo))
            .and_then(|()| fs::write(&brief.path, format!("Source: {}\n\nApproved: {}\n", brief.source, brief.approval)));
        if let Err(e) = written {
            applied.error = Some(format!("failed to write {}: {e}", rel(repo, &brief.path)));
            return applied;
        }
        println!("canon migrate: wrote {} (Source: {}); the Approved: line left in {} is no longer read", rel(repo, &brief.path), brief.source, brief.source);
    }
    if let Some(gitkeep) = &plan.gitkeep {
        let removed = if git && is_tracked(repo, gitkeep) { git_run(repo, &["rm", "-q", "--", &rel(repo, gitkeep)]) } else { fs::remove_file(gitkeep).map_err(|e| e.to_string()) };
        if let Err(e) = removed {
            applied.error = Some(format!("failed to remove {}: {e}", rel(repo, gitkeep)));
            return applied;
        }
        println!("canon migrate: removed {}", rel(repo, gitkeep));
    }
    if plan.drop_source.is_some() {
        // Only empty directories go: whatever else the repo keeps there stays.
        for dir in ["openspec/changes/archive", "openspec/changes", "openspec"] {
            let _ = fs::remove_dir(repo.join(dir));
        }
    }
    applied
}

fn move_dir(repo: &Path, from: &Path, to: &Path, git: bool) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create {}: {e}", rel(repo, parent)))?;
    }
    if git && is_tracked(repo, from) {
        git_run(repo, &["mv", "--", &rel(repo, from), &rel(repo, to)])
    } else {
        fs::rename(from, to).map_err(|e| format!("failed to move {} -> {}: {e}", rel(repo, from), rel(repo, to)))
    }
}

fn move_file(repo: &Path, from: &Path, to: &Path, git: bool) -> Result<(), String> {
    if to.exists() {
        return Err(format!("{} already exists", rel(repo, to)));
    }
    move_dir(repo, from, to, git)
}

fn git_run(repo: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git").args(args).current_dir(repo).output().map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("`git {}` failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn git_output(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(repo).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn in_git_work_tree(repo: &Path) -> bool {
    git_output(repo, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|out| out.trim() == "true")
}

/// Whether git tracks anything at `path`.
fn is_tracked(repo: &Path, path: &Path) -> bool {
    git_output(repo, &["ls-files", "--", &rel(repo, path)]).is_some_and(|out| !out.trim().is_empty())
}

/// `git status` lines for uncommitted changes under `paths`, untracked
/// files included; `None` when they are clean.
fn uncommitted(repo: &Path, paths: &[PathBuf]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let mut args = vec!["status".to_string(), "--porcelain".to_string(), "--untracked-files=all".to_string(), "--".to_string()];
    args.extend(paths.iter().map(|path| rel(repo, path)));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git_output(repo, &args)?;
    let lines: Vec<String> = out.lines().map(|line| format!("  {line}")).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn tracked_index_files(repo: &Path) -> usize {
    git_output(repo, &["ls-files", "--", paths::SCENARIO_INDEX_GITIGNORE]).map(|out| out.lines().count()).unwrap_or(0)
}

/// The full index rebuild (module doc). Exit `1` when a root does not
/// sync clean: the migration stands, and `canon inventory sync` lists
/// what to fix.
fn rebuild_index(repo: &Path) -> i32 {
    match crate::inventory::run_sync(repo, None) {
        Ok(outcome) if outcome.is_clean() => {
            println!("canon migrate: rebuilt the scenario index: {} record(s) written", outcome.total_written());
            0
        }
        Ok(outcome) => {
            print!("{}", crate::inventory::format_human(&outcome));
            eprintln!("canon migrate: the files moved, but the scenario index rebuild found the problems above; fix them, then run `canon inventory sync`");
            1
        }
        Err(e) => {
            eprintln!("canon migrate: the files moved, but the scenario index was not rebuilt: {e}; run `canon inventory sync`");
            EXIT_REFUSED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_ENTRY: &str = "# head\r\nspecs:\r\n  roots:\r\n    - id: canon # own corpus\r\n      root: specs  # the old default\r\n    - id: game\r\n      root: examples/specs\r\nplans:\r\n  sources: []\r\n";

    #[test]
    fn a_root_edit_keeps_comments_and_crlf_and_adds_former_roots() {
        let edit = RootConfigEdit::Entry { index: 0, new_root: ".canon/specs".into(), former: "specs".into(), explicit_root: true };
        let out = edit_spec_root(ROOT_ENTRY, &edit).unwrap();
        assert_eq!(
            out,
            "# head\r\nspecs:\r\n  roots:\r\n    - id: canon # own corpus\r\n      root: .canon/specs  # the old default\r\n      former_roots: [specs]\r\n    - id: game\r\n      root: examples/specs\r\nplans:\r\n  sources: []\r\n"
        );
    }

    #[test]
    fn an_entry_without_root_gains_only_former_roots() {
        let text = "specs:\n  roots:\n    - id: root\nwrite:\n  promote: auto\n";
        let edit = RootConfigEdit::Entry { index: 0, new_root: ".canon/specs".into(), former: "specs".into(), explicit_root: false };
        assert_eq!(edit_spec_root(text, &edit).unwrap(), "specs:\n  roots:\n    - id: root\n      former_roots: [specs]\nwrite:\n  promote: auto\n");
    }

    #[test]
    fn an_absent_specs_key_gains_a_section() {
        let edit = RootConfigEdit::Section { new_root: ".canon/specs".into(), former: "specs".into() };
        let out = edit_spec_root("write:\n  promote: auto\n", &edit).unwrap();
        let doc: Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(doc["specs"]["roots"][0]["root"], Value::from(".canon/specs"));
        assert_eq!(doc["specs"]["roots"][0]["former_roots"][0], Value::from("specs"));
        assert!(out.starts_with("write:\n  promote: auto\n\n# The .feature corpus"), "{out}");
    }

    #[test]
    fn a_flow_style_roots_list_is_refused_not_guessed() {
        let edit = RootConfigEdit::Entry { index: 0, new_root: ".canon/specs".into(), former: "specs".into(), explicit_root: true };
        let err = edit_spec_root("specs: { roots: [{ id: root, root: specs }] }\n", &edit).unwrap_err();
        assert!(err.contains("set it by hand"), "{err}");
    }

    #[test]
    fn the_init_openspec_source_is_removed_and_an_empty_list_written() {
        let text = "plans:\n  # Where changes live\n  sources:\n    - dialect: openspec\n      root: .\nwrite:\n  promote: auto\n";
        assert_eq!(drop_plan_source(text, 0).unwrap(), "plans:\n  # Where changes live\n  sources: []\nwrite:\n  promote: auto\n");
        let two = "plans:\n  sources:\n    - dialect: openspec\n      root: .\n    # superpowers\n    - dialect: superpowers\n      root: docs/plans\n";
        assert_eq!(drop_plan_source(two, 0).unwrap(), "plans:\n  sources:\n    # superpowers\n    - dialect: superpowers\n      root: docs/plans\n");
    }
}
