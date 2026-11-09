//! `canon artifacts verify` and `canon artifacts move` (0.23 D3; 0.28 D1, D6): the
//! bytes behind evidence digests, off git.
//!
//! # `verify [--full]`
//! Reads the gate's own set of bound files (the latest evidence's and the
//! current reviews', [`canon_gate::artifact_store::bound_files`]) and asks
//! the configured store ([`canon_gate::artifact_store::ArtifactStore`])
//! about each. Default: presence and size/digest metadata (one HEAD per
//! distinct digest for an object store); `--full` downloads and re-hashes.
//! Exit `0` when every file is stored (or, local store only, still proven
//! by the working tree, as the gate does), `1` when a file is missing or
//! altered, `2` when the store cannot be used or reached — an unreachable
//! store is a refusal, never a pass.
//!
//! It also counts the stale staging temp files of the local store (0.39, [`crate::stale_temps`]) and says to run `canon artifacts prune` for
//! them; it never deletes one, and they never change the exit: they are no bound file's problem. A local store directory it cannot read leaves
//! the count unknown, which it says (never as zero).
//!
//! # `move --tier <tier> [--commit --plan-digest D]`
//! Moves a repo's local blobs to the object-store tier `--tier` names. The bare
//! run previews (the uploads and removals as `effects[]`, with a `planDigest`)
//! and writes nothing; `--commit --plan-digest` does exactly that plan:
//! 1. every blob in `.canon/artifacts/sha256` is hashed (a blob that does
//!    not match its name is never migrated) and uploaded by digest key,
//!    read back, and — when the object was already there — re-hashed from
//!    the store, because the local copy is about to go;
//! 2. only when every blob verified, `canon.yaml` gains
//!    `artifact_store: { backend: <to>, prefix: artifacts/ }` (a block canon
//!    did not write is never rewritten: it must already name `--tier`);
//! 3. then the local copies are removed and `.canon/artifacts/` is added to
//!    `.gitignore`.
//!
//! Local copies leave only after the store is proven to hold their bytes,
//! and the config switches before they leave, so a run stopped anywhere
//! leaves a repo the gate still passes and the next run finishes. A second
//! run finds nothing to do. `--tier local` is a no-op on a local repo; moving
//! bytes back from an object store is not supported.

use std::path::{Path, PathBuf};

use canon_gate::artifact_store::{
    artifacts_config, bound_files, check_bound_files, is_sha256_hex, sha256_reader, store_dir, ArtifactStore, ArtifactStoreError, FileStatus, Observed, StoreLock, StoreOutcome, OBJECT_STORE_HINT,
};
use canon_gate::{GateContext, GateCtx};
use canon_policy::SchemaRegistry;
use canon_store::artifact_blobs::{ArtifactsBackend, ArtifactsConfig, DEFAULT_PREFIX};
use chrono::Utc;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli_error::{CliError, CliErrorExt as _};
use crate::context::resolve_repo_root;
use crate::json_out::Report;
use crate::safety::{Effect, Mode, Plan, Preview, Reads};

/// The `.gitignore` line an object-store repo gains: the local store is
/// gone, and a stray local blob must never be committed.
const GITIGNORE_LINE: &str = ".canon/artifacts/";

/// A refusal for a store that cannot be opened (an unusable `artifact_store:`
/// block) or attached; the caller emits it under its own command.
pub(crate) fn store_error(e: &ArtifactStoreError) -> CliError {
    store_refusal(e, e.to_string())
}

/// The refusal for `e` with `message`: the code and reason [`store_code`] gives it and its own hint.
pub(crate) fn store_refusal(e: &ArtifactStoreError, message: impl Into<String>) -> CliError {
    let (code, reason) = store_code(e);
    let error = CliError::refused(code, message).hint(e.hint());
    match reason {
        Some(reason) => error.reason(reason),
        None => error,
    }
}

