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
//! # Finding the cases
//! Every case whose name, classname, or (Cucumber) tag contains the
//! scenario id — dotted (`cart.add.04`) or with dots and hyphens as
//! underscores (`cart_add_04`, the form a test function can carry) — as a
//! whole token, PLUS every case named by a `--report-case <name>`
//! (repeatable; the case whose name equals it, or ends with `::<name>`,
//! so a Rust test is named by its function). Each bound case is its own
//! attachment, carrying that case's name and outcome — a Scenario
//! Outline's examples, or three tests for one scenario, are all recorded,
//! never folded into one. Naming a case cannot hide another: the
//! id-carrying cases are bound whether or not `--report-case` is given.
//! No match at all refuses, and so does a `--report-case` that names no
//! case: a binding that names nothing is not a binding.
//!
//! # Storing the bytes (0.14 D4)
//! [`bind`] only reads. It returns each bound file's resolved path and
//! digest alongside the attachments, and `canon evidence add` copies the
//! bytes into the artifact store (`canon_gate::artifact_store`) after
//! every refusal has passed, just before the record is staged. A file
//! larger than the size limit is refused here (exit 2), naming the
//! override flag.
//!
//! # Refusals
//! A file outside the repository, an unreadable or unparseable report,
//! a file over the size limit, or no matching case is a usage error
//! (exit 2). A `faithful` verdict backed by a report in which ANY bound
//! case failed or was skipped is refused (exit 1): the report
//! contradicts the claim.

use std::io::Read;
use std::path::{Path, PathBuf};

use canon_model::{EvidenceAttachment, EvidenceVerdict, ReportFormat, ReportOutcome};
use quick_xml::events::{BytesStart, Event};
use sha2::{Digest, Sha256};

/// The largest report canon parses into memory, whatever the store's
/// size limit allows.
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
    /// `--report-case`, repeatable: cases bound IN ADDITION to the ones
    /// carrying the scenario id.
    pub report_cases: &'a [String],
    /// The scenario id the default case match looks for.
    pub scenario_id: Option<&'a str>,
    pub verdict: EvidenceVerdict,
    /// The store's size limit for one file, in bytes.
    pub max_bytes: u64,
}

/// One file [`bind`] read: where it is now and the digest recorded for
/// it, so the caller can store exactly those bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundFile {
    pub resolved: PathBuf,
    pub sha256: String,
}

/// What [`bind`] produced: the record's attachments and the files
/// behind them, one entry per distinct digest.
#[derive(Debug, Default)]
pub struct Bound {
    pub attachments: Vec<EvidenceAttachment>,
    pub files: Vec<BoundFile>,
}

impl Bound {
    fn add_file(&mut self, resolved: PathBuf, sha256: &str) {
        if !self.files.iter().any(|f| f.sha256 == sha256) {
            self.files.push(BoundFile { resolved, sha256: sha256.to_string() });
        }
    }
}

