//! `canon evidence add --all-scenarios --from-ci` (0.40): the CI environment as a record's `ci` claim.
//!
//! The one supported provider is GitHub Actions. `--from-ci` reads its variables when it is given and never
//! recognises CI by itself, so a record written by a person in a CI shell stays a person's. The values are
//! what the environment says: a claim a person can read and check at the run's address, not proof (nothing
//! signs it). Another provider is one more row of [`REQUIRED`] and the URL built in [`CiEnv::into_run`].

use canon_model::{CiReport, CiRun};

use crate::cli_error::{CliError, CliErrorExt as _};

/// The provider name written to `ci.provider`.
pub const PROVIDER: &str = "github-actions";

/// The variables a record needs, in the order they are checked: the name, and what it holds.
const REQUIRED: [(&str, &str); 6] = [
    ("GITHUB_ACTIONS", "`true` in a GitHub Actions job"),
    ("GITHUB_RUN_ID", "the run number"),
    ("GITHUB_RUN_ATTEMPT", "the attempt of the run, a whole number"),
    ("GITHUB_SERVER_URL", "the server, `https://github.com`"),
    ("GITHUB_REPOSITORY", "`<owner>/<repository>`"),
    ("GITHUB_SHA", "the commit the run is on"),
];

/// What the environment said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiEnv {
    run: String,
    attempt: u64,
    server: String,
    repository: String,
    sha: String,
    workflow: Option<String>,
    job: Option<String>,
}