/// The code (and reason) of an artifact store that cannot be used before anything is written, by what is wrong:
/// - an `artifact_store:` section that is unusable is the config's (`CONFIG_INVALID`, reason `ARTIFACT_STORE`; `CONFIG_DENIED`
///   when canon.yaml may not be read): the user wrote it;
/// - a tier that cannot be attached (its endpoint, credentials or bucket are unset) is `ARTIFACTS_STORE_REQUIRED`: waiting does
///   not set them;
/// - a store directory that this user may not read or enter is `ARTIFACTS_STORE_DENIED`; one that is no real directory (a symlink, a
///   regular file, a path that is no directory) is `ARTIFACTS_STORE_INVALID`; nothing else an operating system says becomes
///   `UNAVAILABLE`;
/// - a digest that is not 64 lowercase hex characters is `ARTIFACTS_DIGEST_INVALID`;
/// - a store lock another process holds, an object store that did not answer, an operating-system error a repeat can pass (`EIO`, an
///   interrupted or timed-out call) is `ARTIFACTS_STORE_UNAVAILABLE`, the one retryable code of these.
///
/// A file that changed since it was bound is the caller's own code (`EVIDENCE_BINDING_STALE`, `REVIEW_ARTIFACT_STALE`).
pub(crate) fn store_code(e: &ArtifactStoreError) -> (&'static str, Option<&'static str>) {
    use crate::read_cause::{of_io, Cause};
    use canon_gate::artifact_store::ArtifactConfigError;
    match e {
        ArtifactStoreError::Changed { .. } => ("EVIDENCE_BINDING_STALE", None),
        ArtifactStoreError::Config(ArtifactConfigError::Read { source, .. }) if of_io(source) == Cause::Denied => ("CONFIG_DENIED", None),
        ArtifactStoreError::Config(ArtifactConfigError::Read { source, .. }) if of_io(source) == Cause::Unavailable => ("CONFIG_UNAVAILABLE", None),
        ArtifactStoreError::Config(_) => ("CONFIG_INVALID", Some("ARTIFACT_STORE")),
        ArtifactStoreError::Unattached { .. } => ("ARTIFACTS_STORE_REQUIRED", None),
        ArtifactStoreError::UnsafeStoreDir => ("ARTIFACTS_STORE_INVALID", None),
        ArtifactStoreError::InvalidDigest(_) => ("ARTIFACTS_DIGEST_INVALID", None),
        ArtifactStoreError::Locked(_) | ArtifactStoreError::Remote { .. } => ("ARTIFACTS_STORE_UNAVAILABLE", None),
        ArtifactStoreError::Io(io) => match of_io(io) {
            Cause::Denied => ("ARTIFACTS_STORE_DENIED", None),
            Cause::Unavailable => ("ARTIFACTS_STORE_UNAVAILABLE", None),
            Cause::NotFound | Cause::WrongPath | Cause::Other => ("ARTIFACTS_STORE_INVALID", None),
        },
    }
}

/// Whether `e` is what the artifact store reports before it has written
/// anything: a `canon.yaml` that cannot be used, a tier that cannot be
/// attached, a store directory that is not a real directory, a lock another
/// process holds, a digest that is not one. That is a precondition (exit 2);
/// a remote or file-system failure while blobs are being written is a failure
/// during the work (exit 1).
pub(crate) fn before_first_write(e: &ArtifactStoreError) -> bool {
    matches!(e, ArtifactStoreError::Config(_) | ArtifactStoreError::Unattached { .. } | ArtifactStoreError::UnsafeStoreDir | ArtifactStoreError::Locked(_) | ArtifactStoreError::InvalidDigest(_) | ArtifactStoreError::Changed { .. })
}

/// The code `evidence add` reports when binding or storing a bound file fails:
/// `EVIDENCE_BINDING_STALE` for a file that changed since it was bound, the code of [`store_code`] (exit 2) when the store could
/// not be used before anything was written, `ARTIFACTS_STORE_FAILED` (exit 1) when a write to it failed and nothing was kept; the reason is the one
/// [`store_code`] gives.
pub(crate) fn evidence_store_code(e: &ArtifactStoreError) -> (&'static str, Option<&'static str>) {
    match e {
        ArtifactStoreError::Changed { .. } => ("EVIDENCE_BINDING_STALE", None),
        _ if before_first_write(e) => store_code(e),
        _ => ("ARTIFACTS_STORE_FAILED", None),
    }
}

/// What a store of bound files writes first: the repo's artifact-store lock and,
/// for the local store, the blob directory. An object store has no local
/// directory; reaching it is [`store_error`]'s `ARTIFACTS_STORE_UNAVAILABLE`.
pub(crate) fn store_targets(repo: &Path) -> Vec<PathBuf> {
    let mut targets = vec![repo.join(canon_model::paths::ARTIFACT_STORE_LOCK_FILE)];
    if ArtifactStore::open(repo).is_ok_and(|store| store.is_local()) {
        targets.push(store_dir(repo));
    }
    targets
}

/// The code of a local blob that cannot be read: the system refuses it (`ARTIFACTS_BLOB_DENIED`), it is no file
/// (`ARTIFACTS_STORE_INVALID`), or the system failed in a way a repeat can pass (`ARTIFACTS_STORE_UNAVAILABLE`).
fn unreadable_blob_code(e: &std::io::Error) -> &'static str {
    use crate::read_cause::{of_io, Cause};
    match of_io(e) {
        Cause::Denied => "ARTIFACTS_BLOB_DENIED",
        Cause::Unavailable => "ARTIFACTS_STORE_UNAVAILABLE",
        Cause::NotFound | Cause::WrongPath | Cause::Other => "ARTIFACTS_STORE_INVALID",
    }
}

