//! `canon evidence add --artifact/--report` (experimental evidence
//! binding): bind an attestation to files the team's own runner or agent
//! already produced.
//!
//! Canon never runs a test — it cannot, across every language and every
//! agent-driven QA tool a team uses. It READS what those tools wrote:
//!
//! - `--artifact <path>` (repeatable): any file — a trace, a screenshot
//!   archive, an agent QA log, a review document — bound by sha256.
//! - `--report <junit|cucumber|swift-testing>:<path>` (repeatable): a
//!   machine-readable test report. Canon parses it, finds the case for
//!   this evidence's scenario, and records the case and its outcome
//!   alongside the digest. `swift-testing` is Swift Testing's JSON-lines
//!   event stream (`swift test --experimental-event-stream-output`,
//!   0.18 D1): a test's display name is its case name, else its
//!   function name, and its outcome comes from its events (a failing
//!   issue fails it, `testSkipped` skips it, a test that started and
//!   ended without one passed, one that never ended failed).
//!
//! # Finding the cases
//! A case is bound when it is the scenario's OWN case: its leading
//! scenario id — the first scenario-id-shaped token
//! (`<area>.<surface>.<nn>`) in its full name, read as the runner
//! writes it: classname (a JUnit describe path) first, then name;
//! dotted (`cart.add.04`) or with dots and hyphens as underscores
//! (`cart_add_04`, the form a test function can carry) — equals the
//! scenario id. A Cucumber tag equal to the scenario id also binds. A
//! case that only MENTIONS another id after its own
//! (`game.session.04: restarting as in game.session.01`) is never bound
//! to the mentioned one. PLUS every case named by a
//! `--report-case <name>` (repeatable; the case whose name equals it,
//! or ends with `::<name>`, so a Rust test is named by its function).
//! Each bound case is its own attachment, carrying that case's name and
//! outcome — a Scenario Outline's examples, or three tests for one
//! scenario, are all recorded, never folded into one. Naming a case
//! cannot hide another: the scenario's own cases are bound whether or
//! not `--report-case` is given. No match at all refuses, and so does a
//! `--report-case` that names no case: a binding that names nothing is
//! not a binding.
//!
//! # One report, many records (0.18 D1)
//! With no `--scenario-id`, a task's evidence binds EVERY case in the
//! report ([`BindRequest::all_cases`]), so a faithful verdict needs all
//! of them to pass. `--all-scenarios` ([`bind_all_scenarios`]) binds the
//! report once and splits it by owner: each indexed scenario that owns
//! a case (the rule above) gets its own record, with its own cases and
//! a verdict from them. A case whose id is not indexed is named and
//! skipped. Every record pins the same report blob.
//!
//! # Attachments by test (0.24 D4, #83)
//! A report says which file belongs to which test, so a record binds the
//! files of ITS cases and no other: a JUnit `<testcase>`'s `<system-out>`
//! lines `[[ATTACHMENT|<path>]]` (Playwright's reporter; the path is
//! relative to the report's directory, an absolute one must lie inside
//! the repository) and a Cucumber step's `embeddings` (base64 bytes,
//! stored by digest under a name derived from the report, the case and
//! the embedding's index). Such a file that is missing, escapes the
//! repository or exceeds the size limit refuses the bind, naming the
//! case and the path. A bare `--artifact` is bound to the record of a
//! single scenario; the batch refuses it (it would bind to every
//! record) and takes `--artifact <scenario-id>=<path>` instead
//! ([`parse_artifact`]).
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
use std::sync::Arc;

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
    /// Every refusal says how to get unstuck (`hint`) and, when one flag
    /// caused it, which (`field`).
    Usage { message: String, hint: String, field: Option<&'static str> },
    /// The report contradicts the verdict: 1.
    Contradiction(String),
}

impl BindError {
    fn usage(message: impl Into<String>, hint: impl Into<String>) -> Self {
        BindError::Usage { message: message.into(), hint: hint.into(), field: None }
    }

    /// Names the flag the refused file was given with.
    fn on(self, flag: &'static str) -> Self {
        match self {
            BindError::Usage { message, hint, .. } => BindError::Usage { message, hint, field: Some(flag) },
            other => other,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            BindError::Usage { .. } => 2,
            BindError::Contradiction(_) => 1,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            BindError::Usage { message, .. } | BindError::Contradiction(message) => message,
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            BindError::Usage { hint, .. } => Some(hint),
            BindError::Contradiction(_) => None,
        }
    }

    pub fn field(&self) -> Option<&'static str> {
        match self {
            BindError::Usage { field, .. } => *field,
            BindError::Contradiction(_) => None,
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
    /// The files the report names as this case's own.
    pub attachments: Vec<CaseFile>,
}

/// A file a report says belongs to one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseFile {
    /// A JUnit `[[ATTACHMENT|<path>]]` path, as written: relative to the
    /// report's directory.
    Path(String),
    /// A Cucumber embedding, decoded.
    Embedded { mime: String, bytes: Arc<[u8]> },
}

/// `--artifact <path>` or `--artifact <scenario-id>=<path>`: the scenario
/// id scopes the file to that scenario's record. The part before the
/// first `=` is a scope only when it is shaped like a scenario id
/// (`<area>.<surface>.<nn>`), so a path containing `=` stays a path;
/// write `./<id>=<file>` for a file literally named that way.
pub fn parse_artifact(value: &Path) -> (Option<String>, PathBuf) {
    if let Some((id, path)) = value.to_str().and_then(|v| v.split_once('=')) {
        if canon_model::ScenarioId::parse(id).is_ok() {
            return (Some(id.to_string()), PathBuf::from(path));
        }
    }
    (None, value.to_path_buf())
}

/// Parse `--report`'s `<format>:<path>`.
pub fn parse_report_spec(value: &str) -> Result<(ReportFormat, PathBuf), String> {
    let (format, path) = value
        .split_once(':')
        .ok_or_else(|| format!("`{value}` is not <junit|cucumber|swift-testing>:<path>"))?;
    let format = match format {
        "junit" => ReportFormat::Junit,
        "cucumber" => ReportFormat::Cucumber,
        "swift-testing" => ReportFormat::SwiftTesting,
        other => {
            return Err(format!(
                "`{other}` is not a supported report format (expected junit, cucumber or swift-testing)"
            ))
        }
    };
    if path.is_empty() {
        return Err(format!("`{value}` names no file"));
    }
    Ok((format, PathBuf::from(path)))
}

/// Inputs for [`bind`].
#[derive(Debug)]
pub struct BindRequest<'a> {
    pub repo: &'a Path,
    pub artifacts: &'a [PathBuf],
    pub reports: &'a [(ReportFormat, PathBuf)],
    /// `--report-case`, repeatable: cases bound IN ADDITION to the ones
    /// carrying the scenario id.
    pub report_cases: &'a [String],
    /// The scenario id the default case match looks for.
    pub scenario_id: Option<&'a str>,
    /// Bind every case of every report (0.18 D1): a task's evidence with
    /// no `--scenario-id` and no `--report-case`.
    pub all_cases: bool,
    /// `--verdict` as claimed (an audit's result counts as a claim). `None`:
    /// with a report bound the verdict is derived from the bound cases
    /// ([`derived_verdict`]), else `faithful`. A claimed `faithful` over a
    /// failed or skipped case is refused.
    pub verdict: Option<EvidenceVerdict>,
    /// The store's size limit for one file, in bytes.
    pub max_bytes: u64,
}

/// One file [`bind`] read: where it is now and the digest recorded for
/// it, so the caller can store exactly those bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundFile {
    pub resolved: PathBuf,
    pub sha256: String,
    /// The bytes of a Cucumber embedding: there is no file, and
    /// `resolved` is only the name the record shows.
    pub embedded: Option<Arc<[u8]>>,
}

impl BoundFile {
    fn on_disk(resolved: PathBuf, sha256: String) -> Self {
        BoundFile { resolved, sha256, embedded: None }
    }

    /// The bytes behind the digest.
    fn reader(&self) -> std::io::Result<Box<dyn Read>> {
        match &self.embedded {
            Some(bytes) => Ok(Box::new(std::io::Cursor::new(Arc::clone(bytes)))),
            None => Ok(Box::new(std::fs::File::open(&self.resolved)?)),
        }
    }
}

