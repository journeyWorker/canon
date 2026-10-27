//! Records that point at nothing committed (0.30 `record-links`).
//!
//! A finding names a change and a review, an evidence record a task and a
//! scenario, a subject its changes. Nothing used to say when the thing named
//! has no source: in canon's own ledger 184 findings pointed at a change with
//! no source left. `canon query` marks such a reference and counts it, and
//! `canon status` reports the count per kind of reference.
//!
//! # Only what is committed is a source
//! A reference resolves against sources every checkout of one commit shares:
//!
//! - **changes**: the directories of `.canon/changes`, the changes the
//!   configured plan sources declare, and committed ledger records of kind
//!   `change`;
//! - **tasks**: the tasks those plan sources declare;
//! - **scenarios**: the scenario ids of the spec corpus, and committed ledger
//!   records of kind `scenario`;
//! - **reviews**: the `review_id` of every committed review in the ledger or the
//!   archive.
//!
//! A ledger record counts when git would commit it: tracked, or not ignored
//! ([`visible_files`]). A derived record the repo's `.gitignore` leaves out
//! (the scenario and change records `canon inventory sync` and `canon ingest
//! plans` write) exists on one machine and not on a fresh clone, so it is no
//! source, and two checkouts of one commit report the same references. Outside
//! git, every ledger file counts.
//!
//! The reviews a 0.23 `canon migrate` re-pinned, and the pins of a review
//! written before 0.23, are not references and are never looked at. `canon
//! gate check` does not read any of this: its verdict does not change.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use canon_model::{RawRecord, RecordKind};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::inventory::SyncCtx;

/// Why the sources could not be read, so no reference was checked.
#[derive(Debug, thiserror::Error)]
pub enum SourcesError {
    #[error("the plan sources cannot be read ({0})")]
    Plans(#[from] crate::plans::PlansError),
    #[error("the spec roots cannot be read ({0})")]
    Specs(#[from] crate::inventory::InventoryError),
}

/// The kind of thing a reference points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Change,
    Review,
    Task,
    Scenario,
}

impl Target {
    fn as_str(self) -> &'static str {
        match self {
            Target::Change => "change",
            Target::Review => "review",
            Target::Task => "task",
            Target::Scenario => "scenario",
        }
    }
}

/// One reference to a source nothing committed holds: the field it is in,
/// the kind of thing it names, and the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Unresolved {
    pub field: &'static str,
    pub target: &'static str,
    pub id: String,
}

/// Whether `canon query --kind <kind>` and `canon status` check this kind's references.
pub fn is_checked(kind: RecordKind) -> bool {
    matches!(kind, RecordKind::Finding | RecordKind::Evidence | RecordKind::Subject)
}

/// What is committed, as the module doc defines it, read once.
#[derive(Debug)]
pub struct Sources {
    changes: BTreeSet<String>,
    tasks: BTreeSet<String>,
    scenarios: BTreeSet<String>,
    /// The ids of the committed reviews, read the first time a review id is looked up.
    reviews: OnceCell<BTreeSet<String>>,
    ledger: CommittedLedger,
}

impl Sources {
    /// Read the sources of the repo rooted at `repo`.
    pub fn load(repo: &Path) -> Result<Self, SourcesError> {
        let sync = SyncCtx::from_repo(repo)?;
        let (mut changes, tasks) = crate::plans::declared_ids(repo)?;
        let mut scenarios = BTreeSet::new();
        for root in sync.spec_roots(None)? {
            scenarios.extend(crate::inventory::scan_feature_corpus(&root.root).into_iter().map(|(id, _, _)| id.to_string()));
        }
        let ledger = CommittedLedger::of(repo, &sync.ledger_root);
        changes.extend(ledger.field(RecordKind::Change, "change_id"));
        scenarios.extend(ledger.field(RecordKind::Scenario, "scenario_id"));
        Ok(Self { changes, tasks, scenarios, reviews: OnceCell::new(), ledger })
    }

    fn resolves(&self, target: Target, id: &str) -> bool {
        match target {
            Target::Change => self.changes.contains(id),
            Target::Task => self.tasks.contains(id),
            Target::Scenario => self.scenarios.contains(id),
            Target::Review => self.reviews.get_or_init(|| self.ledger.field(RecordKind::Review, "review_id")).contains(id),
        }
    }

    /// The references in `body`, a record of `kind`, that no source holds, in field order.
    pub fn unresolved(&self, kind: RecordKind, body: &Value) -> Vec<Unresolved> {
        let mut found = Vec::new();
        let mut check = |field: &'static str, target: Target, id: &str| {
            if !self.resolves(target, id) {
                found.push(Unresolved { field, target: target.as_str(), id: id.to_string() });
            }
        };
        let text = |field: &str| body.get(field).and_then(Value::as_str).filter(|id| !id.is_empty());
        match kind {
            RecordKind::Finding => {
                if let Some(id) = text("change_id") {
                    check("change_id", Target::Change, id);
                }
                if let Some(id) = text("review_id") {
                    check("review_id", Target::Review, id);
                }
            }
            RecordKind::Evidence => {
                if let Some(id) = text("task_id") {
                    check("task_id", Target::Task, id);
                }
                if let Some(id) = text("scenario_id") {
                    check("scenario_id", Target::Scenario, id);
                }
            }
            RecordKind::Subject => {
                for id in body.get("change_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                    check("change_ids", Target::Change, id);
                }
            }
            _ => {}
        }
        found
    }
}