/// The one code a refusal that found several blob problems carries: a blob whose content is broken (`ARTIFACTS_BLOB_CORRUPT`) outranks one that
/// cannot be read at all, because repeating or re-permitting cannot mend it; then the system's refusal, a path that is no file, a repeat.
fn blob_problem_code(problems: &[(&'static str, String)]) -> &'static str {
    ["ARTIFACTS_BLOB_CORRUPT", "ARTIFACTS_BLOB_DENIED", "ARTIFACTS_STORE_INVALID", "ARTIFACTS_STORE_UNAVAILABLE"].into_iter().find(|code| problems.iter().any(|(c, _)| c == code)).unwrap_or("ARTIFACTS_BLOB_CORRUPT")
}

/// What to do about the blob problem [`blob_problem_code`] chose.
fn blob_hint(code: &str) -> &'static str {
    match code {
        "ARTIFACTS_BLOB_DENIED" => "make the local blob the problem line names readable by this user, then run the command again",
        "ARTIFACTS_STORE_INVALID" => "something that is no file stands where a blob belongs under .canon/artifacts/sha256: remove it, then run the command again",
        "ARTIFACTS_STORE_UNAVAILABLE" => "run the command again: the system failed the read, or another process changed a blob while it was uploaded",
        _ => "a blob that does not match its name proves nothing: delete it (`git rm`) if no record needs it, or re-run and re-attest the evidence, then run the command again",
    }
}

/// `canon artifacts verify`.
pub fn run_verify(repo: &Path, full: bool, json: bool) -> u8 {
    const CMD: &str = "canon artifacts verify";
    let repo = resolve_repo_root(repo);
    let gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(context) => context,
        Err(e) => {
            return crate::read_cause::config_error(e).emit("canon artifacts verify", json);
        }
    };
    let store = match ArtifactStore::open(&repo) {
        Ok(store) => store,
        Err(e) => return store_error(&e).emit("canon artifacts verify", json),
    };
    if bound_files(&gate_context).is_empty() {
        crate::say!(json, "{CMD}: no bound files to check");
    }
    let checks = check_bound_files(&gate_context, &store, full);
    if let Some(Observed::Unavailable(why)) = checks.iter().map(|c| &c.blob).find(|b| matches!(b, Observed::Unavailable(_))) {
        return CliError::refused("ARTIFACTS_STORE_UNAVAILABLE", format!("the artifact store could not be asked about {} bound file(s): {why}", checks.len())).hint(OBJECT_STORE_HINT).emit("canon artifacts verify", json);
    }
    let stored = checks.iter().filter(|c| c.status == FileStatus::Stored).count();
    let tree_only = checks.iter().filter(|c| c.status == FileStatus::WorkingTreeOnly).count();
    let problems: Vec<&canon_gate::artifact_store::FileCheck> = checks.iter().filter(|c| c.status == FileStatus::Failed).collect();
    let how = if full { "re-hashed" } else { "checked" };
    crate::say!(
        json,
        "{CMD}: {} bound file(s) {how} against {} — stored {stored}, verified by the working tree only {tree_only}, problems {}",
        checks.len(),
        store.location(),
        problems.len()
    );
    let problem_lines: Vec<String> = problems
        .iter()
        .map(|c| {
            let kind = match &c.blob {
                Observed::Missing => "missing from the store".to_string(),
                other => format!("does not match its name: {}", other.as_str()),
            };
            format!("{} `{}` (sha256 {}): {kind}", c.file.subject, c.file.path, c.file.sha256)
        })
        .collect();
    for line in &problem_lines {
        crate::say!(json, "  problem {line}");
    }
    let temps = StaleTempsFound::read(&repo);
    if let Some(line) = temps.line() {
        crate::say!(json, "  {line}");
    }
    let data = ArtifactsVerifyReport {
        stale_temps: temps,
        backend: match artifacts_config(&repo).map(|c| c.backend) {
            Ok(ArtifactsBackend::Tier { name, .. }) => name,
            _ => "local".to_string(),
        },
        full,
        bound: checks.len(),
        stored,
        working_tree_only: tree_only,
        problems: problems
            .iter()
            .zip(&problem_lines)
            .map(|(c, line)| VerifyProblem { subject: &c.file.subject, path: &c.file.path, sha256: &c.file.sha256, blob: c.blob.as_str(), detail: line })
            .collect(),
    };
    let mut coverage = crate::coverage::Coverage::new();
    coverage.count("boundFiles", checks.len()).count("stored", stored).count("workingTreeOnly", tree_only);
    if problems.is_empty() {
        if json {
            crate::json_out::data_with_meta("artifacts verify", &data, coverage.meta());
        }
        return 0;
    }
    CliError::failed("ARTIFACTS_VERIFY_FAILED", format!("{} bound file(s) are missing from the artifact store or altered", problems.len()))
        .hint("re-upload a file the working tree still holds with `canon artifacts store`, or re-run and re-attest it with `canon evidence add`; see data.problems")
        .emit_with_data_and_meta("canon artifacts verify", json, &data, coverage.meta())
}

/// `canon artifacts verify`: what the check looked at and the files that failed it.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsVerifyReport<'a> {
    /// `local`, or the name of the artifact store tier.
    pub backend: String,
    /// Whether every blob was re-hashed (`--full`).
    pub full: bool,
    /// Files the ledger's evidence binds.
    pub bound: usize,
    /// Files whose blob is in the store.
    pub stored: usize,
    /// Files proven by the working tree alone (local store only).
    pub working_tree_only: usize,
    /// The stale staging temp files a dead write left in the local store: they change neither `problems` nor the exit.
    pub stale_temps: StaleTempsFound,
    pub problems: Vec<VerifyProblem<'a>>,
}

