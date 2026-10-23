//! Retry safety for the ledger-appending writes (0.21 D5).
//!
//! `evidence add` and `review add` append a record stamped with the current
//! time, so a caller that repeats a call (a lost response, a timeout, an
//! agent that is not sure the first one landed) used to write a second,
//! identical record. [`repeats_latest`] finds that case: the newest record of
//! the same subject already says exactly what the new one would say, ignoring
//! only what the write itself stamps (`at`, and the `run_seq` and
//! `staging_id` a promotion adds). The caller then writes nothing and reports
//! the existing record with `meta.unchanged: true`.
//!
//! A repeat is also not a repeat when something was recorded about the
//! existing record after it: a review pins the evidence it saw, so evidence
//! added again after a review is deliberate (it supersedes what the review
//! saw and names the re-pin), whatever its content. `followed_by` names that
//! later record kind.
//!
//! Only the *latest* record of a subject counts. A record that differs in
//! between (a divergent verdict, then a faithful one again) is a new fact and
//! is written, so the ledger keeps every change of state and drops only the
//! repeat of the current one.

use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// Keys a write or a promotion stamps; they never make two records differ.
const STAMPED: [&str; 3] = ["at", "run_seq", "staging_id"];

/// Where a written record lives, as a report says it: the staged location and
/// the ledger path once promoted (`null` where absent).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Landing {
    pub staged: Option<String>,
    pub committed: Option<String>,
}

/// Where an identical, already-written record lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Existing {
    /// Staging location (`_staging/` relative), when the record is still staged.
    pub staged: Option<String>,
    /// Ledger-relative path, once the record is committed.
    pub committed: Option<String>,
}

impl Existing {
    /// The `staged`/`committed` pair a write reports (`null` where absent).
    pub(crate) fn landing(&self) -> Landing {
        Landing { staged: self.staged.clone(), committed: self.committed.clone() }
    }

    /// Human wording for where the record already is.
    pub(crate) fn describe(&self) -> String {
        match (&self.committed, &self.staged) {
            (Some(path), _) => format!("committed as {path}"),
            (None, Some(path)) => format!("staged at {path} (`canon gate promote` commits it)"),
            (None, None) => "recorded".to_string(),
        }
    }
}

fn comparable(body: &Value) -> Value {
    let mut body = body.clone();
    if let Some(object) = body.as_object_mut() {
        for key in STAMPED {
            object.remove(key);
        }
    }
    body
}

fn at_of(body: &Value) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    body.get("at").and_then(Value::as_str).and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
}

/// Every parsed `*.json` record under `<root>/kind=<kind>/`, with its path
/// relative to `root`. Unreadable or malformed files are skipped: this only
/// looks for a repeat, and the gate reports malformed records itself.
fn records_under(root: &Path, kind: &str) -> Vec<(String, Value)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Value)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_dir() {
                walk(root, &path, out);
            } else if file_type.is_file() && path.extension().is_some_and(|e| e == "json") {
                let Ok(bytes) = std::fs::read(&path) else { continue };
                let Ok(body) = serde_json::from_slice::<Value>(&bytes) else { continue };
                if let Ok(relative) = path.strip_prefix(root) {
                    out.push((relative.display().to_string(), body));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &root.join(format!("kind={kind}")), &mut out);
    out
}

