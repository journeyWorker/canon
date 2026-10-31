//! What a run reports: one entry per check, and whether the level is met.

use serde::Serialize;
use serde_json::Value;

/// Whether a failure of the check fails the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Strength {
    /// A failure, or a missing fixture entry, fails the level.
    Must,
    /// A failure is an advisory, reported and never failing.
    Should,
}

/// The five results of a check (approaches/checker.md 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckResult {
    /// Exercised and held.
    Pass,
    /// Exercised and violated; for a SHOULD check, an advisory.
    Fail,
    /// The check needs a fixture entry the tool did not provide.
    Unprovided,
    /// The check does not apply: no envelope the run saw carries what it judges (`L0.next` without a `meta.next`, `L0.page` without a `meta.page`).
    Unsupported,
    /// The rule cannot be violated alone (an L1 matter: no L0 check is ever `entailed`).
    Entailed,
}

/// One check and what it found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// `L0.<name>`.
    pub id: &'static str,
    pub strength: Strength,
    pub result: CheckResult,
    /// The numbers of the contract's rules the check enforces.
    pub rules: Vec<&'static str>,
    /// Why the check failed, or which fixture entry it lacks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Things seen that the contract only recommends against (stderr output that carries no error or result). They never change the result.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<String>,
    /// The invocations the verdict rests on: argv, exit, and what the binary printed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observed: Vec<Value>,
}

/// The run: the claimed level, whether it is met, and every check.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub level: &'static str,
    pub passed: bool,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn new(level: &'static str, checks: Vec<Check>) -> Self {
        let passed = !checks.iter().any(|check| {
            check.strength == Strength::Must
                && matches!(check.result, CheckResult::Fail | CheckResult::Unprovided)
        });
        Self {
            level,
            passed,
            checks,
        }
    }

    pub fn find(&self, id: &str) -> Option<&Check> {
        self.checks.iter().find(|check| check.id == id)
    }

    fn count(&self, strength: Strength, result: CheckResult) -> usize {
        self.checks
            .iter()
            .filter(|check| check.strength == strength && check.result == result)
            .count()
    }

    /// The text of the report: one line per check, then one summary line. The text is not part of the contract.
    pub fn text(&self) -> String {
        let mut lines: Vec<String> = self.checks.iter().map(Check::line).collect();
        lines.push(self.summary());
        lines.join("\n")
    }

    /// The summary line.
    pub fn summary(&self) -> String {
        let must = |result| self.count(Strength::Must, result);
        let advisories = self.count(Strength::Should, CheckResult::Fail);
        let not_applicable = self.checks.iter().filter(|check| check.result == CheckResult::Unsupported).count();
        let verdict = if self.passed { "met" } else { "not met" };
        format!(
            "{} {verdict}: {} MUST passed, {} MUST failed, {} MUST unprovided, {not_applicable} not applicable, {advisories} SHOULD advisories",
            self.level,
            must(CheckResult::Pass),
            must(CheckResult::Fail),
            must(CheckResult::Unprovided)
        )
    }
}

impl Check {
    fn line(&self) -> String {
        let result = match self.result {
            CheckResult::Pass => "pass",
            CheckResult::Fail => "fail",
            CheckResult::Unprovided => "unprovided",
            CheckResult::Unsupported => "unsupported",
            CheckResult::Entailed => "entailed",
        };
        let strength = match self.strength {
            Strength::Must => "MUST",
            Strength::Should => "SHOULD",
        };
        let base = format!(
            "{result:<10} {strength:<6} {} (spec {})",
            self.id,
            self.rules.join(", ")
        );
        match &self.reason {
            Some(reason) => format!("{base}: {reason}"),
            None => base,
        }
    }

    /// The `id`s of the checks that fail the level, for the error message.
    pub fn blocks_level(&self) -> bool {
        self.strength == Strength::Must
            && matches!(self.result, CheckResult::Fail | CheckResult::Unprovided)
    }
}

#[cfg(test)]
mod tests {
    use super::{Check, CheckResult, Report, Strength};

    fn check(id: &'static str, strength: Strength, result: CheckResult) -> Check {
        Check {
            id,
            strength,
            result,
            rules: vec!["1"],
            reason: None,
            advisories: Vec::new(),
            observed: Vec::new(),
        }
    }

    #[test]
    fn atip_check_25_a_level_is_met_unless_a_must_check_fails_or_is_unprovided() {
        let all = |result| {
            Report::new(
                "L0",
                vec![
                    check("L0.a", Strength::Must, CheckResult::Pass),
                    check("L0.b", Strength::Must, result),
                ],
            )
        };
        assert!(all(CheckResult::Pass).passed);
        assert!(all(CheckResult::Unsupported).passed);
        assert!(all(CheckResult::Entailed).passed);
        assert!(!all(CheckResult::Fail).passed);
        assert!(
            !all(CheckResult::Unprovided).passed,
            "a tool cannot pass by withholding fixtures"
        );
        let advisory = Report::new(
            "L0",
            vec![check("L0.a", Strength::Should, CheckResult::Fail)],
        );
        assert!(advisory.passed, "a SHOULD failure never fails the level");
    }

    #[test]
    fn atip_check_05_the_text_has_a_line_per_check_and_one_summary() {
        let mut failing = check("L0.b", Strength::Must, CheckResult::Fail);
        failing.reason = Some("it broke".to_string());
        let report = Report::new(
            "L0",
            vec![check("L0.a", Strength::Should, CheckResult::Pass), failing],
        );
        let text = report.text();
        assert_eq!(text.lines().count(), 3);
        assert!(
            text.contains("fail       MUST   L0.b (spec 1): it broke"),
            "{text}"
        );
        assert!(
            text.ends_with(
                "L0 not met: 0 MUST passed, 1 MUST failed, 0 MUST unprovided, 0 not applicable, 0 SHOULD advisories"
            ),
            "{text}"
        );
    }
}
