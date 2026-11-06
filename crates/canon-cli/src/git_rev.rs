//! Git revisions and the code tree (0.28 D7b).
//!
//! [`resolve_rev`] turns any git revision into a full sha. [`code_tree`]
//! names "the code" of a working tree or a commit as a git tree id that
//! ignores everything that is not code, so a record can say which code it
//! saw and a later commit with the same code compares equal.
//!
//! # `code.exclude` in `canon.yaml`
//! ```yaml
//! code:
//!   exclude: ["docs/**", "*.md"]   # the default when the key is absent
//! ```
//! Globs are matched against repo-relative paths with `/` separators:
//! - `*` matches any run of characters except `/`; `?` one character except `/`;
//!   `**` matches any run of characters including `/`; every other character is literal.
//! - A pattern without `/` matches the file name at any depth (`*.md` matches
//!   `README.md` and `a/b/c.md`).
//! - A pattern with `/` is anchored at the repo root (a leading `/` is allowed);
//!   `docs/**` matches everything below `docs/`, and a pattern that names a
//!   directory (`docs`) also excludes everything below it.
//!
//! `.canon/` and every configured `specs.roots[]` directory inside the repo
//! are always excluded, whatever `code.exclude` says.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cli_error::{CliError, CliErrorExt as _};

/// The code of a working tree or a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeTree {
    /// The git tree id of the code files.
    pub tree: String,
    /// The working tree's code differs from `HEAD`'s (always false for a named rev).
    pub dirty: bool,
}

const DEFAULT_EXCLUDE: [&str; 2] = ["docs/**", "*.md"];
const NO_COMMIT_HINT: &str = "run the command inside a git work tree that has at least one commit (`git init` and `git commit`)";

/// The `COMMIT_REQUIRED` refusal with its hint.
pub fn no_commit(message: impl Into<String>) -> CliError {
    CliError::refused("COMMIT_REQUIRED", message).hint(NO_COMMIT_HINT)
}

/// Where a computation of the code tree keeps what it writes: a temporary index and a temporary object
/// directory. Git writes the blobs of `add -A` and the trees of `write-tree` into the object directory; the
/// repository's own objects (and its own alternates) are read through `GIT_ALTERNATE_OBJECT_DIRECTORIES`. So the
/// run reads the repository and writes nothing in it, `.git` included: a refusal changes nothing (atip/1 8.2),
/// and a read command (`gate check`) does not grow `.git/objects`. A code tree is only ever compared as an id,
/// never read back as an object, so its objects need not outlive the run.
#[derive(Debug)]
struct Scratch {
    index: PathBuf,
    objects: PathBuf,
    /// The repository's object directory and the ones its `objects/info/alternates` lists, `:`-joined.
    alternates: std::ffi::OsString,
}

impl Scratch {
    fn new(repo: &Path, dir: &Path) -> Result<Self, String> {
        let real = text(git(repo, None, &["rev-parse", "--path-format=absolute", "--git-path", "objects"], None)?);
        let real = PathBuf::from(real);
        let mut dirs = vec![real.clone()];
        if let Ok(listed) = std::fs::read_to_string(real.join("info/alternates")) {
            dirs.extend(listed.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| real.join(l)));
        }
        let alternates = std::env::join_paths(dirs).map_err(|e| e.to_string())?;
        let objects = dir.join("objects");
        std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
        Ok(Self { index: dir.join("index"), objects, alternates })
    }
}

