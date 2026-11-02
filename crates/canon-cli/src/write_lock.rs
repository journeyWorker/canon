//! The repository write lock (atip/1 12.7, canon 0.35).
//!
//! A destructive command's commit recomputes its plan, compares its digest with the one it was given and then makes the effects.
//! Spec 12.7 makes it a MUST that the plan cannot change between the comparison and the effects, so a commit holds one lock
//! over the whole span, from before it recomputes the plan to after its last effect: [`PLAN_LOCK`], an OS advisory lock on
//! [`canon_model::paths::REPO_WRITE_LOCK_FILE`] taken through [`canon_store::file_lock::FileLock`], the base of the scenario-index
//! lock and the artifact-store lock (`flock`/`LockFileEx`, a bounded wait, released when the holder exits however it exits, the
//! file never deleted). A preview holds nothing.
//!
//! Every other writer of what a plan reads takes the same lock while it writes, so the span cannot be entered by a writer
//! half way: [`POLICY`] lists every leaf that writes and says which hold, with the reason for each that does not. `main` takes
//! the lock for the leaf of the run ([`hold_for_call`]) after every refusal that needs no state, the scenario-index refresh
//! that a reader may owe ([`crate::inventory::refresh_stale_index`]) takes it for its write, and
//! [`crate::dashboard_briefs`] takes it for the one brief the page approves. A holder that needs the lock again (the refresh of
//! a writer) finds it held: the lock is re-entrant in its thread.
//!
//! **The lock file** is made by `canon init` ([`create_file`]) and, in any other repository, by the first call that needs to lock it
//! ([`hold_within`]: opened with create, never truncated, never deleted), so a repository set up by an older canon, or one that
//! never ran `canon init`, is locked from its first call and no call runs unlocked for want of the file. The call that makes it also keeps it out of git: a line in `.git/info/exclude` unless
//! `.gitignore` already names it (`init` and `migrate` write that line to `.gitignore`), so the first lock never edits a tracked
//! file, and no plan reads it (`safety::path_digest` skips it). A repository with no `canon.yaml` has no plan to protect and
//! nothing locks for it. The lock file is process state, not a state of the repository: a call that is refused after it took the lock
//! (a stale digest, a read-only store) leaves the file behind, as it leaves any lock file of a refused writer.
//!
//! **Order.** This lock is the outermost one: taken before the scenario-index lock and the artifact-store lock, never while
//! holding either, so two processes cannot wait for each other. A wait that runs out is `REPO_WRITE_BUSY` (exit 2, retryable):
//! nothing was started.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use canon_model::paths::REPO_WRITE_LOCK_FILE;
use canon_store::file_lock::{FileLock, FileLockError};

use crate::cli_error::{CliError, CliErrorExt as _};
use crate::schema::{Effect, META};

/// What the lock guards, in the words of the code's meaning and of the refusals.
pub const PLAN_LOCK: &str = "the repository write lock";

/// How long a writer waits for the lock before it gives up and starts nothing.
const WAIT: Duration = Duration::from_secs(30);

