//! Running the binary under test (approaches/checker.md 3).
//!
//! The fixture's `setup` runs once into a template directory. Every
//! invocation then gets its own copy of that template as its working
//! directory, with `HOME` pointing into the copy, an environment of only
//! `PATH`, `HOME`, `LANG=C.UTF-8` and the fixture's own variables (plus the
//! call's, for the checks that vary the environment), stdin closed
//! (`/dev/null`), and the fixture's time limit. A check that needs a terminal
//! runs the binary with stdout on a pseudo-terminal. Telemetry opt-outs are
//! never set: a write the tool does on its own is what the effects check must
//! see.

use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use tempfile::TempDir;

use crate::fixture::Fixture;
use crate::snapshot::{changed, copy_tree, take, Snapshot};

/// How an invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    /// Killed by a signal.
    Signal(i32),
    /// Still running at the time limit; the checker killed it.
    TimedOut,
}

/// One invocation: argv, the variables added to the environment, and whether stdout is a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Stdout on a pseudo-terminal.
    pub pty: bool,
    /// Stdin on a pseudo-terminal that never sends anything: a process that reads it waits (a prompt).
    pub stdin_pty: bool,
    /// A time limit shorter than the fixture's, for the call that must not wait.
    pub limit: Option<Duration>,
}

impl Call {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            env: Vec::new(),
            pty: false,
            stdin_pty: false,
            limit: None,
        }
    }

    /// The same call with one more environment variable.
    #[must_use]
    pub fn with_env(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_string(), value.to_string()));
        self
    }

    /// The same call with stdout on a pseudo-terminal.
    #[must_use]
    pub fn on_pty(mut self) -> Self {
        self.pty = true;
        self
    }

    /// The same call with stdin on a terminal nobody types on, and at most `limit` to finish.
    #[must_use]
    pub fn waiting_on_stdin(mut self, limit: Duration) -> Self {
        self.stdin_pty = true;
        self.limit = Some(limit);
        self
    }

    /// Whether argv selects JSON mode: the word `--json` before the first `--` (spec 4.4).
    pub fn json_mode(&self) -> bool {
        atip::wants_json(&self.argv)
    }
}

/// What one invocation did.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub call: Call,
    pub exit: Exit,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The paths the invocation changed in its working directory (`./path`) and in `HOME` (`~/path`); measured only when it exited 2.
    pub changes: Vec<String>,
}

/// Why the checker could not run the binary or build the template.
#[derive(Debug)]
pub enum RunError {
    /// The binary could not be started.
    Spawn(String),
    /// A `setup` command failed.
    Setup(String),
    /// The checker's own scratch space failed.
    Scratch(String),
}

/// The binary under test and the template every invocation copies.
#[derive(Debug)]
pub struct Runner {
    bin: PathBuf,
    scratch: TempDir,
    template: PathBuf,
    template_files: Snapshot,
    fixture_env: Vec<(String, String)>,
    timeout: Duration,
    sequences: usize,
}

impl Runner {
    /// Builds the template by running the fixture's `setup` once.
    pub fn new(bin: &Path, fixture: &Fixture) -> Result<Self, RunError> {
        let scratch = tempfile::Builder::new()
            .prefix("atip-check-")
            .tempdir()
            .map_err(|error| RunError::Scratch(error.to_string()))?;
        let template = scratch.path().join("template");
        for part in ["work", "home"] {
            std::fs::create_dir_all(template.join(part))
                .map_err(|error| RunError::Scratch(error.to_string()))?;
        }
        let mut runner = Self {
            bin: bin.to_path_buf(),
            scratch,
            template,
            template_files: Snapshot::new(),
            fixture_env: fixture
                .env
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            timeout: fixture.timeout,
            sequences: 0,
        };
        for command in &fixture.setup {
            let template = runner.template.clone();
            let outcome = runner
                .execute(&Call::new(command.iter().cloned()), &template)
                .map_err(|error| RunError::Spawn(error.to_string()))?;
            if outcome.exit != Exit::Code(0) {
                let detail = match outcome.exit {
                    Exit::Code(code) => format!("exited {code}"),
                    Exit::Signal(signal) => format!("was killed by signal {signal}"),
                    Exit::TimedOut => {
                        format!("ran over the {} second limit", runner.timeout.as_secs_f64())
                    }
                };
                return Err(RunError::Setup(format!(
                    "the setup command `{}` {detail}: {}",
                    command.join(" "),
                    tail(&outcome.stderr)
                )));
            }
        }
        runner.template_files =
            Self::both(&runner.template).map_err(|error| RunError::Scratch(error.to_string()))?;
        Ok(runner)
    }

