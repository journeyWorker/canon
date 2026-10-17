//! `canon artifacts verify` and `canon artifacts migrate` (0.23 D3): the
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
//! # `migrate --to <backend> [--dry-run]`
//! Moves a repo's local blobs to the object-store tier `--to` names:
//! 1. every blob in `.canon/artifacts/sha256` is hashed (a blob that does
//!    not match its name is never migrated) and uploaded by digest key,
//!    read back, and — when the object was already there — re-hashed from
//!    the store, because the local copy is about to go;
//! 2. only when every blob verified, `canon.yaml` gains
//!    `artifact_store: { backend: <to>, prefix: artifacts/ }` (a block canon
//!    did not write is never rewritten: it must already name `--to`);
//! 3. then the local copies are removed and `.canon/artifacts/` is added to
//!    `.gitignore`.
//!
//! Local copies leave only after the store is proven to hold their bytes,
//! and the config switches before they leave, so a run stopped anywhere
//! leaves a repo the gate still passes and the next run finishes. A second
//! run finds nothing to do. `--to local` is a no-op on a local repo; moving
//! bytes back from an object store is not supported.

use std::path::{Path, PathBuf};

use canon_gate::artifact_store::{
    artifacts_config, bound_files, check_bound_files, is_sha256_hex, sha256_reader, store_dir, ArtifactStore, FileStatus, Observed, StoreError, StoreLock, StoreOutcome, OBJECT_STORE_HINT,
};
use canon_gate::{GateContext, GateCtx};
use canon_policy::SchemaRegistry;
use canon_store::artifact_blobs::{ArtifactsBackend, ArtifactsConfig, DEFAULT_PREFIX};
use chrono::Utc;

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;

/// The `.gitignore` line an object-store repo gains: the local store is
/// gone, and a stray local blob must never be committed.
const GITIGNORE_LINE: &str = ".canon/artifacts/";

/// A refusal for a store that cannot be opened (an unusable `artifact_store:`
/// block) or attached; the caller emits it under its own command.
fn store_error(e: &StoreError) -> CliError {
    let code = match e {
        StoreError::Config(_) => "CONFIG_UNREADABLE",
        _ => "ARTIFACTS_STORE_UNREACHABLE",
    };
    CliError::refused(code, e.to_string()).hint(e.hint())
}