/// When a leaf holds the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    /// For the whole run, unless the run writes nothing (`--dry-run`).
    Always,
    /// For the whole run of a commit (`--commit`): a bare run is a preview.
    OnCommit,
    /// For the whole run of a call that gives the option, the call that writes (`subject move <id>` reads).
    With(&'static str),
    /// For the whole run unless the call gives the option: a run that polls (`ingest records --watch`) holds it for each pass
    /// (`crate::run::ingest`), never for the polling, which would block every other writer for as long as it runs.
    Unless(&'static str),
    /// Never, for the reason in its row of [`POLICY`]: what the leaf writes is not read by any plan, or the leaf holds it itself.
    Never,
}

const CANON_DIR_WRITER: &str = "writes under .canon/, which `migrate` rewrites and `migrate --revert` restores whole";
const RECORD_WRITER: &str = "appends records to the ledger, which `purge`, `tier age` and `ledger compact` read and `migrate` renames";

/// Every leaf whose [`META`] effect is not `read`, and whether it holds the lock: the audit of the writers of what a plan reads
/// (spec 12.7). A leaf that writes and is not here fails the audit test, so a new writer must be judged when it is added.
/// The reasons are what the CHANGELOG of 0.35 lists.
pub const POLICY: &[(&str, Holds, &str)] = &[
    ("artifacts move", Holds::OnCommit, "a destructive commit: uploads, rewrites canon.yaml and .gitignore, removes the local blobs"),
    ("artifacts store", Holds::Always, "writes blobs into the artifact store that `artifacts move` hashes and removes, and refreshes the scenario index"),
    ("brief approve", Holds::Always, "rewrites the `Approved:` line of a brief, a file `migrate` rewrites and re-binds"),
    ("change new", Holds::Always, "writes a Change record and its brief: a record `migrate` renames and a file it moves"),
    ("context-pack add", Holds::Always, CANON_DIR_WRITER),
    ("dashboard", Holds::Never, "a server that runs until stopped, so it cannot hold the lock for its run: it regenerates only the derived snapshot, and the one brief a page approves is written under the lock (`dashboard_briefs::approve`)"),
    ("demo init", Holds::Never, "scaffolds a throwaway demo repository of its own: no plan of another repository reads it"),
    ("demo review", Holds::Never, "writes one record into the throwaway demo repository `demo init` made"),
    ("dispatch begin", Holds::Always, "writes the run manifest and a Run record"),
    ("dispatch end", Holds::Always, "rewrites the run manifest and its Run record"),
    ("divergence accept", Holds::Always, RECORD_WRITER),
    ("divergence add", Holds::Always, RECORD_WRITER),
    ("divergence defer", Holds::Always, RECORD_WRITER),
    ("divergence resolve", Holds::Always, RECORD_WRITER),
    ("evidence add", Holds::Always, "appends evidence records and stores their bound files in the artifact store, which `artifacts move` reads"),
    ("evidence audit", Holds::Always, "appends an audit record and stores its bound files in the artifact store"),
    ("export", Holds::Never, "writes the --output file and one line to the append-only access audit, which no plan reads"),
    ("feature new", Holds::Always, "writes a .feature file under a spec root, which `migrate` moves"),
    ("finding add", Holds::Always, RECORD_WRITER),
    ("finding close", Holds::Always, RECORD_WRITER),
    ("gate install-hooks", Holds::Always, "writes the git hook and .canon/pre-commit.sh, files `migrate` rewrites"),
    ("gate promote", Holds::Always, RECORD_WRITER),
    ("task done", Holds::Always, "flips a task checkbox in a plan file and records its evidence"),
    ("ingest records", Holds::Unless("watch"), "appends records to the ledger; `--watch` polls until stopped, so it holds the lock for each pass, not for the polling"),
    ("ingest plans", Holds::Always, RECORD_WRITER),
    ("ingest sessions", Holds::Unless("watch"), "appends records to the ledger; `--watch` polls until stopped, so it holds the lock for each pass, not for the polling"),
    ("init", Holds::Always, "writes canon.yaml, .gitignore and the skill files, which `artifacts move` and `migrate` read; `init --check` and `--dry-run` write nothing"),
    ("inventory sync", Holds::Always, "rewrites the scenario index records in the ledger; `--dry-run` writes nothing"),
    ("strategy promote", Holds::Always, RECORD_WRITER),
    ("strategy rollback", Holds::Always, RECORD_WRITER),
    ("ledger compact", Holds::OnCommit, "a destructive commit: moves superseded ledger files into the archive"),
    ("migrate", Holds::OnCommit, "a destructive commit (`--revert` too): moves, rewrites and renames what the plan lists"),
    ("plugin sync", Holds::Always, RECORD_WRITER),
    ("policy approve", Holds::Always, "writes .canon/policy-approval.yaml, a file `migrate` backs up"),
    ("prompt add", Holds::Always, CANON_DIR_WRITER),
    ("purge", Holds::OnCommit, "a destructive commit: deletes records and appends the audit line"),
    ("report build", Holds::Never, "writes its scratch files under the system temp dir and a derived report no plan reads"),
    ("report snapshot", Holds::Never, "writes the derived, gitignored dashboard snapshot no plan reads"),
    ("review change", Holds::Always, RECORD_WRITER),
    ("review scenario", Holds::Always, RECORD_WRITER),
    ("scenario new", Holds::Always, "appends a scenario to a .feature file under a spec root, which `migrate` moves"),
    ("skills install", Holds::Always, "writes the skill files and the install lock, which `migrate` rewrites"),
    ("subject adopt", Holds::Always, RECORD_WRITER),
    ("subject new", Holds::Always, RECORD_WRITER),
    ("subject move", Holds::With("to"), "`--to` rewrites the subject's records; without it the leaf reads"),
    ("subject unadopt", Holds::Always, RECORD_WRITER),
    ("tier age", Holds::OnCommit, "a destructive commit: moves records from one rung to another"),
];

/// The leaves of [`META`] whose effect is not `read` and that [`POLICY`] does not judge.
pub fn unjudged() -> Vec<&'static str> {
    META.iter().filter(|row| row.effect != Effect::Read && !POLICY.iter().any(|(path, ..)| *path == row.path)).map(|row| row.path).collect()
}

