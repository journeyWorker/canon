//! The rules of spec 10.13 that JSON Schema cannot express, over the documents of a `schema` answer.
//!
//! OWNER: the rules agent. The signatures below are the contract the checks are written against.

use serde_json::Value;

use super::model::{CommandDoc, GroupDoc, Tree};
use crate::claim::Claim;

/// What a rule found in one document: `(condition, problem)`, one pair per condition judged, `None` when it holds.
pub type Findings = Vec<(&'static str, Option<String>)>;

/// `L1.tree`: the conditions are `level`, `groups-first`, `unique-ids`, `entry-path` (rule 1 for each entry).
/// `answer` is the whole `schema --json` envelope; `claimed_level` is `"L1"`.
pub fn tree_findings(answer: &Value, tree: &Tree, claimed_level: &str) -> Findings {
    let _ = (answer, tree, claimed_level);
    Vec::new()
}

/// `L1.walk`, for a group document: `group-path` (rule 1: `command` is `path` joined by one space), `group-children` (the children are the tree's, exactly).
pub fn group_findings(group: &GroupDoc, tree: &Tree) -> Findings {
    let _ = (group, tree);
    Vec::new()
}

/// `L1.walk`, for a command document: spec 10.13 rules 1-11 and 13 (rule 12 is `L1.examples`).
/// The conditions are `rule-1`, `rule-2-refs`, `rule-2-compile`, `rule-2-outputs`, `rule-3-defaults`, `rule-3-domain-profile`, `rule-3-domain-pattern`,
/// `rule-3-domain-compile`, `rule-4-names`, `rule-4-codes`, `rule-4-relation-once`, `rule-5`, `rule-6`, `rule-7`, `rule-8`, `rule-9`, `rule-10`, `rule-11`, `rule-13`.
/// `exit_of` gives the registered exit of a code; `claim` decides whether a declared map is a cut node (rule 16).
pub fn command_findings(doc: &CommandDoc, tool: &str, claim: Claim, exit_of: &dyn Fn(&str) -> Option<i64>) -> Findings {
    let _ = (doc, tool, claim, exit_of);
    Vec::new()
}