/// What [`bind`] produced: the record's attachments and the files
/// behind them, one entry per distinct digest.
#[derive(Debug)]
pub struct Bound {
    pub attachments: Vec<EvidenceAttachment>,
    pub files: Vec<BoundFile>,
    /// The verdict the record carries: the claimed one, else what the bound
    /// cases show, else `faithful`.
    pub verdict: EvidenceVerdict,
}

impl Bound {
    fn add_file(&mut self, file: BoundFile) {
        add_unique_file(&mut self.files, file);
    }
}

fn add_unique_file(files: &mut Vec<BoundFile>, file: BoundFile) {
    if !files.iter().any(|f| f.sha256 == file.sha256) {
        files.push(file);
    }
}

/// Add an opaque attachment unless the record already binds those bytes.
fn push_unique(attachments: &mut Vec<EvidenceAttachment>, attachment: EvidenceAttachment) {
    if !attachments.iter().any(|a| a.format.is_none() && a.sha256 == attachment.sha256) {
        attachments.push(attachment);
    }
}

/// Read and hash each opaque `--artifact` file — inside the repository,
/// within the store's size limit — returning its repository-relative
/// path and the file behind it. Writes nothing. `canon review add
/// --artifact` (0.16 D5) binds through this, so a review's files meet
/// exactly the checks an evidence record's do.
pub fn bind_artifacts(repo: &Path, artifacts: &[PathBuf], max_bytes: u64) -> Result<Vec<(String, BoundFile)>, BindError> {
    let repo = canonical_repo(repo)?;
    artifacts.iter().map(|path| hash_artifact(&repo, path, max_bytes)).collect()
}

fn canonical_repo(repo: &Path) -> Result<PathBuf, BindError> {
    repo.canonicalize().map_err(|e| BindError::usage(format!("resolve repository root: {e}"), "run canon from inside the repository, or pass --repo <root>"))
}

fn hash_artifact(repo: &Path, path: &Path, max_bytes: u64) -> Result<(String, BoundFile), BindError> {
    let (relative, resolved, file) = open_in_repo(repo, path, max_bytes).map_err(|e| e.on("--artifact"))?;
    let sha256 = canon_gate::artifact_store::sha256_reader(file)
        .map_err(|e| BindError::usage(format!("read {}: {e}", path.display()), "check the file is readable by this user").on("--artifact"))?;
    Ok((relative, BoundFile::on_disk(resolved, sha256)))
}

/// Copy every bound file into the artifact store, all or nothing,
/// returning how many blobs this call wrote (a blob already stored
/// intact, or the same bytes bound twice, is not newly stored). Called
/// once every refusal has passed, just before the record is written, so
/// a record never names a blob the store lacks. The whole transaction
/// holds the repo's artifact-store lock
/// ([`canon_gate::artifact_store::StoreLock`]); a lock that stays busy
/// is reported against the first file. Every file is first re-read,
/// re-hashed and staged to a temp file in the store; only when all of
/// them have staged are they published. A file that changed since it
/// was hashed is refused rather than stored under a digest its bytes no
/// longer have, and then nothing is published. The error names that
/// file.
pub fn store_files<'f>(repo: &Path, files: &'f [BoundFile]) -> Result<usize, (&'f BoundFile, canon_gate::artifact_store::StoreError)> {
    store_files_held(repo, files).map(|held| held.stored())
}

/// [`store_files`], keeping the store lock for a caller that writes
/// several records naming these blobs (0.18): if one of those writes
/// fails, [`HeldStore::rollback`] removes exactly the blobs this call
/// published, while no other writer could have published them meanwhile.
pub fn store_files_held<'f>(repo: &Path, files: &'f [BoundFile]) -> Result<HeldStore, (&'f BoundFile, canon_gate::artifact_store::StoreError)> {
    use canon_gate::artifact_store::{ArtifactStore, StoreError, StoreLock};

    let Some(first) = files.first() else {
        return Ok(HeldStore { lock: None, published: Vec::new(), uploaded: 0 });
    };
    let lock = StoreLock::acquire(repo).map_err(|e| (first, e))?;
    // The backend is read UNDER the lock, so a `canon artifacts move`
    // that finished while this call waited is seen, and no blob is
    // written to a local store the repo no longer uses.
    let store = ArtifactStore::open(repo).map_err(|e| (first, e))?;
    let object = match store {
        ArtifactStore::Local => None,
        ArtifactStore::Object(object) => Some(object),
    };
    if let Some(object) = object {
        // Every file is re-read and re-hashed before anything is
        // uploaded: one that changed since it was bound refuses the whole
        // call, so a record never names bytes the store lacks. The local
        // lock is released first; the lock only guards the local store,
        // and an upload must not hold other writers off for its duration.
        drop(lock);
        for file in files {
            let actual = file
                .reader()
                .and_then(canon_gate::artifact_store::sha256_reader)
                .map_err(|e| (file, StoreError::from(e)))?;
            if actual != file.sha256 {
                return Err((file, StoreError::Changed { expected: file.sha256.clone(), actual }));
            }
        }
        let mut uploaded = 0;
        for file in files {
            let stored = file.reader().map_err(StoreError::from).and_then(|reader| object.upload(&file.sha256, reader)).map_err(|e| (file, e))?;
            if stored == canon_gate::artifact_store::StoreOutcome::Stored {
                uploaded += 1;
            }
        }
        return Ok(HeldStore { lock: None, published: Vec::new(), uploaded });
    }
    let mut staged = Vec::new();
    let mut sources = Vec::new();
    for file in files {
        let blob = file
            .reader()
            .map_err(StoreError::from)
            .and_then(|reader| lock.stage(&file.sha256, reader))
            .map_err(|e| (file, e))?;
        if let Some(blob) = blob {
            staged.push(blob);
            sources.push(file);
        }
    }
    let published = lock.publish_paths(staged).map_err(|(index, e)| (sources[index], e))?;
    Ok(HeldStore { lock: Some(lock), published, uploaded: 0 })
}

/// The blobs one [`store_files_held`] call stored, with the local store
/// lock still held (none for an object store). Dropping it keeps them
/// and releases the lock.
#[derive(Debug)]
pub struct HeldStore {
    lock: Option<canon_gate::artifact_store::StoreLock>,
    published: Vec<PathBuf>,
    uploaded: usize,
}

impl HeldStore {
    /// How many blobs this call newly stored.
    pub fn stored(&self) -> usize {
        self.published.len() + self.uploaded
    }

    /// Remove the local blobs this call published, then release the lock.
    /// Uploaded objects stay: a content-addressed object another writer
    /// may already count as stored is never deleted, and an orphan costs
    /// only its bytes.
    pub fn rollback(self) {
        if let Some(lock) = &self.lock {
            lock.unpublish(&self.published);
        }
    }
}

/// The line a write command prints after [`store_files`].
pub fn stored_line(repo: &Path, command: &str, bound: usize, stored: usize) -> String {
    match canon_gate::artifact_store::ArtifactStore::open(repo) {
        Ok(store @ canon_gate::artifact_store::ArtifactStore::Object(_)) => format!("{command}: artifact store holds {bound} bound file(s) in {} ({stored} newly uploaded) — nothing to commit", store.location()),
        _ => format!(
            "{command}: artifact store holds {bound} bound file(s) under {}/ ({stored} newly stored) — commit them with the ledger",
            canon_model::paths::ARTIFACTS_SHA256_DIR
        ),
    }
}

/// One `--report` read and parsed: where it lives, its digest, and its
/// cases in report order.
struct ParsedReport {
    format: ReportFormat,
    /// The path as given, for messages.
    given: PathBuf,
    relative: String,
    resolved: PathBuf,
    sha256: String,
    cases: Vec<ReportCase>,
}

impl ParsedReport {
    fn attachment(&self, case: &ReportCase) -> EvidenceAttachment {
        EvidenceAttachment {
            path: self.relative.clone(),
            sha256: self.sha256.clone(),
            format: Some(self.format),
            case: Some(case.name.clone()),
            outcome: Some(case.outcome),
        }
    }

