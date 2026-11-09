//! The one way canon runs `git` (0.39, `canon-0-39-git-boundary`).
//!
//! Every call goes through [`GitCall`]; `scripts/check_git_boundary.py` rejects a `Command::new("git")` anywhere
//! else in `crates/*/src`. What the path fixes, so that the answer of a call depends on the repository it names and
//! on nothing the shell holds:
//!
//! - **Environment.** An allow list (0.40, `canon-0-40-git-env-allowlist`): of the `GIT_*` variables of the process
//!   only the eleven names of [`ALLOWED_ENV`] reach the child, every other one is removed ([`env_to_remove`]);
//!   `HOME`, `PATH`, `SSH_AUTH_SOCK` and the XDG variables, which are not `GIT_*`, stay. A call sets `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY` and `GIT_ALTERNATE_OBJECT_DIRECTORIES` only when it
//!   hands over a [`ScratchIndex`], the three together.
//! - **Configuration.** `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, `core.quotepath=false`,
//!   `core.fsmonitor=false`, `color.ui=false`, `core.abbrev=auto`, `diff.suppressBlankEmpty=false` (a diff has no option for it), no pager and a closed stdin (or the bytes the call feeds).
//! - **Time and size.** A call ends within [`DEADLINE`] and reads at most [`OUTPUT_LIMIT`] bytes of stdout and of
//!   stderr each ([`TREE_OUTPUT_LIMIT`] for a call that lists a whole tree, [`GitCall::tree_listing`]). Past
//!   either, the git child is killed (only the child: a grandchild that holds a pipe is not waited for) and the
//!   output read so far is dropped. The deadline has no knob in a release build; a debug build reads
//!   `CANON_TEST_GIT_DEADLINE_MS` so a test can shorten it.
//!
//! A file name is read from NUL-terminated output (`-z`) by the caller: `core.quotepath=false` alone leaves a
//! name with a quote or a newline escaped.
//!
//! A long-lived child (`cat-file --batch`) is built with [`GitCall::command`]: the same command line, the caller
//! owns the pipes and the lifetime.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a one-shot call may run. The value a `git add -A` over a large tree finishes in.
pub const DEADLINE: Duration = Duration::from_secs(300);
/// What a one-shot call may write to stdout and, separately, to stderr.
pub const OUTPUT_LIMIT: usize = 8 * MIB;
/// What a call that lists a whole tree may write to stdout and to stderr ([`GitCall::tree_listing`]).
pub const TREE_OUTPUT_LIMIT: usize = 64 * MIB;

const MIB: usize = 1024 * 1024;
/// How often a running call looks at the clock and at the child.
const POLL: Duration = Duration::from_millis(2);

/// The `GIT_*` names that reach a git child, one by one. Every other `GIT_*` of the process environment is removed
/// ([`env_to_remove`]); a name git adds later is closed by default. What passes cannot change the bytes of a local
/// answer:
///
/// - the three that choose the host's gitconfig files (what those files hold is pinned by [`DIFF_SHAPE`],
///   [`STATUS_SHAPE`] and the `-c` options of [`GitCall::command`]);
/// - the eight that only matter when a partial clone fetches a missing object from its remote.
pub const ALLOWED_ENV: [&str; 11] = [
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_SSH_VARIANT",
    "GIT_ASKPASS",
    "GIT_PROXY_COMMAND",
    "GIT_SSL_CAINFO",
    "GIT_SSL_CAPATH",
    "GIT_SSL_NO_VERIFY",
];

/// What the working-tree diff that canon reads and hashes (`diff --no-ext-diff --binary <SHAPE> HEAD`) is given, so
/// that no gitconfig changes its shape. Each value is git's default: a user with no configuration gets the bytes
/// of 0.39.
pub const DIFF_SHAPE: [&str; 9] = [
    "--no-color",
    "-U3",
    "--inter-hunk-context=0",
    "--src-prefix=a/",
    "--dst-prefix=b/",
    "--find-renames",
    "--diff-algorithm=myers",
    "--no-relative",
    "--ignore-submodules=none",
];

