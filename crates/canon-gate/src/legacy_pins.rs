//! What a legacy review's pinned commit says about its scenario (0.16 D6).
//!
//! A review written before 0.15 carries no `pins`, but it carries `pin`,
//! the commit it reviewed. That commit holds the scenario as the reviewer
//! saw it, so its per-scenario digest — the exact normalization `canon
//! inventory sync` stores as `Scenario.scenario_digest`
//! ([`canon_fmt::gherkin::scenario_digests`]) applied to the file's text
//! at that commit — tells whether the scenario has changed since. This
//! module reads those digests out of git; [`crate::review_gate`] makes the
//! decision (current, stale, or undecidable) from them, the same way for
//! `canon gate check`, `canon subject status` and `canon status`.
//!
//! # Where the git read happens
//! [`crate::GateContext::load`] stays free of process I/O (as it stays
//! free of the wall clock): it leaves [`crate::GateContext::legacy_pins`]
//! empty. The CLI calls [`resolve`] once per run, after loading, and
//! stores the result in the context. `resolve` needs to know which
//! `.feature` file holds each scenario now and where each spec root lives;
//! only the CLI parses `canon.yaml`'s `specs.roots`, so it passes them in
//! as [`SpecSources`]. The `sources` closure runs only when there is a
//! legacy review to check, so a repo without one never scans its spec
//! corpus or starts git. A context nobody resolved reads every legacy
//! review as undecidable, which is the 0.15 behavior.
//!
//! # How git is read
//! One `git cat-file --batch` process serves every read of a run. Each
//! pin is checked once (`<pin>^{commit}`), each `(pin, file)` is read and
//! digested once, and each `(pin, spec root directory)` is listed once
//! (`git ls-tree`) when a scenario is not in its current file at the pin
//! and its old file has to be found: the root's directory now, then each
//! directory `canon migrate` moved the root from (0.17 D6), so a review
//! pinned before the move still finds its scenario.
//! `GIT_NO_LAZY_FETCH=1` keeps a partial clone
//! from fetching a missing object: a read never touches the network.
//!
//! # Undecidable
//! A pin that is not a hex object id, is not a commit in this clone
//! (shallow clone, rewritten history, not a git checkout at all), or a
//! scenario id that no `.feature` file of its root holds at that commit,
//! is undecidable. An undecidable review keeps counting, and the advisory
//! names how many there are; a missing commit never becomes a violation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use canon_model::{ProjectId, Review, ScenarioId, SpecDigest};

use crate::context::GateContext;
use crate::review_gate::in_effect_reviews;
use crate::spec_coverage::latest_scenarios;

/// What a legacy review's pinned commit holds for its scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedScenario {
    /// The scenario's digest at the pinned commit.
    Digest(SpecDigest),
    /// The pin is not a commit this clone has.
    CommitMissing,
    /// The commit is here, but no `.feature` file of the scenario's spec
    /// root holds the scenario id at it.
    ScenarioMissing,
}

/// [`PinnedScenario`] per `(pin, project_id, scenario_id)` of every
/// in-effect legacy review of a live scenario ([`resolve`]). Empty until
/// resolved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyPins {
    at_pin: BTreeMap<(String, ProjectId, ScenarioId), PinnedScenario>,
}

impl LegacyPins {
    /// What `review`'s pin holds for its scenario; `None` when it was not
    /// resolved.
    pub fn get(&self, review: &Review) -> Option<&PinnedScenario> {
        self.at_pin.get(&(review.pin.clone(), review.project_id.clone(), review.scenario_id.clone()))
    }

    /// Record what `pin` holds for `(project_id, scenario_id)`.
    pub fn insert(&mut self, pin: impl Into<String>, project_id: ProjectId, scenario_id: ScenarioId, pinned: PinnedScenario) {
        self.at_pin.insert((pin.into(), project_id, scenario_id), pinned);
    }
}

/// Where the spec corpus lives now, repo-relative: the `.feature` file
/// holding each scenario, and the directories each spec root's history
/// lives under, searched at a pinned commit when the scenario was in
/// another file then: the root's directory now, then every directory
/// `canon migrate` moved it from (`specs.roots[].former_roots`, 0.17 D6).
/// A former directory is the same project's corpus at the commits before
/// the move, so the search never reaches another root's scenarios.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecSources {
    pub files: BTreeMap<(ProjectId, ScenarioId), PathBuf>,
    pub roots: BTreeMap<ProjectId, Vec<PathBuf>>,
}

/// Read the pinned commit of every in-effect review without `pins` whose
/// scenario is live, once per `(pin, file)` (module doc). `sources` is
/// called only when there is such a review.
pub fn resolve(ctx: &GateContext, sources: impl FnOnce() -> SpecSources) -> LegacyPins {
    let live: BTreeSet<(&ProjectId, &ScenarioId)> = latest_scenarios(ctx).into_iter().map(|s| (&s.project_id, &s.scenario_id)).collect();
    let targets: BTreeSet<(&str, &ProjectId, &ScenarioId)> = in_effect_reviews(ctx)
        .into_iter()
        .filter(|r| r.pins.is_none() && live.contains(&(&r.project_id, &r.scenario_id)))
        .map(|r| (r.pin.as_str(), &r.project_id, &r.scenario_id))
        .collect();
    let mut resolved = LegacyPins::default();
    if targets.is_empty() {
        return resolved;
    }
    let sources = sources();
    let mut git = PinReader::new(&ctx.ctx.repo);
    for (pin, project_id, scenario_id) in targets {
        let pinned = git.scenario_at(pin, project_id, scenario_id, &sources);
        resolved.insert(pin, project_id.clone(), scenario_id.clone(), pinned);
    }
    resolved
}