    /// The files `case` names as its own (module doc), each read inside
    /// the repository and the size limit. Any that cannot be refuses,
    /// naming the case and the path.
    fn case_files(&self, repo: &Path, case: &ReportCase, max_bytes: u64) -> Result<Vec<(EvidenceAttachment, BoundFile)>, BindError> {
        let refusal = |written: &str, why: String| {
            BindError::usage(
                format!("case `{}` in {} names the attachment `{written}`, which cannot be bound: {why}", case.name, self.given.display()),
                format!(
                    "an attachment path in the report is resolved against the report's directory and must stay inside the repository; keep the run's output there (or regenerate the report), raise the limit with {} <MiB>, or drop the attachment from the test",
                    canon_gate::MAX_ARTIFACT_FLAG
                ),
            )
            .on("--report")
        };
        let mut out = Vec::new();
        for (index, file) in case.attachments.iter().enumerate() {
            let (path, bound) = match file {
                CaseFile::Path(written) => {
                    let given = Path::new(written);
                    let candidate = if given.is_absolute() { given.to_path_buf() } else { self.resolved.parent().unwrap_or(Path::new("")).join(given) };
                    let (relative, resolved, opened) = open_in_repo(repo, &candidate, max_bytes).map_err(|e| refusal(written, e.message().to_string()))?;
                    let sha256 = canon_gate::artifact_store::sha256_reader(opened).map_err(|e| refusal(written, format!("read it: {e}")))?;
                    (relative, BoundFile::on_disk(resolved, sha256))
                }
                CaseFile::Embedded { mime, bytes } => {
                    let name = format!("{}.attachments/{}/{}.{}", self.relative, slug(&case.name), index + 1, mime_extension(mime));
                    if bytes.len() as u64 > max_bytes {
                        return Err(refusal(&name, format!("it is {} bytes, over the artifact store's limit of {max_bytes} bytes", bytes.len())));
                    }
                    let sha256 = format!("{:x}", Sha256::digest(bytes));
                    (name.clone(), BoundFile { resolved: PathBuf::from(&name), sha256, embedded: Some(Arc::clone(bytes)) })
                }
            };
            out.push((EvidenceAttachment { path, sha256: bound.sha256.clone(), format: None, case: None, outcome: None }, bound));
        }
        Ok(out)
    }

    /// The refusal of a faithful verdict over `matched`, when any of
    /// them did not pass.
    fn contradiction(&self, matched: &[&ReportCase]) -> Option<String> {
        let contradicting: Vec<String> = matched
            .iter()
            .filter(|c| c.outcome != ReportOutcome::Passed)
            .map(|c| format!("`{}` as {}", c.name, outcome_name(c.outcome)))
            .collect();
        (!contradicting.is_empty()).then(|| format!("{} records case {}", self.given.display(), contradicting.join(", ")))
    }
}

/// Bind the files of `cases` into `attachments` (once per digest) and
/// return the files behind the new ones.
fn bind_case_files(report: &ParsedReport, repo: &Path, cases: &[&ReportCase], max_bytes: u64, attachments: &mut Vec<EvidenceAttachment>) -> Result<Vec<BoundFile>, BindError> {
    let mut files = Vec::new();
    for case in cases {
        for (attachment, file) in report.case_files(repo, case, max_bytes)? {
            push_unique(attachments, attachment);
            add_unique_file(&mut files, file);
        }
    }
    Ok(files)
}

/// A case name as one path segment.
fn slug(name: &str) -> String {
    let slug: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' }).collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() { "case".to_string() } else { slug.chars().take(80).collect() }
}

fn mime_extension(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/csv" => "csv",
        "application/json" => "json",
        "application/zip" => "zip",
        "video/webm" => "webm",
        "video/mp4" => "mp4",
        _ => "bin",
    }
}

const FAITHFUL_NEEDS_PASSES: &str = "a faithful verdict needs every bound case to pass (attest the verdict the report shows, or fix the test)";

fn read_report(repo: &Path, format: ReportFormat, path: &Path, max_bytes: u64) -> Result<ParsedReport, BindError> {
    read_report_file(repo, format, path, max_bytes).map_err(|e| e.on("--report"))
}

fn read_report_file(repo: &Path, format: ReportFormat, path: &Path, max_bytes: u64) -> Result<ParsedReport, BindError> {
    let (relative, resolved, file) = open_in_repo(repo, path, max_bytes)?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| BindError::usage(format!("read {}: {e}", path.display()), "check the file is readable by this user and not being written by the test run still"))?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(BindError::usage(
            format!("{} is larger than {MAX_REPORT_BYTES} bytes; bind it with --artifact instead", path.display()),
            "bind the file as an opaque --artifact, or split the report per suite",
        ));
    }
    #[expect(clippy::map_err_ignore, reason = "`BindError::usage` takes a message and a hint, no source; the message already says the bytes are not UTF-8")]
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| BindError::usage(format!("{} is not UTF-8", path.display()), "write the report as UTF-8 (a test runner's XML/JSON output is), or bind the file as an opaque --artifact"))?;
    let cases = match format {
        ReportFormat::Junit => parse_junit(text),
        ReportFormat::Cucumber => parse_cucumber(text),
        ReportFormat::SwiftTesting => parse_swift_testing(text),
    }
    .map_err(|e| {
        BindError::usage(
            format!("{} is not a readable {} report: {e}", path.display(), format_name(format)),
            "pass --report <format>:<path> with the format the file really is (junit, cucumber or swift-testing), regenerate the report if the run was cut short, or bind the file as an opaque --artifact",
        )
    })?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(ParsedReport { format, given: path.to_path_buf(), relative, resolved, sha256, cases })
}

/// Read, hash, and (for reports) match every requested file. Writes
/// nothing (module doc).
pub fn bind(request: &BindRequest<'_>) -> Result<Bound, BindError> {
    let repo = canonical_repo(request.repo)?;
    let mut bound = Bound { attachments: Vec::new(), files: Vec::new(), verdict: EvidenceVerdict::Faithful };
    let mut outcomes: Vec<ReportOutcome> = Vec::new();

    for path in request.artifacts {
        let (relative, file) = hash_artifact(&repo, path, request.max_bytes)?;
        push_unique(&mut bound.attachments, EvidenceAttachment { path: relative, sha256: file.sha256.clone(), format: None, case: None, outcome: None });
        bound.add_file(file);
    }

    let mut named_unmatched: Vec<&str> = request.report_cases.iter().map(String::as_str).collect();
    for (format, path) in request.reports {
        let report = read_report(&repo, *format, path, request.max_bytes)?;
        let matched: Vec<&ReportCase> = if request.all_cases {
            report.cases.iter().collect()
        } else {
            match_cases(&report.cases, request.report_cases, request.scenario_id)
                .map_err(|e| {
                    BindError::usage(format!("{}: {e}", path.display()), "name the cases to bind with --report-case <name>, or start each test name with the scenario id so it is picked up by default")
                        .on("--report-case")
                })?
        };
        named_unmatched.retain(|name| !matched.iter().any(|c| names_case(c, name)));
        if request.verdict == Some(EvidenceVerdict::Faithful) {
            if let Some(contradiction) = report.contradiction(&matched) {
                return Err(BindError::Contradiction(format!("{contradiction}; {FAITHFUL_NEEDS_PASSES}")));
            }
        }
        outcomes.extend(matched.iter().map(|case| case.outcome));
        bound.add_file(BoundFile::on_disk(report.resolved.clone(), report.sha256.clone()));
        bound.attachments.extend(matched.iter().map(|case| report.attachment(case)));
        for file in bind_case_files(&report, &repo, &matched, request.max_bytes, &mut bound.attachments)? {
            bound.add_file(file);
        }
    }
    if let Some(name) = named_unmatched.first() {
        return Err(BindError::usage(
            format!("--report-case `{name}` names no case in any --report; a binding that names nothing is not a binding"),
            "copy a case name from the report (the suffix after `::` is enough), or drop --report-case to bind the cases carrying the scenario id",
        )
        .on("--report-case"));
    }
    bound.verdict = request.verdict.unwrap_or_else(|| derived_verdict(outcomes));
    Ok(bound)
}