/// What `status --porcelain=v1 -z <SHAPE>` is given, for the same reason as [`DIFF_SHAPE`].
pub const STATUS_SHAPE: [&str; 4] = ["--untracked-files=normal", "--find-renames", "--no-branch", "--ignore-submodules=none"];

/// What `diff --name-only -z --no-renames <SHAPE> <range>` is given: a submodule update is a change whatever
/// `diff.ignoreSubmodules` says.
pub const NAMES_SHAPE: [&str; 1] = ["--ignore-submodules=none"];

/// The names of `names` that start with `GIT_` and are not in [`ALLOWED_ENV`]: what a git child must not inherit.
/// A pure function of the names, so a test needs no `set_var`.
pub fn env_to_remove<I>(names: I) -> Vec<OsString>
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    names
        .into_iter()
        .filter(|name| {
            let text = name.as_ref().to_string_lossy();
            text.starts_with("GIT_") && !ALLOWED_ENV.contains(&&*text)
        })
        .map(|name| name.as_ref().to_os_string())
        .collect()
}

/// Why a git call did not give an answer.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// The process did not start (no `git` on `PATH`, no such working directory).
    #[error("could not start `git {subcommand}`: {source}")]
    Spawn { subcommand: String, #[source] source: io::Error },
    /// The call ran past its deadline and the git child was killed.
    #[error("`git {subcommand}` did not finish within {deadline} and was stopped")]
    Deadline { subcommand: String, deadline: String },
    /// The call wrote past the output limit of a stream and the git child was killed.
    #[error("`git {subcommand}` wrote more than {limit} to {stream} and was stopped")]
    OutputLimit { subcommand: String, stream: &'static str, limit: String },
    /// Git ran and exited unsuccessfully. `stderr` is what it said.
    #[error("`git {subcommand}` failed{}", exit_detail(*.code, .stderr))]
    Exit { subcommand: String, code: Option<i32>, stderr: Vec<u8> },
    /// Waiting for the child failed.
    #[error("`git {subcommand}`: {source}")]
    Wait { subcommand: String, #[source] source: io::Error },
    /// The scratch index could not be prepared.
    #[error("could not prepare a scratch index: {0}")]
    Scratch(#[source] io::Error),
}

fn exit_detail(code: Option<i32>, stderr: &[u8]) -> String {
    let said = String::from_utf8_lossy(stderr);
    let said = said.trim();
    match (said.is_empty(), code) {
        (false, _) => format!(": {said}"),
        (true, Some(code)) => format!(" (exit status {code})"),
        (true, None) => " (killed by a signal)".to_string(),
    }
}

impl GitError {
    /// Whether git ran and said no (an exit status), as opposed to a call that never got an answer: it did not
    /// start, ran past its deadline or wrote past its limit.
    pub fn is_exit(&self) -> bool {
        matches!(self, Self::Exit { .. })
    }

    /// Whether the call was stopped for time or size: the answer is unknown, not "no".
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Deadline { .. } | Self::OutputLimit { .. })
    }

    /// What git said on stderr, trimmed; empty for a failure that is not an exit status.
    pub fn stderr_text(&self) -> String {
        match self {
            Self::Exit { stderr, .. } => String::from_utf8_lossy(stderr).trim().to_string(),
            _ => String::new(),
        }
    }
}

/// What a successful call wrote.
#[derive(Debug)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitOutput {
    /// Stdout as text, lossy, with the surrounding white space trimmed.
    pub fn stdout_trimmed(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }
}

/// A temporary index with the object directories it needs: the files of a code tree are written there, so a run
/// reads the repository and writes nothing in it. The three go to git together, or not at all.
#[derive(Debug, Clone)]
pub struct ScratchIndex {
    index: PathBuf,
    objects: PathBuf,
    alternates: OsString,
}

impl ScratchIndex {
    /// `index` is the temporary index file, `objects` the temporary object directory git writes to, and
    /// `alternates` the repository's own object directory and its alternates, joined as `GIT_ALTERNATE_OBJECT_DIRECTORIES`.
    pub fn new(index: PathBuf, objects: PathBuf, alternates: OsString) -> Self {
        Self { index, objects, alternates }
    }

    /// The temporary index file.
    pub fn index_file(&self) -> &Path {
        &self.index
    }