fn git(repo: &Path, scratch: Option<&Scratch>, args: &[&str], stdin: Option<&[u8]>) -> Result<std::process::Output, String> {
    use std::io::Write;
    let mut cmd = Command::new("git");
    cmd.current_dir(repo).args(args).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(scratch) = scratch {
        cmd.env("GIT_INDEX_FILE", &scratch.index).env("GIT_OBJECT_DIRECTORY", &scratch.objects).env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &scratch.alternates);
    }
    let mut child = cmd.spawn().map_err(|e| format!("cannot run git: {e}"))?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(bytes).map_err(|e| format!("git {}: {e}", args[0]))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("git {}: {e}", args[0]))?;
    if out.status.success() {
        Ok(out)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn text(out: std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The full sha of `rev` (`git rev-parse --verify <rev>^{commit}`, run in
/// `repo`). A directory outside a git work tree, a repo with no commits and a
/// revision git does not know are all `COMMIT_REQUIRED`.
pub fn resolve_rev(repo: &Path, rev: &str) -> Result<String, CliError> {
    let spec = format!("{rev}^{{commit}}");
    git(repo, None, &["rev-parse", "--verify", "--quiet", &spec], None).map(text).map_err(|why| no_commit(format!("`{rev}` names no commit in this directory{}", if why.is_empty() { String::new() } else { format!(": {why}") })))
}

/// The revision every commit option defaults to.
pub const DEFAULT_REV: &str = "HEAD";

/// What a record that is "about a commit" stores: the full sha of the
/// revision and the code it saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The full sha of the revision.
    pub sha: canon_model::Sha,
    /// The git tree id of the code the command saw.
    pub code_tree: String,
    /// The working tree's code differs from `HEAD`'s.
    pub worktree_dirty: bool,
}

/// Resolve a commit option (`--rev`, `--resolution-rev`, `--introduced-rev`).
///
/// The default [`DEFAULT_REV`] stands for the working tree as it is now:
/// `codeTree` is the working tree's, and `worktreeDirty` says whether it
/// differs from `HEAD`. Any other value names a commit, whose own code tree is
/// recorded (`worktreeDirty` false). Uncommitted code is recorded, never
/// refused; outside a git work tree, in a repo with no commit, or for a
/// revision git does not know the answer is `COMMIT_REQUIRED`.
pub fn bind(repo: &Path, rev: &str) -> Result<Bound, CliError> {
    let sha = resolve_rev(repo, rev)?;
    let sha = canon_model::Sha::parse(&sha).map_err(|e| no_commit(format!("`{rev}` resolves to `{sha}`, which is not a 40-digit sha: {e}")))?;
    let CodeTree { tree, dirty } = code_tree(repo, (rev != DEFAULT_REV).then_some(rev))?;
    Ok(Bound { sha, code_tree: tree, worktree_dirty: dirty })
}

/// The code tree of the working tree as it is now, for a record that carries
/// one only when there is one (an approval, 0.37): `None` outside a git work
/// tree and in one with no commit, where [`bind`] refuses with
/// `COMMIT_REQUIRED`; any other failure (a malformed `canon.yaml`) stays an error.
pub fn code_tree_if_committed(repo: &Path) -> Result<Option<String>, CliError> {
    if resolve_rev(repo, DEFAULT_REV).is_err() {
        return Ok(None);
    }
    match code_tree(repo, None) {
        Ok(code) => Ok(Some(code.tree)),
        Err(e) if e.code == "COMMIT_REQUIRED" => Ok(None),
        Err(e) => Err(e),
    }
}

/// The `code.exclude` globs of `<repo>/canon.yaml` ([`DEFAULT_EXCLUDE`] when
/// the file or the key is absent). A malformed file or glob is `CONFIG_INVALID`.
pub fn code_exclude(repo: &Path) -> Result<Vec<String>, CliError> {
    let bad = |why: String| crate::read_cause::config_error(why);
    let defaults = || DEFAULT_EXCLUDE.iter().map(std::string::ToString::to_string).collect();
    let Ok(raw) = std::fs::read_to_string(repo.join("canon.yaml")) else { return Ok(defaults()) };
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).map_err(|e| bad(format!("parsing `canon.yaml`: {e}")))?;
    let Some(code) = doc.get("code") else { return Ok(defaults()) };
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawCode {
        exclude: Option<Vec<String>>,
    }
    let parsed: RawCode = serde_yaml::from_value(code.clone()).map_err(|e| bad(format!("canon.yaml `code:` section is malformed: {e}")))?;
    let globs = parsed.exclude.unwrap_or_else(defaults);
    if let Some(empty) = globs.iter().find(|g| g.trim_matches('/').is_empty()) {
        return Err(bad(format!("canon.yaml `code.exclude` has an empty glob `{empty}`")));
    }
    Ok(globs)
}

/// `glob` against a repo-relative path (see the module docs for the semantics).
pub fn glob_matches(glob: &str, path: &str) -> bool {
    let anchored = glob.contains('/');
    let glob = glob.trim_start_matches('/');
    if anchored {
        // A directory name also covers everything below it.
        path.match_indices('/').any(|(i, _)| wild(glob.as_bytes(), &path.as_bytes()[..i])) || wild(glob.as_bytes(), path.as_bytes())
    } else {
        // Any path component: the file's own name, or a directory of that name above it.
        path.split('/').any(|name| wild(glob.as_bytes(), name.as_bytes()))
    }
}

fn wild(glob: &[u8], text: &[u8]) -> bool {
    match glob.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) if rest.first() == Some(&b'*') => {
            let rest = &rest[rest.iter().take_while(|&&c| c == b'*').count()..];
            (0..=text.len()).any(|i| wild(rest, &text[i..]))
        }
        Some((b'*', rest)) => (0..=text.iter().position(|&c| c == b'/').unwrap_or(text.len())).any(|i| wild(rest, &text[i..])),
        Some((b'?', rest)) => text.first().is_some_and(|&c| c != b'/') && wild(rest, &text[1..]),
        Some((c, rest)) => text.first() == Some(c) && wild(rest, &text[1..]),
    }
}