/// Read, hash, and (for reports) match every requested file. Writes
/// nothing (module doc).
pub fn bind(request: &BindRequest<'_>) -> Result<Bound, BindError> {
    let repo = request
        .repo
        .canonicalize()
        .map_err(|e| BindError::Usage(format!("resolve repository root: {e}")))?;
    let mut bound = Bound::default();

    for path in request.artifacts {
        let (relative, resolved, file) = open_in_repo(&repo, path, request.max_bytes)?;
        let sha256 = canon_gate::artifact_store::sha256_reader(file)
            .map_err(|e| BindError::Usage(format!("read {}: {e}", path.display())))?;
        bound.add_file(resolved, &sha256);
        bound.attachments.push(EvidenceAttachment {
            path: relative,
            sha256,
            format: None,
            case: None,
            outcome: None,
        });
    }

    let mut named_unmatched: Vec<&str> = request.report_cases.iter().map(String::as_str).collect();
    for (format, path) in request.reports {
        let (relative, resolved, file) = open_in_repo(&repo, path, request.max_bytes)?;
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
        let matched = match_cases(&cases, request.report_cases, request.scenario_id)
            .map_err(|e| BindError::Usage(format!("{}: {e}", path.display())))?;
        named_unmatched.retain(|name| !matched.iter().any(|c| names_case(c, name)));
        if request.verdict == EvidenceVerdict::Faithful {
            let contradicting: Vec<String> = matched
                .iter()
                .filter(|c| c.outcome != ReportOutcome::Passed)
                .map(|c| format!("`{}` as {}", c.name, outcome_name(c.outcome)))
                .collect();
            if !contradicting.is_empty() {
                return Err(BindError::Contradiction(format!(
                    "{} records case {}; a faithful verdict needs every bound case to pass (attest the verdict the report shows, or fix the test)",
                    path.display(),
                    contradicting.join(", ")
                )));
            }
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        bound.add_file(resolved, &sha256);
        for case in matched {
            bound.attachments.push(EvidenceAttachment {
                path: relative.clone(),
                sha256: sha256.clone(),
                format: Some(*format),
                case: Some(case.name.clone()),
                outcome: Some(case.outcome),
            });
        }
    }
    if let Some(name) = named_unmatched.first() {
        return Err(BindError::Usage(format!(
            "--report-case `{name}` names no case in any --report; a binding that names nothing is not a binding"
        )));
    }
    Ok(bound)
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
/// path) and require it to resolve inside the repository and fit the
/// store's size limit, returning its repository-relative, `/`-separated
/// path and its resolved location. A path outside the repo would record
/// a machine-local location no reviewer can find.
fn open_in_repo(repo: &Path, path: &Path, max_bytes: u64) -> Result<(String, PathBuf, std::fs::File), BindError> {
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
    let size = file
        .metadata()
        .map_err(|e| BindError::Usage(format!("stat {}: {e}", path.display())))?
        .len();
    if size > max_bytes {
        return Err(BindError::Usage(format!(
            "{} is {size} bytes, over the artifact store's limit of {max_bytes} bytes; raise it with {} <MiB> if the file belongs in git",
            path.display(),
            canon_gate::MAX_ARTIFACT_FLAG
        )));
    }
    Ok((relative, resolved, file))
}

/// Whether `--report-case <name>` names `case`.
fn names_case(case: &ReportCase, name: &str) -> bool {
    case.name == name || case.name.ends_with(&format!("::{name}"))
}

/// Every case this evidence binds (module doc): the ones carrying the
/// scenario id, plus the ones `explicit` names, each once, in report
/// order. Empty is an error.
pub fn match_cases<'c>(
    cases: &'c [ReportCase],
    explicit: &[String],
    scenario_id: Option<&str>,
) -> Result<Vec<&'c ReportCase>, String> {
    if explicit.is_empty() && scenario_id.is_none() {
        return Err(
            "no --scenario-id to match the report by; name the case with --report-case"
                .to_string(),
        );
    }
    let underscored = scenario_id.map(|id| id.replace(['.', '-'], "_"));
    let carries_id = |c: &ReportCase| {
        let (Some(id), Some(underscored)) = (scenario_id, underscored.as_deref()) else {
            return false;
        };
        c.tags.iter().any(|t| t == id)
            || [&c.name, &c.group]
                .iter()
                .any(|text| contains_token(text, id) || contains_token(text, underscored))
    };
    let matched: Vec<&ReportCase> = cases
        .iter()
        .filter(|c| carries_id(c) || explicit.iter().any(|name| names_case(c, name)))
        .collect();
    if matched.is_empty() {
        let mut wanted = Vec::new();
        if let Some(id) = scenario_id {
            wanted.push(format!("for scenario `{id}`"));
        }
        wanted.extend(explicit.iter().map(|n| format!("named `{n}`")));
        return Err(format!(
            "no case {} among its {} case(s); put the scenario id in the test name or tag, or name the case with --report-case",
            wanted.join(" or "),
            cases.len()
        ));
    }
    Ok(matched)
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

    fn names(matched: &[&ReportCase]) -> Vec<(String, ReportOutcome)> {
        matched.iter().map(|c| (c.name.clone(), c.outcome)).collect()
    }

    /// The id must match as a whole token (`cart.add.01` is not
    /// `cart.add.010`), and the underscored form finds a Rust test fn.
    #[test]
    fn scenario_ids_match_whole_tokens_in_either_form() {
        let cases = parse_junit(JUNIT).unwrap();
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.01")).unwrap()),
            vec![("cart.add.01 adds & counts".to_string(), ReportOutcome::Passed)]
        );
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.04")).unwrap())[0].1,
            ReportOutcome::Passed
        );
        assert!(match_cases(&cases, &[], Some("cart.add.09"))
            .unwrap_err()
            .contains("5 case(s)"));
        assert_eq!(
            names(&match_cases(&cases, &["cart_add_04_refuses_out_of_stock".to_string()], None).unwrap())[0].0,
            "spec_coverage::tests::cart_add_04_refuses_out_of_stock",
            "--report-case names a Rust test by its function"
        );
        assert!(match_cases(&cases, &[], None).is_err());
    }

    /// 0.14 D5: `--report-case` is repeatable and ADDS to the id-carrying
    /// cases — naming one passing case never hides another that carries
    /// the scenario id.
    #[test]
    fn named_cases_add_to_the_id_carrying_cases_and_never_replace_them() {
        let cases = parse_junit(JUNIT).unwrap();
        let explicit = vec!["cart.add.02 flaky".to_string(), "cart.add.03 broken".to_string()];
        assert_eq!(
            names(&match_cases(&cases, &explicit, Some("cart.add.01")).unwrap()),
            vec![
                ("cart.add.01 adds & counts".to_string(), ReportOutcome::Passed),
                ("cart.add.02 flaky".to_string(), ReportOutcome::Skipped),
                ("cart.add.03 broken".to_string(), ReportOutcome::Failed),
            ]
        );
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
    /// it; a pending step is not a pass; every Outline example is bound.
    #[test]
    fn cucumber_tags_backgrounds_and_outlines() {
        let cases = parse_cucumber(CUCUMBER).unwrap();
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.01")).unwrap())[0].1,
            ReportOutcome::Failed,
            "the failed Background belongs to this scenario"
        );
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.02")).unwrap())[0].1,
            ReportOutcome::Passed
        );
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.03")).unwrap())[0].1,
            ReportOutcome::Skipped
        );
        assert_eq!(
            names(&match_cases(&cases, &[], Some("cart.add.04")).unwrap()),
            vec![("Outline".to_string(), ReportOutcome::Passed), ("Outline".to_string(), ReportOutcome::Failed)]
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
            report_cases: &[],
            scenario_id: Some("cart.add.01"),
            verdict,
            max_bytes: 25 * 1024 * 1024,
        }
    }

    #[test]
    fn bind_records_digests_and_refuses_contradictions_and_outside_files() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("trace.zip"), b"trace-bytes").unwrap();
        std::fs::write(repo.path().join("junit.xml"), JUNIT).unwrap();

        let artifacts = vec![repo.path().join("trace.zip")];
        let reports = vec![(ReportFormat::Junit, repo.path().join("junit.xml"))];
        let bound = bind(&request(repo.path(), &artifacts, &reports, EvidenceVerdict::Faithful)).unwrap();
        let attached = &bound.attachments;
        assert_eq!(attached[0].path, "trace.zip");
        assert_eq!(attached[0].sha256, format!("{:x}", Sha256::digest(b"trace-bytes")));
        assert_eq!(attached[1].outcome, Some(ReportOutcome::Passed));
        assert_eq!(
            (attached[1].path.as_str(), attached[1].format, attached[1].case.as_deref()),
            ("junit.xml", Some(ReportFormat::Junit), Some("cart.add.01 adds & counts"))
        );
        assert_eq!(attached[1].sha256, format!("{:x}", Sha256::digest(JUNIT.as_bytes())));
        assert_eq!(
            bound.files.iter().map(|f| f.sha256.as_str()).collect::<Vec<_>>(),
            vec![attached[0].sha256.as_str(), attached[1].sha256.as_str()],
            "every bound file is handed back for the store, once per digest"
        );
        assert!(!repo.path().join(".canon").exists(), "bind only reads; storing is the caller's step");

        let failing = BindRequest { scenario_id: Some("cart.add.03"), ..request(repo.path(), &[], &reports, EvidenceVerdict::Faithful) };
        assert!(matches!(bind(&failing), Err(BindError::Contradiction(_))), "a faithful claim over a failed case is refused");
        let explicit = vec!["cart.add.03 broken".to_string()];
        let hidden = BindRequest { report_cases: &explicit, ..request(repo.path(), &[], &reports, EvidenceVerdict::Faithful) };
        assert!(
            matches!(bind(&hidden), Err(BindError::Contradiction(m)) if m.contains("cart.add.03 broken")),
            "a failed case named alongside a passing one refuses faithful"
        );
        let honest = BindRequest { scenario_id: Some("cart.add.03"), ..request(repo.path(), &[], &reports, EvidenceVerdict::Divergent) };
        assert_eq!(bind(&honest).unwrap().attachments[0].outcome, Some(ReportOutcome::Failed), "a divergent verdict may cite the failing case");
        let unknown = vec!["no_such_test".to_string()];
        let nameless = BindRequest { report_cases: &unknown, ..request(repo.path(), &[], &reports, EvidenceVerdict::Faithful) };
        assert!(matches!(bind(&nameless), Err(BindError::Usage(m)) if m.contains("no_such_test")), "a --report-case that names nothing refuses");

        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("x.log"), b"x").unwrap();
        let elsewhere = vec![outside.path().join("x.log")];
        let err = bind(&request(repo.path(), &elsewhere, &[], EvidenceVerdict::Faithful)).unwrap_err();
        assert!(matches!(&err, BindError::Usage(m) if m.contains("outside the repository")), "{err:?}");

        let oversized = BindRequest { max_bytes: 4, ..request(repo.path(), &artifacts, &[], EvidenceVerdict::Faithful) };
        assert!(
            matches!(bind(&oversized), Err(BindError::Usage(m)) if m.contains("--max-artifact-mib")),
            "a file over the size limit refuses, naming the override flag"
        );
    }

    #[test]
    fn report_spec_parses_format_and_path() {
        assert_eq!(
            parse_report_spec("junit:target/nextest/junit.xml").unwrap(),
            (ReportFormat::Junit, PathBuf::from("target/nextest/junit.xml"))
        );
        assert!(parse_report_spec("tap:out.tap").is_err());
        assert!(parse_report_spec("junit:").is_err());
        assert!(parse_report_spec("junit.xml").is_err());
    }
}