/// `canon artifacts verify`.
pub fn run_verify(repo: &Path, full: bool, json: bool) -> i32 {
    const CMD: &str = "canon artifacts verify";
    let repo = resolve_repo_root(repo);
    let gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(context) => context,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again").emit("canon artifacts verify", json);
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
    let data = serde_json::json!({
        "backend": match artifacts_config(&repo).map(|c| c.backend) { Ok(ArtifactsBackend::Tier { name, .. }) => name, _ => "local".to_string() },
        "full": full,
        "bound": checks.len(),
        "stored": stored,
        "workingTreeOnly": tree_only,
        "problems": problems.iter().zip(&problem_lines).map(|(c, line)| serde_json::json!({
            "subject": c.file.subject, "path": c.file.path, "sha256": c.file.sha256, "blob": c.blob.as_str(), "detail": line,
        })).collect::<Vec<_>>(),
    });
    if problems.is_empty() {
        if json {
            crate::json_out::data("artifacts verify", data);
        }
        return 0;
    }
    CliError::failed("ARTIFACTS_VERIFY_FAILED", format!("{} bound file(s) are missing from the artifact store or altered", problems.len()))
        .hint("re-upload a file the working tree still holds with `canon evidence vault`, or re-run and re-attest it with `canon evidence add`; see data.problems")
        .emit_with_data("canon artifacts verify", json, data)
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

/// `canon artifacts migrate`.
pub fn run_migrate(repo: &Path, to: &str, dry_run: bool, json: bool) -> i32 {
    const CMD: &str = "canon artifacts migrate";
    let refuse = |message: String, hint: &str| CliError::refused("ARTIFACTS_MIGRATE_REFUSED", message).hint(hint.to_string()).emit("canon artifacts migrate", json);
    let repo = resolve_repo_root(repo);

    let current = match artifacts_config(&repo) {
        Ok(current) => current,
        Err(e) => return store_error(&e).emit("canon artifacts migrate", json),
    };
    if to == "local" {
        if matches!(current.backend, ArtifactsBackend::Local) {
            crate::say!(json, "{CMD}: the artifact store is already local; nothing to move");
            if json {
                crate::json_out::data("artifacts migrate", serde_json::json!({ "backend": "local", "dryRun": dry_run, "blobs": 0, "uploaded": 0, "alreadyInStore": 0, "removedLocal": 0, "configWritten": false, "gitignoreAdded": false, "skipped": [] }));
            }
            return 0;
        }
        return refuse(
            "moving artifact bytes back from an object store to `local` is not supported".to_string(),
            "the bytes stay in the store; to read them again, keep `artifacts.backend` as it is",
        );
    }
    let (target, new_yaml) = match resolve_target(&repo, to) {
        Ok(resolved) => resolved,
        Err(message) => return refuse(message, "`--to` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)"),
    };
    let store = match ArtifactStore::from_config(target) {
        Ok(store) => store,
        Err(e) => return store_error(&e).emit("canon artifacts migrate", json),
    };
    let ArtifactStore::Object(object) = &store else {
        return refuse(format!("`{to}` is not an object-store tier"), "`--to` names an `s3` tier of canon.yaml's `tiers:` (for example `cold`)");
    };

    // The local store is held for the whole move, so no writer adds a
    // local blob between the listing and the removal.
    let _lock = if dry_run {
        None
    } else {
        match StoreLock::acquire(&repo) {
            Ok(lock) => Some(lock),
            Err(e) => return refuse(e.to_string(), e.hint()),
        }
    };
    let (blobs, skipped) = match local_blobs(&repo) {
        Ok(listing) => listing,
        Err(e) => return CliError::refused("REPO_UNREADABLE", format!("cannot read the local artifact store: {e}")).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again").emit("canon artifacts migrate", json),
    };

    let (mut uploaded, mut already) = (0usize, 0usize);
    let mut problems: Vec<String> = Vec::new();
    let mut verified: Vec<&LocalBlob> = Vec::new();
    for blob in &blobs {
        let local = match std::fs::File::open(&blob.path).and_then(sha256_reader) {
            Ok(actual) if actual == blob.digest => actual,
            Ok(actual) => {
                problems.push(format!("{}: its bytes hash to {actual}, not its name; it is not migrated", blob.digest));
                continue;
            }
            Err(e) => {
                problems.push(format!("{}: cannot be read: {e}", blob.digest));
                continue;
            }
        };
        if dry_run {
            match store.observe(&repo, &local, false) {
                Observed::Sha256(_) => already += 1,
                Observed::Unavailable(why) => return CliError::refused("ARTIFACTS_STORE_UNREACHABLE", format!("could not ask {}: {why}", store.display(&local))).hint(OBJECT_STORE_HINT).emit("canon artifacts migrate", json),
                _ => uploaded += 1,
            }
            verified.push(blob);
            continue;
        }
        let outcome = match std::fs::File::open(&blob.path).map_err(StoreError::from).and_then(|file| object.upload(&blob.digest, file)) {
            Ok(outcome) => outcome,
            Err(e @ (StoreError::Remote { .. } | StoreError::Unattached { .. })) => {
                return CliError::refused("ARTIFACTS_STORE_UNREACHABLE", format!("uploading {} failed after {uploaded} upload(s): {e}", store.display(&blob.digest)))
                    .hint(format!("{OBJECT_STORE_HINT}; no local blob was removed, and a second run resumes where this one stopped"))
                    .emit("canon artifacts migrate", json);
            }
            Err(StoreError::Changed { actual, .. }) => {
                problems.push(format!("{}: its bytes changed while being read (now {actual}); it is not migrated", blob.digest));
                continue;
            }
            Err(e) => return refuse(format!("could not migrate {}: {e}", blob.digest), e.hint()),
        };
        match outcome {
            StoreOutcome::Stored => uploaded += 1,
            StoreOutcome::AlreadyStored => {
                // The local copy is about to go: the object already there
                // must hold these bytes, not just share their size.
                match store.observe(&repo, &blob.digest, true) {
                    Observed::Sha256(_) => already += 1,
                    other => {
                        problems.push(format!("{}: the object already in the store does not verify ({}); it is not migrated", blob.digest, other.as_str()));
                        continue;
                    }
                }
            }
        }
        verified.push(blob);
    }
    if !problems.is_empty() {
        for line in &problems {
            crate::say!(json, "  problem {line}");
        }
        return refuse(
            format!("{} of {} local blob(s) cannot be migrated: {}", problems.len(), blobs.len(), problems.join("; ")),
            "a blob that does not match its name proves nothing: delete it (`git rm`) if no record needs it, or re-run and re-attest the evidence, then run the command again; nothing was removed and canon.yaml is unchanged",
        );
    }

    let mut config_written = false;
    let mut removed = 0usize;
    let mut gitignore_added = false;
    if !dry_run {
        if let Some(yaml) = &new_yaml {
            if let Err(e) = canon_store::atomic::write_atomic(&repo.join("canon.yaml"), yaml.as_bytes()) {
                return CliError::refused("WRITE_FAILED", format!("failed to update canon.yaml: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon artifacts migrate", json);
            }
            config_written = true;
        }
        for blob in &verified {
            match std::fs::remove_file(&blob.path) {
                Ok(()) => removed += 1,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return CliError::refused("WRITE_FAILED", format!("failed to remove {}: {e}", blob.path.display()))
                        .hint("the bytes are safe in the store and canon.yaml already names it; fix the permissions and run the command again to finish the removal")
                        .emit("canon artifacts migrate", json);
                }
            }
        }
        // Empty directories only: anything still there (a skipped entry) stays.
        let dir = store_dir(&repo);
        let _ = std::fs::remove_dir(&dir);
        if let Some(parent) = dir.parent() {
            let _ = std::fs::remove_dir(parent);
        }
        gitignore_added = match ensure_gitignored(&repo) {
            Ok(added) => added,
            Err(e) => return CliError::refused("WRITE_FAILED", format!("failed to update .gitignore: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon artifacts migrate", json),
        };
    }

    let verb = if dry_run { "would " } else { "" };
    crate::say!(
        json,
        "{CMD}: {} local blob(s) -> {} — {verb}upload {uploaded}, already in the store {already}{}",
        blobs.len(),
        store.location(),
        if dry_run { String::new() } else { format!(", removed {removed} local cop{}", if removed == 1 { "y" } else { "ies" }) }
    );
    if dry_run {
        if new_yaml.is_some() {
            crate::say!(json, "  would add to canon.yaml: artifact_store: {{ backend: {to}, prefix: {DEFAULT_PREFIX} }}");
        }
        crate::say!(json, "  would remove {} local cop{} and add {GITIGNORE_LINE} to .gitignore", verified.len(), if verified.len() == 1 { "y" } else { "ies" });
    } else {
        if config_written {
            crate::say!(json, "  canon.yaml: added artifact_store: {{ backend: {to}, prefix: {DEFAULT_PREFIX} }}");
        }
        if gitignore_added {
            crate::say!(json, "  .gitignore: added {GITIGNORE_LINE}");
        }
        if removed > 0 || config_written {
            crate::say!(json, "commit the removal of {}/ and the canon.yaml change with the ledger", canon_model::paths::ARTIFACTS_SHA256_DIR);
        }
    }
    for name in &skipped {
        crate::say!(json, "  left in place (not a blob): {name}");
    }
    let data = serde_json::json!({
        "backend": to,
        "dryRun": dry_run,
        "blobs": blobs.len(),
        "uploaded": uploaded,
        "alreadyInStore": already,
        "removedLocal": removed,
        "configWritten": config_written,
        "gitignoreAdded": gitignore_added,
        "skipped": skipped,
    });
    if dry_run {
        if json {
            crate::json_out::data("artifacts migrate", data);
        }
    } else {
        crate::next::finish(&repo, "artifacts migrate", &crate::next::Scope::Repo, false, json, data);
    }
    0
}
