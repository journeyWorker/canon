//! `canon ledger compact [--dry-run] [--json]` (0.23 D2): move every
//! superseded version of a natural-keyed record out of the ledger into
//! the archive, once.
//!
//! The ledger (`.canon/ledger`) then holds the current decision per key
//! and the archive (`.canon/archive`, same layout and file names, tracked
//! in git) holds the rest. Appends already do this for the key they write
//! ([`canon_store::git_tier::GitTier`]'s write path); compaction brings a
//! ledger written before 0.23 to the same state, and finishes any move a
//! crash interrupted. It is idempotent: a second run finds nothing to
//! move. `canon migrate` runs it.
//!
//! What counts as superseded, and why the gate and `canon status` read
//! the same after the move, is [`canon_store::archive`]'s one definition.
//! `--dry-run` reports what would move and writes nothing.
//!
//! Exit codes: `0` compacted (or nothing to do); `1` a move failed (what
//! moved stays moved, the rest is untouched, run it again); `2` refused
//! (the ledger has no archive: its root is not a `ledger` directory).

use std::path::Path;

use canon_gate::GateCtx;
use canon_store::archive::KindCompaction;
use canon_store::git_tier::GitTier;
use serde_json::{json, Value};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;

/// One compaction pass over `repo`'s ledger: every archived kind's
/// outcome, in kind order. `Err` is the refusal or failure to report.
pub fn compact(repo: &Path, dry_run: bool) -> Result<Vec<KindCompaction>, CliError> {
    let ctx = GateCtx::from_repo(repo).map_err(|e| {
        CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again")
    })?;
    let tier = GitTier::new(&ctx.ledger_root);
    if tier.archive_root().is_none() {
        return Err(CliError::refused(
            "LEDGER_ARCHIVE_UNAVAILABLE",
            format!("the ledger root `{}` is not a directory named `ledger`, so it has no archive beside it and keeps every version", ctx.ledger_root.display()),
        )
        .hint("name the local tier's root `<dir>/ledger` in canon.yaml (`tiers.local.root`, default `.canon/ledger`) to archive superseded versions"));
    }
    tier.compact(dry_run).map_err(|e| {
        CliError::failed("LEDGER_COMPACT_FAILED", e.to_string())
            .hint("fix what the message names (a read-only ledger or archive, a full disk), then run `canon ledger compact` again; versions already moved stay in the archive")
    })
}

fn kib(bytes: u64) -> String {
    format!("{:.1} KiB", bytes as f64 / 1024.0)
}

/// The archive's path relative to `repo`, for messages.
fn archive_label(repo: &Path) -> String {
    GateCtx::from_repo(repo)
        .ok()
        .and_then(|ctx| canon_store::archive::archive_root_of(&ctx.ledger_root))
        .map(|root| root.strip_prefix(repo).unwrap_or(&root).display().to_string())
        .unwrap_or_default()
}

fn data(repo: &Path, dry_run: bool, kinds: &[KindCompaction]) -> Value {
    let rows: Vec<Value> = kinds
        .iter()
        .map(|k| json!({ "kind": k.kind.as_str(), "files": k.files, "live": k.live, "superseded": k.superseded, "moved": k.moved, "bytes": k.bytes }))
        .collect();
    json!({
        "dryRun": dry_run,
        "archive": archive_label(repo),
        "kinds": rows,
        "files": kinds.iter().map(|k| k.files).sum::<usize>(),
        "live": kinds.iter().map(|k| k.live).sum::<usize>(),
        "superseded": kinds.iter().map(|k| k.superseded).sum::<usize>(),
        "moved": kinds.iter().map(|k| k.moved).sum::<usize>(),
        "bytes": kinds.iter().map(|k| k.bytes).sum::<u64>(),
    })
}

/// The lines `canon ledger compact` (and `canon migrate`) print.
pub fn lines(repo: &Path, dry_run: bool, kinds: &[KindCompaction]) -> Vec<String> {
    let superseded: usize = kinds.iter().map(|k| k.superseded).sum();
    let archive = archive_label(repo);
    let mut out = Vec::new();
    for k in kinds.iter().filter(|k| k.superseded > 0) {
        let verb = if dry_run { "would move" } else { "moved" };
        let n = if dry_run { k.superseded } else { k.moved };
        out.push(format!("  {:<16} {verb} {n} of {} file(s) ({}); {} stay", k.kind.as_str(), k.files, kib(k.bytes), k.live));
    }
    if superseded == 0 {
        out.push("canon ledger compact: nothing to move; the ledger already holds one file per key".to_string());
    } else if dry_run {
        out.push(format!("canon ledger compact: --dry-run, {superseded} superseded version(s) would move to {archive}; nothing written"));
    } else {
        let moved: usize = kinds.iter().map(|k| k.moved).sum();
        out.push(format!("canon ledger compact: moved {moved} superseded version(s) to {archive}; `canon query --history` reads them"));
    }
    out
}

pub fn run_compact(repo: &Path, dry_run: bool, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let kinds = match compact(&repo, dry_run) {
        Ok(kinds) => kinds,
        Err(e) => return e.emit("canon ledger compact", json),
    };
    for line in lines(&repo, dry_run, &kinds) {
        crate::say!(json, "{line}");
    }
    if json {
        crate::json_out::data("ledger compact", data(&repo, dry_run, &kinds));
    }
    0
}

/// What `canon migrate` says about compaction: the lines of a pass that
/// moved (or, under `dry_run`, would move) something, and none when the
/// ledger is already one file per key or has no archive (a custom ledger
/// root keeps every version, and migrate is not refused over it).
pub fn migrate_lines(repo: &Path, dry_run: bool) -> Result<Vec<String>, CliError> {
    match compact(repo, dry_run) {
        Ok(kinds) if kinds.iter().all(|k| k.superseded == 0) => Ok(Vec::new()),
        Ok(kinds) => Ok(lines(repo, dry_run, &kinds)),
        Err(e) if e.code == "LEDGER_ARCHIVE_UNAVAILABLE" => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// After a batch writer removed the records it wrote: put back the
/// archived versions those records had superseded, so the ledger holds
/// the current decision it held before the batch. A failure is reported
/// and left for `canon ledger compact`'s inverse to find on the next
/// append; it never turns the writer's own error into a different one.
pub(crate) fn restore_after_rollback(ledger_root: &Path, kind: canon_model::RecordKind) {
    if let Err(e) = GitTier::new(ledger_root).restore_unsuperseded(kind) {
        eprintln!("canon: rolled back, but could not restore the archived {} version(s) the rolled-back records had replaced ({e}); copy them back from the archive beside the ledger", kind.as_str());
    }
}