thread_local! {
    /// The locks this thread holds, by lock file, with how many guards stand for each: a second guard of the same lock
    /// (a writer's index refresh) does not wait for the thread that holds it.
    static HELD: RefCell<HashMap<PathBuf, (usize, FileLock)>> = RefCell::new(HashMap::new());
}

/// The lock, held until this guard and every guard of the same lock in this thread are dropped.
#[derive(Debug)]
pub struct Hold {
    file: PathBuf,
}

impl Drop for Hold {
    fn drop(&mut self) {
        HELD.with(|held| {
            let mut held = held.borrow_mut();
            if let Some((guards, _)) = held.get_mut(&self.file) {
                *guards -= 1;
                if *guards == 0 {
                    held.remove(&self.file);
                }
            }
        });
    }
}

/// Why [`hold_within`] holds nothing.
#[derive(Debug)]
pub enum Refused {
    /// Another process held the lock for the whole wait.
    Busy(Duration),
    /// The lock file could not be created or locked.
    Io(PathBuf, std::io::Error),
}

impl Refused {
    /// The refusal a command answers with: `REPO_WRITE_BUSY` (exit 2, retryable) or `WRITE_UNAVAILABLE`.
    pub fn error(&self) -> CliError {
        match self {
            Refused::Busy(waited) => CliError::refused(
                "REPO_WRITE_BUSY",
                format!("another canon process has held {PLAN_LOCK} (`{REPO_WRITE_LOCK_FILE}`) for the whole {}s wait; nothing was started", waited.as_secs()),
            )
            .hint("a canon command that writes or commits is still running in this repository: run the same command again in a moment"),
            Refused::Io(file, cause) => CliError::refused("WRITE_UNAVAILABLE", format!("cannot lock `{}`: {cause}", file.display()))
                .hint("make the lock file readable (its permissions, a mounted volume), then run the command again; nothing was changed"),
        }
    }
}

/// How long a call waits: [`WAIT`], or the milliseconds of `CANON_TEST_REPO_LOCK_WAIT_MS` in a debug build (the tests' way to
/// reach the end of the wait without waiting thirty seconds).
fn wait() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(millis) = std::env::var("CANON_TEST_REPO_LOCK_WAIT_MS").ok().and_then(|text| text.parse::<u64>().ok()) {
        return Duration::from_millis(millis);
    }
    WAIT
}

/// The lock file of the repository rooted at `repo`, under the root as the file system names it (`.`, `..` and links resolved), so
/// that the lock a writer holds and the one its index refresh asks for are one lock whichever way each was given the root.
fn lock_file(repo: &Path) -> PathBuf {
    std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf()).join(REPO_WRITE_LOCK_FILE)
}

/// The lock for one pass of a polling run (`ingest --watch`): held until the returned guard is dropped, which the loop does before
/// it sleeps. A dry run writes nothing and holds nothing. `Err` is the refusal the run answers with.
pub fn hold_pass(repo: &Path, dry_run: bool) -> Result<Option<Hold>, CliError> {
    if dry_run {
        return Ok(None);
    }
    match hold_within(&crate::context::resolve_repo_root(repo), wait()) {
        Ok(hold) => Ok(Some(hold)),
        Err(Refused::Io(..)) => Ok(None),
        Err(busy) => Err(busy.error()),
    }
}

/// A guard for the lock this thread already holds, if it does.
fn reenter(file: &Path) -> Option<Hold> {
    HELD.with(|held| {
        held.borrow_mut().get_mut(file).map(|(guards, _)| {
            *guards += 1;
            Hold { file: file.to_path_buf() }
        })
    })
}

fn remember(file: PathBuf, lock: FileLock) -> Hold {
    HELD.with(|held| held.borrow_mut().insert(file.clone(), (1, lock)));
    Hold { file }
}