/// The existing record that `body` would repeat, if any.
///
/// `subject` says whether a stored record belongs to the same subject as
/// `body` (the same scenario, the same reviewer, …). Among the records of
/// that subject in the committed ledger (`ledger_root`) and, when `staging`
/// is given, its staging area, the newest one is compared with `body`
/// ignoring [`STAMPED`] keys; `Some` only when they are equal.
///
/// `followed_by` is `(kind, matches)`: a record of that kind that matches and
/// is newer than the existing record means the write is deliberate, not a
/// repeat.
pub(crate) fn repeats_latest(
    ledger_root: &Path,
    staging: Option<&Path>,
    kind: &str,
    body: &Value,
    subject: impl Fn(&Value) -> bool,
    followed_by: Option<(&str, &dyn Fn(&Value) -> bool)>,
) -> Option<Existing> {
    let mut latest: Option<(Existing, Value)> = None;
    let mut consider = |existing: Existing, stored: Value| {
        if !subject(&stored) {
            return;
        }
        if latest.as_ref().is_none_or(|(_, held)| at_of(&stored) >= at_of(held)) {
            latest = Some((existing, stored));
        }
    };
    for (path, stored) in records_under(ledger_root, kind) {
        consider(Existing { staged: None, committed: Some(path) }, stored);
    }
    if let Some(staging) = staging {
        for (path, stored) in records_under(staging, kind) {
            consider(Existing { staged: Some(path), committed: None }, stored);
        }
    }
    let (existing, stored) = latest?;
    if comparable(&stored) != comparable(body) {
        return None;
    }
    if let Some((later_kind, matches)) = followed_by {
        let stored_at = at_of(&stored);
        if records_under(ledger_root, later_kind).iter().any(|(_, later)| matches(later) && at_of(later) > stored_at) {
            return None;
        }
    }
    Some(existing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, body: &Value) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(body).unwrap()).unwrap();
    }

    fn rec(at: &str, verdict: &str) -> Value {
        serde_json::json!({ "scenario_id": "s", "at": at, "verdict": verdict })
    }

    #[test]
    fn only_the_newest_record_of_the_subject_counts() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path();
        write(ledger, "kind=k/a.json", &rec("2026-01-01T00:00:00Z", "faithful"));
        let same_subject = |v: &Value| v["scenario_id"] == "s";
        assert!(repeats_latest(ledger, None, "k", &rec("2026-02-01T00:00:00Z", "faithful"), same_subject, None).is_some());
        assert!(repeats_latest(ledger, None, "k", &rec("2026-02-01T00:00:00Z", "divergent"), same_subject, None).is_none());

        // A different record landed in between: the old identical one no longer counts.
        write(ledger, "kind=k/b.json", &rec("2026-01-02T00:00:00Z", "divergent"));
        assert!(repeats_latest(ledger, None, "k", &rec("2026-02-01T00:00:00Z", "faithful"), same_subject, None).is_none());
    }

    #[test]
    fn promotion_stamps_and_the_staging_area_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path();
        let mut committed = rec("2026-01-01T00:00:00Z", "faithful");
        committed["run_seq"] = serde_json::json!(3);
        committed["staging_id"] = serde_json::json!("abc");
        write(ledger, "kind=k/a.json", &committed);
        let found = repeats_latest(ledger, None, "k", &rec("2026-02-01T00:00:00Z", "faithful"), |_| true, None).unwrap();
        assert_eq!(found.committed.as_deref(), Some("kind=k/a.json"));

        let staging = ledger.join("_staging");
        write(&staging, "kind=k/b.json", &rec("2026-03-01T00:00:00Z", "faithful"));
        let found = repeats_latest(ledger, Some(&staging), "k", &rec("2026-04-01T00:00:00Z", "faithful"), |_| true, None).unwrap();
        assert_eq!(found.staged.as_deref(), Some("kind=k/b.json"));
    }

    #[test]
    fn a_later_record_that_depends_on_it_makes_the_write_deliberate() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path();
        write(ledger, "kind=k/a.json", &rec("2026-01-01T00:00:00Z", "faithful"));
        let body = rec("2026-03-01T00:00:00Z", "faithful");
        let dependent = |v: &Value| v["scenario_id"] == "s";
        let followed = Some(("dep", &dependent as &dyn Fn(&Value) -> bool));
        assert!(repeats_latest(ledger, None, "k", &body, |_| true, followed).is_some(), "nothing followed it yet");
        write(ledger, "kind=dep/r.json", &rec("2026-02-01T00:00:00Z", "x"));
        assert!(repeats_latest(ledger, None, "k", &body, |_| true, followed).is_none(), "a review landed after it");
    }
}
