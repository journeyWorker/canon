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
//! A pattern is a `code.exclude` glob ([`crate::git_rev::glob_matches`]). A
//! scenario's scope is its surface's key, else its area's key, else none.
//!
//! The scopes only shorten what an approved chore asks for
//! ([`crate::reverify`]): a scenario whose latest evidence is on an older code
//! tree needs no new evidence from the chore when no path that changed since
//! lies in its scope. They are the policy's, so they stand only while the
//! policy is the one a human approved ([`crate::policy_approval`]); a policy
//! edited after its approval fails closed there, and here it declares no scope.
//! A key that is absent, malformed or approved by nobody leaves every verdict as
//! it was.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use canon_model::{paths, ScenarioId};

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
        let Ok(serde_yaml::Value::Mapping(top)) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else { return Self::default() };
        let Some(value) = top.get("code_scopes") else { return Self::default() };
        match parse(value) {
            Ok(declared) => Self { declared, problem: None, approved },
            Err(problem) => Self { declared: BTreeMap::new(), problem: Some(problem), approved },
        }
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
            for pattern in patterns {
                if !files.iter().any(|file| crate::git_rev::glob_matches(pattern, file)) {
                    out.push(format!("code scope `{key}`: the pattern `{pattern}` matches no file of the code tree; fix it in {} and have the human approve the policy again", paths::POLICY_FILE));
                }
            }
        }
        out
    }
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
        declared.insert(key.to_string(), globs);
    }
    Ok(declared)
}