/// The lock of the repository rooted at `repo`, created when the file is not there yet, waiting up to `wait` for another
/// holder. Held again by a thread that holds it.
pub fn hold_within(repo: &Path, wait: Duration) -> Result<Hold, Refused> {
    let file = lock_file(repo);
    if let Some(hold) = reenter(&file) {
        return Ok(hold);
    }
    let made = !file.exists();
    match FileLock::acquire(&file, wait) {
        Ok(lock) => {
            if made {
                keep_out_of_git(repo);
            }
            Ok(remember(file, lock))
        }
        Err(FileLockError::Busy(waited)) => Err(Refused::Busy(waited)),
        Err(FileLockError::Io(cause)) => Err(Refused::Io(file, cause)),
    }
}

/// Creates the lock file of the repository rooted at `repo` when it is not there, without holding it: `canon init` makes it beside
/// the `.gitignore` line that names it.
pub fn create_file(repo: &Path) -> std::io::Result<()> {
    let file = repo.join(REPO_WRITE_LOCK_FILE);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(file).map(drop)
}

/// Keeps the lock file the call just made out of `git status`: its line in `.git/info/exclude`, unless `.gitignore` names it
/// already or the repository has no `.git` directory (a worktree's `.git` is a file; its checkout is ignored by the `.gitignore`
/// line `init` writes). Local state the call owns, so no tracked file is edited; a failure leaves the file untracked and is not
/// an error.
fn keep_out_of_git(repo: &Path) {
    let root = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let named = |file: &Path| std::fs::read_to_string(file).is_ok_and(|text| text.lines().any(|line| line.trim().trim_start_matches('/') == REPO_WRITE_LOCK_FILE));
    let info = root.join(".git/info");
    if named(&root.join(".gitignore")) || !root.join(".git").is_dir() || named(&info.join("exclude")) {
        return;
    }
    let _ = std::fs::create_dir_all(&info);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(info.join("exclude")) {
        use std::io::Write as _;
        let _ = writeln!(file, "/{REPO_WRITE_LOCK_FILE}");
    }
}

/// [`hold_within`] for the wait a command makes, as the refusal it answers with.
pub fn hold(repo: &Path) -> Result<Hold, CliError> {
    hold_within(repo, wait()).map_err(|refused| refused.error())
}

fn policy_of(command: &str) -> Option<Holds> {
    POLICY.iter().find(|(path, ..)| *path == command).map(|(_, holds, _)| *holds)
}

/// Whether the run of `command` holds the lock, given whether it gave `--commit` or `--dry-run` or the option a row asks for.
/// `None` for a leaf that does not write.
pub fn holds(command: &str, has: impl Fn(&str) -> bool) -> Option<bool> {
    let (_, policy, _) = POLICY.iter().find(|(path, ..)| *path == command)?;
    let writes_nothing = has("dry_run") || (command == "init" && has("check"));
    Some(match policy {
        Holds::Always => !writes_nothing,
        Holds::OnCommit => has("commit"),
        Holds::With(option) => !writes_nothing && has(option),
        Holds::Unless(option) => !writes_nothing && !has(option),
        Holds::Never => false,
    })
}

