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
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use tempfile::TempDir;

use crate::fixture::Fixture;
use crate::snapshot::{changed, copy_tree, take, Snapshot};

/// How long a `notRun` command has to end after SIGTERM before it gets SIGKILL.
pub const SERVED_GRACE: Duration = Duration::from_secs(2);
/// How long after its first document a `notRun` command is watched for ending on its own, before it is signalled.
const SERVED_SETTLE_MILLIS: u64 = 200;

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
    /// Stdin and stdout on one pseudo-terminal in canonical mode with echo off and output untranslated, for the call
    /// that must refuse and not prompt (`L0.no-prompt`, approaches/checker.md 3): see [`Runner::run_on_terminal`].
    pub terminal: bool,
    /// A time limit shorter than the fixture's, for the call that must not wait.
    pub limit: Option<Duration>,
    /// Measure what the call changed in the directory and `HOME` whatever its exit (`schema` writes nothing, approaches/checker.md 4); without it only a refusal (exit 2) is measured.
    pub measure: bool,
    /// Run in an empty directory with an empty `HOME` and only `PATH`, `HOME` and `LANG` in the environment: no fixture state, no fixture variables (`L1.no-state`).
    pub empty: bool,
    /// Run in an empty directory and an empty `HOME` with the fixture's environment: the fixture state without its `setup` (`L1.validators`: a validator reads no state, spec 10.8.3).
    pub without_setup: bool,
    /// Run this program instead of the tool (a `prepare` command of the fixture).
    pub program: Option<String>,
    /// What stdin holds (`--input -`); without it stdin is closed.
    pub stdin: Option<Vec<u8>>,
    /// The run of a `notRun` command (approaches/checker.md 5): read stdout up to the end of its first JSON document, check the process is still
    /// running, then signal it (SIGTERM, SIGKILL after [`SERVED_GRACE`]).
    pub served: bool,
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
            terminal: false,
            limit: None,
            measure: false,
            empty: false,
            without_setup: false,
            program: None,
            stdin: None,
            served: false,
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

    /// The same call with stdin and stdout on a canonical-mode terminal.
    #[must_use]
    pub fn on_terminal(mut self) -> Self {
        self.terminal = true;
        self
    }

    /// The same call with its changes to the directory and `HOME` measured whatever its exit.
    #[must_use]
    pub fn measured(mut self) -> Self {
        self.measure = true;
        self
    }

    /// The same call in a copy of the fixture state made without `setup`: an empty directory and `HOME`, the fixture's environment.
    #[must_use]
    pub fn without_setup(mut self) -> Self {
        self.without_setup = true;
        self
    }

    /// The same call with `bytes` on stdin (a document for `--input -`).
    #[must_use]
    pub fn with_stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(bytes.into());
        self
    }

    /// The same call as the run of a command that does not end on its own: until its first JSON document, then ended by a signal.
    #[must_use]
    pub fn until_first_document(mut self) -> Self {
        self.served = true;
        self
    }

    /// The same call in an empty directory and a minimal environment, measured.
    #[must_use]
    pub fn in_empty_state(mut self) -> Self {
        self.empty = true;
        self.measure = true;
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
    /// How long the invocation ran.
    pub elapsed: Duration,
    /// What a terminal run saw at its deadline, when the process was still running then.
    pub at_deadline: Option<AtDeadline>,
    /// The stdout of the fixture's probe, run in the same copy after the invocation exited 2; `None` when it was not run.
    pub probe_after: Option<Vec<u8>>,
    /// The number of the sequence (the copy of the fixture state) the invocation ran in: [`Runner::run_after`] continues it.
    pub sequence: usize,
    /// The run of a `notRun` command that printed its first JSON document and was ended by the checker's signal: its exit is the signal's, its stdout stops at the end of that document.
    pub served: bool,
    /// The run of a `notRun` command that ended on its own within the time limit, before or after its first document: the fixture is wrong, the command belongs in no `notRun`.
    pub ended_alone: bool,
}