/// Inputs for [`bind_all_scenarios`].
#[derive(Debug)]
pub struct BatchRequest<'a> {
    pub repo: &'a Path,
    /// `--artifact <scenario-id>=<path>`: each bound to that scenario's
    /// record only. A bare `--artifact` is not a batch input (it would
    /// bind to every record); the caller refuses it.
    pub scoped_artifacts: &'a [(String, PathBuf)],
    pub reports: &'a [(ReportFormat, PathBuf)],
    /// The indexed scenario ids of the spec root, the ones a record may
    /// be written for.
    pub indexed: &'a [&'a str],
    /// `--verdict` as given: every record claims it. `None`: each
    /// record's verdict comes from its own cases ([`derived_verdict`]).
    pub verdict: Option<EvidenceVerdict>,
    pub max_bytes: u64,
}

/// One record of a batch: the scenario, its verdict, and what it binds
/// (the shared artifacts, then its own cases).
#[derive(Debug)]
pub struct ScenarioBinding {
    pub scenario_id: String,
    pub verdict: EvidenceVerdict,
    pub attachments: Vec<EvidenceAttachment>,
}

/// What [`bind_all_scenarios`] produced.
#[derive(Debug, Default)]
pub struct BatchBound {
    /// In the order of [`BatchRequest::indexed`].
    pub scenarios: Vec<ScenarioBinding>,
    /// Every bound file, once per digest: shared by every record.
    pub files: Vec<BoundFile>,
    /// The scenario ids cases lead with that are not indexed, each with
    /// its first case, sorted. These cases bind nothing.
    pub unindexed: Vec<(String, String)>,
    /// How many cases carry no scenario id at all.
    pub without_id: usize,
}

/// A record's verdict from its own cases when none was claimed: any
/// failed case makes it `divergent`; all passed, `faithful`; otherwise
/// (a skipped case, none failed) `not-applicable`, since the report
/// shows neither.
pub fn derived_verdict(outcomes: impl IntoIterator<Item = ReportOutcome>) -> EvidenceVerdict {
    let mut verdict = EvidenceVerdict::Faithful;
    for outcome in outcomes {
        match outcome {
            ReportOutcome::Failed => return EvidenceVerdict::Divergent,
            ReportOutcome::Skipped => verdict = EvidenceVerdict::NotApplicable,
            ReportOutcome::Passed => {}
        }
    }
    verdict
}

/// `--all-scenarios` (module doc): read every file once, and split the
/// reports' cases by the indexed scenario that owns them. Refuses (and
/// so writes nothing) when no indexed scenario owns a case, and when a
/// claimed `faithful` verdict meets any record's failed or skipped case,
/// naming every such case.
pub fn bind_all_scenarios(request: &BatchRequest<'_>) -> Result<BatchBound, BindError> {
    use std::collections::BTreeMap;

    let repo = canonical_repo(request.repo)?;
    let mut batch = BatchBound::default();
    let mut scoped = Vec::new();
    for (id, path) in request.scoped_artifacts {
        let (relative, file) = hash_artifact(&repo, path, request.max_bytes)?;
        let attachment = EvidenceAttachment { path: relative, sha256: file.sha256.clone(), format: None, case: None, outcome: None };
        scoped.push((id.as_str(), attachment, file));
    }
    let reports = request
        .reports
        .iter()
        .map(|(format, path)| read_report(&repo, *format, path, request.max_bytes))
        .collect::<Result<Vec<_>, _>>()?;

    let mut unindexed: BTreeMap<String, String> = BTreeMap::new();
    for report in &reports {
        for case in report.cases.iter().filter(|c| !request.indexed.iter().any(|id| owns_scenario(c, id))) {
            match case_scenario_token(case) {
                Some(id) => {
                    unindexed.entry(id).or_insert_with(|| case.name.clone());
                }
                None => batch.without_id += 1,
            }
        }
    }
    batch.unindexed = unindexed.into_iter().collect();

    let mut contradictions = Vec::new();
    for id in request.indexed {
        let mut attachments: Vec<EvidenceAttachment> = Vec::new();
        for (_, attachment, _) in scoped.iter().filter(|(scope, _, _)| scope == id) {
            push_unique(&mut attachments, attachment.clone());
        }
        let mut outcomes = Vec::new();
        for report in &reports {
            let matched: Vec<&ReportCase> = report.cases.iter().filter(|c| owns_scenario(c, id)).collect();
            if request.verdict == Some(EvidenceVerdict::Faithful) {
                if let Some(contradiction) = report.contradiction(&matched) {
                    contradictions.push(format!("{id}: {contradiction}"));
                }
            }
            outcomes.extend(matched.iter().map(|c| c.outcome));
            attachments.extend(matched.iter().map(|case| report.attachment(case)));
            for file in bind_case_files(report, &repo, &matched, request.max_bytes, &mut attachments)? {
                add_unique_file(&mut batch.files, file);
            }
        }
        if outcomes.is_empty() {
            continue;
        }
        let verdict = request.verdict.unwrap_or_else(|| derived_verdict(outcomes));
        batch.scenarios.push(ScenarioBinding { scenario_id: id.to_string(), verdict, attachments });
    }
    if !contradictions.is_empty() {
        return Err(BindError::Contradiction(format!(
            "{}; {FAITHFUL_NEEDS_PASSES}, so no record of the batch is written (drop --verdict to record each scenario's own verdict)",
            contradictions.join("; ")
        )));
    }
    if batch.scenarios.is_empty() {
        let cases: usize = reports.iter().map(|r| r.cases.len()).sum();
        let named = if batch.unindexed.is_empty() {
            String::new()
        } else {
            format!(" (not indexed: {})", batch.unindexed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(", "))
        };
        return Err(BindError::usage(
            format!("no case among the report's {cases} case(s) belongs to an indexed scenario{named}; start each test name with its scenario id or tag it, and index the scenarios (`canon inventory sync`)"),
            "name each test after its scenario id (or tag it with the id), run `canon inventory sync`, then retry",
        )
        .on("--report"));
    }
    for (scope, attachment, _) in &scoped {
        if !batch.scenarios.iter().any(|s| s.scenario_id == *scope) {
            return Err(BindError::usage(
                format!("--artifact `{scope}={}` names a scenario with no record in this batch; the batch writes a record only for an indexed scenario of --project-id that owns a case in the report", attachment.path),
                "check the scenario id, index the scenario (`canon inventory sync`) and name its test after the id so the report carries a case for it, or drop the scoped --artifact",
            )
            .on("--artifact"));
        }
    }
    for (_, _, file) in scoped {
        add_unique_file(&mut batch.files, file);
    }
    for report in &reports {
        add_unique_file(&mut batch.files, BoundFile::on_disk(report.resolved.clone(), report.sha256.clone()));
    }
    Ok(batch)
}

/// The scenario id `case` leads with, as written (module doc): a tag
/// shaped like one, else the leading id of its classname, then its name.
/// `None` when it carries none.
fn case_scenario_token(case: &ReportCase) -> Option<String> {
    if let Some(tag) = case.tags.iter().find(|t| leading_id_end(t) == Some(t.len()) && t.split('.').count() == 3) {
        return Some(tag.clone());
    }
    let text = if leading_id_end(&case.group).is_some() { &case.group } else { &case.name };
    let end = leading_id_end(text)?;
    let bytes = text.as_bytes();
    // `leading_id_end` guarantees `<area><sep><surface><sep><digits>`.
    let digits = bytes[..end].iter().rev().take_while(|b| b.is_ascii_digit()).count();
    let separator_at = end - digits - 1;
    let separator = bytes[separator_at];
    let segment = |b: &&u8| b.is_ascii_lowercase() || b.is_ascii_digit() || (separator == b'.' && **b == b'-');
    let surface_start = separator_at - bytes[..separator_at].iter().rev().take_while(segment).count();
    let area_end = surface_start - 1;
    let start = area_end - bytes[..area_end].iter().rev().take_while(segment).count();
    Some(text[start..end].to_string())
}

