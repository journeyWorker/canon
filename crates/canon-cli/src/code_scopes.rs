//! Code scopes (0.38, `canon-0-38-code-scopes`, #130): the policy's `code_scopes`
//! key says, per scenario area or surface, which code paths the scenario's
//! evidence is about.
//!
//! ```yaml
//! code_scopes:
//!   core: ["core/**"]               # every scenario whose id starts `core.`
//!   app: ["app/**", "core/**"]
//!   app.bulkread: ["app/bulk/**"]   # the surface `app.bulkread` (area.surface) wins over `app`
//! ```
//!
//! A pattern is a `code.exclude` glob ([`canon_model::path_glob`]). A
//! scenario's scope is its surface's key, else its area's key, else none.
//!
//! The scopes only shorten what an approved chore asks for
//! ([`crate::reverify`]): a scenario whose latest evidence is on an older code
//! tree needs no new evidence from the chore when no path that changed since
//! lies in its scope. They are the policy's, so they stand only while the
//! policy is the one a human approved ([`crate::policy_approval`]); a policy
//! edited after its approval fails closed there, and here it declares no scope.
//! A key that is absent, malformed or approved by nobody leaves every verdict as
//! it was. So does a key none of whose patterns matches a file of the code tree
//! ([`live`]): a misspelt pattern (matching is byte-exact) fails closed, it
//! never exempts. An empty list is malformed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use canon_model::path_glob::{Anchor, Glob};
use canon_model::{paths, ScenarioId};
use canon_store::git::GitCall;

/// The `code_scopes` of a repo's policy and whether the policy stands approved.
#[derive(Debug, Default)]
pub struct CodeScopes {
    declared: BTreeMap<String, Vec<String>>,
    /// Why the key could not be read as a map of path-pattern lists.
    problem: Option<String>,
    approved: bool,
}

impl CodeScopes {
    /// Read `<repo>/.canon/policy.yaml`. No file, no key and a key that is not a
    /// well-formed map all declare no scope.
    pub fn read(repo: &Path) -> Self {
        let approved = matches!(crate::policy_approval::approval_state(repo), crate::policy_approval::ApprovalState::Approved(_));
        let Ok(raw) = std::fs::read_to_string(repo.join(paths::POLICY_FILE)) else { return Self::default() };
        let (declared, problem) = declared_in(&raw);
        Self { declared, problem, approved }
    }

    /// Whether the policy declares any scope (or tries to).
    pub fn declares(&self) -> bool {
        !self.declared.is_empty() || self.problem.is_some()
    }

    /// The scope key and patterns of `scenario`: its surface's, else its area's.
    /// `None` without a declaration and while the policy is not approved.
    pub fn of(&self, scenario: &ScenarioId) -> Option<(&str, &[String])> {
        if !self.approved {
            return None;
        }
        let surface = format!("{}.{}", scenario.area(), scenario.surface());
        let found = [surface.as_str(), scenario.area()].into_iter().find_map(|key| self.declared.get_key_value(key));
        found.map(|(key, patterns)| (key.as_str(), patterns.as_slice()))
    }

    /// What is wrong with the declarations (0.38 F5): a pattern that matches no file
    /// of the code tree, a key that matches no scenario, a key that is not a map of
    /// lists. Empty for a policy without `code_scopes`. `files` is the code tree's
    /// files; `None` when it cannot be listed, which leaves the patterns unjudged.
    pub fn warnings<'a>(&self, files: Option<&[String]>, scenarios: impl Iterator<Item = &'a ScenarioId>) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(problem) = &self.problem {
            out.push(format!("`code_scopes` in {} is malformed ({problem}), so no scenario has a code scope; the key is a map of scenario areas or surfaces to lists of path patterns", paths::POLICY_FILE));
        }
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for scenario in scenarios {
            seen.insert(scenario.area().to_string());
            seen.insert(format!("{}.{}", scenario.area(), scenario.surface()));
        }
        for (key, patterns) in &self.declared {
            if !seen.contains(key) {
                out.push(format!("code scope `{key}` matches no scenario: a key is the area (`core`) or the surface (`app.bulkread`) of a scenario id"));
            }
            let Some(files) = files else { continue };
            let dead_key = !live(patterns, Some(files));
            for pattern in patterns {
                let glob = Glob::new(pattern, Anchor::Anywhere);
                if !files.iter().any(|file| glob.matches(file)) {
                    let rule = if dead_key { "; no pattern of the key matches a file, so the key declares no scope and its scenarios follow the 0.37 rule" } else { "" };
                    out.push(format!("code scope `{key}`: the pattern `{pattern}` matches no file of the code tree{rule}; fix it in {} and have the human approve the policy again", paths::POLICY_FILE));
                }
            }
        }
        out
    }
}