    fn both(root: &Path) -> std::io::Result<Snapshot> {
        let mut all = Snapshot::new();
        for (part, label) in [("work", "."), ("home", "~")] {
            for (path, entry) in take(&root.join(part))? {
                all.insert(format!("{label}{path}"), entry);
            }
        }
        Ok(all)
    }

    /// Runs `call` once in its own fresh copy of the template.
    pub fn run(&mut self, call: &Call) -> Result<Outcome, RunError> {
        self.sequences += 1;
        let sequence = self
            .scratch
            .path()
            .join(format!("sequence-{}", self.sequences));
        for part in ["work", "home"] {
            copy_tree(&self.template.join(part), &sequence.join(part))
                .map_err(|error| RunError::Scratch(error.to_string()))?;
        }
        let mut outcome = self
            .execute(call, &sequence)
            .map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))?;
        if outcome.exit == Exit::Code(2) {
            let after =
                Self::both(&sequence).map_err(|error| RunError::Scratch(error.to_string()))?;
            outcome.changes = changed(&self.template_files, &after);
        }
        Ok(outcome)
    }

    fn execute(&self, call: &Call, root: &Path) -> std::io::Result<Outcome> {
        let mut command = Command::new(&self.bin);
        command
            .args(&call.argv)
            .current_dir(root.join("work"))
            .env_clear()
            .env(
                "PATH",
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            )
            .env("HOME", root.join("home"))
            .env("LANG", "C.UTF-8")
            .envs(self.fixture_env.iter().map(|(name, value)| (name, value)))
            .envs(call.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .process_group(0);
        // Terminals: stdout's slave end goes to the child and a copy stays here until the output has been read;
        // stdin's pair is only held, so a process that reads it waits.
        let mut keep: Vec<std::os::fd::OwnedFd> = Vec::new();
        let stdout_terminal = if call.pty {
            let pty = raw_terminal()?;
            let kept = pty.slave.try_clone()?;
            command.stdout(Stdio::from(pty.slave));
            Some((std::fs::File::from(pty.master), kept))
        } else {
            command.stdout(Stdio::piped());
            None
        };
        if call.stdin_pty {
            let pty = nix::pty::openpty(None, None).map_err(std::io::Error::from)?;
            command.stdin(Stdio::from(pty.slave));
            keep.push(pty.master);
        }
        let mut child = command.spawn()?;
        drop(command);
        let ended = Arc::new(AtomicBool::new(false));
        let stdout = match (stdout_terminal, child.stdout.take()) {
            (Some((master, slave)), _) => terminal_reader(master, slave, Arc::clone(&ended)),
            (None, Some(pipe)) => reader(pipe),
            (None, None) => thread::spawn(Vec::new),
        };
        let stderr = child
            .stderr
            .take()
            .map_or_else(|| thread::spawn(Vec::new), reader);
        let exit = wait(&mut child, call.limit.unwrap_or(self.timeout))?;
        // Whatever the tool left running in its group must not hold the output open.
        kill_group(&child);
        ended.store(true, Ordering::Release);
        let stdout = stdout.join().unwrap_or_default();
        drop(keep);
        Ok(Outcome {
            call: call.clone(),
            exit,
            stdout,
            stderr: stderr.join().unwrap_or_default(),
            changes: Vec::new(),
        })
    }
}

/// A pseudo-terminal in raw mode: no line translation (`\n` stays `\n`), no echo.
fn raw_terminal() -> std::io::Result<nix::pty::OpenptyResult> {
    let pty = nix::pty::openpty(None, None).map_err(std::io::Error::from)?;
    let mut settings = nix::sys::termios::tcgetattr(&pty.slave).map_err(std::io::Error::from)?;
    nix::sys::termios::cfmakeraw(&mut settings);
    nix::sys::termios::tcsetattr(&pty.slave, nix::sys::termios::SetArg::TCSANOW, &settings)
        .map_err(std::io::Error::from)?;
    Ok(pty)
}