impl crate::json_out::Report for ArtifactsVerifyReport<'_> {
    /// 1.1: `staleTemps`.
    const SCHEMA_VERSION: &'static str = "1.1";
}

/// What `artifacts verify` found of the stale temp files (`.tmp-<sha256>-<pid>-<nanos>` older than five minutes) of the local store.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaleTempsFound {
    /// How many there are; absent when the store directory could not be read (the count is then unknown, not zero).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<usize>,
    /// Their bytes together; absent with `files`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Why the store directory could not be read, when it could not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreadable: Option<String>,
}

impl StaleTempsFound {
    fn read(repo: &Path) -> Self {
        match crate::stale_temps::find(repo, std::time::SystemTime::now()) {
            Ok(found) => Self { files: Some(found.len()), bytes: Some(found.iter().map(|temp| temp.bytes).sum()), unreadable: None },
            Err(e) => Self { files: None, bytes: None, unreadable: Some(e.to_string()) },
        }
    }

    /// The line the human output gets: none when there are none.
    fn line(&self) -> Option<String> {
        match (self.files, self.bytes, &self.unreadable) {
            (Some(files), Some(bytes), _) if files > 0 => Some(format!("stale temp file(s) {files} ({bytes} bytes) left by writes that died; `canon artifacts prune` lists and deletes them")),
            (_, _, Some(why)) => Some(format!("stale temp files: could not be counted, the artifact store directory cannot be read: {why}")),
            _ => None,
        }
    }
}

/// One bound file `artifacts verify` found missing or altered.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyProblem<'a> {
    pub subject: &'a str,
    pub path: &'a str,
    pub sha256: &'a str,
    /// What the store holds: `missing`, or what it observed instead.
    pub blob: &'a str,
    pub detail: &'a str,
}

/// One blob in the local store.
pub(crate) struct LocalBlob {
    pub(crate) digest: String,
    pub(crate) path: PathBuf,
}

/// The regular files of the local store named by a digest, sorted, and
/// the names of entries that are neither those nor a stale temp file.
pub(crate) fn local_blobs(repo: &Path) -> std::io::Result<(Vec<LocalBlob>, Vec<String>)> {
    let dir = store_dir(repo);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err(std::io::Error::other(format!("{} is not a real directory", dir.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
        Err(e) => return Err(e),
    }
    let (mut blobs, mut skipped) = (Vec::new(), Vec::new());
    for entry in canon_store::eio::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && is_sha256_hex(&name) {
            blobs.push(LocalBlob { digest: name, path: entry.path() });
        } else if !name.starts_with(".tmp-") {
            skipped.push(name);
        }
    }
    blobs.sort_by(|a, b| a.digest.cmp(&b.digest));
    skipped.sort();
    Ok((blobs, skipped))
}

/// The `artifact_store:` block to append when `canon.yaml` has none.
fn artifacts_block(to: &str) -> String {
    format!("artifact_store:\n  backend: {to}\n  prefix: {DEFAULT_PREFIX}\n")
}

/// The target config for `--to`, and the new `canon.yaml` text when the
/// block still has to be written. `Err` is the refusal message.
fn resolve_target(repo: &Path, to: &str) -> Result<(ArtifactsConfig, Option<String>), String> {
    let path = repo.join("canon.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("reading `{}`: {e}", path.display()))?;
    let has_block = serde_yaml::from_str::<serde_yaml::Value>(&text).map_err(|e| format!("parsing `{}`: {e}", path.display()))?.get("artifact_store").is_some();
    if has_block {
        let current = ArtifactsConfig::from_yaml_at(&text, repo).map_err(|e| e.to_string())?;
        return match &current.backend {
            ArtifactsBackend::Tier { name, .. } if name == to => Ok((current, None)),
            ArtifactsBackend::Tier { name, .. } => Err(format!("canon.yaml's `artifact_store:` block already names tier `{name}`, not `{to}`; blobs live there, and canon does not move them between object stores")),
            ArtifactsBackend::Local => Err(format!("canon.yaml's `artifact_store:` block says `backend: local`; set it to `backend: {to}` yourself (canon does not rewrite a block it did not write), then run the command again")),
        };
    }
    let separator = if text.ends_with('\n') { "\n" } else { "\n\n" };
    let candidate = format!("{text}{separator}{}", artifacts_block(to));
    let config = ArtifactsConfig::from_yaml_at(&candidate, repo).map_err(|e| e.to_string())?;
    Ok((config, Some(candidate)))
}

/// Whether `<repo>/.gitignore` already lists the local store.
fn gitignored(repo: &Path) -> bool {
    std::fs::read_to_string(repo.join(".gitignore")).unwrap_or_default().lines().any(|line| line.trim() == GITIGNORE_LINE)
}

/// `.canon/artifacts/` in `<repo>/.gitignore`, appended under a comment
/// when absent. `true` when it was added.
fn ensure_gitignored(repo: &Path) -> std::io::Result<bool> {
    use std::io::Write;
    let path = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == GITIGNORE_LINE) {
        return Ok(false);
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(format!("# the artifact bytes live in the object store (canon.yaml `artifact_store:`), never in git.\n{GITIGNORE_LINE}\n").as_bytes())?;
    Ok(true)
}

/// `canon artifacts move`'s `data`: the plan and what the run did to the blobs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsMoveReport {
    /// The tier the blobs live in after the move (`local` for the no-op).
    pub backend: String,
    /// Blobs in the local store.
    pub blobs: usize,
    /// Blobs this run uploaded (0 in a preview).
    pub uploaded: usize,
    /// Blobs the store already held, verified.
    pub already_in_store: usize,
    /// Local copies this run removed (0 in a preview).
    pub removed_local: usize,
    pub config_written: bool,
    pub gitignore_added: bool,
    /// Entries of the local store that are not blobs and stay.
    pub skipped: Vec<String>,
    #[serde(flatten)]
    pub plan: Preview,
}

impl Report for ArtifactsMoveReport {
    /// 1.1: the plan is `atip::plan::Preview` (`planDigest` is `sha256:` and 64 hex digits).
    const SCHEMA_VERSION: &'static str = "1.1";
}

/// What the read-only pass found: the plan's effects and the blobs that
/// verified (the ones a commit uploads and removes).
struct MovePlan<'a> {
    effects: Vec<Effect>,
    verified: Vec<&'a LocalBlob>,
    already: usize,
}