/// The lock for the leaf `parsed` names, taken by `main` once every refusal that needs no state is behind it and before the
/// leaf runs, and released when the run ends. `Ok(None)` for a leaf that does not hold it, a run that writes nothing and a
/// directory with no `canon.yaml` (nothing there for a plan to read); the refusal is the exit code of the answer already printed.
pub fn hold_for_call(command_tree: &clap::Command, parsed: &clap::ArgMatches) -> Result<Option<Hold>, ExitCode> {
    let mut names = Vec::new();
    let mut leaf = parsed;
    while let Some((name, sub)) = leaf.subcommand() {
        names.push(name);
        leaf = sub;
    }
    let command = names.join(" ");
    let has = |name: &str| leaf.try_get_one::<bool>(name).ok().flatten().copied().unwrap_or(false) || (leaf.try_contains_id(name).unwrap_or(false) && leaf.try_get_one::<bool>(name).is_err());
    if !holds(&command, has).unwrap_or(false) || !crate::run::entry::leaf_takes_repo(command_tree, &names) {
        return Ok(None);
    }
    let repo = leaf.get_one::<PathBuf>("repo").map_or_else(|| Path::new("."), PathBuf::as_path);
    let root = crate::context::resolve_repo_root(repo);
    let json = leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false);
    if !root.join("canon.yaml").is_file() {
        return Ok(None);
    }
    match hold_within(&root, wait()) {
        Ok(hold) => Ok(Some(hold)),
        // A lock file that cannot be made (a read-only `.canon`, a volume without locks) is a refusal for a commit, which must not
        // act unlocked, and nothing for any other leaf: the write it was going to make is the refusal it would have answered with.
        Err(Refused::Io(..)) if !matches!(policy_of(&command), Some(Holds::OnCommit)) => Ok(None),
        Err(refused) => Err(ExitCode::from(refused.error().emit(&format!("canon {command}"), json))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_leaf_that_writes_is_judged_once() {
        assert_eq!(unjudged(), Vec::<&str>::new(), "a leaf that writes must say whether it holds the repository write lock (`write_lock::POLICY`)");
        let mut seen = std::collections::BTreeSet::new();
        for (path, ..) in POLICY {
            assert!(META.iter().any(|row| row.path == *path && row.effect != Effect::Read), "`{path}` is not a leaf that writes");
            assert!(seen.insert(*path), "`{path}` is judged twice");
        }
    }

    #[test]
    fn a_second_guard_in_one_thread_does_not_wait_and_the_last_drop_releases() {
        let dir = tempfile::TempDir::new().unwrap();
        let outer = hold_within(dir.path(), Duration::ZERO).unwrap();
        let inner = hold_within(dir.path(), Duration::ZERO).unwrap();
        // Another thread holds nothing of this thread's: it waits and gives up.
        let path = dir.path().to_path_buf();
        let other = std::thread::spawn(move || hold_within(&path, Duration::from_millis(100)).is_err()).join().unwrap();
        assert!(other, "another thread waits for the holder");
        drop(inner);
        let path = dir.path().to_path_buf();
        assert!(std::thread::spawn(move || hold_within(&path, Duration::from_millis(100)).is_err()).join().unwrap(), "one guard is left");
        drop(outer);
        let path = dir.path().to_path_buf();
        assert!(std::thread::spawn(move || hold_within(&path, Duration::from_secs(5)).is_ok()).join().unwrap(), "the last drop releases");
        assert!(dir.path().join(REPO_WRITE_LOCK_FILE).is_file(), "the lock file is never deleted");
    }

    #[test]
    fn the_root_is_the_same_lock_however_it_is_spelled() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let outer = hold_within(&dir.path().join("sub/.."), Duration::ZERO).unwrap();
        // A refresh given the root another way does not wait for the writer that holds it.
        let inner = hold_within(dir.path(), Duration::ZERO).unwrap();
        let again = hold_within(&dir.path().join("./sub/.."), Duration::ZERO).unwrap();
        drop((outer, inner, again));
    }

    #[test]
    fn a_wait_that_runs_out_is_repo_write_busy_and_retryable() {
        let dir = tempfile::TempDir::new().unwrap();
        let _held = FileLock::acquire(&dir.path().join(REPO_WRITE_LOCK_FILE), Duration::ZERO).unwrap();
        let error = hold_within(dir.path(), Duration::from_millis(80)).unwrap_err().error();
        assert_eq!((error.code, error.exit, error.retryable), ("REPO_WRITE_BUSY", 2, true));
    }

    #[test]
    fn the_policy_reads_the_options_of_the_call() {
        let given = |names: &'static [&'static str]| move |name: &str| names.contains(&name);
        assert_eq!(holds("evidence add", given(&[])), Some(true));
        assert_eq!(holds("evidence add", given(&["dry_run"])), Some(false));
        assert_eq!(holds("purge", given(&[])), Some(false), "a preview");
        assert_eq!(holds("purge", given(&["commit"])), Some(true));
        assert_eq!(holds("subject move", given(&[])), Some(false), "`subject move <id>` reads");
        assert_eq!(holds("subject move", given(&["to"])), Some(true));
        assert_eq!(holds("init", given(&["check"])), Some(false));
        assert_eq!(holds("export", given(&[])), Some(false));
        assert_eq!(holds("ingest records", given(&[])), Some(true));
        assert_eq!(holds("ingest records", given(&["watch"])), Some(false), "a watcher holds it per pass");
        assert_eq!(holds("ingest sessions", given(&["watch", "dry_run"])), Some(false));
        assert_eq!(holds("status", given(&[])), None);
    }
}
