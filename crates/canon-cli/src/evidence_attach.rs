//! `canon evidence add --artifact/--report` (experimental evidence
//! binding): bind an attestation to files the team's own runner or agent
//! already produced.
//!
//! Canon never runs a test — it cannot, across every language and every
//! agent-driven QA tool a team uses. It READS what those tools wrote:
//!
//! - `--artifact <path>` (repeatable): any file — a trace, a screenshot
//!   archive, an agent QA log, a review document — bound by sha256.
//! - `--report <junit|cucumber>:<path>` (repeatable): a machine-readable
//!   test report. Canon parses it, finds the case for this evidence's
//!   scenario, and records the case and its outcome alongside the digest.
//!
//! # Finding the case
//! With `--report-case <name>`, the case whose name equals it (or ends
//! with `::<name>`, so a Rust test is named by its function). Otherwise
//! the case whose name, classname, or (Cucumber) tag contains the
//! scenario id — dotted (`cart.add.04`) or with dots and hyphens as
//! underscores (`cart_add_04`, the form a test function can carry) — as a
//! whole token. Several matches (a Scenario Outline's examples) fold to
//! the worst outcome. No match refuses: a binding that names nothing is
//! not a binding.
//!
//! # Refusals
//! A file outside the repository, an unreadable or unparseable report,
//! or no matching case is a usage error (exit 2). A `faithful` verdict
//! backed by a report whose case failed or was skipped is refused
//! (exit 1): the report contradicts the claim.

use std::io::Read;
use std::path::{Path, PathBuf};

use canon_model::{EvidenceAttachment, EvidenceVerdict, ReportFormat, ReportOutcome};
use quick_xml::events::{BytesStart, Event};
use sha2::{Digest, Sha256};

/// The largest report canon parses into memory. Artifacts are hashed
/// streaming and have no limit.
const MAX_REPORT_BYTES: u64 = 64 * 1024 * 1024;

/// Why binding refused, graded to the `canon evidence add` exit code.
#[derive(Debug)]
pub enum BindError {
    /// The invocation is wrong (missing file, bad format, no case): 2.
    Usage(String),
    /// The report contradicts the verdict: 1.
    Contradiction(String),
}

impl BindError {
    pub fn exit_code(&self) -> i32 {
        match self {
            BindError::Usage(_) => 2,
            BindError::Contradiction(_) => 1,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            BindError::Usage(m) | BindError::Contradiction(m) => m,
        }
    }
}

/// One parsed report case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportCase {
    pub name: String,
    /// JUnit `classname`; Cucumber feature name.
    pub group: String,
    /// Cucumber scenario tags (`@` stripped); empty for JUnit.
    pub tags: Vec<String>,
    pub outcome: ReportOutcome,
}

/// Parse `--report`'s `<format>:<path>`.
pub fn parse_report_spec(value: &str) -> Result<(ReportFormat, PathBuf), String> {
    let (format, path) = value
        .split_once(':')
        .ok_or_else(|| format!("`{value}` is not <junit|cucumber>:<path>"))?;
    let format = match format {
        "junit" => ReportFormat::Junit,
        "cucumber" => ReportFormat::Cucumber,
        other => {
            return Err(format!(
                "`{other}` is not a supported report format (expected junit or cucumber)"
            ))
        }
    };
    if path.is_empty() {
        return Err(format!("`{value}` names no file"));
    }
    Ok((format, PathBuf::from(path)))
}

/// Inputs for [`bind`].
pub struct BindRequest<'a> {
    pub repo: &'a Path,
    pub artifacts: &'a [PathBuf],
    pub reports: &'a [(ReportFormat, PathBuf)],
    pub report_case: Option<&'a str>,
    /// The scenario id the default case match looks for.
    pub scenario_id: Option<&'a str>,
    pub verdict: EvidenceVerdict,
}

