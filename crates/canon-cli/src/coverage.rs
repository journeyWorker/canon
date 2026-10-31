//! What a check examined (0.28 D3): `meta.coverage`.
//!
//! A check that looked at nothing must not say `clean`. Each check builds a
//! [`Coverage`] while it runs; when [`Coverage::examined_nothing`] is true and
//! the check found no violation, the report is `unverifiable` (`data.clean`
//! false, `data.unverifiable` true) and the exit is 1.

use serde_json::{json, Map, Value};
use crate::cli_error::CliErrorExt as _;

/// One spec root a check looked at.
#[derive(Debug)]
struct RootSeen {
    id: String,
    path: String,
    examined: usize,
}

/// One spec root a check could not look at, and why.
#[derive(Debug)]
struct RootSkipped {
    path: String,
    reason: String,
}

#[derive(Debug, Default)]
pub struct Coverage {
    roots: Vec<RootSeen>,
    skipped: Vec<RootSkipped>,
    counts: Vec<(&'static str, usize)>,
}

impl Coverage {
    pub fn new() -> Self {
        Self::default()
    }

    /// A spec root the check examined, with how many units (files, scenarios)
    /// of it the check looked at.
    pub fn root(&mut self, id: impl Into<String>, path: impl Into<String>, examined: usize) -> &mut Self {
        self.roots.push(RootSeen { id: id.into(), path: path.into(), examined });
        self
    }

    /// A spec root the check skipped, with the reason.
    pub fn skip(&mut self, path: impl Into<String>, reason: impl Into<String>) -> &mut Self {
        self.skipped.push(RootSkipped { path: path.into(), reason: reason.into() });
        self
    }

    /// How many `what` (scenarios, subjects, files, …) the check examined.
    pub fn count(&mut self, what: &'static str, n: usize) -> &mut Self {
        self.counts.push((what, n));
        self
    }

    /// True when every count is zero and no root held a unit to examine: the check had
    /// nothing to be right or wrong about.
    pub fn examined_nothing(&self) -> bool {
        self.counts.iter().all(|(_, n)| *n == 0) && self.roots.iter().all(|root| root.examined == 0)
    }

    /// The `meta.coverage` value.
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        if !self.roots.is_empty() {
            out.insert("specRoots".into(), Value::Array(self.roots.iter().map(|r| json!({ "id": r.id, "path": r.path, "examined": r.examined })).collect()));
        }
        for (what, n) in &self.counts {
            out.insert((*what).to_string(), json!(n));
        }
        out.insert("skipped".into(), Value::Array(self.skipped.iter().map(|s| json!({ "path": s.path, "reason": s.reason })).collect()));
        Value::Object(out)
    }

    /// `{ "coverage": … }`, the extra `meta` a check passes to the envelope.
    pub fn meta(&self) -> Value {
        json!({ "coverage": self.to_json() })
    }
}

/// Marks `data` (an object report) as unverifiable: `clean` false,
/// `unverifiable` true.
pub fn mark_unverifiable(data: &mut Value) {
    if let Some(object) = data.as_object_mut() {
        object.insert("clean".into(), json!(false));
        object.insert("unverifiable".into(), json!(true));
    }
}

/// The refusal-shaped failure every unverifiable check returns.
pub fn unverifiable_error(command: &str, what: &str, hint: impl Into<String>) -> crate::cli_error::CliError {
    crate::cli_error::CliError::failed("UNVERIFIABLE", format!("`{command}` examined {what}, so it cannot report clean")).hint(hint)
}
