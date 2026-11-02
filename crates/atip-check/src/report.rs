//! What a run reports: one entry per check, and whether the level is met.

use serde::Serialize;
use serde_json::Value;

use crate::claim::Claim;

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

/// One thing a check found, as `data.checks[].findings[]` lists it (approaches/checker.md 2). `kind` is the name of the
/// condition the check judges, or one of `missing-fixture-entry`, `refused-precondition`, `ran-over-time-limit`; it is
/// public spelling of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detail {
    pub kind: &'static str,
    /// What `reason` is made of, whole.
    pub message: String,
    /// The argv of the invocation the finding comes from, when one invocation decided it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
    /// The registered error code the finding rests on (`refused-precondition`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl Detail {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            argv: None,
            code: None,
        }
    }

    #[must_use]
    pub fn with_argv(mut self, argv: &[String]) -> Self {
        self.argv = Some(argv.to_vec());
        self
    }

    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
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
    /// Everything a `fail` or `unprovided` check found, whole and in order: `reason` is the first three messages, cut.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Detail>,
    /// Whether `reason` leaves something of `findings` out (text only: it is why the line says where the rest is).
    #[serde(skip)]
    pub cut: bool,
    /// Facts the check found that do not change its result: the rules an `entailed` check rests on, a verified Unicode-version difference between the two regular-expression engines.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// The write and destructive commands of the tool's documents that no fixture entry covers, named by `L1.constraints`, `L1.domains` and `L1.validators`: the checker never runs them, so they are outside what those checks probe (contract 1.1.2). Information only: it never changes the result.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unlisted: Vec<String>,
    /// The commands the fixture names under `notRun`: the checker never ran them, so this check, which runs commands, did not run them either. Information only: it never changes the result, and a command left out of the run is always on the report.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_run: Vec<String>,
    /// The runs of those commands that this check did not make, each `<command>: <what>: notRun`: runs that would succeed and so never end.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
    /// The invocations the verdict rests on: argv, exit, and what the binary printed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observed: Vec<Value>,
}

/// The run: the claimed level, whether it is met, and every check.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub level: &'static str,
    /// The contract minor the tool claimed and the checks judged.
    #[serde(rename = "contractVersion")]
    pub contract_version: &'static str,
    pub passed: bool,
    /// The programs the fixture's `prepare` ran before `setup`, each as its argv: what was made outside the tool, for whoever reviews the claim.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prepare: Vec<Vec<String>>,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn new(level: &'static str, claim: Claim, checks: Vec<Check>) -> Self {
        let passed = !checks.iter().any(|check| {
            check.strength == Strength::Must
                && matches!(check.result, CheckResult::Fail | CheckResult::Unprovided)
        });
        Self {
            level,
            contract_version: claim.as_str(),
            passed,
            prepare: Vec::new(),
            checks,
        }
    }

    /// The same report naming the `prepare` steps that built the state it ran against.
    #[must_use]
    pub fn with_prepare(mut self, prepare: Vec<Vec<String>>) -> Self {
        self.prepare = prepare;
        self
    }

    /// How many SHOULD checks failed.
    pub fn advisories(&self) -> usize {
        self.count(Strength::Should, CheckResult::Fail)
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
        let mut lines: Vec<String> = self.prepare.iter().map(|step| format!("prepare: {}", step.join(" "))).collect();
        lines.extend(self.checks.iter().map(Check::line));
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
        let base = if self.notes.is_empty() {
            base
        } else {
            format!("{base} [{}]", self.notes.join("; "))
        };
        let base = if self.unlisted.is_empty() {
            base
        } else {
            format!("{base} (unlisted writes, not probed: {})", self.unlisted.join(", "))
        };
        let base = if self.not_run.is_empty() {
            base
        } else {
            format!("{base} (not run: {})", self.not_run.join(", "))
        };
        match &self.reason {
            Some(reason) if self.cut => format!("{base}: {reason} (all in --json findings)"),
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
    use crate::claim::Claim;

    fn check(id: &'static str, strength: Strength, result: CheckResult) -> Check {
        Check {
            id,
            strength,
            result,
            rules: vec!["1"],
            reason: None,
            advisories: Vec::new(),
            findings: Vec::new(),
            cut: false,
            notes: Vec::new(),
            unlisted: Vec::new(),
            not_run: Vec::new(),
            skipped: Vec::new(),
            observed: Vec::new(),
        }
    }

    #[test]
    fn atip_check_25_a_level_is_met_unless_a_must_check_fails_or_is_unprovided() {
        let all = |result| {
            Report::new(
                "L0",
                Claim::V1_1,
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
            Claim::V1_1,
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
            Claim::V1_1,
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

    #[test]
    fn atip_check_82_a_cut_reason_says_in_text_where_the_rest_is() {
        let mut failing = check("L0.b", Strength::Must, CheckResult::Fail);
        failing.reason = Some("a; b; c; and 2 more".to_string());
        failing.cut = true;
        let report = Report::new("L0", Claim::V1_1, vec![failing]);
        assert!(report.text().contains("c; and 2 more (all in --json findings)\n"), "{}", report.text());
        let mut whole = check("L0.b", Strength::Must, CheckResult::Fail);
        whole.reason = Some("a; b".to_string());
        let report = Report::new("L0", Claim::V1_1, vec![whole]);
        assert!(!report.text().contains("findings"), "{}", report.text());
        assert!(!serde_json::to_string(&report.checks[0]).expect("json").contains("cut"));
    }
}