/// Reads a terminal's master end to its end of file.
///
/// While the process runs, and until nothing more is waiting after it ended (`ended`), `slave` stays open here:
/// closing the last slave end before the master has been read can throw unread output away, so the master never
/// reports a hang-up early. Then `slave` is closed and the read goes on to EOF (the hang-up error), so nothing the
/// process wrote is cut.
fn terminal_reader(
    mut master: std::fs::File,
    slave: std::os::fd::OwnedFd,
    ended: Arc<AtomicBool>,
) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let ended_before = ended.load(Ordering::Acquire);
            let mut fds = [nix::poll::PollFd::new(
                master.as_fd(),
                nix::poll::PollFlags::POLLIN,
            )];
            let waiting =
                nix::poll::poll(&mut fds, nix::poll::PollTimeout::from(20u8)).unwrap_or(0) > 0;
            if waiting {
                match master.read(&mut chunk) {
                    Ok(read) if read > 0 => bytes.extend_from_slice(&chunk[..read]),
                    _ => return bytes,
                }
            } else if ended_before {
                break;
            }
        }
        drop(slave);
        while let Ok(read) = master.read(&mut chunk) {
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        bytes
    })
}

fn reader<R: Read + Send + 'static>(mut source: R) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 8192];
        while let Ok(read) = source.read(&mut chunk) {
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        bytes
    })
}

fn kill_group(child: &Child) {
    if let Ok(pid) = i32::try_from(child.id()) {
        // ESRCH is the usual answer: the group is already gone.
        let _gone = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }
}