    /// The scratch of `repo` kept in `dir`: an index there and an object directory below it, reading the
    /// repository's objects (and the ones its `objects/info/alternates` lists) through the alternates.
    pub fn open(repo: &Path, dir: &Path) -> Result<Self, GitError> {
        let real = GitCall::new(repo).args(["rev-parse", "--path-format=absolute", "--git-path", "objects"]).run()?.stdout_trimmed();
        let real = PathBuf::from(real);
        let mut dirs = vec![real.clone()];
        if let Ok(listed) = std::fs::read_to_string(real.join("info/alternates")) {
            dirs.extend(listed.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| real.join(l)));
        }
        let alternates = std::env::join_paths(dirs).map_err(|e| GitError::Scratch(io::Error::other(e)))?;
        let objects = dir.join("objects");
        std::fs::create_dir_all(&objects).map_err(GitError::Scratch)?;
        Ok(Self::new(dir.join("index"), objects, alternates))
    }
}

/// One git call: where, what, and what it may cost. Build it, then [`run`](Self::run) it, or take the hardened
/// [`command`](Self::command) for a child that outlives the call.
#[derive(Debug)]
pub struct GitCall<'a> {
    repo: &'a Path,
    args: Vec<OsString>,
    stdin: Option<&'a [u8]>,
    scratch: Option<&'a ScratchIndex>,
    no_lazy_fetch: bool,
    output_limit: usize,
    deadline: Duration,
}

impl<'a> GitCall<'a> {
    /// A call run in `repo` (the working directory; git finds the repository from there).
    pub fn new(repo: &'a Path) -> Self {
        Self { repo, args: Vec::new(), stdin: None, scratch: None, no_lazy_fetch: false, output_limit: OUTPUT_LIMIT, deadline: deadline() }
    }

    /// The arguments after `git`: the subcommand and its options.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args.extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    /// Bytes fed to git's stdin. Without them stdin is closed.
    #[must_use]
    pub fn stdin(mut self, bytes: &'a [u8]) -> Self {
        self.stdin = Some(bytes);
        self
    }

    /// Run against a temporary index and object directories instead of the repository's own.
    #[must_use]
    pub fn scratch(mut self, scratch: &'a ScratchIndex) -> Self {
        self.scratch = Some(scratch);
        self
    }

    /// Do not fetch a missing object from a promisor remote (`GIT_NO_LAZY_FETCH=1`).
    #[must_use]
    pub fn no_lazy_fetch(mut self) -> Self {
        self.no_lazy_fetch = true;
        self
    }

    /// The call lists a whole tree: it may write [`TREE_OUTPUT_LIMIT`] instead of [`OUTPUT_LIMIT`].
    #[must_use]
    pub fn tree_listing(mut self) -> Self {
        self.output_limit = TREE_OUTPUT_LIMIT;
        self
    }