/// The ledger and the archive files git would commit.
#[derive(Debug)]
struct CommittedLedger {
    repo: PathBuf,
    /// The ledger root, then the archive root when the ledger has one.
    roots: Vec<PathBuf>,
    /// The repo-relative files git commits or would commit; `None` outside git
    /// (and for a root outside the repo), where every file counts.
    visible: Option<BTreeSet<PathBuf>>,
}

impl CommittedLedger {
    fn of(repo: &Path, ledger_root: &Path) -> Self {
        let mut roots = vec![ledger_root.to_path_buf()];
        roots.extend(canon_store::archive::archive_root_of(ledger_root));
        let visible = visible_files(repo, &roots);
        Self { repo: repo.to_path_buf(), roots, visible }
    }

    /// The values of `field` in every committed record of `kind`.
    fn field(&self, kind: RecordKind, field: &str) -> BTreeSet<String> {
        let mut values = BTreeSet::new();
        for root in &self.roots {
            let mut files = Vec::new();
            json_files(&root.join(format!("kind={}", kind.as_str())), &mut files);
            for file in files {
                let relative = file.strip_prefix(&self.repo).unwrap_or(&file);
                if self.visible.as_ref().is_some_and(|visible| !visible.contains(relative)) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&file) else { continue };
                let Ok(body) = serde_json::from_slice::<Value>(&bytes) else { continue };
                if let Some(value) = body.get(field).and_then(Value::as_str) {
                    values.insert(value.to_string());
                }
            }
        }
        values
    }
}

/// Every `*.json` file under `dir`, any depth.
fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => json_files(&path, out),
            Ok(kind) if kind.is_file() && path.extension().is_some_and(|e| e == "json") => out.push(path),
            _ => {}
        }
    }
}

/// The repo-relative files under `roots` that git tracks or would add
/// (`git ls-files --cached --others --exclude-standard`): a gitignored file is
/// not in the set. `None` when `repo` is not a git work tree, git cannot run,
/// or a root lies outside the repo.
fn visible_files(repo: &Path, roots: &[PathBuf]) -> Option<BTreeSet<PathBuf>> {
    let specs: Vec<&Path> = roots.iter().map(|root| root.strip_prefix(repo).ok()).collect::<Option<_>>()?;
    let output = Command::new("git").args(["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--"]).args(specs).current_dir(repo).output().ok()?;
    output.status.success().then(|| output.stdout.split(|byte| *byte == 0).filter(|path| !path.is_empty()).map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned())).collect())
}

/// How many references are unresolved, in all and per kind of reference
/// (`<record kind>.<field>`, e.g. `finding.change_id`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DanglingSummary {
    pub count: usize,
    pub by_reference: BTreeMap<String, usize>,
}

impl DanglingSummary {
    fn add(&mut self, kind: RecordKind, unresolved: &[Unresolved]) {
        for reference in unresolved {
            *self.by_reference.entry(format!("{}.{}", kind.as_str(), reference.field)).or_insert(0) += 1;
        }
        self.count += unresolved.len();
    }

    /// `finding.change_id 3, evidence.task_id 1`, or `None` when nothing dangles.
    pub fn describe(&self) -> Option<String> {
        (self.count > 0).then(|| self.by_reference.iter().map(|(reference, count)| format!("{reference} {count}")).collect::<Vec<_>>().join(", "))
    }
}

/// The references of the records of one kind a query returned.
#[derive(Debug, Default)]
pub struct QueryScan {
    pub summary: DanglingSummary,
    /// The unresolved references of each record that has any, by the record's key.
    pub by_record: BTreeMap<String, Vec<Unresolved>>,
}

impl QueryScan {
    /// Check `records` (all of kind `kind`), keyed by `key_of`.
    pub fn of(sources: &Sources, kind: RecordKind, records: &[RawRecord], key_of: impl Fn(&RawRecord) -> String) -> Self {
        let mut scan = Self::default();
        for raw in records {
            let unresolved = sources.unresolved(kind, &raw.0);
            if unresolved.is_empty() {
                continue;
            }
            scan.summary.add(kind, &unresolved);
            scan.by_record.insert(key_of(raw), unresolved);
        }
        scan
    }
}