fn wait(child: &mut Child, limit: Duration) -> std::io::Result<Exit> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(exit_of(status));
        }
        if started.elapsed() >= limit {
            kill_group(child);
            child.kill().unwrap_or_default();
            child.wait()?;
            return Ok(Exit::TimedOut);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn exit_of(status: ExitStatus) -> Exit {
    match (status.code(), status.signal()) {
        (Some(code), _) => Exit::Code(code),
        (None, Some(signal)) => Exit::Signal(signal),
        (None, None) => Exit::Code(-1),
    }
}

/// The last 200 characters of `bytes` as text, for a failure message.
fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let skip = text.chars().count().saturating_sub(200);
    text.chars().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::fixture::Fixture;

    use super::{Call, Exit, Runner};

    fn fixture(setup: &[&[&str]], timeout: Duration) -> Fixture {
        Fixture {
            setup: setup
                .iter()
                .map(|command| command.iter().map(|word| (*word).to_string()).collect())
                .collect(),
            env: [("FIXTURE_VAR".to_string(), "yes".to_string())].into(),
            timeout,
            probe: None,
            registry: None,
        }
    }

    fn sh(script: &str) -> Call {
        Call::new(["-c", script])
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn atip_check_28_the_environment_is_only_path_home_lang_and_the_declared_variables() {
        let mut runner = Runner::new(
            "/usr/bin/env".as_ref(),
            &fixture(&[], Duration::from_secs(10)),
        )
        .expect("runner");
        let outcome = runner
            .run(&Call::new(Vec::<String>::new()).with_env("EXTRA", "1"))
            .expect("run");
        let mut names: Vec<String> = text(&outcome.stdout)
            .lines()
            .map(|line| line.split('=').next().unwrap_or_default().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["EXTRA", "FIXTURE_VAR", "HOME", "LANG", "PATH"]);
        assert!(text(&outcome.stdout).contains("LANG=C.UTF-8"));
    }

    #[test]
    fn atip_check_28_stdin_is_closed_and_a_reader_sees_the_end_at_once() {
        let mut runner = Runner::new("/bin/sh".as_ref(), &fixture(&[], Duration::from_secs(10)))
            .expect("runner");
        let outcome = runner.run(&sh("cat; echo done")).expect("run");
        assert_eq!(
            (outcome.exit, text(&outcome.stdout).trim()),
            (Exit::Code(0), "done")
        );
    }

    #[test]
    fn atip_check_24_every_invocation_runs_in_a_fresh_copy_of_the_setup_state_with_home_inside_it()
    {
        let mut runner = Runner::new(
            "/bin/sh".as_ref(),
            &fixture(
                &[&["-c", "echo seed > seeded; echo h > \"$HOME/home-seed\""]],
                Duration::from_secs(10),
            ),
        )
        .expect("runner");
        let first = runner.run(&sh("cat seeded \"$HOME/home-seed\"; echo scribble >> seeded; echo x > \"$HOME/new\"; pwd; echo $HOME")).expect("run");
        let second = runner
            .run(&sh(
                "cat seeded; test ! -e \"$HOME/new\" && echo no-new; pwd; echo $HOME",
            ))
            .expect("run");
        let (first, second) = (text(&first.stdout), text(&second.stdout));
        assert!(first.starts_with("seed\nh\n"), "{first}");
        assert!(
            second.starts_with("seed\nno-new\n"),
            "the second run saw the first run's writes: {second}"
        );
        let paths: Vec<&str> = first
            .lines()
            .skip(2)
            .chain(second.lines().skip(2))
            .collect();
        assert_eq!(paths.len(), 4);
        assert_ne!(
            paths[0], paths[2],
            "each invocation has its own working directory"
        );
        assert!(
            paths[1].starts_with(paths[0].trim_end_matches("/work")),
            "HOME is inside the copy: {paths:?}"
        );
    }

    #[test]
    fn atip_check_30_a_refusal_that_changes_the_directory_or_home_names_the_paths() {
        let mut runner = Runner::new(
            "/bin/sh".as_ref(),
            &fixture(&[&["-c", "echo a > keep"]], Duration::from_secs(10)),
        )
        .expect("runner");
        let quiet = runner.run(&sh("exit 2")).expect("run");
        assert!(quiet.changes.is_empty(), "{:?}", quiet.changes);
        let noisy = runner
            .run(&sh(
                "echo b > keep; mkdir -p \"$HOME/.cache\"; echo t > \"$HOME/.cache/log\"; exit 2",
            ))
            .expect("run");
        assert_eq!(noisy.changes, ["./keep", "~/.cache", "~/.cache/log"]);
        let accepted = runner.run(&sh("echo b > keep; exit 0")).expect("run");
        assert!(accepted.changes.is_empty(), "only a refusal is measured");
    }

    #[test]
    fn atip_check_28_a_pseudo_terminal_run_has_a_terminal_on_stdout_and_a_pipe_run_does_not() {
        let mut runner = Runner::new("/bin/sh".as_ref(), &fixture(&[], Duration::from_secs(10)))
            .expect("runner");
        let script = "if test -t 1; then printf 'tty\\n'; else printf 'pipe\\n'; fi";
        assert_eq!(
            text(&runner.run(&sh(script)).expect("run").stdout),
            "pipe\n"
        );
        assert_eq!(
            text(&runner.run(&sh(script).on_pty()).expect("run").stdout),
            "tty\n",
            "a raw terminal keeps the newline as written"
        );
    }

    #[test]
    fn atip_check_28_everything_a_process_writes_to_a_terminal_is_read_however_fast_it_exits() {
        let mut runner = Runner::new("/bin/sh".as_ref(), &fixture(&[], Duration::from_secs(10)))
            .expect("runner");
        let script = "printf '{\"a\":1}\\n'; printf 'x%.0s' $(seq 1 3000); printf '\\nend\\n'";
        let expected = format!("{{\"a\":1}}\n{}\nend\n", "x".repeat(3000));
        // Under load the process is gone before the first read: its output must still be there.
        let hogs: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| (0..3_000_000u64).fold(0u64, u64::wrapping_add)))
            .collect();
        for round in 0..60 {
            assert_eq!(
                text(&runner.run(&sh(script).on_pty()).expect("run").stdout),
                expected,
                "round {round}"
            );
        }
        for hog in hogs {
            hog.join().expect("hog");
        }
    }

    #[test]
    fn atip_check_26_a_run_over_the_limit_is_killed_with_its_group_and_reported() {
        let mut runner = Runner::new(
            "/bin/sh".as_ref(),
            &fixture(&[], Duration::from_millis(300)),
        )
        .expect("runner");
        let started = std::time::Instant::now();
        let outcome = runner.run(&sh("sleep 30 & sleep 30")).expect("run");
        assert_eq!(outcome.exit, Exit::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the group is killed, the output is not held open by the child"
        );
    }

    #[test]
    fn atip_check_04_a_signal_exit_and_a_binary_that_does_not_start_are_told_apart() {
        let mut runner = Runner::new("/bin/sh".as_ref(), &fixture(&[], Duration::from_secs(10)))
            .expect("runner");
        assert_eq!(
            runner.run(&sh("kill -9 $$")).expect("run").exit,
            Exit::Signal(9)
        );
        let mut missing = Runner::new(
            "/nonexistent/atip-check-bin".as_ref(),
            &fixture(&[], Duration::from_secs(10)),
        )
        .expect("a template needs no binary without setup");
        missing
            .run(&Call::new(["--version"]))
            .expect_err("a binary that does not exist does not start");
    }

    #[test]
    fn atip_check_27_a_failing_setup_command_is_reported_by_name() {
        let error = Runner::new(
            "/bin/sh".as_ref(),
            &fixture(
                &[&["-c", "echo broken >&2; exit 3"]],
                Duration::from_secs(10),
            ),
        )
        .expect_err("setup fails");
        let super::RunError::Setup(reason) = error else {
            panic!("expected a setup error")
        };
        assert!(
            reason.contains("exited 3") && reason.contains("broken"),
            "{reason}"
        );
    }
}
