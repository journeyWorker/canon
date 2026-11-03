//! One `atip check` run that the tests of a binary share (`canon-0-35-split-atip-check`).
//!
//! canon's conformance is proved once: `atip check` on the canon binary and the committed fixture, at one level. The tests that judge
//! canon against that run read rows of its report; they do not run the checker again. A test that plants a violation in canon or in the
//! fixture needs a run of its own and keeps calling `check_binary` itself.
//!
//! One process (`cargo test`) runs every test of a binary, so a `LazyLock` in the binary holds the report. `cargo nextest` runs each test in
//! a process of its own, so the first test to ask runs the checker and writes the report under `CARGO_TARGET_TMPDIR`, in a directory named
//! after the nextest run (`NEXTEST_RUN_ID`); the others wait on a lock file and read it. A run that panicked leaves a
//! `.failed` file and the others fail with it at once; directories of earlier runs are removed after a day. The report is the JSON the checker prints, plus its text.

#![expect(dead_code, reason = "each test binary that includes this module reads a subset of the rows")]

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use atip_check::report::Report;
use serde_json::{json, Value};

/// A report as the tests read it: the JSON of `atip check --json` (its `data`) and the text of the run.
#[derive(Debug)]
pub struct Rows {
    json: Value,
    text: String,
}

impl Rows {
    fn of(report: &Report) -> Self {
        Self { json: serde_json::to_value(report).expect("the report is JSON"), text: report.text() }
    }

    /// The report, whole.
    pub fn json(&self) -> &Value {
        &self.json
    }

    /// The text of the run, for the message of an assertion.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn passed(&self) -> bool {
        self.json["passed"] == true
    }

    pub fn level(&self) -> &str {
        self.json["level"].as_str().expect("level")
    }

    pub fn contract_version(&self) -> &str {
        self.json["contractVersion"].as_str().expect("contractVersion")
    }

    /// Every check, in the order the run reported them.
    pub fn checks(&self) -> &[Value] {
        self.json["checks"].as_array().expect("checks")
    }

    /// The check `id`, if the run reported it.
    pub fn find(&self, id: &str) -> Option<&Value> {
        self.checks().iter().find(|check| check["id"] == id)
    }

    /// The result of the check `id` (`pass`, `fail`, `unprovided`, `unsupported` or `entailed`).
    pub fn result(&self, id: &str) -> Option<&str> {
        self.find(id).and_then(|check| check["result"].as_str())
    }

    /// The ids of the checks whose result `keep` accepts, with their strength (`MUST` or `SHOULD`).
    pub fn ids_where(&self, keep: impl Fn(&str, &str) -> bool) -> Vec<&str> {
        self.checks().iter().filter(|check| keep(check["strength"].as_str().unwrap_or_default(), check["result"].as_str().unwrap_or_default())).map(|check| check["id"].as_str().expect("id")).collect()
    }

    /// How many SHOULD checks failed.
    pub fn advisories(&self) -> usize {
        self.ids_where(|strength, result| strength == "SHOULD" && result == "fail").len()
    }

    /// The programs the fixture's `prepare` ran, each as its argv.
    pub fn prepare(&self) -> Vec<Vec<&str>> {
        self.json["prepare"].as_array().into_iter().flatten().map(|step| step.as_array().expect("a step").iter().map(|word| word.as_str().expect("a word")).collect()).collect()
    }

    /// The ids of the MUST checks that are neither a pass nor unsupported.
    pub fn red_musts(&self) -> Vec<&str> {
        self.ids_where(|strength, result| strength == "MUST" && !matches!(result, "pass" | "unsupported"))
    }
}

/// Report directories of earlier nextest runs older than this are removed when a run makes its own.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Removes the report directories of earlier nextest runs (`root/<run id>`), so `target/tmp/atip-report` does not grow with every run. A
/// directory younger than a day may belong to a run that is still going, in the same target directory, and stays.
fn remove_old_runs(root: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let old = entry.metadata().and_then(|meta| meta.modified()).ok().and_then(|at| at.elapsed().ok()).is_some_and(|age| age > STALE_AFTER);
        if old && entry.path() != current {
            // A directory another process removes at the same moment is gone either way.
            drop(std::fs::remove_dir_all(entry.path()));
        }
    }
}

/// The text of a panic, as the test harness would print it.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|text| (*text).to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "the run panicked".to_string())
}

/// The report of the run `run` makes, once for the binary and, under nextest, once for the nextest run.
///
/// `key` names the run (`canon-l1`): the level and the binary it judges. `run` is the one full `atip check`; it is called by one test
/// at most, and by none when an earlier test of the nextest run wrote the report. When it panics, the panic is written next to the report
/// (`<key>.failed`) and every other test that asks fails with it at once, without running the checker again.
pub fn shared(key: &str, run: impl FnOnce() -> Report) -> Rows {
    let Some(id) = std::env::var_os("NEXTEST_RUN_ID") else { return Rows::of(&run()) };
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("atip-report");
    let dir = root.join(id);
    let fresh = !dir.exists();
    std::fs::create_dir_all(&dir).expect("create the report directory");
    if fresh {
        remove_old_runs(&root, &dir);
    }
    let lock: File = OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(format!("{key}.lock"))).expect("open the lock");
    // Held until `lock` is dropped, at the end of this function: a test that comes second waits here until the first has written the report.
    lock.lock().expect("lock the report");
    let failed = dir.join(format!("{key}.failed"));
    if let Ok(message) = std::fs::read_to_string(&failed) {
        panic!("the shared run of `atip check` ({key}) failed in another test of this nextest run: {message}");
    }
    let path = dir.join(format!("{key}.json"));
    if let Ok(text) = std::fs::read_to_string(&path) {
        let stored: Value = serde_json::from_str(&text).expect("the stored report is JSON");
        return Rows { json: stored["json"].clone(), text: stored["text"].as_str().expect("the stored text").to_string() };
    }
    let report = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(report) => report,
        Err(payload) => {
            std::fs::write(&failed, panic_text(payload.as_ref())).expect("write the failure");
            std::panic::resume_unwind(payload)
        }
    };
    let rows = Rows::of(&report);
    let partial = dir.join(format!("{key}.json.partial"));
    std::fs::write(&partial, json!({"json": rows.json, "text": rows.text}).to_string()).expect("write the report");
    std::fs::rename(&partial, &path).expect("publish the report");
    rows
}