/// What a terminal run showed when it was still running at its deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtDeadline {
    /// What the process had printed on the terminal by then.
    pub output: Vec<u8>,
    /// Whether the process exited or printed within two seconds after the checker wrote the end-of-file character.
    pub reacted_to_eof: bool,
}

/// Why the checker could not run the binary or build the template.
#[derive(Debug)]
pub enum RunError {
    /// The binary could not be started.
    Spawn(String),
    /// A `setup` command failed.
    Setup(String),
    /// A `prepare` command failed or could not be started: a defect of the fixture, not of the tool.
    Prepare(String),
    /// The fixture does not fit the tool's documents (a `notRun` command the tool does not have): known only after the tool was asked.
    Fixture(String),
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
    /// The probe command with `--json`: asked for the remote state after a refusal.
    probe: Option<Vec<String>>,
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
            probe: fixture.probe.clone().map(|mut words| {
                words.push("--json".to_string());
                words
            }),
        };
        for command in &fixture.prepare {
            let template = runner.template.clone();
            let Some((program, args)) = command.split_first() else { continue };
            if resolves_to(program, &template, &runner.bin) {
                return Err(RunError::Prepare(format!(
                    "the prepare command `{}` runs the binary under test: the state the tool keeps is built by `setup`, through the tool's own argv",
                    command.join(" ")
                )));
            }
            let mut call = Call::new(args.iter().cloned());
            call.program = Some(program.clone());
            let outcome = runner
                .execute(&call, &template)
                .map_err(|error| RunError::Prepare(format!("the prepare command `{}` cannot be started: {error}", command.join(" "))))?;
            if outcome.exit != Exit::Code(0) {
                let detail = match outcome.exit {
                    Exit::Code(code) => format!("exited {code}"),
                    Exit::Signal(signal) => format!("was killed by signal {signal}"),
                    Exit::TimedOut => format!("ran over the {} second limit", runner.timeout.as_secs_f64()),
                };
                return Err(RunError::Prepare(format!(
                    "the prepare command `{}` {detail}: {}",
                    command.join(" "),
                    tail(&outcome.stderr)
                )));
            }
        }
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

    /// The data the probe reports in a copy nobody has touched, when it reports the same twice in two copies (a
    /// probe whose answer depends on the time or the path cannot show a remote change); `None` without a probe.
    pub fn remote_baseline(&mut self) -> Result<Option<serde_json::Value>, RunError> {
        let Some(probe) = self.probe.clone() else {
            return Ok(None);
        };
        let mut states = Vec::new();
        for _ in 0..2 {
            let outcome = self.run_plain(&Call::new(probe.iter().cloned()))?;
            let data = serde_json::from_slice::<serde_json::Value>(&outcome.stdout)
                .ok()
                .filter(|document| document.get("ok") == Some(&serde_json::Value::Bool(true)))
                .and_then(|document| document.get("data").cloned());
            states.push(data);
        }
        Ok(match (states.pop().flatten(), states.pop().flatten()) {
            (Some(second), Some(first)) if first == second => Some(first),
            _ => None,
        })
    }

    /// The directory of sequence `number`.
    fn sequence_dir(&self, number: usize) -> PathBuf {
        self.scratch.path().join(format!("sequence-{number}"))
    }

    /// A fresh copy of the template as the next sequence's directory, or an empty one for a call in the empty state.
    fn sequence(&mut self, empty: bool) -> Result<(usize, PathBuf), RunError> {
        self.sequences += 1;
        let number = self.sequences;
        let sequence = self.sequence_dir(number);
        for part in ["work", "home"] {
            if empty {
                std::fs::create_dir_all(sequence.join(part))
                    .map_err(|error| RunError::Scratch(error.to_string()))?;
            } else {
                copy_tree(&self.template.join(part), &sequence.join(part))
                    .map_err(|error| RunError::Scratch(error.to_string()))?;
            }
        }
        Ok((number, sequence))
    }

    fn run_plain(&mut self, call: &Call) -> Result<Outcome, RunError> {
        let (_, sequence) = self.sequence(false)?;
        self.execute(call, &sequence)
            .map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))
    }

    /// What a call left behind: the changed paths of the directory and `HOME` since `before` (measured for a refusal and for a call that asks), and, after a refusal, the probe's answer.
    fn after_call(&self, outcome: &mut Outcome, sequence: &Path, before: &Snapshot) -> Result<(), RunError> {
        if outcome.exit != Exit::Code(2) && !outcome.call.measure {
            return Ok(());
        }
        let after = Self::both(sequence).map_err(|error| RunError::Scratch(error.to_string()))?;
        outcome.changes = changed(before, &after);
        if outcome.exit == Exit::Code(2) {
            if let Some(probe) = &self.probe {
                outcome.probe_after = self
                    .execute(&Call::new(probe.iter().cloned()), sequence)
                    .ok()
                    .map(|probed| probed.stdout);
            }
        }
        Ok(())
    }

    /// Runs `call` once in its own fresh copy of the template (an empty one for a call in the empty state).
    pub fn run(&mut self, call: &Call) -> Result<Outcome, RunError> {
        let (number, sequence) = self.sequence(call.empty || call.without_setup)?;
        let mut outcome = (if call.served { self.execute_served(call, &sequence) } else { self.execute(call, &sequence) })
            .map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))?;
        outcome.sequence = number;
        let before = if call.empty || call.without_setup { Snapshot::new() } else { self.template_files.clone() };
        self.after_call(&mut outcome, &sequence, &before)?;
        Ok(outcome)
    }

    /// Runs `call` in the directory of sequence `number`, as it is after the calls that ran there before: a follow-up that depends on
    /// the state an earlier call left (a page and the next page it points to, a `next` entry and the answer that carried it).
    /// What it changed is measured against the state it found.
    pub fn run_after(&mut self, number: usize, call: &Call) -> Result<Outcome, RunError> {
        let sequence = self.sequence_dir(number);
        let before = Self::both(&sequence).map_err(|error| RunError::Scratch(error.to_string()))?;
        let mut outcome = self
            .execute(call, &sequence)
            .map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))?;
        outcome.sequence = number;
        self.after_call(&mut outcome, &sequence, &before)?;
        Ok(outcome)
    }

    /// Runs `calls` all at once in one fresh copy of the template: requests that meet at the same state. The number of the
    /// sequence they ran in and one outcome each, in order; none is measured.
    pub fn run_concurrently(&mut self, calls: &[Call]) -> Result<(usize, Vec<Outcome>), RunError> {
        let (number, sequence) = self.sequence(false)?;
        let runner = &*self;
        let results: Vec<std::io::Result<Outcome>> = thread::scope(|scope| {
            let handles: Vec<_> = calls.iter().map(|call| scope.spawn(|| runner.execute(call, &sequence))).collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|_| Err(std::io::Error::other("an invocation thread panicked"))))
                .collect()
        });
        let mut outcomes = Vec::with_capacity(results.len());
        for result in results {
            let mut outcome = result.map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))?;
            outcome.sequence = number;
            outcomes.push(outcome);
        }
        Ok((number, outcomes))
    }

    /// The state of sequence `number` as text: two sequences with the same files, sizes and contents give the same text.
    pub fn fingerprint(&self, number: usize) -> Result<String, RunError> {
        let state = Self::both(&self.sequence_dir(number)).map_err(|error| RunError::Scratch(error.to_string()))?;
        Ok(format!("{state:?}"))
    }

    /// Runs `call` with stdin and stdout on one terminal (canonical input, no echo, output untranslated), in its
    /// own fresh copy. A process still running after `deadline` gets the end-of-file character at the start of a
    /// line and two more seconds; [`Outcome::at_deadline`] says what it did.
    pub fn run_on_terminal(&mut self, call: &Call, deadline: Duration) -> Result<Outcome, RunError> {
        let (number, sequence) = self.sequence(false)?;
        let mut outcome = self
            .execute_terminal(call, &sequence, deadline)
            .map_err(|error| RunError::Spawn(format!("{}: {error}", self.bin.display())))?;
        outcome.sequence = number;
        let before = self.template_files.clone();
        self.after_call(&mut outcome, &sequence, &before)?;
        Ok(outcome)
    }

    fn command(&self, call: &Call, root: &Path) -> Command {
        let mut command = match &call.program {
            Some(program) => Command::new(program),
            None => Command::new(&self.bin),
        };
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
            .envs(
                self.fixture_env
                    .iter()
                    .filter(|_| !call.empty)
                    .map(|(name, value)| (name, value)),
            )
            .envs(call.env.iter().map(|(name, value)| (name, value)))
            .stdin(if call.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stderr(Stdio::piped())
            .process_group(0);
        command
    }

    fn execute(&self, call: &Call, root: &Path) -> std::io::Result<Outcome> {
        let started = Instant::now();
        let mut command = self.command(call, root);
        // A terminal on stdout: the slave end goes to the child and a copy stays here until the output has been read.
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let stdout_terminal = if call.pty {
            let pty = raw_terminal()?;
            let kept = pty.slave.try_clone()?;
            command.stdout(Stdio::from(pty.slave));
            Some((std::fs::File::from(pty.master), kept))
        } else {
            command.stdout(Stdio::piped());
            None
        };
        let mut child = command.spawn()?;
        // The document goes in from a thread of its own: a tool that answers before it reads must not block this one.
        if let (Some(bytes), Some(mut pipe)) = (call.stdin.clone(), child.stdin.take()) {
            thread::spawn(move || {
                let _ = std::io::Write::write_all(&mut pipe, &bytes);
            });
        }
        drop(command);
        let ended = Arc::new(AtomicBool::new(false));
        let stdout = match (stdout_terminal, child.stdout.take()) {
            (Some((master, slave)), _) => {
                terminal_reader(master, slave, Arc::clone(&ended), Arc::clone(&buffer))
            }
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
        Ok(Outcome {
            call: call.clone(),
            exit,
            stdout,
            stderr: stderr.join().unwrap_or_default(),
            changes: Vec::new(),
            elapsed: started.elapsed(),
            at_deadline: None,
            probe_after: None,
            sequence: 0,
            served: false,
            ended_alone: false,
        })
    }

    /// The run of a `notRun` command: until the end of its first JSON document, then SIGTERM, and SIGKILL when the process outlives [`SERVED_GRACE`].
    /// A process that ends on its own before or after the document is `ended_alone`; one that prints no document within the time limit is killed and is `TimedOut`.
    fn execute_served(&self, call: &Call, root: &Path) -> std::io::Result<Outcome> {
        let started = Instant::now();
        let mut command = self.command(call, root);
        command.stdout(Stdio::piped());
        let mut child = command.spawn()?;
        drop(command);
        let printed = Arc::new(Mutex::new(Vec::new()));
        let reading = child.stdout.take().map(|mut pipe| {
            let sink = Arc::clone(&printed);
            thread::spawn(move || {
                let mut chunk = [0u8; 8192];
                while let Ok(read) = pipe.read(&mut chunk) {
                    if read == 0 {
                        break;
                    }
                    if let Ok(mut bytes) = sink.lock() {
                        bytes.extend_from_slice(&chunk[..read]);
                    }
                }
            })
        });
        let stderr = child.stderr.take().map_or_else(|| thread::spawn(Vec::new), reader);
        let limit = call.limit.unwrap_or(self.timeout);
        // The end of the first JSON document in what stdout holds so far, with the newline that closes its line when it is there.
        let first_document_end = |bytes: &[u8]| -> Option<usize> {
            let mut documents = serde_json::Deserializer::from_slice(bytes).into_iter::<serde_json::Value>();
            match documents.next() {
                Some(Ok(_)) => Some(documents.byte_offset()),
                _ => None,
            }
        };
        let mut ended_alone = false;
        let mut served = false;
        let mut exit = Exit::TimedOut;
        let mut end: Option<usize> = None;
        loop {
            let seen = printed.lock().map(|bytes| bytes.clone()).unwrap_or_default();
            if let Some(at) = first_document_end(&seen) {
                end = Some(at);
                break;
            }
            if let Some(status) = child.try_wait()? {
                // Ended before a document: whatever it printed up to its end is read, but it is a fixture error all the same.
                exit = exit_of(status);
                ended_alone = true;
                break;
            }
            if started.elapsed() >= limit {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        if end.is_some() {
            // The closing newline of the document's line, then a moment for a process that prints its document and ends.
            thread::sleep(Duration::from_millis(SERVED_SETTLE_MILLIS));
            if let Some(status) = child.try_wait()? {
                exit = exit_of(status);
                ended_alone = true;
            } else {
                let group = i32::try_from(child.id()).ok().map(Pid::from_raw);
                if let Some(group) = group {
                    // ESRCH is the usual answer for a group that is already gone.
                    let _gone = killpg(group, Signal::SIGTERM);
                }
                let asked = Instant::now();
                let ended = loop {
                    if let Some(status) = child.try_wait()? {
                        break Some(status);
                    }
                    if asked.elapsed() >= SERVED_GRACE {
                        break None;
                    }
                    thread::sleep(Duration::from_millis(5));
                };
                exit = match ended {
                    Some(status) => exit_of(status),
                    None => {
                        kill_group(&child);
                        child.kill().unwrap_or_default();
                        exit_of(child.wait()?)
                    }
                };
                served = true;
            }
        } else if !ended_alone {
            kill_group(&child);
            child.kill().unwrap_or_default();
            child.wait()?;
        }
        // Whatever the tool left running in its group must not hold the output open.
        kill_group(&child);
        if let Some(reading) = reading {
            let _joined = reading.join();
        }
        let mut stdout = printed.lock().map(|bytes| bytes.clone()).unwrap_or_default();
        if let Some(at) = first_document_end(&stdout) {
            // Up to the end of the first document and the newline that closes its line; what a process printed after it while it was being ended is not read.
            let line_end = if stdout.get(at) == Some(&b'\n') { at + 1 } else { at };
            if served {
                stdout.truncate(line_end);
            }
        }
        Ok(Outcome {
            call: call.clone(),
            exit,
            stdout,
            stderr: stderr.join().unwrap_or_default(),
            changes: Vec::new(),
            elapsed: started.elapsed(),
            at_deadline: None,
            probe_after: None,
            sequence: 0,
            served,
            ended_alone,
        })
    }

    /// The run of [`Runner::run_on_terminal`].
    fn execute_terminal(&self, call: &Call, root: &Path, deadline: Duration) -> std::io::Result<Outcome> {
        let started = Instant::now();
        let mut command = self.command(call, root);
        let pty = canonical_terminal()?;
        let end_of_file = pty.end_of_file;
        let kept = pty.slave.try_clone()?;
        command.stdin(Stdio::from(pty.slave.try_clone()?));
        command.stdout(Stdio::from(pty.slave));
        let master = std::fs::File::from(pty.master);
        let mut writer = master.try_clone()?;
        let mut child = command.spawn()?;
        drop(command);
        let ended = Arc::new(AtomicBool::new(false));
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let stdout = terminal_reader(master, kept, Arc::clone(&ended), Arc::clone(&buffer));
        let stderr = child
            .stderr
            .take()
            .map_or_else(|| thread::spawn(Vec::new), reader);
        let printed = || buffer.lock().map_or(0, |bytes| bytes.len());
        let mut at_deadline = None;
        let exit = loop {
            if let Some(status) = child.try_wait()? {
                break exit_of(status);
            }
            if started.elapsed() >= deadline {
                let output = buffer.lock().map(|bytes| bytes.clone()).unwrap_or_default();
                std::io::Write::write_all(&mut writer, &[end_of_file])?;
                let grace = Instant::now();
                let mut reacted = false;
                let mut ended_by = None;
                while grace.elapsed() < Duration::from_secs(2) {
                    if let Some(status) = child.try_wait()? {
                        reacted = true;
                        ended_by = Some(exit_of(status));
                        break;
                    }
                    if printed() > output.len() {
                        reacted = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                at_deadline = Some(AtDeadline {
                    output,
                    reacted_to_eof: reacted,
                });
                if let Some(exit) = ended_by {
                    break exit;
                }
                kill_group(&child);
                child.kill().unwrap_or_default();
                child.wait()?;
                break Exit::TimedOut;
            }
            thread::sleep(Duration::from_millis(5));
        };
        kill_group(&child);
        ended.store(true, Ordering::Release);
        stdout.join().unwrap_or_default();
        let output = buffer.lock().map(|bytes| bytes.clone()).unwrap_or_default();
        Ok(Outcome {
            call: call.clone(),
            exit,
            stdout: output,
            stderr: stderr.join().unwrap_or_default(),
            changes: Vec::new(),
            elapsed: started.elapsed(),
            at_deadline,
            probe_after: None,
            sequence: 0,
            served: false,
            ended_alone: false,
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

/// A pseudo-terminal as an interactive session has it: input in canonical mode (`ICANON`) with echo off, so that the
/// end-of-file character written at the start of a line makes a reader's `read` return 0; output untranslated
/// (`OPOST` off, so a `\n` the tool writes arrives as `\n`, not `\r\n`).
struct CanonicalTerminal {
    master: std::os::fd::OwnedFd,
    slave: std::os::fd::OwnedFd,
    end_of_file: u8,
}

fn canonical_terminal() -> std::io::Result<CanonicalTerminal> {
    use nix::sys::termios::{tcgetattr, tcsetattr, InputFlags, LocalFlags, OutputFlags, SetArg, SpecialCharacterIndices};
    let pty = nix::pty::openpty(None, None).map_err(std::io::Error::from)?;
    let mut settings = tcgetattr(&pty.slave).map_err(std::io::Error::from)?;
    settings.local_flags.insert(LocalFlags::ICANON);
    settings
        .local_flags
        .remove(LocalFlags::ECHO | LocalFlags::ECHOE | LocalFlags::ECHOK | LocalFlags::ECHONL);
    settings.output_flags.remove(OutputFlags::OPOST);
    settings.input_flags.remove(InputFlags::ICRNL | InputFlags::INLCR | InputFlags::IGNCR);
    tcsetattr(&pty.slave, SetArg::TCSANOW, &settings).map_err(std::io::Error::from)?;
    let end_of_file = settings.control_chars[SpecialCharacterIndices::VEOF as usize];
    Ok(CanonicalTerminal {
        master: pty.master,
        slave: pty.slave,
        end_of_file,
    })
}

/// Reads a terminal's master end to its end of file, into `buffer` as it goes.
///
/// While the process runs, and until nothing more is waiting after it ended (`ended`), `slave` stays open here:
/// closing the last slave end before the master has been read can throw unread output away, so the master never
/// reports a hang-up early. Then `slave` is closed and the read goes on to EOF (the hang-up error), so nothing the
/// process wrote is cut.
fn terminal_reader(
    mut master: std::fs::File,
    slave: std::os::fd::OwnedFd,
    ended: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<u8>>>,
) -> JoinHandle<Vec<u8>> {
    let push = move |buffer: &Arc<Mutex<Vec<u8>>>, bytes: &[u8]| {
        if let Ok(mut all) = buffer.lock() {
            all.extend_from_slice(bytes);
        }
    };
    thread::spawn(move || {
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
                    Ok(read) if read > 0 => push(&buffer, &chunk[..read]),
                    _ => break,
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
            push(&buffer, &chunk[..read]);
        }
        buffer.lock().map(|all| all.clone()).unwrap_or_default()
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
/// Whether `program` (a path, or a name looked up on `PATH`) is the same file as `bin`.
fn resolves_to(program: &str, directory: &Path, bin: &Path) -> bool {
    let Ok(bin) = bin.canonicalize() else { return false };
    let candidates: Vec<PathBuf> = if program.contains('/') {
        vec![directory.join(program)]
    } else {
        std::env::var_os("PATH").map(|path| std::env::split_paths(&path).map(|dir| dir.join(program)).collect()).unwrap_or_default()
    };
    candidates.iter().any(|candidate| candidate.canonicalize().is_ok_and(|resolved| resolved == bin))
}

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
            ..Fixture::empty()
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
        assert_eq!(noisy.changes, ["./keep", "~/.cache/log"]);
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

    #[test]
    fn atip_checkl1_prepare_programs_run_before_setup_with_the_fixture_environment_and_are_not_a_change_a_refusal_made() {
        let mut prepared = fixture(&[&["-c", "cat prepared.txt >&2; echo \"$FIXTURE_VAR\" >&2; exit 2"]], Duration::from_secs(10));
        prepared.prepare = vec![vec!["/usr/bin/env".to_string(), "sh".to_string(), "-c".to_string(), "echo prepared > prepared.txt".to_string()]];
        let runner = Runner::new("/bin/sh".as_ref(), &prepared);
        let error = runner.expect_err("the setup command exits 2");
        let super::RunError::Setup(reason) = error else { panic!("a setup error") };
        assert!(reason.contains("prepared") && reason.contains("yes"), "the program ran first, with the fixture's variable: {reason}");
        // A refusal that changes nothing is not charged with the file the prepare command made.
        let mut quiet = fixture(&[], Duration::from_secs(10));
        quiet.prepare = vec![vec!["/usr/bin/env".to_string(), "sh".to_string(), "-c".to_string(), "echo prepared > prepared.txt".to_string()]];
        let mut runner = Runner::new("/bin/sh".as_ref(), &quiet).expect("runner");
        let outcome = runner.run(&sh("test -f prepared.txt; exit 2")).expect("run");
        assert_eq!(outcome.exit, Exit::Code(2));
        assert!(outcome.changes.is_empty(), "{:?}", outcome.changes);
    }

    #[test]
    fn atip_checkl1_a_failing_prepare_command_is_a_defect_of_the_fixture_named_by_its_command() {
        let mut failing = fixture(&[], Duration::from_secs(10));
        failing.prepare = vec![vec!["/usr/bin/env".to_string(), "sh".to_string(), "-c".to_string(), "echo no git >&2; exit 4".to_string()]];
        let error = Runner::new("/bin/sh".as_ref(), &failing).expect_err("prepare fails");
        let super::RunError::Prepare(reason) = error else { panic!("a prepare error") };
        assert!(reason.contains("exited 4") && reason.contains("no git"), "{reason}");
        let mut missing = fixture(&[], Duration::from_secs(10));
        missing.prepare = vec![vec!["/nonexistent/atip-prepare".to_string()]];
        let error = Runner::new("/bin/sh".as_ref(), &missing).expect_err("prepare cannot start");
        assert!(matches!(error, super::RunError::Prepare(reason) if reason.contains("cannot be started")));
    }

    #[test]
    fn atip_checkl1_a_prepare_step_that_runs_the_binary_under_test_is_a_fixture_error() {
        let mut fixture = fixture(&[], Duration::from_secs(10));
        fixture.prepare = vec![vec!["/bin/sh".to_string(), "-c".to_string(), "true".to_string()]];
        let error = Runner::new("/bin/sh".as_ref(), &fixture).expect_err("the prepare step is the binary");
        assert!(matches!(error, super::RunError::Prepare(reason) if reason.contains("runs the binary under test")));
        fixture.prepare = vec![vec!["sh".to_string(), "-c".to_string(), "true".to_string()]];
        let error = Runner::new("/bin/sh".as_ref(), &fixture).expect_err("found on PATH");
        assert!(matches!(error, super::RunError::Prepare(reason) if reason.contains("runs the binary under test")));
    }
}
