//! Starter policy levels (0.16 D3, issue #23): the three `spec_coverage`
//! shapes `canon init --policy light|standard|strict` writes, and how
//! `canon status` names the level a `.canon/policy.yaml` is at.
//!
//! - `light`: `require_evidence: true`.
//! - `standard`: light plus `require_cases: [failure]`.
//! - `strict` (the default): standard plus `require_review: {}`.
//!
//! The starter writes the level as its first line,
//! `# canon policy level: <level>`. [`detect`] reads that line when the
//! leading comment block carries one naming a known level; otherwise it
//! infers the level from the resolved `spec_coverage` section, and a
//! section that matches no level exactly (an extra `scope`, a
//! `require_review` with non-default settings, `require_audit`, …) is
//! `custom`.

use std::path::Path;

use canon_gate::{PolicyDiagnostic, PolicyResolution, RequireReview, SpecCoverage};
use canon_model::paths;

/// A starter policy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PolicyLevel {
    /// Evidence on every scenario.
    Light,
    /// Light, plus a `@case:failure` scenario per feature surface.
    Standard,
    /// Standard, plus independent review.
    Strict,
}

impl PolicyLevel {
    pub const ALL: [PolicyLevel; 3] = [PolicyLevel::Light, PolicyLevel::Standard, PolicyLevel::Strict];

    pub fn as_str(self) -> &'static str {
        match self {
            PolicyLevel::Light => "light",
            PolicyLevel::Standard => "standard",
            PolicyLevel::Strict => "strict",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == name)
    }
}

/// The level line's prefix, up to the level name.
const LEVEL_LINE_PREFIX: &str = "# canon policy level: ";

const HEADER: &str = "\
# .canon/policy.yaml -- scaffolded by `canon init`.
#
# This file is what `canon gate check` grades the work against. A human
# should review and approve it, and every later change to it, with
# `canon policy approve --by <who>`: the agent doing the work should not
# write the rules it is graded by.
spec_coverage:
  # Every scenario in the spec corpus needs evidence (`canon evidence
  # add`). Add `scope: [building, verifying]` to check only scenarios
  # whose subject is in one of those statuses.
  require_evidence: true
";

const CASES_DOC: &str = "  # Every feature surface (`<area>.<surface>`) needs at least one
  # `@case:failure` scenario, so no feature is specified by its golden
  # path alone.
";
const CASES_KEY: &str = "  require_cases: [failure]\n";

const REVIEW_DOC: &str = "  # Every scenario of a subject in `verifying` or `shipped` needs a
  # review (`canon review add`) by an actor other than its evidence
  # author, and an open blocker finding on an adopted change blocks.
";
const REVIEW_KEY: &str = "  require_review: {}\n";

const RAISE: &str = "  # To raise the level, uncomment what the next level adds below and
  # change the level line at the top of this file; a human approves the
  # edit with `canon policy approve --by <who>`.
";

/// Comment every line of `text` out (`  # ` after the indent).
fn commented(text: &str) -> String {
    text.lines().map(|line| format!("  # {}\n", line.trim_start())).collect()
}

/// The starter `.canon/policy.yaml` for `level`. Every level parses with
/// zero diagnostics through `canon_gate::PolicyResolution::resolve`, and
/// [`detect`] infers `level` from its sections alone.
pub fn starter_policy(level: PolicyLevel) -> String {
    let mut out = format!("{LEVEL_LINE_PREFIX}{}\n{HEADER}", level.as_str());
    match level {
        PolicyLevel::Light => {
            out.push_str(RAISE);
            out.push_str("  #\n  # standard:\n");
            out.push_str(CASES_DOC);
            out.push_str(&commented(CASES_KEY));
            out.push_str("  #\n  # strict, on top of standard:\n");
            out.push_str(REVIEW_DOC);
            out.push_str(&commented(REVIEW_KEY));
        }
        PolicyLevel::Standard => {
            out.push_str(CASES_DOC);
            out.push_str(CASES_KEY);
            out.push_str(RAISE);
            out.push_str("  #\n  # strict:\n");
            out.push_str(REVIEW_DOC);
            out.push_str(&commented(REVIEW_KEY));
        }
        PolicyLevel::Strict => {
            out.push_str(CASES_DOC);
            out.push_str(CASES_KEY);
            out.push_str(REVIEW_DOC);
            out.push_str(REVIEW_KEY);
        }
    }
    out
}

/// The level a `# canon policy level: <level>` line in `text`'s leading
/// comment block names, when it names a known one.
fn level_from_comment(text: &str) -> Option<PolicyLevel> {
    text.lines()
        .map(str::trim)
        .take_while(|line| line.is_empty() || line.starts_with('#'))
        .find_map(|line| line.strip_prefix(LEVEL_LINE_PREFIX))
        .and_then(|name| PolicyLevel::parse(name.trim()))
}