/// Read, hash, and (for reports) match every requested file.
pub fn bind(request: &BindRequest<'_>) -> Result<Vec<EvidenceAttachment>, BindError> {
    let repo = request
        .repo
        .canonicalize()
        .map_err(|e| BindError::Usage(format!("resolve repository root: {e}")))?;
    let mut attachments = Vec::new();

    for path in request.artifacts {
        let (relative, file) = open_in_repo(&repo, path)?;
        attachments.push(EvidenceAttachment {
            path: relative,
            sha256: sha256_stream(file, path)?,
            format: None,
            case: None,
            outcome: None,
        });
    }

    for (format, path) in request.reports {
        let (relative, file) = open_in_repo(&repo, path)?;
        let mut bytes = Vec::new();
        file.take(MAX_REPORT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| BindError::Usage(format!("read {}: {e}", path.display())))?;
        if bytes.len() as u64 > MAX_REPORT_BYTES {
            return Err(BindError::Usage(format!(
                "{} is larger than {MAX_REPORT_BYTES} bytes; bind it with --artifact instead",
                path.display()
            )));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| BindError::Usage(format!("{} is not UTF-8", path.display())))?;
        let cases = match format {
            ReportFormat::Junit => parse_junit(text),
            ReportFormat::Cucumber => parse_cucumber(text),
        }
        .map_err(|e| {
            BindError::Usage(format!(
                "{} is not a readable {} report: {e}",
                path.display(),
                format_name(*format)
            ))
        })?;
        let (case, outcome) = match_case(&cases, request.report_case, request.scenario_id)
            .map_err(|e| BindError::Usage(format!("{}: {e}", path.display())))?;
        if request.verdict == EvidenceVerdict::Faithful && outcome != ReportOutcome::Passed {
            return Err(BindError::Contradiction(format!(
                "{} records case `{case}` as {}; a faithful verdict needs a passing case (attest the verdict the report shows, or fix the test)",
                path.display(),
                outcome_name(outcome)
            )));
        }
        attachments.push(EvidenceAttachment {
            path: relative,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            format: Some(*format),
            case: Some(case),
            outcome: Some(outcome),
        });
    }
    Ok(attachments)
}

fn format_name(format: ReportFormat) -> &'static str {
    match format {
        ReportFormat::Junit => "JUnit XML",
        ReportFormat::Cucumber => "Cucumber JSON",
    }
}

fn outcome_name(outcome: ReportOutcome) -> &'static str {
    match outcome {
        ReportOutcome::Passed => "passed",
        ReportOutcome::Failed => "failed",
        ReportOutcome::Skipped => "skipped",
    }
}

/// Open `path` (relative to the CWD as given, like every other CLI
/// path) and require it to resolve inside the repository, returning its
/// repository-relative, `/`-separated path. A path outside the repo would
/// record a machine-local location no reviewer can find.
fn open_in_repo(repo: &Path, path: &Path) -> Result<(String, std::fs::File), BindError> {
    let resolved = path
        .canonicalize()
        .map_err(|e| BindError::Usage(format!("{}: {e}", path.display())))?;
    if !resolved.is_file() {
        return Err(BindError::Usage(format!(
            "{} is not a file",
            path.display()
        )));
    }
    let relative = resolved.strip_prefix(repo).map_err(|_| {
        BindError::Usage(format!(
            "{} is outside the repository; bound files are recorded by repository-relative path",
            path.display()
        ))
    })?;
    let relative = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let file = std::fs::File::open(&resolved)
        .map_err(|e| BindError::Usage(format!("open {}: {e}", path.display())))?;
    Ok((relative, file))
}

fn sha256_stream(mut file: std::fs::File, path: &Path) -> Result<String, BindError> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| BindError::Usage(format!("read {}: {e}", path.display())))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Pick the case this evidence is about (module doc) and fold multiple
/// matches to the worst outcome.
pub fn match_case(
    cases: &[ReportCase],
    explicit: Option<&str>,
    scenario_id: Option<&str>,
) -> Result<(String, ReportOutcome), String> {
    let matched: Vec<&ReportCase> = match (explicit, scenario_id) {
        (Some(name), _) => cases
            .iter()
            .filter(|c| c.name == name || c.name.ends_with(&format!("::{name}")))
            .collect(),
        (None, Some(id)) => {
            let underscored = id.replace(['.', '-'], "_");
            cases
                .iter()
                .filter(|c| {
                    c.tags.iter().any(|t| t == id)
                        || [&c.name, &c.group].iter().any(|text| {
                            contains_token(text, id) || contains_token(text, &underscored)
                        })
                })
                .collect()
        }
        (None, None) => {
            return Err(
                "no --scenario-id to match the report by; name the case with --report-case"
                    .to_string(),
            )
        }
    };
    let Some(first) = matched.first() else {
        let wanted = explicit
            .map(|n| format!("named `{n}`"))
            .unwrap_or_else(|| format!("for scenario `{}`", scenario_id.unwrap_or_default()));
        return Err(format!(
            "no case {wanted} among its {} case(s); put the scenario id in the test name or tag, or name the case with --report-case",
            cases.len()
        ));
    };
    let outcome = if matched.iter().any(|c| c.outcome == ReportOutcome::Failed) {
        ReportOutcome::Failed
    } else if matched.iter().any(|c| c.outcome == ReportOutcome::Skipped) {
        ReportOutcome::Skipped
    } else {
        ReportOutcome::Passed
    };
    let case = if matched.len() == 1 {
        first.name.clone()
    } else {
        format!("{} (+{} more)", first.name, matched.len() - 1)
    };
    Ok((case, outcome))
}