fn format_name(format: ReportFormat) -> &'static str {
    match format {
        ReportFormat::Junit => "JUnit XML",
        ReportFormat::Cucumber => "Cucumber JSON",
        ReportFormat::SwiftTesting => "Swift Testing event stream",
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
    let resolved = path.canonicalize().map_err(|e| {
        BindError::usage(format!("{}: {e}", path.display()), "check the path exists and is readable from the directory canon runs in; a relative path is resolved against it")
    })?;
    if !resolved.is_file() {
        return Err(BindError::usage(
            format!("{} is not a file", path.display()),
            "name a file, not a directory; archive a directory first (zip or tar) and bind the archive",
        ));
    }
    #[expect(clippy::map_err_ignore, reason = "`BindError::usage` takes a message and a hint, no source; `StripPrefixError` says nothing the message does not")]
    let relative = resolved.strip_prefix(repo).map_err(|_| {
        BindError::usage(
            format!("{} is outside the repository; bound files are recorded by repository-relative path", path.display()),
            "copy the file anywhere under the repository and bind that path; canon stores the bytes under .canon/artifacts/sha256 either way",
        )
    })?;
    let relative = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let file = std::fs::File::open(&resolved)
        .map_err(|e| BindError::usage(format!("open {}: {e}", path.display()), "check the file is readable by this user (permissions) and not locked by another process"))?;
    let size = file
        .metadata()
        .map_err(|e| BindError::usage(format!("stat {}: {e}", path.display()), "check the file is readable by this user (permissions) and still exists"))?
        .len();
    if size > max_bytes {
        return Err(BindError::usage(
            format!(
                "{} is {size} bytes, over the artifact store's limit of {max_bytes} bytes; raise it with {} <MiB> if the file belongs in git",
                path.display(),
                canon_gate::MAX_ARTIFACT_FLAG
            ),
            format!("raise the limit with {} <MiB>, or bind a smaller file (trim or compress it)", canon_gate::MAX_ARTIFACT_FLAG),
        ));
    }
    Ok((relative, resolved, file))
}

/// Whether `--report-case <name>` names `case`.
fn names_case(case: &ReportCase, name: &str) -> bool {
    case.name == name || case.name.ends_with(&format!("::{name}"))
}

/// Every case this evidence binds (module doc): the ones whose own
/// scenario id is this one, plus the ones `explicit` names, each once,
/// in report order. Empty is an error.
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
    let matched: Vec<&ReportCase> = cases
        .iter()
        .filter(|c| scenario_id.is_some_and(|id| owns_scenario(c, id)) || explicit.iter().any(|name| names_case(c, name)))
        .collect();
    if matched.is_empty() {
        let mut wanted = Vec::new();
        if let Some(id) = scenario_id {
            wanted.push(format!("for scenario `{id}`"));
        }
        wanted.extend(explicit.iter().map(|n| format!("named `{n}`")));
        return Err(format!(
            "no case {} among its {} case(s); start the test name with the scenario id or tag it, or name the case with --report-case",
            wanted.join(" or "),
            cases.len()
        ));
    }
    Ok(matched)
}

/// Whether `case` is `id`'s own case (module doc): a Cucumber tag equal
/// to `id`, or `id` as the case's LEADING scenario id — the first
/// scenario-id-shaped token of its full name, read as a runner writes
/// it: the classname (a describe path) first, then the name. A case
/// for `game.session.04` that mentions `game.session.01` later is
/// `.04`'s alone.
fn owns_scenario(case: &ReportCase, id: &str) -> bool {
    if case.tags.iter().any(|t| t == id) {
        return true;
    }
    let underscored = id.replace(['.', '-'], "_");
    let text = if leading_id_end(&case.group).is_some() { &case.group } else { &case.name };
    leading_id_end(text).is_some_and(|end| {
        let lead = &text[..end];
        [id, underscored.as_str()].iter().any(|form| {
            lead.strip_suffix(form).is_some_and(|before| !before.chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric()))
        })
    })
}

/// The byte offset where `text`'s first scenario-id-shaped token ends,
/// or `None` when it has none. A token is shaped like a scenario id
/// (`<area>.<surface>.<nn>`, `nn` two or more digits) when it ends in
/// a run of 2+ digits with no ASCII alphanumeric after it, preceded by
/// either `.` and two dot-joined `[a-z0-9-]` segments (`cart.add.04`)
/// or `_` and two underscore-joined `[a-z0-9]` segments (`cart_add_04`,
/// the form a test function carries, hyphens and dots both `_`).
fn leading_id_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let bounded = i == bytes.len() || !bytes[i].is_ascii_alphanumeric();
        if bounded && i - start >= 2 && start > 0 && ends_with_two_segments(&bytes[..start - 1], bytes[start - 1]) {
            return Some(i);
        }
    }
    None
}

/// Whether `before` ends with two `separator`-joined, non-empty
/// segments (`.`: `[a-z0-9-]+`; `_`: `[a-z0-9]+`).
fn ends_with_two_segments(before: &[u8], separator: u8) -> bool {
    let segment = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || (separator == b'.' && b == b'-');
    if separator != b'.' && separator != b'_' {
        return false;
    }
    let surface = before.iter().rev().take_while(|b| segment(**b)).count();
    let rest = &before[..before.len() - surface];
    surface > 0 && rest.last() == Some(&separator) && rest[..rest.len() - 1].last().is_some_and(|b| segment(*b))
}