/// The level `spec_coverage` matches exactly, if any.
fn level_from_sections(policy: &PolicyResolution) -> Option<PolicyLevel> {
    let Some(SpecCoverage::Active { require_evidence: true, scope, exclude_lanes, require_cases, require_review, require_audit: None }) = &policy.spec_coverage
    else {
        return None;
    };
    if !scope.is_empty() || !exclude_lanes.is_empty() {
        return None;
    }
    let default_review = RequireReview { scope: RequireReview::DEFAULT_SCOPE.to_vec(), distinct_actor: true, block_on_findings: true };
    match (require_cases.as_slice(), require_review) {
        ([], None) => Some(PolicyLevel::Light),
        ([case], None) if case == "failure" => Some(PolicyLevel::Standard),
        ([case], Some(review)) if case == "failure" && *review == default_review => Some(PolicyLevel::Strict),
        _ => None,
    }
}

/// The level `canon status` reports for `repo`'s policy: `None` when
/// `.canon/policy.yaml` is absent; otherwise the level line's level, else
/// the level the sections match, else `custom`.
pub fn detect(repo: &Path, policy: &PolicyResolution) -> Option<&'static str> {
    if policy.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::Missing { .. })) {
        return None;
    }
    let text = std::fs::read_to_string(repo.join(paths::POLICY_FILE)).unwrap_or_default();
    let level = level_from_comment(&text).or_else(|| level_from_sections(policy));
    Some(level.map_or("custom", PolicyLevel::as_str))
}

/// What `canon status` adds after a level to say how to raise it.
pub fn raise_hint(level: &str) -> Option<&'static str> {
    match level {
        "light" => Some(
            "raise it by uncommenting `require_cases: [failure]` (standard) and `require_review: {}` (strict) in .canon/policy.yaml and changing its level line; a human approves the edit with `canon policy approve --by <who>`",
        ),
        "standard" => Some(
            "raise it by uncommenting `require_review: {}` (strict) in .canon/policy.yaml and changing its level line; a human approves the edit with `canon policy approve --by <who>`",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(text: &str) -> (tempfile::TempDir, PolicyResolution) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(paths::CANON_DIR)).unwrap();
        std::fs::write(dir.path().join(paths::POLICY_FILE), text).unwrap();
        let policy = PolicyResolution::resolve(dir.path(), &canon_policy::SchemaRegistry::load());
        (dir, policy)
    }

    #[test]
    fn every_starter_level_loads_clean_and_is_inferred_from_its_sections_alone() {
        for level in PolicyLevel::ALL {
            let text = starter_policy(level);
            assert!(text.starts_with(&format!("# canon policy level: {}\n", level.as_str())), "{text}");
            let (dir, policy) = resolve(&text);
            assert!(policy.diagnostics.is_empty(), "{level:?}: {:?}", policy.diagnostics);
            assert_eq!(level_from_sections(&policy), Some(level), "{text}");
            assert_eq!(detect(dir.path(), &policy), Some(level.as_str()));
        }
    }

    #[test]
    fn the_level_line_wins_and_an_unknown_level_line_falls_back_to_the_sections() {
        let body = starter_policy(PolicyLevel::Strict).replacen("# canon policy level: strict\n", "", 1);
        let (dir, policy) = resolve(&format!("# canon policy level: light\n{body}"));
        assert_eq!(detect(dir.path(), &policy), Some("light"));
        let (dir, policy) = resolve(&format!("# canon policy level: extreme\n{body}"));
        assert_eq!(detect(dir.path(), &policy), Some("strict"));
        let (dir, policy) = resolve(&body);
        assert_eq!(detect(dir.path(), &policy), Some("strict"));
    }

    #[test]
    fn a_policy_matching_no_level_is_custom_and_an_absent_one_has_no_level() {
        for text in [
            "spec_coverage:\n  require_evidence: true\n  scope: [building]\n",
            "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n  require_review: {distinct_actor: false}\n",
            "spec_coverage:\n  require_evidence: true\n  require_audit: {rate: 0.5}\n",
            "spec_coverage:\n  require_evidence: false\n",
            "staleness:\n  max_commits_behind: 3\n",
        ] {
            let (dir, policy) = resolve(text);
            assert_eq!(detect(dir.path(), &policy), Some("custom"), "{text}");
        }
        let dir = tempfile::tempdir().unwrap();
        let policy = PolicyResolution::resolve(dir.path(), &canon_policy::SchemaRegistry::load());
        assert_eq!(detect(dir.path(), &policy), None);
    }
}