/// `needle` occurs in `haystack` with no ASCII alphanumeric directly on
/// either side, so `cart.add.04` never matches `cart.add.040`.
fn contains_token(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let before = haystack[..start].chars().next_back();
        let after = haystack[start + needle.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric())
            && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

/// Parse JUnit XML: every `<testcase>`, failed when it holds `<failure>`
/// or `<error>`, skipped when it holds `<skipped>`, passed otherwise.
pub fn parse_junit(text: &str) -> Result<Vec<ReportCase>, String> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut cases = Vec::new();
    let mut open: Option<ReportCase> = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == b"testcase" => open = Some(testcase(&e)?),
            Event::Empty(e) if e.local_name().as_ref() == b"testcase" => cases.push(testcase(&e)?),
            Event::Start(e) | Event::Empty(e) => {
                if let Some(case) = open.as_mut() {
                    match e.local_name().as_ref() {
                        b"failure" | b"error" => case.outcome = ReportOutcome::Failed,
                        b"skipped" if case.outcome != ReportOutcome::Failed => {
                            case.outcome = ReportOutcome::Skipped
                        }
                        _ => {}
                    }
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"testcase" => {
                if let Some(case) = open.take() {
                    cases.push(case);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if open.is_some() {
        return Err("unterminated <testcase>".to_string());
    }
    if cases.is_empty() {
        return Err("no <testcase> element".to_string());
    }
    Ok(cases)
}

fn testcase(e: &BytesStart<'_>) -> Result<ReportCase, String> {
    let attr = |name: &str| -> Result<String, String> {
        match e.try_get_attribute(name).map_err(|err| err.to_string())? {
            Some(a) => a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .map_err(|err| err.to_string()),
            None => Ok(String::new()),
        }
    };
    let name = attr("name")?;
    if name.is_empty() {
        return Err("a <testcase> has no name".to_string());
    }
    Ok(ReportCase {
        name,
        group: attr("classname")?,
        tags: Vec::new(),
        outcome: ReportOutcome::Passed,
    })
}

/// Parse Cucumber JSON: every scenario element; failed when any step or
/// hook (including its feature's preceding Background) failed, skipped
/// when any is skipped/pending/undefined/ambiguous or nothing ran.
pub fn parse_cucumber(text: &str) -> Result<Vec<ReportCase>, String> {
    let features: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let features = features
        .as_array()
        .ok_or("the top level is not an array of features")?;
    let mut cases = Vec::new();
    for feature in features {
        let group = feature
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let mut background: Option<ReportOutcome> = None;
        for element in feature
            .get("elements")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let outcome = element_outcome(element);
            if element.get("type").and_then(|v| v.as_str()) == Some("background") {
                background = Some(outcome);
                continue;
            }
            let outcome = background.take().map_or(outcome, |b| worse(b, outcome));
            let name = element
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let tags = element
                .get("tags")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .map(|t| t.trim_start_matches('@').to_string())
                .collect();
            cases.push(ReportCase {
                name,
                group: group.clone(),
                tags,
                outcome,
            });
        }
    }
    if cases.is_empty() {
        return Err("no scenario element".to_string());
    }
    Ok(cases)
}

fn element_outcome(element: &serde_json::Value) -> ReportOutcome {
    let mut statuses = Vec::new();
    for key in ["before", "steps", "after"] {
        for item in element
            .get(key)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            statuses.push(
                item.pointer("/result/status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("undefined")
                    .to_string(),
            );
        }
    }
    if statuses.iter().any(|s| s == "failed") {
        ReportOutcome::Failed
    } else if statuses.is_empty() || statuses.iter().any(|s| s != "passed") {
        ReportOutcome::Skipped
    } else {
        ReportOutcome::Passed
    }
}

fn worse(a: ReportOutcome, b: ReportOutcome) -> ReportOutcome {
    let rank = |o| match o {
        ReportOutcome::Passed => 0,
        ReportOutcome::Skipped => 1,
        ReportOutcome::Failed => 2,
    };
    if rank(a) >= rank(b) {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const JUNIT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="canon-gate">
    <testcase classname="canon-gate" name="spec_coverage::tests::cart_add_04_refuses_out_of_stock"/>
    <testcase classname="cart" name="cart.add.01 adds &amp; counts"></testcase>
    <testcase classname="cart" name="cart.add.010 later"><failure message="boom"/></testcase>
    <testcase classname="cart" name="cart.add.02 flaky"><skipped/></testcase>
    <testcase classname="cart" name="cart.add.03 broken"><system-out>x</system-out><error type="E"/></testcase>
  </testsuite>
</testsuites>"#;

    #[test]
    fn junit_outcomes_and_escaped_names_parse() {
        let cases = parse_junit(JUNIT).unwrap();
        let outcomes: Vec<(&str, ReportOutcome)> =
            cases.iter().map(|c| (c.name.as_str(), c.outcome)).collect();
        assert_eq!(
            outcomes,
            vec![
                (
                    "spec_coverage::tests::cart_add_04_refuses_out_of_stock",
                    ReportOutcome::Passed
                ),
                ("cart.add.01 adds & counts", ReportOutcome::Passed),
                ("cart.add.010 later", ReportOutcome::Failed),
                ("cart.add.02 flaky", ReportOutcome::Skipped),
                ("cart.add.03 broken", ReportOutcome::Failed),
            ]
        );
        assert!(
            parse_junit("<testsuites/>").is_err(),
            "a report with no case binds nothing"
        );
        assert!(parse_junit("<testsuite><testcase name=\"x\">").is_err());
    }

    /// The id must match as a whole token (`cart.add.01` is not
    /// `cart.add.010`), and the underscored form finds a Rust test fn.
    #[test]
    fn scenario_ids_match_whole_tokens_in_either_form() {
        let cases = parse_junit(JUNIT).unwrap();
        assert_eq!(
            match_case(&cases, None, Some("cart.add.01")).unwrap(),
            (
                "cart.add.01 adds & counts".to_string(),
                ReportOutcome::Passed
            )
        );
        assert_eq!(
            match_case(&cases, None, Some("cart.add.04")).unwrap().1,
            ReportOutcome::Passed
        );
        assert!(match_case(&cases, None, Some("cart.add.09"))
            .unwrap_err()
            .contains("5 case(s)"));
        assert_eq!(
            match_case(&cases, Some("cart_add_04_refuses_out_of_stock"), None)
                .unwrap()
                .0,
            "spec_coverage::tests::cart_add_04_refuses_out_of_stock",
            "--report-case names a Rust test by its function"
        );
        assert!(match_case(&cases, None, None).is_err());
    }

    const CUCUMBER: &str = r#"[
      {"name": "Cart", "elements": [
        {"type": "background", "name": "", "steps": [{"result": {"status": "failed"}}]},
        {"type": "scenario", "name": "Adding", "tags": [{"name": "@cart.add.01"}], "steps": [{"result": {"status": "passed"}}]},
        {"type": "scenario", "name": "Refusing", "tags": [{"name": "@cart.add.02"}],
         "steps": [{"result": {"status": "passed"}}], "after": [{"result": {"status": "passed"}}]},
        {"type": "scenario", "name": "Pending", "tags": [{"name": "@cart.add.03"}], "steps": [{"result": {"status": "pending"}}]},
        {"type": "scenario", "name": "Outline", "tags": [{"name": "@cart.add.04"}], "steps": [{"result": {"status": "passed"}}]},
        {"type": "scenario", "name": "Outline", "tags": [{"name": "@cart.add.04"}], "steps": [{"result": {"status": "failed"}}]}
      ]}
    ]"#;

    /// Tags match exactly; a failed Background fails the scenario after
    /// it; a pending step is not a pass; an Outline folds to its worst.
    #[test]
    fn cucumber_tags_backgrounds_and_outlines() {
        let cases = parse_cucumber(CUCUMBER).unwrap();
        assert_eq!(
            match_case(&cases, None, Some("cart.add.01")).unwrap().1,
            ReportOutcome::Failed,
            "the failed Background belongs to this scenario"
        );
        assert_eq!(
            match_case(&cases, None, Some("cart.add.02")).unwrap().1,
            ReportOutcome::Passed
        );
        assert_eq!(
            match_case(&cases, None, Some("cart.add.03")).unwrap().1,
            ReportOutcome::Skipped
        );
        assert_eq!(
            match_case(&cases, None, Some("cart.add.04")).unwrap(),
            ("Outline (+1 more)".to_string(), ReportOutcome::Failed)
        );
        assert!(parse_cucumber("{}").is_err());
    }

    fn request<'a>(
        repo: &'a Path,
        artifacts: &'a [PathBuf],
        reports: &'a [(ReportFormat, PathBuf)],
        verdict: EvidenceVerdict,
    ) -> BindRequest<'a> {
        BindRequest {
            repo,
            artifacts,
            reports,
            report_case: None,
            scenario_id: Some("cart.add.01"),
            verdict,
        }
    }

    #[test]
    fn bind_records_digests_and_refuses_contradictions_and_outside_files() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("trace.zip"), b"trace-bytes").unwrap();
        std::fs::write(repo.path().join("junit.xml"), JUNIT).unwrap();

        let artifacts = vec![repo.path().join("trace.zip")];
        let reports = vec![(ReportFormat::Junit, repo.path().join("junit.xml"))];
        let bound = bind(&request(
            repo.path(),
            &artifacts,
            &reports,
            EvidenceVerdict::Faithful,
        ))
        .unwrap();
        assert_eq!(bound[0].path, "trace.zip");
        assert_eq!(
            bound[0].sha256,
            format!("{:x}", Sha256::digest(b"trace-bytes"))
        );
        assert_eq!(bound[1].outcome, Some(ReportOutcome::Passed));
        assert_eq!(
            (
                bound[1].path.as_str(),
                bound[1].format,
                bound[1].case.as_deref()
            ),
            (
                "junit.xml",
                Some(ReportFormat::Junit),
                Some("cart.add.01 adds & counts")
            )
        );
        assert_eq!(
            bound[1].sha256,
            format!("{:x}", Sha256::digest(JUNIT.as_bytes()))
        );

        let failing = BindRequest {
            scenario_id: Some("cart.add.03"),
            ..request(repo.path(), &[], &reports, EvidenceVerdict::Faithful)
        };
        assert!(
            matches!(bind(&failing), Err(BindError::Contradiction(_))),
            "a faithful claim over a failed case is refused"
        );
        let honest = BindRequest {
            scenario_id: Some("cart.add.03"),
            ..request(repo.path(), &[], &reports, EvidenceVerdict::Divergent)
        };
        assert_eq!(
            bind(&honest).unwrap()[0].outcome,
            Some(ReportOutcome::Failed),
            "a divergent verdict may cite the failing case"
        );

        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("x.log"), b"x").unwrap();
        let elsewhere = vec![outside.path().join("x.log")];
        let err = bind(&request(
            repo.path(),
            &elsewhere,
            &[],
            EvidenceVerdict::Faithful,
        ))
        .unwrap_err();
        assert!(
            matches!(&err, BindError::Usage(m) if m.contains("outside the repository")),
            "{err:?}"
        );
    }

    #[test]
    fn report_spec_parses_format_and_path() {
        assert_eq!(
            parse_report_spec("junit:target/nextest/junit.xml").unwrap(),
            (
                ReportFormat::Junit,
                PathBuf::from("target/nextest/junit.xml")
            )
        );
        assert!(parse_report_spec("tap:out.tap").is_err());
        assert!(parse_report_spec("junit:").is_err());
        assert!(parse_report_spec("junit.xml").is_err());
    }
}