/// Parse JUnit XML: every `<testcase>`, failed when it holds `<failure>`
/// or `<error>`, skipped when it holds `<skipped>`, passed otherwise.
pub fn parse_junit(text: &str) -> Result<Vec<ReportCase>, String> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut cases = Vec::new();
    let mut open: Option<ReportCase> = None;
    // The text of the open case's `<system-out>`, where Playwright's
    // reporter writes one `[[ATTACHMENT|<path>]]` line per attachment.
    let mut system_out: Option<String> = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == b"testcase" => open = Some(testcase(&e)?),
            Event::Empty(e) if e.local_name().as_ref() == b"testcase" => cases.push(testcase(&e)?),
            Event::Start(e) if e.local_name().as_ref() == b"system-out" && open.is_some() => system_out = Some(String::new()),
            Event::Text(t) => {
                if let Some(buffer) = system_out.as_mut() {
                    buffer.push_str(&t.decode().map_err(|e| e.to_string())?);
                }
            }
            Event::CData(c) => {
                if let Some(buffer) = system_out.as_mut() {
                    buffer.push_str(&c.decode().map_err(|e| e.to_string())?);
                }
            }
            Event::GeneralRef(r) => {
                if let Some(buffer) = system_out.as_mut() {
                    if let Some(c) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                        buffer.push(c);
                    } else {
                        match r.decode().map_err(|e| e.to_string())?.as_ref() {
                            "amp" => buffer.push('&'),
                            "lt" => buffer.push('<'),
                            "gt" => buffer.push('>'),
                            "quot" => buffer.push('"'),
                            "apos" => buffer.push('\''),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"system-out" => {
                if let (Some(text), Some(case)) = (system_out.take(), open.as_mut()) {
                    case.attachments.extend(attachment_lines(&text));
                }
            }
            Event::Start(e) | Event::Empty(e) => {
                if let Some(case) = open.as_mut() {
                    match e.local_name().as_ref() {
                        b"failure" | b"error" => case.outcome = ReportOutcome::Failed,
                        b"skipped" if case.outcome != ReportOutcome::Failed => {
                            case.outcome = ReportOutcome::Skipped;
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

/// The `[[ATTACHMENT|<path>]]` lines of one `<system-out>`.
fn attachment_lines(text: &str) -> Vec<CaseFile> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("[[ATTACHMENT|")?.strip_suffix("]]"))
        .filter(|path| !path.is_empty())
        .map(|path| CaseFile::Path(path.to_string()))
        .collect()
}

fn testcase(e: &BytesStart<'_>) -> Result<ReportCase, String> {
    let attr = |name: &str| -> Result<String, String> {
        match e.try_get_attribute(name).map_err(|err| err.to_string())? {
            Some(a) => a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(std::borrow::Cow::into_owned)
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
        attachments: Vec::new(),
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
        let mut background: Option<(ReportOutcome, Vec<CaseFile>)> = None;
        for element in feature
            .get("elements")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let outcome = element_outcome(element);
            let files = element_embeddings(element)?;
            if element.get("type").and_then(|v| v.as_str()) == Some("background") {
                background = Some((outcome, files));
                continue;
            }
            let (outcome, attachments) = match background.take() {
                Some((before, mut earlier)) => {
                    earlier.extend(files);
                    (worse(before, outcome), earlier)
                }
                None => (outcome, files),
            };
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
                attachments,
            });
        }
    }
    if cases.is_empty() {
        return Err("no scenario element".to_string());
    }
    Ok(cases)
}

/// The embeddings of a Cucumber element's hooks and steps (`embeddings`,
/// or the newer `attachments`): base64 `data` with a `mime_type`
/// (`mimeType`, `media_type`, `media.type`); a `body` whose
/// `contentEncoding` is `IDENTITY` is the bytes as written.
fn element_embeddings(element: &serde_json::Value) -> Result<Vec<CaseFile>, String> {
    use base64::Engine;

    let mut files = Vec::new();
    for key in ["before", "steps", "after"] {
        for item in element.get(key).and_then(|v| v.as_array()).into_iter().flatten() {
            for list in ["embeddings", "attachments"] {
                for embedding in item.get(list).and_then(|v| v.as_array()).into_iter().flatten() {
                    let text = ["data", "body"].iter().find_map(|k| embedding.get(*k).and_then(|v| v.as_str())).unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    let mime = ["mime_type", "mimeType", "media_type", "mediaType"]
                        .iter()
                        .find_map(|k| embedding.get(*k).and_then(|v| v.as_str()))
                        .or_else(|| embedding.pointer("/media/type").and_then(|v| v.as_str()))
                        .unwrap_or_default()
                        .to_string();
                    let identity = ["contentEncoding", "content_encoding"].iter().find_map(|k| embedding.get(*k).and_then(|v| v.as_str())).is_some_and(|e| e.eq_ignore_ascii_case("identity"));
                    let bytes = if identity {
                        text.as_bytes().to_vec()
                    } else {
                        let compact: String = text.split_whitespace().collect();
                        base64::engine::general_purpose::STANDARD.decode(compact).map_err(|e| format!("an embedding of a {key} item is not valid base64: {e}"))?
                    };
                    files.push(CaseFile::Embedded { mime, bytes: bytes.into() });
                }
            }
        }
    }
    Ok(files)
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

/// Parse Swift Testing's JSON-lines event stream (`swift test
/// --experimental-event-stream-output`, module doc). A `test` record of
/// kind `function` declares a case, named by its `displayName`, else
/// its `name`; suites are not cases. A declared test with no event did
/// not run and is not a case. Its outcome comes from its `event`
/// records: an `issueRecorded` that is a failure (`isFailure`, or for a
/// stream without it an issue neither known nor a warning), or a
/// `testEnded` whose messages carry the `fail` symbol, fails it;
/// `testSkipped` skips it; `testStarted` then `testEnded` passes it;
/// started and never ended fails it. A suite's failing issue fails every
/// test inside it. A suite whose `testEnded` reports a failure that no
/// test or suite inside it recorded (setup or teardown failed) fails
/// every test inside it too; when a test inside did fail, the suite's
/// end only reports that, and its other tests keep their own outcome.
pub fn parse_swift_testing(text: &str) -> Result<Vec<ReportCase>, String> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct Seen {
        started: bool,
        ended: bool,
        skipped: bool,
        /// A failing issue recorded against it.
        failed: bool,
        /// Its `testEnded` reported a failure.
        ended_failed: bool,
    }
    let field = |value: &serde_json::Value, key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let mut functions: Vec<(String, String)> = Vec::new();
    let mut suites: Vec<String> = Vec::new();
    let mut seen: HashMap<String, Seen> = HashMap::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let at = index + 1;
        let record: serde_json::Value = serde_json::from_str(line).map_err(|e| format!("line {at}: {e}"))?;
        let payload = record.get("payload").ok_or_else(|| format!("line {at}: a record without a payload"))?;
        match record.get("kind").and_then(|v| v.as_str()) {
            Some("test") => {
                let id = field(payload, "id").ok_or_else(|| format!("line {at}: a test without an id"))?;
                if field(payload, "kind").as_deref() == Some("suite") {
                    suites.push(id);
                    continue;
                }
                let name = field(payload, "displayName")
                    .or_else(|| field(payload, "name"))
                    .filter(|n| !n.is_empty())
                    .ok_or_else(|| format!("line {at}: test `{id}` has no name"))?;
                functions.push((id, name));
            }
            Some("event") => {
                let Some(test_id) = field(payload, "testID") else { continue };
                let entry = seen.entry(test_id).or_default();
                match field(payload, "kind").as_deref() {
                    Some("testStarted") => entry.started = true,
                    Some("testEnded") => {
                        entry.ended = true;
                        entry.ended_failed |= payload.get("messages").and_then(|m| m.as_array()).is_some_and(|messages| {
                            messages.iter().any(|m| m.get("symbol").and_then(|s| s.as_str()) == Some("fail"))
                        });
                    }
                    Some("testSkipped") => entry.skipped = true,
                    Some("issueRecorded") => entry.failed |= payload.get("issue").is_some_and(issue_is_failure),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if functions.is_empty() {
        return Err("no test record of kind `function`".to_string());
    }
    let inside = |id: &str, suite: &str| id.strip_prefix(suite).is_some_and(|rest| rest.starts_with('/'));
    let own_failure = |id: &str, function: bool| seen.get(id).is_some_and(|s| s.failed || (function && s.ended_failed));
    let failed_suites: Vec<&String> = suites
        .iter()
        .filter(|suite| {
            let Some(state) = seen.get(*suite) else { return false };
            let explained = || {
                functions.iter().any(|(id, _)| inside(id, suite) && own_failure(id, true)) || suites.iter().any(|other| inside(other, suite) && own_failure(other, false))
            };
            state.failed || (state.ended_failed && !explained())
        })
        .collect();
    let cases: Vec<ReportCase> = functions
        .iter()
        .filter_map(|(id, name)| {
            let seen = seen.get(id)?;
            let in_failed_suite = failed_suites.iter().any(|suite| inside(id, suite));
            let outcome = if seen.failed || seen.ended_failed || in_failed_suite || (seen.started && !seen.ended && !seen.skipped) {
                ReportOutcome::Failed
            } else if seen.skipped {
                ReportOutcome::Skipped
            } else if seen.started {
                ReportOutcome::Passed
            } else {
                return None;
            };
            Some(ReportCase { name: name.clone(), group: String::new(), tags: Vec::new(), outcome, attachments: Vec::new() })
        })
        .collect();
    if cases.is_empty() {
        return Err("no test ran (no testStarted or testSkipped event names a declared test)".to_string());
    }
    Ok(cases)
}

/// Whether a Swift Testing `issue` fails its test: its `isFailure` when
/// the stream carries it, else an issue that is neither a known issue nor
/// a warning.
fn issue_is_failure(issue: &serde_json::Value) -> bool {
    if let Some(failure) = issue.get("isFailure").and_then(serde_json::Value::as_bool) {
        return failure;
    }
    let known = issue.get("isKnown").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let warning = issue.get("severity").and_then(|v| v.as_str()) == Some("warning");
    !known && !warning
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
        parse_junit("<testsuite><testcase name=\"x\">").unwrap_err();
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
        match_cases(&cases, &[], None).unwrap_err();
    }

    /// 0.14 acceptance rerun G7: a case binds only to its OWN leading
    /// scenario id. A case for `game.session.04` that mentions
    /// `game.session.01` binds to `.04` and never to `.01`, in dotted or
    /// underscored form; an id in the classname (a describe path) leads
    /// the name.
    #[test]
    fn a_case_binds_to_its_leading_scenario_id_never_to_one_it_mentions() {
        let report = r#"<testsuite>
            <testcase classname="src/session.test.ts" name="game.session.01: a new run starts at wave one"/>
            <testcase classname="src/session.test.ts" name="game.session.04: restarting resets the run as in game.session.01"><failure/></testcase>
            <testcase classname="session" name="tests::game_session_05_restart_mirrors_game_session_01"/>
            <testcase classname="game.session.06 &gt; pause" name="freezes the clock, unlike game.session.01"/>
            <testcase classname="session" name="build v1.2.10 then game.session.07"/>
        </testsuite>"#;
        let cases = parse_junit(report).unwrap();
        let bound = |id: &str| names(&match_cases(&cases, &[], Some(id)).unwrap()).into_iter().map(|(name, _)| name).collect::<Vec<_>>();

        assert_eq!(bound("game.session.01"), ["game.session.01: a new run starts at wave one"], "the mentions in .04, .05 and .06 bind nothing to .01");
        assert_eq!(bound("game.session.04"), ["game.session.04: restarting resets the run as in game.session.01"]);
        assert_eq!(bound("game.session.05"), ["tests::game_session_05_restart_mirrors_game_session_01"], "underscored leading id");
        assert_eq!(bound("game.session.06"), ["freezes the clock, unlike game.session.01"], "the describe path's id leads the name's mention");
        assert!(
            match_cases(&cases, &[], Some("game.session.07")).unwrap_err().contains("start the test name with the scenario id"),
            "an earlier id-shaped token (v1.2.10) leads, so the case is not .07's own"
        );
        assert_eq!(
            names(&match_cases(&cases, &["build v1.2.10 then game.session.07".to_string()], Some("game.session.07")).unwrap())[0].0,
            "build v1.2.10 then game.session.07",
            "--report-case still binds a case by name"
        );
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
        parse_cucumber("{}").unwrap_err();
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
            all_cases: false,
            verdict: Some(verdict),
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
        assert!(matches!(bind(&nameless), Err(BindError::Usage { message: m, .. }) if m.contains("no_such_test")), "a --report-case that names nothing refuses");

        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("x.log"), b"x").unwrap();
        let elsewhere = vec![outside.path().join("x.log")];
        let err = bind(&request(repo.path(), &elsewhere, &[], EvidenceVerdict::Faithful)).unwrap_err();
        assert!(matches!(&err, BindError::Usage { message: m, hint: h, field: Some("--artifact") } if m.contains("outside the repository") && h.contains("copy the file")), "{err:?}");

        let oversized = BindRequest { max_bytes: 4, ..request(repo.path(), &artifacts, &[], EvidenceVerdict::Faithful) };
        assert!(
            matches!(bind(&oversized), Err(BindError::Usage { message: m, .. }) if m.contains("--max-artifact-mib")),
            "a file over the size limit refuses, naming the override flag"
        );
    }

    /// A file that changed after it was bound refuses the whole store:
    /// the files before it are staged but never published, and no temp
    /// file is left behind.
    #[test]
    fn store_files_publishes_nothing_when_any_file_fails_to_stage() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("a.png"), b"first").unwrap();
        std::fs::write(repo.path().join("b.png"), b"second").unwrap();
        let paths = vec![repo.path().join("a.png"), repo.path().join("b.png")];
        let files: Vec<BoundFile> = bind_artifacts(repo.path(), &paths, 1024).unwrap().into_iter().map(|(_, f)| f).collect();
        std::fs::write(repo.path().join("b.png"), b"rewritten").unwrap();

        let (failed, error) = store_files(repo.path(), &files).unwrap_err();
        assert_eq!(failed.sha256, files[1].sha256, "the error names the file that changed");
        assert!(matches!(error, canon_gate::artifact_store::StoreError::Changed { .. }), "{error}");
        let store = canon_gate::artifact_store::store_dir(repo.path());
        assert_eq!(std::fs::read_dir(&store).unwrap().count(), 0, "nothing published and no temp file left");

        std::fs::write(repo.path().join("b.png"), b"second").unwrap();
        assert_eq!(store_files(repo.path(), &files).unwrap(), 2);
        assert_eq!(store_files(repo.path(), &files).unwrap(), 0, "already stored intact");
        assert_eq!(std::fs::read_dir(&store).unwrap().count(), 2);
    }

    #[test]
    fn report_spec_parses_format_and_path() {
        assert_eq!(
            parse_report_spec("junit:target/nextest/junit.xml").unwrap(),
            (ReportFormat::Junit, PathBuf::from("target/nextest/junit.xml"))
        );
        parse_report_spec("tap:out.tap").unwrap_err();
        parse_report_spec("junit:").unwrap_err();
        parse_report_spec("junit.xml").unwrap_err();
        assert_eq!(
            parse_report_spec("swift-testing:.build/events.jsonl").unwrap(),
            (ReportFormat::SwiftTesting, PathBuf::from(".build/events.jsonl"))
        );
    }

    /// The event stream a real `swift test --experimental-event-stream-output`
    /// run wrote (Swift 6.4, `tests/fixtures/swift-testing`).
    const SWIFT_EVENTS: &str = include_str!("../tests/fixtures/swift-testing/events.jsonl");

    /// evidence.add.85: display name first, then the function name; a
    /// failing issue fails, `testSkipped` skips, started-and-ended passes,
    /// a parameterized test is one case; suites are not cases.
    #[test]
    fn evidence_add_85_swift_testing_stream_parses_names_and_outcomes() {
        let mut cases: Vec<(String, ReportOutcome)> = parse_swift_testing(SWIFT_EVENTS).unwrap().into_iter().map(|c| (c.name, c.outcome)).collect();
        cases.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            cases,
            vec![
                ("shop.cart.01 adding an item counts it".to_string(), ReportOutcome::Passed),
                ("shop.cart.02 an empty cart counts zero".to_string(), ReportOutcome::Passed),
                ("shop.cart.03 adding twice keeps both".to_string(), ReportOutcome::Passed),
                ("shop.cart.05 a deliberately failing expectation".to_string(), ReportOutcome::Failed),
                ("shop.cart.06 not run yet".to_string(), ReportOutcome::Skipped),
                ("shop_cart_04_count_never_negative()".to_string(), ReportOutcome::Passed),
            ]
        );
        let parsed = parse_swift_testing(SWIFT_EVENTS).unwrap();
        assert_eq!(names(&match_cases(&parsed, &[], Some("shop.cart.04")).unwrap())[0].0, "shop_cart_04_count_never_negative()", "the function name carries the id");

        let test = |id: &str| format!(r#"{{"kind":"test","payload":{{"id":"M.S/{id}()","kind":"function","name":"{id}()"}},"version":0}}"#);
        let event = |id: &str, kind: &str, extra: &str| format!(r#"{{"kind":"event","payload":{{"kind":"{kind}","testID":"M.S/{id}()"{extra}}},"version":0}}"#);
        let stream = [
            r#"{"kind":"test","payload":{"id":"M.S","kind":"suite","name":"S"},"version":0}"#.to_string(),
            test("a_b_01"),
            test("a_b_02"),
            test("a_b_03"),
            test("a_b_04"),
            event("a_b_01", "testStarted", ""),
            event("a_b_01", "issueRecorded", r#","issue":{"isKnown":true}"#),
            event("a_b_01", "testEnded", ""),
            event("a_b_02", "testStarted", ""),
            event("a_b_02", "issueRecorded", r#","issue":{"isKnown":false}"#),
            event("a_b_02", "testEnded", ""),
            event("a_b_03", "testStarted", ""),
        ]
        .join("\n");
        let outcomes: Vec<(String, ReportOutcome)> = parse_swift_testing(&stream).unwrap().into_iter().map(|c| (c.name, c.outcome)).collect();
        assert_eq!(
            outcomes,
            vec![
                ("a_b_01()".to_string(), ReportOutcome::Passed),
                ("a_b_02()".to_string(), ReportOutcome::Failed),
                ("a_b_03()".to_string(), ReportOutcome::Failed),
            ],
            "a known issue passes (a stream without isFailure), an unknown one fails, a test that never ended fails, a test that never ran is no case"
        );
        let suite_issue = format!("{stream}\n{}", r#"{"kind":"event","payload":{"kind":"issueRecorded","testID":"M.S","issue":{"isFailure":true}},"version":0}"#);
        assert!(parse_swift_testing(&suite_issue).unwrap().iter().all(|c| c.outcome == ReportOutcome::Failed), "a suite's failing issue fails every test in it");
        let suite_end_fail = r#"{"kind":"event","payload":{"kind":"testEnded","testID":"M.S","messages":[{"symbol":"fail","text":"Suite \"S\" failed after 0.001 seconds with 1 issue."}]},"version":0}"#;
        let unexplained = [
            r#"{"kind":"test","payload":{"id":"M.S","kind":"suite","name":"S"},"version":0}"#.to_string(),
            test("a_b_01"),
            event("a_b_01", "testStarted", ""),
            event("a_b_01", "testEnded", ""),
            suite_end_fail.to_string(),
        ]
        .join("\n");
        assert_eq!(
            parse_swift_testing(&unexplained).unwrap()[0].outcome,
            ReportOutcome::Failed,
            "a suite whose end reports a failure no test inside it recorded fails its tests, with no issueRecorded for them"
        );
        let explained = format!("{stream}\n{suite_end_fail}");
        assert_eq!(
            parse_swift_testing(&explained).unwrap()[0],
            ReportCase { name: "a_b_01()".to_string(), group: String::new(), tags: Vec::new(), outcome: ReportOutcome::Passed, attachments: Vec::new() },
            "a suite's end that reports a failing test inside it leaves that test's siblings as they ended"
        );
        let ended_failed = [test("a_b_01"), event("a_b_01", "testStarted", ""), event("a_b_01", "testEnded", r#","messages":[{"symbol":"fail","text":"failed"}]"#)].join("\n");
        assert_eq!(parse_swift_testing(&ended_failed).unwrap()[0].outcome, ReportOutcome::Failed, "a test whose own end reports a failure failed");

        assert!(parse_swift_testing("<testsuite/>").unwrap_err().starts_with("line 1:"), "not JSON lines");
        assert!(parse_swift_testing(&test("a_b_01")).unwrap_err().contains("no test ran"));
        assert!(parse_swift_testing("").unwrap_err().contains("no test record"));
    }

    /// evidence.add.80: each indexed scenario's record carries its own
    /// cases and a verdict derived from them, after the shared artifact;
    /// the report is one file.
    #[test]
    fn evidence_add_80_a_batch_splits_the_report_by_owner() {
        let repo = TempDir::new().unwrap();
        let report = r#"<testsuite>
            <testcase classname="cart" name="shop.cart.01 adds"/>
            <testcase classname="cart" name="shop.cart.02 refuses"><failure/></testcase>
            <testcase classname="cart" name="shop.cart.02 counts"/>
            <testcase classname="cart" name="tests::shop_cart_03_skips"><skipped/></testcase>
            <testcase classname="cart" name="shop.cart.09 is not indexed"/>
            <testcase classname="cart" name="helper works"/>
        </testsuite>"#;
        std::fs::write(repo.path().join("junit.xml"), report).unwrap();
        std::fs::write(repo.path().join("trace.zip"), b"trace").unwrap();
        let reports = vec![(ReportFormat::Junit, repo.path().join("junit.xml"))];
        let artifacts = vec![("shop.cart.01".to_string(), repo.path().join("trace.zip"))];
        let request = BatchRequest {
            repo: repo.path(),
            scoped_artifacts: &artifacts,
            reports: &reports,
            indexed: &["shop.cart.01", "shop.cart.02", "shop.cart.03", "shop.cart.04"],
            verdict: None,
            max_bytes: 1024 * 1024,
        };
        let batch = bind_all_scenarios(&request).unwrap();
        let summary: Vec<(&str, EvidenceVerdict, Vec<Option<&str>>)> = batch
            .scenarios
            .iter()
            .map(|s| (s.scenario_id.as_str(), s.verdict, s.attachments.iter().map(|a| a.case.as_deref()).collect()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("shop.cart.01", EvidenceVerdict::Faithful, vec![None, Some("shop.cart.01 adds")]),
                ("shop.cart.02", EvidenceVerdict::Divergent, vec![Some("shop.cart.02 refuses"), Some("shop.cart.02 counts")]),
                ("shop.cart.03", EvidenceVerdict::NotApplicable, vec![Some("tests::shop_cart_03_skips")]),
            ],
            "shop.cart.04 owns no case and gets no record"
        );
        assert_eq!(batch.files.len(), 2, "the artifact and the report, once each");
        assert_eq!(batch.unindexed, vec![("shop.cart.09".to_string(), "shop.cart.09 is not indexed".to_string())]);
        assert_eq!(batch.without_id, 1);

        let faithful = bind_all_scenarios(&BatchRequest { verdict: Some(EvidenceVerdict::Faithful), ..request }).unwrap_err();
        assert!(
            matches!(&faithful, BindError::Contradiction(m) if m.contains("shop.cart.02: ") && m.contains("`shop.cart.02 refuses` as failed") && m.contains("shop.cart.03: ") && m.contains("no record of the batch is written")),
            "{faithful:?}"
        );
        let nobody = bind_all_scenarios(&BatchRequest { indexed: &["shop.cart.05"], ..request }).unwrap_err();
        assert!(matches!(&nobody, BindError::Usage { message: m, .. } if m.contains("not indexed: shop.cart.01, shop.cart.02, shop.cart.09, shop_cart_03")), "{nobody:?}");
        assert_eq!(derived_verdict([ReportOutcome::Passed, ReportOutcome::Skipped, ReportOutcome::Failed]), EvidenceVerdict::Divergent);
    }

    /// evidence.add.84: with `all_cases` every case is bound, so one
    /// failed case refuses a faithful verdict.
    #[test]
    fn evidence_add_84_all_cases_binds_every_case() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("junit.xml"), JUNIT).unwrap();
        let reports = vec![(ReportFormat::Junit, repo.path().join("junit.xml"))];
        let all = BindRequest { scenario_id: None, all_cases: true, ..request(repo.path(), &[], &reports, EvidenceVerdict::Divergent) };
        assert_eq!(bind(&all).unwrap().attachments.len(), 5);
        let faithful = BindRequest { scenario_id: None, all_cases: true, ..request(repo.path(), &[], &reports, EvidenceVerdict::Faithful) };
        assert!(matches!(bind(&faithful), Err(BindError::Contradiction(m)) if m.contains("cart.add.010 later")));
    }

    /// A testcase's attachments are the `[[ATTACHMENT|..]]` lines of ITS
    /// system-out (text, CDATA, entities), not another case's nor a
    /// suite-level one.
    #[test]
    fn junit_attachments_belong_to_their_case() {
        let cases = parse_junit(
            "<testsuite><system-out>[[ATTACHMENT|suite.png]]</system-out>\
             <testcase name=\"a.b.01\"><system-out><![CDATA[\n[[ATTACHMENT|x/1.png]]\n\n  [[ATTACHMENT|x/2.png]]\nnot one\n]]></system-out></testcase>\
             <testcase name=\"a.b.02\"><system-out>[[ATTACHMENT|a&amp;b.png]]</system-out></testcase>\
             <testcase name=\"a.b.03\"/></testsuite>",
        )
        .unwrap();
        let paths = |case: &ReportCase| -> Vec<String> { case.attachments.iter().map(|f| match f { CaseFile::Path(p) => p.clone(), CaseFile::Embedded { .. } => unreachable!() }).collect() };
        assert_eq!(paths(&cases[0]), ["x/1.png", "x/2.png"]);
        assert_eq!(paths(&cases[1]), ["a&b.png"]);
        assert!(cases[2].attachments.is_empty());
    }

    /// `<id>=<path>` scopes only when the prefix is shaped like a scenario id.
    #[test]
    fn artifact_scope_needs_an_id_shaped_prefix() {
        assert_eq!(parse_artifact(Path::new("cart.add.01=shots/a.png")), (Some("cart.add.01".to_string()), PathBuf::from("shots/a.png")));
        assert_eq!(parse_artifact(Path::new("shots/a=b.png")), (None, PathBuf::from("shots/a=b.png")));
        assert_eq!(parse_artifact(Path::new("./cart.add.01=a.png")), (None, PathBuf::from("./cart.add.01=a.png")));
    }
}