/// `canon artifacts move --tier <TIER> [--commit --plan-digest D]`: the bare
/// run previews the uploads and removals; `--commit` does exactly that plan.
pub fn run_move(repo: &crate::cli::RepoArgs, to: &str, mode: &Mode, json: bool) -> u8 {
    const CMD: &str = "canon artifacts move";
    let refuse = |code: &'static str, message: String, hint: &str| CliError::refused(code, message).hint(hint.to_string()).emit("canon artifacts move", json);
    let repo = resolve_repo_root(repo);

    let current = match artifacts_config(&repo) {
        Ok(current) => current,
        Err(e) => return store_error(&e).emit("canon artifacts move", json),
    };
    if to == "local" {
        if matches!(current.backend, ArtifactsBackend::Local) {
            // Nothing to move: the empty plan still takes the digest handshake.
            let plan = match Plan::new("artifacts move", Vec::new(), Reads::new().path(&repo, "canon.yaml")) {
                Ok(plan) => plan,
                Err(e) => return e.emit("canon artifacts move", json),
            };
            let commit = match plan.gate(mode) {
                Ok(commit) => commit,
                Err(e) => return e.emit("canon artifacts move", json),
            };
            if commit {
                crate::json_out::mark_unchanged();
            }
            crate::say!(json, "{CMD}: the artifact store is already local; nothing to move");
            let report = ArtifactsMoveReport { backend: "local".into(), blobs: 0, uploaded: 0, already_in_store: 0, removed_local: 0, config_written: false, gitignore_added: false, skipped: Vec::new(), plan: if commit { plan.committed() } else { plan.preview() } };
            if json {
                if commit {
                    crate::json_out::data("artifacts move", &report);
                } else {
                    // The empty plan still takes the handshake: the preview's `meta.next[0]` is the commit (spec 12.5).
                    crate::json_out::data_with_next("artifacts move", &report, &plan.next_step("the store is already local, so the plan is empty; committing it takes its planDigest"));
                }
            }
            return 0;
        }
        return refuse(
            "ARTIFACTS_TIER_INVALID",
            "moving artifact bytes back from an object store to `local` is not supported".to_string(),
            "the bytes stay in the store; to read them again, keep `artifact_store.backend` as it is",
        );
    }
    let (target, new_yaml) = match resolve_target(&repo, to) {
        Ok(resolved) => resolved,
        Err(message) => return refuse("ARTIFACTS_TIER_INVALID", message, "`--tier` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)"),
    };
    let store = match ArtifactStore::from_config(target) {
        Ok(store) => store,
        Err(e) => return store_error(&e).emit("canon artifacts move", json),
    };
    let ArtifactStore::Object(object) = &store else {
        return refuse("ARTIFACTS_TIER_INVALID", format!("`{to}` is not an object-store tier"), "`--tier` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)");
    };
    let (blobs, skipped) = match local_blobs(&repo) {
        Ok(listing) => listing,
        Err(e) => {
            let message = format!("cannot read the local artifact store: {e}");
            return store_refusal(&ArtifactStoreError::Io(e), message).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again").emit("canon artifacts move", json);
        }
    };

    // A commit rewrites canon.yaml and .gitignore after its uploads: refuse a read-only one before anything is uploaded.
    if matches!(mode, Mode::Commit(_)) {
        let (yaml, gitignore) = (repo.join("canon.yaml"), repo.join(".gitignore"));
        // The move also takes the store lock and removes the local blobs after uploading them.
        let (lock, blobs) = (repo.join(canon_model::paths::ARTIFACT_STORE_LOCK_FILE), store_dir(&repo));
        let mut targets: Vec<&Path> = vec![lock.as_path(), blobs.as_path()];
        if new_yaml.is_some() {
            targets.push(&yaml);
        }
        if !gitignored(&repo) {
            targets.push(&gitignore);
        }
        if let Some(code) = crate::write_mode::refuse_unwritable(&targets, CMD, json) {
            return code;
        }
    }

    // The plan: read-only. Every blob is hashed (one that does not match its
    // name is never moved) and the store is asked about its digest.
    let mut problems: Vec<(&'static str, String)> = Vec::new();
    let mut moving = MovePlan { effects: Vec::new(), verified: Vec::new(), already: 0 };
    for blob in &blobs {
        let local = match canon_store::eio::open(&blob.path).and_then(sha256_reader) {
            Ok(actual) if actual == blob.digest => actual,
            Ok(actual) => {
                problems.push(("ARTIFACTS_BLOB_CORRUPT", format!("{}: its bytes hash to {actual}, not its name; it is not moved", blob.digest)));
                continue;
            }
            Err(e) => {
                // A blob that cannot be read says nothing about its content: the system refuses it (DENIED), it is no file (INVALID), or a repeat can pass.
                problems.push((unreadable_blob_code(&e), format!("{}: cannot be read: {e}", blob.digest)));
                continue;
            }
        };
        match store.observe(&repo, &local, false) {
            Observed::Sha256(_) => {
                moving.already += 1;
                moving.effects.push(Effect::record("skip", blob.digest.as_str()).detail("already in the store"));
            }
            Observed::Unavailable(why) => return CliError::refused("ARTIFACTS_STORE_UNAVAILABLE", format!("could not ask {}: {why}", store.display(&local))).hint(OBJECT_STORE_HINT).emit("canon artifacts move", json),
            _ => moving.effects.push(Effect::record("upload", blob.digest.as_str()).detail(format!("to {}", store.display(&blob.digest)))),
        }
        moving.verified.push(blob);
    }
    if !problems.is_empty() {
        for (_, line) in &problems {
            crate::say!(json, "  problem {line}");
        }
        let lines: Vec<&str> = problems.iter().map(|(_, line)| line.as_str()).collect();
        let code = blob_problem_code(&problems);
        return refuse(
            code,
            format!("{} of {} local blob(s) cannot be moved: {}", problems.len(), blobs.len(), lines.join("; ")),
            &format!("{}; nothing was removed and canon.yaml is unchanged", blob_hint(code)),
        );
    }
    let relative = |blob: &LocalBlob| blob.path.strip_prefix(&repo).unwrap_or(&blob.path).display().to_string();
    moving.effects.extend(moving.verified.iter().map(|blob| Effect::path("remove", relative(blob))));
    if new_yaml.is_some() {
        moving.effects.push(Effect::path("rewrite", "canon.yaml").detail(format!("add artifact_store: {{ backend: {to}, prefix: {DEFAULT_PREFIX} }}")));
    }
    if !gitignored(&repo) {
        moving.effects.push(Effect::path("rewrite", ".gitignore").detail(format!("add {GITIGNORE_LINE}")));
    }
    // What the plan read: canon.yaml (the target tier and where the block goes), .gitignore, and the blobs of the local store by
    // content digest (each was hashed above, so its name is its bytes) with the entries that are not blobs.
    let reads = Reads::new()
        .path(&repo, "canon.yaml")
        .path(&repo, ".gitignore")
        .set("blobs", blobs.iter().map(|blob| blob.digest.as_str()))
        .set("not-blobs", skipped.iter().map(String::as_str));
    let plan = match Plan::new("artifacts move", std::mem::take(&mut moving.effects), reads) {
        Ok(plan) => plan,
        Err(e) => return e.emit("canon artifacts move", json),
    };
    let commit = match plan.gate(mode) {
        Ok(commit) => commit,
        Err(e) => return e.emit("canon artifacts move", json),
    };
    let (verified, already) = (moving.verified, moving.already);

    // A commit holds the local store for the whole move, so no writer adds a local blob between the plan and the removal
    // (`main` holds the repository write lock for the whole run). A preview writes nothing and takes no lock.
    let _lock = if commit {
        match StoreLock::acquire(&repo) {
            Ok(lock) => Some(lock),
            Err(e) => return store_error(&e).emit("canon artifacts move", json),
        }
    } else {
        None
    };

    if !commit {
        crate::say!(json, "{CMD}: {} local blob(s) -> {} — {} already in the store", blobs.len(), store.location(), already);
        for name in &skipped {
            crate::say!(json, "  left in place (not a blob): {name}");
        }
        let report = ArtifactsMoveReport { backend: to.to_string(), blobs: blobs.len(), uploaded: 0, already_in_store: already, removed_local: 0, config_written: false, gitignore_added: false, skipped, plan: plan.preview() };
        plan.finish_preview("artifacts move", "the plan only lists the uploads and removals; moving takes its planDigest", json, &report);
        return 0;
    }

    // The commit: upload every verified blob and read it back, then switch the config, then remove the local copies.
    let (mut uploaded, mut already) = (0usize, 0usize);
    let mut uploads: Vec<&LocalBlob> = Vec::new();
    // What went wrong with a blob once the commit was uploading, as the lines of the message: another process rewrote it, a blob cannot be read,
    // an object already in the store is bad. Each is the refusal the same blob would have been before any upload (`ARTIFACTS_BLOB_CORRUPT` for
    // a blob that changed or an object that does not verify, the code of the read error for a blob that cannot be read).
    let mut stopped: Vec<(&'static str, String)> = Vec::new();
    for blob in &verified {
        #[cfg(debug_assertions)]
        race_hook(&blob.path, uploaded);
        let outcome = match std::fs::File::open(&blob.path).map_err(ArtifactStoreError::from).and_then(|file| object.upload(&blob.digest, file)) {
            Ok(outcome) => outcome,
            Err(e @ (ArtifactStoreError::Remote { .. } | ArtifactStoreError::Unattached { .. })) => {
                if uploaded > 0 {
                    // The store stopped answering after uploads landed: the move stops here, and those uploads stay.
                    stopped.push((store_error(&e).code, format!("uploading {} failed after {uploaded} upload(s): {e}; the blobs after it were not tried", store.display(&blob.digest))));
                    break;
                }
                return CliError::failed("ARTIFACTS_STORE_FAILED", format!("uploading {} failed: {e}", store.display(&blob.digest)))
                    .hint(format!("{OBJECT_STORE_HINT}; nothing was uploaded or removed, so the same call can run again"))
                    .emit("canon artifacts move", json);
            }
            Err(ArtifactStoreError::Changed { actual, .. }) => {
                // Another process rewrote the blob while it was uploaded: the blob is not moved, and whatever was uploaded before it stays.
                stopped.push(("ARTIFACTS_BLOB_CORRUPT", format!("{}: its bytes changed while being read (now {actual}); it is not moved", blob.digest)));
                continue;
            }
            Err(e) if uploaded > 0 => {
                stopped.push((store_error(&e).code, format!("{}: could not be read for the upload: {e}; it is not moved", blob.digest)));
                continue;
            }
            Err(e) => return CliError::refused(store_error(&e).code, format!("could not move {}: {e}", blob.digest)).hint(e.hint()).emit("canon artifacts move", json),
        };
        match outcome {
            StoreOutcome::Stored => uploaded += 1,
            StoreOutcome::AlreadyStored => {
                // The local copy is about to go: the object already there
                // must hold these bytes, not just share their size.
                match store.observe(&repo, &blob.digest, true) {
                    Observed::Sha256(_) => already += 1,
                    other => {
                        stopped.push(("ARTIFACTS_BLOB_CORRUPT", format!("{}: the object already in the store does not verify ({}); it is not moved", blob.digest, other.as_str())));
                        continue;
                    }
                }
            }
        }
        uploads.push(blob);
    }
    if !stopped.is_empty() {
        for (_, line) in &stopped {
            crate::say!(json, "  problem {line}");
        }
        let lines: Vec<&str> = stopped.iter().map(|(_, line)| line.as_str()).collect();
        let message = format!("{} of {} local blob(s) cannot be moved: {}", stopped.len(), blobs.len(), lines.join("; "));
        if uploaded == 0 {
            // Nothing was uploaded, so nothing was written: a refusal like a blob that does not hash to its name in the plan.
            return refuse("ARTIFACTS_BLOB_CORRUPT", message, &format!("{}; nothing was uploaded or removed and canon.yaml is unchanged", blob_hint("ARTIFACTS_BLOB_CORRUPT")));
        }
        // Uploads were made, so the command did work and stopped: `ARTIFACTS_MOVE_INCOMPLETE` (one `_INCOMPLETE` per subject, the reason says
        // what stopped it) carries what was uploaded in `data` and says to read the state first.
        let report = ArtifactsMoveReport { backend: to.to_string(), blobs: blobs.len(), uploaded, already_in_store: already, removed_local: 0, config_written: false, gitignore_added: false, skipped, plan: plan.committed() };
        crate::incomplete::read_first("canon artifacts verify --json", "read what the stopped move left: the blobs it uploaded are in the store, no local blob was removed and canon.yaml is unchanged");
        crate::json_out::prepend_next(crate::status::NextStep {
            command: format!("canon artifacts move --tier {to} --json"),
            why: "preview a new plan once the cause is dealt with: the uploaded blobs are `skip` effects of it (the same --plan-digest is stale now)".into(),
            actor: crate::status::StepActor::Agent,
            role: None,
        });
        return CliError::failed("ARTIFACTS_MOVE_INCOMPLETE", format!("stopped after {uploaded} upload(s): {message}"))
            .reason(stopped[0].0)
            .hint("the uploads stay in the store, no local blob was removed and canon.yaml is unchanged; read the state, deal with the blob or object the message names, then preview and commit a new plan (the same --plan-digest is stale now)")
            .emit_with_data("canon artifacts move", json, &report);
    }

    let mut config_written = false;
    let mut removed = 0usize;
    if let Some(yaml) = &new_yaml {
        if let Err(e) = canon_store::atomic::write_atomic(&repo.join("canon.yaml"), yaml.as_bytes()) {
            return CliError::refused("WRITE_FAILED", format!("failed to update canon.yaml: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon artifacts move", json);
        }
        config_written = true;
    }
    for blob in &uploads {
        match std::fs::remove_file(&blob.path) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return CliError::refused("WRITE_FAILED", format!("failed to remove {}: {e}", blob.path.display()))
                    .hint("the bytes are safe in the store and canon.yaml already names it; fix the permissions and run the command again to finish the removal")
                    .emit("canon artifacts move", json);
            }
        }
    }
    // Empty directories only: anything still there (a skipped entry) stays.
    let dir = store_dir(&repo);
    let _ = std::fs::remove_dir(&dir);
    if let Some(parent) = dir.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    let gitignore_added = match ensure_gitignored(&repo) {
        Ok(added) => added,
        Err(e) => return CliError::refused("WRITE_FAILED", format!("failed to update .gitignore: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon artifacts move", json),
    };

    crate::say!(
        json,
        "{CMD}: {} local blob(s) -> {} — uploaded {uploaded}, already in the store {already}, removed {removed} local cop{}",
        blobs.len(),
        store.location(),
        if removed == 1 { "y" } else { "ies" }
    );
    if config_written {
        crate::say!(json, "  canon.yaml: added artifact_store: {{ backend: {to}, prefix: {DEFAULT_PREFIX} }}");
    }
    if gitignore_added {
        crate::say!(json, "  .gitignore: added {GITIGNORE_LINE}");
    }
    if removed > 0 || config_written {
        crate::say!(json, "commit the removal of {}/ and the canon.yaml change with the ledger", canon_model::paths::ARTIFACTS_SHA256_DIR);
    }
    for name in &skipped {
        crate::say!(json, "  left in place (not a blob): {name}");
    }
    if uploaded == 0 && removed == 0 && !config_written && !gitignore_added {
        crate::json_out::mark_unchanged();
    }
    let report = ArtifactsMoveReport { backend: to.to_string(), blobs: blobs.len(), uploaded, already_in_store: already, removed_local: removed, config_written, gitignore_added, skipped, plan: plan.committed() };
    crate::next::finish(&repo, "artifacts move", &crate::next::Scope::Repo, false, json, &report);
    0
}

/// Debug builds only, the tests' seam (like `CANON_TEST_REPO_LOCK_WAIT_MS`): `CANON_TEST_BLOB_RACE_AFTER=<n>` makes the commit rewrite, once, the
/// blob it is about to upload when `n` blobs have been uploaded, as another process would between the plan and the upload.
#[cfg(debug_assertions)]
fn race_hook(path: &Path, uploaded: usize) {
    static FIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if std::env::var("CANON_TEST_BLOB_RACE_AFTER").ok().and_then(|text| text.parse::<usize>().ok()) == Some(uploaded) && !FIRED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
        let _ = std::fs::write(path, b"rewritten by another process");
    }
}

#[cfg(test)]
mod tests {
    use canon_gate::artifact_store::{ArtifactConfigError, ArtifactStoreError};

    use super::{evidence_store_code, store_code};

    /// atip.codestored.11: a canon.yaml the system fails to read in a way a repeat can pass is `CONFIG_UNAVAILABLE`; a refused read is
    /// `CONFIG_DENIED` and one that does not parse is `CONFIG_INVALID` with the reason `ARTIFACT_STORE`.
    #[test]
    fn atip_codestored_11_a_canon_yaml_the_artifact_store_fails_to_read_is_unavailable() {
        let read = |code: i32| ArtifactStoreError::Config(ArtifactConfigError::Read { path: "canon.yaml".into(), source: std::io::Error::from_raw_os_error(code) });
        assert_eq!(store_code(&read(5)), ("CONFIG_UNAVAILABLE", None));
        assert_eq!(store_code(&read(13)), ("CONFIG_DENIED", None));
        assert_eq!(store_code(&read(21)), ("CONFIG_INVALID", Some("ARTIFACT_STORE")));
    }

    /// atip.codesplit.02: a bound file that changed while it was read is `EVIDENCE_BINDING_STALE` (exit 2, not retryable: it binds once what produced
    /// the file is run again); a store that could not be used before anything was written keeps the store's own code, and a write that failed is
    /// `ARTIFACTS_STORE_FAILED` (exit 1).
    #[test]
    fn atip_codesplit_02_a_bound_file_that_changed_is_evidence_binding_stale() {
        let changed = ArtifactStoreError::Changed { expected: "sha256:a".to_string(), actual: "sha256:b".to_string() };
        assert_eq!(evidence_store_code(&changed), ("EVIDENCE_BINDING_STALE", None));
        assert_eq!(super::store_code(&changed), ("EVIDENCE_BINDING_STALE", None));
        use crate::cli_error::CliErrorExt as _;
        let stale = crate::cli_error::CliError::refused("EVIDENCE_BINDING_STALE", "m");
        assert_eq!((stale.exit, stale.retryable), (2, false));
        assert_eq!(evidence_store_code(&ArtifactStoreError::UnsafeStoreDir), ("ARTIFACTS_STORE_INVALID", None));
        assert_eq!(evidence_store_code(&ArtifactStoreError::Remote { tier: "cold".to_string(), reason: "down".to_string() }), ("ARTIFACTS_STORE_FAILED", None));
    }
}