/// The summary over records already read (`canon status` reads them typed): `records` are of `kind`.
/// A record that does not serialize holds no reference to check.
pub fn summarize<'a, T: Serialize + 'a>(sources: &Sources, kind: RecordKind, records: impl IntoIterator<Item = &'a T>, into: &mut DanglingSummary) {
    for body in records.into_iter().filter_map(|record| serde_json::to_value(record).ok()) {
        into.add(kind, &sources.unresolved(kind, &body));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git").args(args).current_dir(dir).status().expect("git must be on PATH for this test").success());
    }

    fn write(dir: &Path, relative: &str, text: &str) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A repo with one change directory, one scenario in the spec corpus, a committed review and an
    /// ignored derived change record.
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "specs:\n  roots:\n    - id: root\n      root: .canon/specs\n");
        write(dir.path(), ".canon/changes/kept-change/brief.md", "# Kept\n");
        write(
            dir.path(),
            ".canon/specs/kind=feature/area=cart/add.feature",
            "Feature: cart add\n  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}\n\n  @cart.add.01\n  Scenario: Add one\n  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}\n    Given an empty cart\n",
        );
        write(dir.path(), ".canon/ledger/kind=review/root_scenario_cart.add.01_00000000.json", "{\"kind\":\"review\",\"review_id\":\"01J9ZXQ4M2K7V8R3T5W6Y0N1PA\"}");
        write(dir.path(), ".canon/ledger/kind=change/derived-only_00000000.json", "{\"kind\":\"change\",\"change_id\":\"derived-only\"}");
        write(dir.path(), ".gitignore", ".canon/ledger/*\n!.canon/ledger/kind=review/\n");
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["add", "-A"]);
        dir
    }

    #[test]
    fn only_committed_sources_resolve_a_reference() {
        let dir = repo();
        let sources = Sources::load(dir.path()).unwrap();
        let finding = |change: &str, review: Option<&str>| {
            let mut body = serde_json::json!({ "change_id": change });
            if let Some(review) = review {
                body["review_id"] = Value::String(review.to_string());
            }
            sources.unresolved(RecordKind::Finding, &body)
        };
        assert!(finding("kept-change", Some("01J9ZXQ4M2K7V8R3T5W6Y0N1PA")).is_empty(), "a change directory and a committed review resolve");
        assert_eq!(
            finding("derived-only", Some("01J9ZXQ4M2K7V8R3T5W6Y0N1PB")),
            vec![
                Unresolved { field: "change_id", target: "change", id: "derived-only".into() },
                Unresolved { field: "review_id", target: "review", id: "01J9ZXQ4M2K7V8R3T5W6Y0N1PB".into() },
            ],
            "a gitignored derived change record is no source, and an unknown review id dangles"
        );
        let evidence = sources.unresolved(RecordKind::Evidence, &serde_json::json!({ "task_id": "kept-change#1", "scenario_id": "cart.add.01" }));
        assert_eq!(evidence, vec![Unresolved { field: "task_id", target: "task", id: "kept-change#1".into() }], "the scenario is in the corpus; the task has no tasks.md");
        let subject = sources.unresolved(RecordKind::Subject, &serde_json::json!({ "change_ids": ["kept-change", "gone"] }));
        assert_eq!(subject, vec![Unresolved { field: "change_ids", target: "change", id: "gone".into() }]);
        assert!(sources.unresolved(RecordKind::Review, &serde_json::json!({ "change_id": "gone" })).is_empty(), "a review's own pins are not checked");
    }

    #[test]
    fn an_untracked_review_that_git_would_add_counts_and_an_ignored_one_does_not() {
        let dir = repo();
        write(dir.path(), ".canon/ledger/kind=review/root_scenario_cart.add.01_11111111.json", "{\"kind\":\"review\",\"review_id\":\"01J9ZXQ4M2K7V8R3T5W6Y0N1PC\"}");
        write(dir.path(), ".canon/ledger/kind=evidence/ignored.json", "{\"kind\":\"evidence\"}");
        let sources = Sources::load(dir.path()).unwrap();
        assert!(sources.unresolved(RecordKind::Finding, &serde_json::json!({ "change_id": "kept-change", "review_id": "01J9ZXQ4M2K7V8R3T5W6Y0N1PC" })).is_empty());
    }

    #[test]
    fn the_summary_counts_per_kind_of_reference() {
        let dir = repo();
        let sources = Sources::load(dir.path()).unwrap();
        let mut summary = DanglingSummary::default();
        summarize(&sources, RecordKind::Finding, &[serde_json::json!({ "change_id": "a" }), serde_json::json!({ "change_id": "b" }), serde_json::json!({ "change_id": "kept-change" })], &mut summary);
        summarize(&sources, RecordKind::Evidence, &[serde_json::json!({ "task_id": "a#1" })], &mut summary);
        assert_eq!(summary.count, 3);
        assert_eq!(summary.by_reference, BTreeMap::from([("evidence.task_id".to_string(), 1), ("finding.change_id".to_string(), 2)]));
        assert_eq!(summary.describe().as_deref(), Some("evidence.task_id 1, finding.change_id 2"));
        assert_eq!(DanglingSummary::default().describe(), None);
    }
}