/// A temporary index file, removed when dropped (also on error paths).
#[derive(Debug)]
struct TempIndex(PathBuf);

impl TempIndex {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("canon-index-{}-{nanos}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        Self(dir)
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Repo-relative directories (no trailing slash) of every configured spec root inside the repo.
fn spec_root_prefixes(repo: &Path) -> Result<Vec<String>, CliError> {
    let roots = crate::inventory::load_spec_roots(&repo.join("canon.yaml"))
        .map_err(|e| crate::read_cause::config_error(e.to_string()))?;
    Ok(roots
        .iter()
        .filter_map(|r| r.root.strip_prefix(repo).ok())
        .filter(|rel| rel.components().all(|c| matches!(c, std::path::Component::Normal(_))))
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
        .collect())
}

/// Write the tree of the temporary index after dropping everything that is not code.
/// Returns the tree id and whether any code file is left.
fn filtered_tree(repo: &Path, index: &Scratch, exclude: &[String], roots: &[String]) -> Result<(String, bool), String> {
    let listing = git(repo, Some(index), &["ls-files", "-z"], None)?.stdout;
    let mut drop = Vec::new();
    let mut kept = false;
    for path in listing.split(|&b| b == 0).filter(|p| !p.is_empty()) {
        let name = String::from_utf8_lossy(path);
        let under = |dir: &str| name.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'));
        if under(".canon") || roots.iter().any(|r| under(r)) || exclude.iter().any(|g| glob_matches(g, &name)) {
            drop.extend_from_slice(path);
            drop.push(0);
        } else {
            kept = true;
        }
    }
    if !drop.is_empty() {
        git(repo, Some(index), &["update-index", "--force-remove", "-z", "--stdin"], Some(&drop))?;
    }
    Ok((text(git(repo, Some(index), &["write-tree"], None)?), kept))
}

/// The code tree of the working tree (`rev` is `None`) or of a commit.
///
/// Files are read into a temporary index: for a commit `git read-tree <sha>`;
/// for the working tree `HEAD`'s tree (when there is a commit) followed by
/// `git add -A`, so tracked and untracked files count, gitignored files do not
/// and a deleted file is gone. `.canon/`, every configured spec root and the
/// `code.exclude` globs ([`code_exclude`]) are then removed and `git write-tree`
/// names the rest. Never `<rev>^{tree}`. `dirty` is whether the working tree's
/// code tree differs from `HEAD`'s (true when there is no commit yet and the
/// working tree holds code); it is false for a named rev. A directory that is
/// not a git work tree is `COMMIT_REQUIRED`.
pub fn code_tree(repo: &Path, rev: Option<&str>) -> Result<CodeTree, CliError> {
    let exclude = code_exclude(repo)?;
    let roots = spec_root_prefixes(repo)?;
    let fail = |why: String| no_commit(format!("git could not compute the code tree: {why}"));
    #[expect(clippy::map_err_ignore, reason = "`no_commit` reports one fixed message; git's own stderr for a non-work-tree is the failure being named")]
    git(repo, None, &["rev-parse", "--is-inside-work-tree"], None).map_err(|_| no_commit("this directory is not a git work tree"))?;
    let head = resolve_rev(repo, "HEAD").ok();
    let tmp = TempIndex::new();
    std::fs::create_dir_all(&tmp.0).map_err(|e| fail(e.to_string()))?;
    let index = Scratch::new(repo, &tmp.0).map_err(fail)?;
    let build = |source: Option<&str>| -> Result<(String, bool), String> {
        let _ = std::fs::remove_file(&index.index);
        match source {
            Some(sha) => {
                git(repo, Some(&index), &["read-tree", sha], None)?;
            }
            None => {
                if let Some(sha) = &head {
                    git(repo, Some(&index), &["read-tree", sha], None)?;
                }
                git(repo, Some(&index), &["add", "-A", "--", "."], None)?;
            }
        }
        filtered_tree(repo, &index, &exclude, &roots)
    };
    match rev {
        Some(rev) => {
            let sha = resolve_rev(repo, rev)?;
            let (tree, _) = build(Some(&sha)).map_err(fail)?;
            Ok(CodeTree { tree, dirty: false })
        }
        None => {
            let (tree, any) = build(None).map_err(fail)?;
            let dirty = match &head {
                Some(sha) => build(Some(sha)).map_err(fail)?.0 != tree,
                None => any,
            };
            Ok(CodeTree { tree, dirty })
        }
    }
}

/// The code of the working tree as it is now, held in a scratch object directory so that the code tree
/// of a commit can be compared with it (0.38): the paths that differ, and the files there are. A code tree is
/// never an object of the repository (it is the commit's tree with `.canon/`, the spec roots and the
/// `code.exclude` globs removed), so the old tree is recomputed from the commit an evidence record names and
/// trusted only when it comes out as the id the record carries.
#[derive(Debug)]
pub struct TreeDiffer {
    repo: PathBuf,
    exclude: Vec<String>,
    roots: Vec<String>,
    _tmp: TempIndex,
    index: Scratch,
    now: String,
}

impl TreeDiffer {
    /// The working tree's code, as [`code_tree`] reads it. Outside a git work tree the answer is `COMMIT_REQUIRED`.
    pub fn new(repo: &Path) -> Result<Self, CliError> {
        let exclude = code_exclude(repo)?;
        let roots = spec_root_prefixes(repo)?;
        let fail = |why: String| no_commit(format!("git could not compute the code tree: {why}"));
        #[expect(clippy::map_err_ignore, reason = "`no_commit` reports one fixed message; git's own stderr for a non-work-tree is the failure being named")]
        git(repo, None, &["rev-parse", "--is-inside-work-tree"], None).map_err(|_| no_commit("this directory is not a git work tree"))?;
        let head = resolve_rev(repo, "HEAD").ok();
        let tmp = TempIndex::new();
        std::fs::create_dir_all(&tmp.0).map_err(|e| fail(e.to_string()))?;
        let index = Scratch::new(repo, &tmp.0).map_err(fail)?;
        let now = (|| -> Result<String, String> {
            if let Some(sha) = &head {
                git(repo, Some(&index), &["read-tree", sha], None)?;
            }
            git(repo, Some(&index), &["add", "-A", "--", "."], None)?;
            Ok(filtered_tree(repo, &index, &exclude, &roots)?.0)
        })()
        .map_err(fail)?;
        Ok(Self { repo: repo.to_path_buf(), exclude, roots, _tmp: tmp, index, now })
    }

    /// The tree id of the working tree's code.
    pub fn tree(&self) -> &str {
        &self.now
    }

    /// Every code file of the working tree, repo-relative with `/` separators.
    pub fn files(&self) -> Option<Vec<String>> {
        let listing = git(&self.repo, Some(&self.index), &["ls-tree", "-r", "-z", "--name-only", &self.now], None).ok()?.stdout;
        Some(listing.split(|&b| b == 0).filter(|p| !p.is_empty()).map(|p| String::from_utf8_lossy(p).into_owned()).collect())
    }

    /// The paths that differ between the code tree of commit `sha` and the working tree's: added, modified and
    /// deleted files, a renamed file as its old and its new path. `None` when the old tree cannot be had: the
    /// commit is gone, or its code tree is not `recorded` (the record was made over uncommitted code, or
    /// `code.exclude` changed since).
    pub fn changed_since(&self, sha: &str, recorded: &str) -> Option<Vec<String>> {
        let _ = std::fs::remove_file(&self.index.index);
        git(&self.repo, Some(&self.index), &["read-tree", sha], None).ok()?;
        let (old, _) = filtered_tree(&self.repo, &self.index, &self.exclude, &self.roots).ok()?;
        if old != recorded {
            return None;
        }
        if old == self.now {
            return Some(Vec::new());
        }
        let listing = git(&self.repo, Some(&self.index), &["diff-tree", "-r", "-z", "--no-renames", "--name-only", &old, &self.now], None).ok()?.stdout;
        let mut paths: Vec<String> = listing.split(|&b| b == 0).filter(|p| !p.is_empty()).map(|p| String::from_utf8_lossy(p).into_owned()).collect();
        paths.sort();
        paths.dedup();
        Some(paths)
    }
}