/// A run's cached reads of pinned commits (module doc).
struct PinReader {
    repo: PathBuf,
    batch: Option<CatFile>,
    commits: BTreeMap<String, bool>,
    files: BTreeMap<(String, String), Option<BTreeMap<String, SpecDigest>>>,
    trees: BTreeMap<(String, String), Vec<String>>,
}

impl PinReader {
    fn new(repo: &Path) -> Self {
        Self { repo: repo.to_path_buf(), batch: CatFile::spawn(repo), commits: BTreeMap::new(), files: BTreeMap::new(), trees: BTreeMap::new() }
    }

    fn scenario_at(&mut self, pin: &str, project_id: &ProjectId, scenario_id: &ScenarioId, sources: &SpecSources) -> PinnedScenario {
        if !self.has_commit(pin) {
            return PinnedScenario::CommitMissing;
        }
        let id = scenario_id.as_str();
        let current = sources.files.get(&(project_id.clone(), scenario_id.clone())).and_then(|path| git_path(path));
        if let Some(file) = &current {
            if let Some(digest) = self.digests(pin, file).and_then(|d| d.get(id)).cloned() {
                return PinnedScenario::Digest(digest);
            }
        }
        let Some(dirs) = sources.roots.get(project_id) else {
            return PinnedScenario::ScenarioMissing;
        };
        let dirs: Vec<String> = dirs.iter().filter_map(|dir| git_path(dir).or_else(|| is_repo_root(dir).then(|| ".".to_string()))).collect();
        for dir in dirs {
            for file in self.feature_files(pin, &dir) {
                if Some(&file) == current.as_ref() {
                    continue;
                }
                if let Some(digest) = self.digests(pin, &file).and_then(|d| d.get(id)).cloned() {
                    return PinnedScenario::Digest(digest);
                }
            }
        }
        PinnedScenario::ScenarioMissing
    }

    /// Whether `pin` names a commit this clone has. Only a hex object id
    /// counts: a pin is the commit a review saw, never a ref that moves.
    fn has_commit(&mut self, pin: &str) -> bool {
        if let Some(known) = self.commits.get(pin) {
            return *known;
        }
        let is_oid = (4..=64).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_hexdigit());
        let present = is_oid && self.batch.as_mut().and_then(|batch| batch.read(&format!("{pin}^{{commit}}"))).is_some_and(|(kind, _)| kind == "commit");
        self.commits.insert(pin.to_string(), present);
        present
    }

    /// The per-scenario digests of `file` at `pin`, read once.
    fn digests(&mut self, pin: &str, file: &str) -> Option<&BTreeMap<String, SpecDigest>> {
        let key = (pin.to_string(), file.to_string());
        if !self.files.contains_key(&key) {
            let read = self.batch.as_mut().and_then(|batch| batch.read(&format!("{pin}:./{file}")));
            let digests = read
                .filter(|(kind, _)| kind == "blob")
                .and_then(|(_, bytes)| String::from_utf8(bytes).ok())
                .map(|text| canon_fmt::gherkin::scenario_digests(&text));
            self.files.insert(key.clone(), digests);
        }
        self.files[&key].as_ref()
    }

    /// Every `.feature` file under `root` at `pin`, listed once.
    fn feature_files(&mut self, pin: &str, root: &str) -> Vec<String> {
        let key = (pin.to_string(), root.to_string());
        if let Some(files) = self.trees.get(&key) {
            return files.clone();
        }
        let listed = git_command(&self.repo).args(["ls-tree", "-r", "--name-only", "-z", pin, "--", root]).output().ok().filter(|out| out.status.success());
        let files: Vec<String> = listed
            .map(|out| out.stdout.split(|b| *b == 0).filter_map(|p| std::str::from_utf8(p).ok()).filter(|p| p.ends_with(".feature")).map(str::to_string).collect())
            .unwrap_or_default();
        self.trees.insert(key, files.clone());
        files
    }
}

/// A repo-relative path as git names it (`/`-separated), or `None` for a
/// path that leaves the repo or cannot be named on one batch line.
fn git_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            _ => return None,
        }
    }
    let joined = parts.join("/");
    (!joined.is_empty() && !joined.contains(['\n', '\r'])).then_some(joined)
}

/// Whether `path` names the repo root itself (an empty or `.` path).
fn is_repo_root(path: &Path) -> bool {
    path.components().all(|component| component == Component::CurDir)
}

fn git_command(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(repo).env("GIT_NO_LAZY_FETCH", "1");
    command
}

/// One `git cat-file --batch` process: a request line in, a header and
/// the object's bytes out.
struct CatFile {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl CatFile {
    fn spawn(repo: &Path) -> Option<Self> {
        let mut child = git_command(repo).args(["cat-file", "--batch"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let stdin = child.stdin.take()?;
        let stdout = BufReader::new(child.stdout.take()?);
        Some(Self { child, stdin, stdout })
    }

    /// The object `spec` names, as `(type, bytes)`; `None` when git has
    /// no such object or the process failed.
    fn read(&mut self, spec: &str) -> Option<(String, Vec<u8>)> {
        if spec.contains(['\n', '\r']) {
            return None;
        }
        writeln!(self.stdin, "{spec}").ok()?;
        self.stdin.flush().ok()?;
        let mut header = String::new();
        self.stdout.read_line(&mut header).ok()?;
        let fields: Vec<&str> = header.split_whitespace().collect();
        let [_, kind, size] = fields.as_slice() else { return None };
        let size: usize = size.parse().ok()?;
        let mut bytes = vec![0; size + 1];
        self.stdout.read_exact(&mut bytes).ok()?;
        bytes.truncate(size);
        Some((kind.to_string(), bytes))
    }
}

impl Drop for CatFile {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