/// Read the CI environment through `get` (the process environment, or a table in a test). Refuses with
/// `CI_ENV_REQUIRED`, naming every variable that is not there: `GITHUB_ACTIONS` that is not `true`, an empty
/// variable, an attempt that is not a whole number of 1 or more.
pub fn read(get: impl Fn(&str) -> Option<String>) -> Result<CiEnv, CliError> {
    let value = |name: &str| get(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut problems: Vec<String> = Vec::new();
    match value("GITHUB_ACTIONS") {
        Some(v) if v == "true" => {}
        Some(v) => problems.push(format!("GITHUB_ACTIONS is `{v}`, not `true`")),
        None => problems.push("GITHUB_ACTIONS is not set".to_string()),
    }
    for (name, holds) in &REQUIRED[1..] {
        if value(name).is_none() {
            problems.push(format!("{name} ({holds}) is empty or not set"));
        }
    }
    let attempt = value("GITHUB_RUN_ATTEMPT").and_then(|v| v.parse::<u64>().ok().filter(|n| *n >= 1));
    if attempt.is_none() && value("GITHUB_RUN_ATTEMPT").is_some() {
        problems.push("GITHUB_RUN_ATTEMPT is not a whole number of 1 or more".to_string());
    }
    let (Some(run), Some(attempt), Some(server), Some(repository), Some(sha)) =
        (value("GITHUB_RUN_ID"), attempt, value("GITHUB_SERVER_URL"), value("GITHUB_REPOSITORY"), value("GITHUB_SHA"))
    else {
        return Err(missing(&problems));
    };
    if !problems.is_empty() {
        return Err(missing(&problems));
    }
    Ok(CiEnv { run, attempt, server, repository, sha, workflow: value("GITHUB_WORKFLOW"), job: value("GITHUB_JOB") })
}

fn missing(problems: &[String]) -> CliError {
    let first = problems.first().and_then(|p| p.split([' ', '`']).next()).unwrap_or("GITHUB_ACTIONS").to_string();
    CliError::refused("CI_ENV_REQUIRED", format!("--from-ci reads the GitHub Actions environment, and it is not there: {}; no record is written", problems.join("; ")))
        .field(first)
        .hint("run this command in a GitHub Actions job (it sets these variables); outside CI, record the batch without --from-ci")
}

impl CiEnv {
    /// The record's `ci` block, with the case counts of `reports`. The URL is the run's address,
    /// `<server>/<repository>/actions/runs/<run>/attempts/<attempt>`.
    pub fn into_run(self, reports: Vec<CiReport>) -> CiRun {
        let url = format!("{}/{}/actions/runs/{}/attempts/{}", self.server.trim_end_matches('/'), self.repository, self.run, self.attempt);
        CiRun { provider: PROVIDER.to_string(), run: self.run, attempt: self.attempt, url, repository: self.repository, sha: self.sha, workflow: self.workflow, job: self.job, reports }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use canon_model::ReportFormat;

    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let table: BTreeMap<String, String> = pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
        move |name| table.get(name).cloned()
    }

    const FULL: [(&str, &str); 6] = [
        ("GITHUB_ACTIONS", "true"),
        ("GITHUB_RUN_ID", "9001"),
        ("GITHUB_RUN_ATTEMPT", "2"),
        ("GITHUB_SERVER_URL", "https://github.com/"),
        ("GITHUB_REPOSITORY", "owner/repo"),
        ("GITHUB_SHA", "0123456789012345678901234567890123456789"),
    ];

    #[test]
    fn the_run_names_the_provider_the_address_and_the_optional_fields_only_when_set() {
        let report = CiReport { path: "r.xml".into(), sha256: "a".repeat(64), format: ReportFormat::Junit, passed: 3, failed: 1, skipped: 2 };
        let run = read(env(&FULL)).unwrap().into_run(vec![report.clone()]);
        assert_eq!((run.provider.as_str(), run.run.as_str(), run.attempt), ("github-actions", "9001", 2));
        assert_eq!(run.url, "https://github.com/owner/repo/actions/runs/9001/attempts/2");
        assert_eq!((run.workflow.as_deref(), run.job.as_deref()), (None, None));
        assert_eq!(run.reports, vec![report]);
        assert_eq!(run.cases_run(), 4, "passed + failed; a skipped case did not run");

        let mut with = FULL.to_vec();
        with.extend([("GITHUB_WORKFLOW", "test"), ("GITHUB_JOB", "offline")]);
        let run = read(env(&with)).unwrap().into_run(Vec::new());
        assert_eq!((run.workflow.as_deref(), run.job.as_deref()), (Some("test"), Some("offline")));
    }

    #[test]
    fn every_variable_that_is_not_there_is_named() {
        for (name, _) in FULL {
            let rest: Vec<(&str, &str)> = FULL.iter().filter(|(k, _)| *k != name).copied().collect();
            let error = read(env(&rest)).unwrap_err();
            assert_eq!(error.code, "CI_ENV_REQUIRED");
            assert!(error.message.contains(name), "{name}: {}", error.message);
        }
        let empty = [("GITHUB_ACTIONS", "true"), ("GITHUB_RUN_ID", ""), ("GITHUB_RUN_ATTEMPT", "1"), ("GITHUB_SERVER_URL", "x"), ("GITHUB_REPOSITORY", " "), ("GITHUB_SHA", "a")];
        let message = read(env(&empty)).unwrap_err().message;
        assert!(message.contains("GITHUB_RUN_ID") && message.contains("GITHUB_REPOSITORY"), "{message}");
    }

    #[test]
    fn github_actions_must_be_true_and_the_attempt_a_positive_whole_number() {
        let false_ci: Vec<(&str, &str)> = FULL.iter().map(|&(k, v)| if k == "GITHUB_ACTIONS" { (k, "false") } else { (k, v) }).collect();
        assert!(read(env(&false_ci)).unwrap_err().message.contains("GITHUB_ACTIONS is `false`"));
        for bad in ["0", "two", "-1", "1.5"] {
            let attempt: Vec<(&str, &str)> = FULL.iter().map(|&(k, v)| if k == "GITHUB_RUN_ATTEMPT" { (k, bad) } else { (k, v) }).collect();
            assert!(read(env(&attempt)).unwrap_err().message.contains("GITHUB_RUN_ATTEMPT is not a whole number"), "{bad}");
        }
    }
}