    /// The command line of the call: the hardened environment and configuration, the arguments, the working
    /// directory. Stdio is the caller's. [`run`](Self::run) starts from this; a child that outlives the call
    /// (`cat-file --batch`) starts from it too.
    pub fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(self.repo).args(["--no-pager", "-c", "core.quotepath=false", "-c", "core.fsmonitor=false", "-c", "color.ui=false", "-c", "core.abbrev=auto", "-c", "diff.suppressBlankEmpty=false"]).args(&self.args);
        for name in env_to_remove(std::env::vars_os().map(|(name, _)| name)) {
            command.env_remove(name);
        }
        command.env("LC_ALL", "C").env("GIT_OPTIONAL_LOCKS", "0").env("GIT_TERMINAL_PROMPT", "0");
        if self.no_lazy_fetch {
            command.env("GIT_NO_LAZY_FETCH", "1");
        }
        if let Some(scratch) = self.scratch {
            command.env("GIT_INDEX_FILE", &scratch.index).env("GIT_OBJECT_DIRECTORY", &scratch.objects).env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &scratch.alternates);
        }
        command
    }

    /// Run the call to its end, its deadline or its output limit. Success is an exit status of zero.
    pub fn run(self) -> Result<GitOutput, GitError> {
        let subcommand = subcommand(&self.args);
        let mut command = self.command();
        command.stdin(if self.stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|source| GitError::Spawn { subcommand: subcommand.clone(), source })?;
        let started = Instant::now();

        let (events, inbox) = mpsc::channel();
        if let (Some(bytes), Some(mut pipe)) = (self.stdin, child.stdin.take()) {
            let bytes = bytes.to_vec();
            // Detached: a grandchild may hold the pipe's read end, and the call does not wait for it. A write that
            // fails is git having stopped reading; its exit status says why.
            std::thread::spawn(move || {
                let _ = pipe.write_all(&bytes);
            });
        }
        if let Some(pipe) = child.stdout.take() {
            read_limited(Stream::Stdout, pipe, self.output_limit, events.clone());
        }
        if let Some(pipe) = child.stderr.take() {
            read_limited(Stream::Stderr, pipe, self.output_limit, events.clone());
        }

        let mut status = None;
        let mut stdout = None;
        let mut stderr = None;
        loop {
            let Some(left) = self.deadline.checked_sub(started.elapsed()) else {
                stop(&mut child);
                return Err(GitError::Deadline { subcommand, deadline: describe_duration(self.deadline) });
            };
            match inbox.recv_timeout(left.min(POLL)) {
                Ok(Event::Done(Stream::Stdout, bytes)) => stdout = Some(bytes),
                Ok(Event::Done(Stream::Stderr, bytes)) => stderr = Some(bytes),
                Ok(Event::Over(stream)) => {
                    stop(&mut child);
                    return Err(GitError::OutputLimit { subcommand, stream: stream.name(), limit: describe_size(self.output_limit) });
                }
                Err(_) => {}
            }
            if status.is_none() {
                status = child.try_wait().map_err(|source| GitError::Wait { subcommand: subcommand.clone(), source })?;
            }
            if let (Some(status), Some(stdout), Some(stderr)) = (status, &mut stdout, &mut stderr) {
                return if status.success() {
                    Ok(GitOutput { stdout: std::mem::take(stdout), stderr: std::mem::take(stderr) })
                } else {
                    Err(GitError::Exit { subcommand, code: status.code(), stderr: std::mem::take(stderr) })
                };
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

enum Event {
    Done(Stream, Vec<u8>),
    Over(Stream),
}

/// Read `pipe` to its end on a thread of its own, up to `limit` bytes. Detached: when the child is killed a
/// grandchild may still hold the pipe, and the call does not wait for the thread.
fn read_limited(stream: Stream, mut pipe: impl Read + Send + 'static, limit: usize, events: mpsc::Sender<Event>) {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = vec![0; 64 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if bytes.len() + n > limit {
                        let _ = events.send(Event::Over(stream));
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = events.send(Event::Done(stream, bytes));
    });
}

/// Kill the git child and reap it. Only the child: its own children are not signalled.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The git subcommand of `args`, for a message: the first argument that is not an option or the value of `-c`/`-C`.
fn subcommand(args: &[OsString]) -> String {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        if text == "-c" || text == "-C" {
            args.next();
        } else if !text.starts_with('-') {
            return text.into_owned();
        }
    }
    String::new()
}

