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
use crate::safety::{Effect, Mode, Plan};

/// The `.gitignore` line an object-store repo gains: the local store is
/// gone, and a stray local blob must never be committed.
const GITIGNORE_LINE: &str = ".canon/artifacts/";

/// A refusal for a store that cannot be opened (an unusable `artifact_store:`
/// block) or attached; the caller emits it under its own command.
fn store_error(e: &ArtifactStoreError) -> CliError {
    let code = match e {
        ArtifactStoreError::Config(_) => "CONFIG_UNREADABLE",
        _ => "ARTIFACTS_STORE_UNREACHABLE",
    };
    CliError::refused(code, e.to_string()).hint(e.hint())
}

/// `canon artifacts verify`.
pub fn run_verify(repo: &Path, full: bool, json: bool) -> u8 {
    const CMD: &str = "canon artifacts verify";
    let repo = resolve_repo_root(repo);
    let gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(context) => context,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon artifacts verify", json);
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
        return CliError::refused("ARTIFACTS_STORE_UNREACHABLE", format!("the artifact store could not be asked about {} bound file(s): {why}", checks.len())).hint(OBJECT_STORE_HINT).emit("canon artifacts verify", json);
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
    let data = ArtifactsVerifyReport {
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
    pub problems: Vec<VerifyProblem<'a>>,
}

impl crate::json_out::Report for ArtifactsVerifyReport<'_> {}

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
struct LocalBlob {
    digest: String,
    path: PathBuf,
}

/// The regular files of the local store named by a digest, sorted, and
/// the names of entries that are neither those nor a stale temp file.
fn local_blobs(repo: &Path) -> std::io::Result<(Vec<LocalBlob>, Vec<String>)> {
    let dir = store_dir(repo);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err(std::io::Error::other(format!("{} is not a real directory", dir.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
        Err(e) => return Err(e),
    }
    let (mut blobs, mut skipped) = (Vec::new(), Vec::new());
    for entry in std::fs::read_dir(&dir)? {
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
    pub plan: Plan,
}

impl Report for ArtifactsMoveReport {}

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
    let refuse = |message: String, hint: &str| CliError::refused("ARTIFACTS_MOVE_REFUSED", message).hint(hint.to_string()).emit("canon artifacts move", json);
    let base = crate::safety::command("artifacts move", &[("--tier", to.to_string())], repo);
    let repo = resolve_repo_root(repo);

    let current = match artifacts_config(&repo) {
        Ok(current) => current,
        Err(e) => return store_error(&e).emit("canon artifacts move", json),
    };
    if to == "local" {
        if matches!(current.backend, ArtifactsBackend::Local) {
            // Nothing to move: the empty plan still takes the digest handshake.
            let plan = Plan::new(Vec::new());
            let commit = match plan.gate(mode, &base) {
                Ok(commit) => commit,
                Err(e) => return e.emit("canon artifacts move", json),
            };
            if commit {
                crate::json_out::mark_unchanged();
            }
            crate::say!(json, "{CMD}: the artifact store is already local; nothing to move");
            let report = ArtifactsMoveReport { backend: "local".into(), blobs: 0, uploaded: 0, already_in_store: 0, removed_local: 0, config_written: false, gitignore_added: false, skipped: Vec::new(), plan: if commit { plan.committed() } else { plan } };
            if json {
                crate::json_out::data("artifacts move", &report);
            }
            return 0;
        }
        return refuse(
            "moving artifact bytes back from an object store to `local` is not supported".to_string(),
            "the bytes stay in the store; to read them again, keep `artifact_store.backend` as it is",
        );
    }
    let (target, new_yaml) = match resolve_target(&repo, to) {
        Ok(resolved) => resolved,
        Err(message) => return refuse(message, "`--tier` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)"),
    };
    let store = match ArtifactStore::from_config(target) {
        Ok(store) => store,
        Err(e) => return store_error(&e).emit("canon artifacts move", json),
    };
    let ArtifactStore::Object(object) = &store else {
        return refuse(format!("`{to}` is not an object-store tier"), "`--tier` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)");
    };

    // A commit holds the local store for the whole move, so no writer adds a
    // local blob between the plan and the removal. A preview writes nothing and takes no lock.
    let _lock = if matches!(mode, Mode::Commit(_)) {
        match StoreLock::acquire(&repo) {
            Ok(lock) => Some(lock),
            Err(e) => return refuse(e.to_string(), e.hint()),
        }
    } else {
        None
    };
    let (blobs, skipped) = match local_blobs(&repo) {
        Ok(listing) => listing,
        Err(e) => return CliError::refused("REPO_UNREADABLE", format!("cannot read the local artifact store: {e}")).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again").emit("canon artifacts move", json),
    };

    // The plan: read-only. Every blob is hashed (one that does not match its
    // name is never moved) and the store is asked about its digest.
    let mut problems: Vec<String> = Vec::new();
    let mut moving = MovePlan { effects: Vec::new(), verified: Vec::new(), already: 0 };
    for blob in &blobs {
        let local = match std::fs::File::open(&blob.path).and_then(sha256_reader) {
            Ok(actual) if actual == blob.digest => actual,
            Ok(actual) => {
                problems.push(format!("{}: its bytes hash to {actual}, not its name; it is not moved", blob.digest));
                continue;
            }
            Err(e) => {
                problems.push(format!("{}: cannot be read: {e}", blob.digest));
                continue;
            }
        };
        match store.observe(&repo, &local, false) {
            Observed::Sha256(_) => {
                moving.already += 1;
                moving.effects.push(Effect::record("skip", blob.digest.as_str()).detail("already in the store"));
            }
            Observed::Unavailable(why) => return CliError::refused("ARTIFACTS_STORE_UNREACHABLE", format!("could not ask {}: {why}", store.display(&local))).hint(OBJECT_STORE_HINT).emit("canon artifacts move", json),
            _ => moving.effects.push(Effect::record("upload", blob.digest.as_str()).detail(format!("to {}", store.display(&blob.digest)))),
        }
        moving.verified.push(blob);
    }
    if !problems.is_empty() {
        for line in &problems {
            crate::say!(json, "  problem {line}");
        }
        return refuse(
            format!("{} of {} local blob(s) cannot be moved: {}", problems.len(), blobs.len(), problems.join("; ")),
            "a blob that does not match its name proves nothing: delete it (`git rm`) if no record needs it, or re-run and re-attest the evidence, then run the command again; nothing was removed and canon.yaml is unchanged",
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
    let plan = Plan::new(std::mem::take(&mut moving.effects));
    let commit = match plan.gate(mode, &base) {
        Ok(commit) => commit,
        Err(e) => return e.emit("canon artifacts move", json),
    };
    let (verified, already) = (moving.verified, moving.already);

    if !commit {
        crate::say!(json, "{CMD}: {} local blob(s) -> {} — {} already in the store", blobs.len(), store.location(), already);
        for name in &skipped {
            crate::say!(json, "  left in place (not a blob): {name}");
        }
        let report = ArtifactsMoveReport { backend: to.to_string(), blobs: blobs.len(), uploaded: 0, already_in_store: already, removed_local: 0, config_written: false, gitignore_added: false, skipped, plan };
        crate::safety::finish_preview(&base, "artifacts move", &report.plan, "the plan only lists the uploads and removals; moving takes its planDigest", json, &report);
        return 0;
    }

    // The commit: upload every verified blob and read it back, then switch the config, then remove the local copies.
    let (mut uploaded, mut already) = (0usize, 0usize);
    let mut uploads: Vec<&LocalBlob> = Vec::new();
    for blob in &verified {
        let outcome = match std::fs::File::open(&blob.path).map_err(ArtifactStoreError::from).and_then(|file| object.upload(&blob.digest, file)) {
            Ok(outcome) => outcome,
            Err(e @ (ArtifactStoreError::Remote { .. } | ArtifactStoreError::Unattached { .. })) => {
                return CliError::refused("ARTIFACTS_STORE_UNREACHABLE", format!("uploading {} failed after {uploaded} upload(s): {e}", store.display(&blob.digest)))
                    .hint(format!("{OBJECT_STORE_HINT}; no local blob was removed, and a second run resumes where this one stopped"))
                    .emit("canon artifacts move", json);
            }
            Err(ArtifactStoreError::Changed { actual, .. }) => {
                problems.push(format!("{}: its bytes changed while being read (now {actual}); it is not moved", blob.digest));
                continue;
            }
            Err(e) => return refuse(format!("could not move {}: {e}", blob.digest), e.hint()),
        };
        match outcome {
            StoreOutcome::Stored => uploaded += 1,
            StoreOutcome::AlreadyStored => {
                // The local copy is about to go: the object already there
                // must hold these bytes, not just share their size.
                match store.observe(&repo, &blob.digest, true) {
                    Observed::Sha256(_) => already += 1,
                    other => {
                        problems.push(format!("{}: the object already in the store does not verify ({}); it is not moved", blob.digest, other.as_str()));
                        continue;
                    }
                }
            }
        }
        uploads.push(blob);
    }
    if !problems.is_empty() {
        for line in &problems {
            crate::say!(json, "  problem {line}");
        }
        return refuse(
            format!("{} of {} local blob(s) cannot be moved: {}", problems.len(), blobs.len(), problems.join("; ")),
            "a blob that does not match its name proves nothing: delete it (`git rm`) if no record needs it, or re-run and re-attest the evidence, then run the command again; nothing was removed and canon.yaml is unchanged",
        );
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