/// Whether a key's `patterns` stand: at least one matches a file of the code tree
/// (`files`). A key none of whose patterns matches, or a tree that cannot be listed,
/// declares no scope: the scenarios follow the rule without scopes.
pub fn live(patterns: &[String], files: Option<&[String]>) -> bool {
    let globs = compile(patterns);
    files.is_some_and(|files| globs.iter().any(|glob| files.iter().any(|file| glob.matches(file))))
}

/// The patterns of a scope, compiled once to be matched against many paths: a `code.exclude` glob, a name at any depth.
pub fn compile(patterns: &[String]) -> Vec<Glob> {
    patterns.iter().map(|pattern| Glob::new(pattern, Anchor::Anywhere)).collect()
}

/// The `code_scopes` of policy text: the keys, and why they could not be read.
fn declared_in(raw: &str) -> (BTreeMap<String, Vec<String>>, Option<String>) {
    let Ok(serde_yaml::Value::Mapping(top)) = serde_yaml::from_str::<serde_yaml::Value>(raw) else { return (BTreeMap::new(), None) };
    let Some(value) = top.get("code_scopes") else { return (BTreeMap::new(), None) };
    match parse(value) {
        Ok(declared) => (declared, None),
        Err(problem) => (BTreeMap::new(), Some(problem)),
    }
}

/// The keys whose scopes differ between the policy that was last approved and the one
/// in force (0.38): `Some` only when the policy is changed since its approval and its
/// `code_scopes` is not the approved policy's, so `canon brief approve` must leave the policy
/// approval stale and a human re-approves it. The approved bytes are found in git history by
/// their digest; when they cannot be found, every key now declared counts as changed. `None`
/// for a first approval, an unchanged policy, an unchanged `code_scopes`, or a policy that
/// declares none.
pub fn changed_since_approval(repo: &Path) -> Option<Vec<String>> {
    let crate::policy_approval::ApprovalState::Changed { approval, .. } = crate::policy_approval::approval_state(repo) else { return None };
    let now_raw = std::fs::read_to_string(repo.join(paths::POLICY_FILE)).ok()?;
    let (now, now_problem) = declared_in(&now_raw);
    if now.is_empty() && now_problem.is_none() {
        // Nothing declared: no scope can have been widened (removing a key only asks for more evidence).
        return None;
    }
    let before = approved_bytes(repo, &approval.policy_digest).map(|raw| declared_in(&raw).0);
    let mut keys: BTreeSet<String> = BTreeSet::new();
    match before {
        Some(before) => {
            for key in now.keys().chain(before.keys()) {
                if now.get(key) != before.get(key) {
                    keys.insert(key.clone());
                }
            }
            if now_problem.is_some() && keys.is_empty() {
                keys.insert("code_scopes".to_string());
            }
        }
        None => {
            keys.extend(now.keys().cloned());
            if now_problem.is_some() {
                keys.insert("code_scopes".to_string());
            }
        }
    }
    (!keys.is_empty()).then(|| keys.into_iter().collect())
}

/// The policy file as it was when its digest was `digest`, from git history.
fn approved_bytes(repo: &Path, digest: &str) -> Option<String> {
    let git = |args: &[&str]| GitCall::new(repo).args(args).run().ok().map(|output| output.stdout);
    let revs = git(&["log", "--format=%H", "--", paths::POLICY_FILE])?;
    for rev in String::from_utf8_lossy(&revs).lines() {
        let Some(bytes) = git(&["show", &format!("{rev}:{}", paths::POLICY_FILE)]) else { continue };
        if crate::policy_approval::policy_digest(&bytes) == digest {
            return String::from_utf8(bytes).ok();
        }
    }
    None
}

fn parse(value: &serde_yaml::Value) -> Result<BTreeMap<String, Vec<String>>, String> {
    let serde_yaml::Value::Mapping(map) = value else { return Err("not a map".to_string()) };
    let mut declared = BTreeMap::new();
    for (key, patterns) in map {
        let Some(key) = key.as_str().filter(|k| !k.trim().is_empty()) else { return Err("a key is not a non-empty string".to_string()) };
        let serde_yaml::Value::Sequence(list) = patterns else { return Err(format!("`{key}` is not a list of path patterns")) };
        let mut globs = Vec::new();
        for pattern in list {
            match pattern.as_str().filter(|p| !p.trim_matches('/').is_empty()) {
                Some(glob) => globs.push(glob.to_string()),
                None => return Err(format!("`{key}` has a pattern that is not a non-empty string")),
            }
        }
        if globs.is_empty() {
            return Err(format!("`{key}` has no pattern"));
        }
        declared.insert(key.to_string(), globs);
    }
    Ok(declared)
}