fn describe_duration(duration: Duration) -> String {
    if duration.subsec_millis() == 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn describe_size(bytes: usize) -> String {
    if bytes.is_multiple_of(MIB) { format!("{} MiB", bytes / MIB) } else { format!("{bytes} bytes") }
}

/// The deadline of a one-shot call: [`DEADLINE`]. A debug build reads `CANON_TEST_GIT_DEADLINE_MS` (milliseconds)
/// so a test can shorten it; a release build has no such variable.
fn deadline() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(millis) = std::env::var("CANON_TEST_GIT_DEADLINE_MS").ok().and_then(|value| value.parse().ok()) {
        return Duration::from_millis(millis);
    }
    DEADLINE
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    impl GitCall<'_> {
        fn within(mut self, deadline: Duration) -> Self {
            self.deadline = deadline;
            self
        }

        fn at_most(mut self, bytes: usize) -> Self {
            self.output_limit = bytes;
            self
        }
    }

    fn envs(command: &Command) -> BTreeMap<String, Option<String>> {
        command.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect()
    }

    fn here() -> PathBuf {
        std::env::current_dir().expect("the test has a working directory")
    }

    /// The thirteen names 0.39 removed.
    const REMOVED_BY_0_39: [&str; 13] = [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_GLOB_PATHSPECS",
        "GIT_NOGLOB_PATHSPECS",
        "GIT_LITERAL_PATHSPECS",
        "GIT_ICASE_PATHSPECS",
    ];

    /// gate.git-boundary.01 (0.39, kept in 0.40): the thirteen names 0.39 removed are still removed, and the command fixes the rest.
    #[test]
    fn gate_git_boundary_01_the_command_clears_the_repository_variables_and_fixes_the_rest() {
        let repo = here();
        let command = GitCall::new(&repo).args(["status"]).command();
        let envs = envs(&command);
        assert_eq!(env_to_remove(REMOVED_BY_0_39).len(), 13, "the allow list lets none of the 0.39 names through");
        assert_eq!(envs["LC_ALL"].as_deref(), Some("C"));
        assert_eq!(envs["GIT_OPTIONAL_LOCKS"].as_deref(), Some("0"));
        assert_eq!(envs["GIT_TERMINAL_PROMPT"].as_deref(), Some("0"));
        assert!(!envs.contains_key("GIT_NO_LAZY_FETCH"), "lazy fetch is left alone unless the call turns it off");
        for kept in ["HOME", "PATH", "SSH_AUTH_SOCK", "XDG_CONFIG_HOME"] {
            assert!(!envs.contains_key(kept), "{kept} is not touched");
        }
        let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["--no-pager", "-c", "core.quotepath=false", "-c", "core.fsmonitor=false", "-c", "color.ui=false", "-c", "core.abbrev=auto", "-c", "diff.suppressBlankEmpty=false", "status"]);
    }

    /// gate.git-env.02 (F1): the name test keeps what the list names, drops every other `GIT_*` (one git does not know too) and leaves a name
    /// that is not `GIT_*` alone; the variables canon sets itself are set on the command whatever the process held.
    #[test]
    fn gate_git_env_02_the_allow_list_keeps_its_names_and_the_command_sets_canons_own_variables() {
        let held = ["GIT_SSH_COMMAND", "GIT_CONFIG_GLOBAL", "GIT_CANON_TEST_UNKNOWN", "GIT_DIFF_OPTS", "PATH", "HOME", "SSH_AUTH_SOCK", "LC_ALL"];
        let removed = env_to_remove(held);
        assert_eq!(removed, [OsString::from("GIT_CANON_TEST_UNKNOWN"), OsString::from("GIT_DIFF_OPTS")]);
        for name in ALLOWED_ENV {
            assert!(env_to_remove([name]).is_empty(), "{name} passes");
        }
        assert_eq!(ALLOWED_ENV.len(), 11);
        // The names that matter are exact: a prefix of an allowed name is not allowed, and `GIT_CONFIG_COUNT` is not `GIT_CONFIG_GLOBAL`.
        assert_eq!(env_to_remove(["GIT_SSH_COMMAND_EXTRA", "GIT_CONFIG", "GIT_CONFIG_COUNT", "git_dir"]).len(), 3, "lower case is a different name");
        // What canon sets is set on the command, so it wins over a removal of the same name.
        let repo = here();
        let scratch = ScratchIndex::new(PathBuf::from("/s/index"), PathBuf::from("/s/objects"), OsString::from("/r/objects"));
        let set = envs(&GitCall::new(&repo).scratch(&scratch).no_lazy_fetch().command());
        assert_eq!(set["GIT_OPTIONAL_LOCKS"].as_deref(), Some("0"));
        assert_eq!(set["GIT_TERMINAL_PROMPT"].as_deref(), Some("0"));
        assert_eq!(set["GIT_NO_LAZY_FETCH"].as_deref(), Some("1"));
        assert_eq!(set["GIT_INDEX_FILE"].as_deref(), Some("/s/index"));
        assert_eq!(set["GIT_OBJECT_DIRECTORY"].as_deref(), Some("/s/objects"));
        assert_eq!(set["GIT_ALTERNATE_OBJECT_DIRECTORIES"].as_deref(), Some("/r/objects"));
    }

    /// gate.git-env.03 (F1, failure path): a name off the list is removed whatever its value, and the thirteen names 0.39 removed
    /// (`GIT_DIR`, `GIT_INDEX_FILE`) are still removed. A child started under a polluted environment really sees none of them.
    #[test]
    fn gate_git_env_03_a_name_off_the_list_is_removed_whatever_its_value_and_the_child_does_not_see_it() {
        let polluted = ["GIT_DIFF_OPTS", "GIT_EXTERNAL_DIFF", "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0", "GIT_CONFIG_PARAMETERS", "GIT_EXEC_PATH", "GIT_TRACE", "GIT_TRACE2", "GIT_PAGER", "GIT_EDITOR", "GIT_AUTHOR_NAME", "GIT_COMMITTER_EMAIL", "GIT_ATTR_SOURCE", "GIT_NO_REPLACE_OBJECTS", "GIT_DISCOVERY_ACROSS_FILESYSTEM", "GIT_QUARANTINE_PATH", "GIT_CANON_TEST_UNKNOWN"];
        assert_eq!(env_to_remove(polluted).len(), polluted.len());
        assert_eq!(env_to_remove(REMOVED_BY_0_39).len(), 13);
        assert_eq!(env_to_remove(["GIT_DIR", "GIT_INDEX_FILE"]).len(), 2);

        // The same through a real child: `env` run with the command's removals applied on top of a polluted environment.
        let mut probe = Command::new("env");
        probe.env_clear().env("PATH", std::env::var_os("PATH").unwrap_or_default());
        for name in polluted.iter().chain(REMOVED_BY_0_39.iter()).chain(ALLOWED_ENV.iter()) {
            probe.env(name, "polluted");
        }
        for name in env_to_remove(probe.get_envs().map(|(name, _)| name)) {
            probe.env_remove(name);
        }
        let out = probe.output().expect("env runs");
        let text = String::from_utf8_lossy(&out.stdout);
        let seen: Vec<&str> = text.lines().filter_map(|line| line.split('=').next()).filter(|name| name.starts_with("GIT_")).collect();
        let mut allowed: Vec<&str> = ALLOWED_ENV.to_vec();
        let mut seen_sorted = seen.clone();
        seen_sorted.sort_unstable();
        allowed.sort_unstable();
        assert_eq!(seen_sorted, allowed, "only the allowed names reach the child");
    }

    #[test]
    fn gate_git_boundary_02_a_scratch_index_sets_the_three_variables_together_and_only_then() {
        let repo = here();
        let scratch = ScratchIndex::new(PathBuf::from("/s/index"), PathBuf::from("/s/objects"), OsString::from("/r/objects"));
        let with = envs(&GitCall::new(&repo).scratch(&scratch).command());
        assert_eq!(with["GIT_INDEX_FILE"].as_deref(), Some("/s/index"));
        assert_eq!(with["GIT_OBJECT_DIRECTORY"].as_deref(), Some("/s/objects"));
        assert_eq!(with["GIT_ALTERNATE_OBJECT_DIRECTORIES"].as_deref(), Some("/r/objects"));
        let without = envs(&GitCall::new(&repo).command());
        for name in ["GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES"] {
            assert_eq!(without[name], None, "{name} is removed when no scratch index is handed over");
        }
    }

    #[test]
    fn lazy_fetch_is_turned_off_only_when_the_call_asks() {
        let repo = here();
        assert_eq!(envs(&GitCall::new(&repo).no_lazy_fetch().command())["GIT_NO_LAZY_FETCH"].as_deref(), Some("1"));
    }

    #[test]
    fn a_call_returns_what_git_wrote() {
        let repo = here();
        let out = GitCall::new(&repo).args(["hash-object", "--stdin"]).stdin(b"canon\n").run().expect("hash-object runs without a repository");
        assert_eq!(out.stdout_trimmed().len(), 40);
    }

    #[test]
    fn a_large_stdin_and_a_large_stdout_do_not_deadlock() {
        let repo = here();
        let input = vec![b'x'; 4 * MIB];
        let hashed = GitCall::new(&repo).args(["hash-object", "--stdin"]).stdin(&input).run().expect("4 MiB of stdin is fed while stdout is read");
        assert_eq!(hashed.stdout_trimmed().len(), 40);
    }

    #[test]
    fn an_exit_status_is_an_exit_error_with_what_git_said() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let err = GitCall::new(dir.path()).args(["rev-parse", "--verify", "no-such-revision^{commit}"]).run().expect_err("git does not know the revision");
        assert!(err.is_exit() && !err.is_stopped(), "{err}");
        assert!(err.to_string().starts_with("`git rev-parse` failed"), "{err}");
        assert!(!err.stderr_text().is_empty() || err.to_string().contains("exit status"), "{err}");
    }

    #[test]
    fn a_missing_working_directory_is_a_start_failure_naming_the_subcommand() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let err = GitCall::new(&dir.path().join("absent")).args(["rev-parse", "HEAD"]).run().expect_err("there is no such directory");
        assert!(matches!(err, GitError::Spawn { .. }), "{err}");
        assert!(err.to_string().contains("`git rev-parse`"), "{err}");
    }

    #[test]
    fn gate_git_boundary_09_a_call_past_its_deadline_is_stopped_and_names_the_subcommand_and_the_deadline() {
        let repo = here();
        let started = Instant::now();
        let err = GitCall::new(&repo).args(["-c", "alias.nap=!sleep 30", "nap"]).within(Duration::from_secs(1)).run().expect_err("the alias sleeps 30 seconds");
        assert!(matches!(err, GitError::Deadline { .. }), "{err}");
        assert!(err.is_stopped());
        assert_eq!(err.to_string(), "`git nap` did not finish within 1s and was stopped");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    }

    #[test]
    fn gate_git_boundary_09_a_grandchild_that_holds_the_pipes_is_not_waited_for() {
        let repo = here();
        let started = Instant::now();
        // git -> sh -> sleep: killing git leaves the sleeps holding stdout and stderr.
        let err = GitCall::new(&repo).args(["-c", "alias.nap=!sleep 30 & sleep 30", "nap"]).within(Duration::from_secs(1)).run().expect_err("the alias outlives the deadline");
        assert!(matches!(err, GitError::Deadline { .. }), "{err}");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    }

    #[test]
    fn gate_git_boundary_09_nine_mib_is_past_the_default_limit_and_inside_the_tree_listing_limit() {
        let repo = here();
        let nine_mib = ["-c", "alias.big=!head -c 9437184 /dev/zero", "big"];
        let err = GitCall::new(&repo).args(nine_mib).run().expect_err("9 MiB is past 8 MiB");
        assert!(matches!(err, GitError::OutputLimit { stream: "stdout", .. }), "{err}");
        assert_eq!(err.to_string(), "`git big` wrote more than 8 MiB to stdout and was stopped");
        let out = GitCall::new(&repo).args(nine_mib).tree_listing().run().expect("9 MiB is inside 64 MiB");
        assert_eq!(out.stdout.len(), 9 * MIB);
    }

    #[test]
    fn gate_git_boundary_09_a_limit_applies_to_each_stream_and_exactly_the_limit_is_allowed() {
        let repo = here();
        let to_stderr = ["-c", "alias.loud=!head -c 2048 /dev/zero >&2", "loud"];
        let err = GitCall::new(&repo).args(to_stderr).at_most(1024).run().expect_err("2048 bytes on stderr is past 1024");
        assert!(matches!(err, GitError::OutputLimit { stream: "stderr", .. }), "{err}");
        assert_eq!(err.to_string(), "`git loud` wrote more than 1024 bytes to stderr and was stopped");
        let exact = GitCall::new(&repo).args(["-c", "alias.exact=!head -c 1024 /dev/zero", "exact"]).at_most(1024).run().expect("exactly the limit is allowed");
        assert_eq!(exact.stdout.len(), 1024);
    }

    #[test]
    fn the_subcommand_skips_options_and_the_values_of_c() {
        let args: Vec<OsString> = ["--no-pager", "-c", "a=b", "-C", "dir", "log", "--oneline"].iter().map(OsString::from).collect();
        assert_eq!(subcommand(&args), "log");
        assert_eq!(subcommand(&[]), "");
    }
}
