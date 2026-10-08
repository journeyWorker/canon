//! The CLI conformance corpus replay: `conformance/` freezes the contracts
//! external consumers of `canon` rely on — exit codes, gate violation lines,
//! refusal messages, JSON output — and this file is CI's enforcement of it
//! (`conformance/README.md` is the human-facing contract).
//!
//! Each case is `conformance/cases/<name>/`: a `case.yaml` (description,
//! env, ordered steps), an optional `repo/` fixture, and
//! `expected/<n>.out` / `expected/<n>.err` per 1-based step `n`. The runner
//! copies `repo/` into a fresh temp dir, `git init`s it, runs every step's
//! `args` against the actually-built binary (`env!("CARGO_BIN_EXE_canon")`)
//! with a cleared environment plus a fixed base, and compares the exit code
//! and the normalized stdout/stderr byte for byte.
//!
//! Cases are **discovered, never listed**: a new directory with a
//! `case.yaml` is replayed the moment it lands, and discovering zero cases
//! is itself a failure so a moved or renamed corpus cannot pass silently.
//!
//! `CANON_CONFORMANCE_BLESS=1` (via `conformance/regenerate.sh`) writes the
//! normalized outputs as the new expectations instead of comparing them.
//! Exit codes are never blessed: they live in the hand-authored
//! `case.yaml`, and a mismatch fails in bless mode too.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const BIN: &str = env!("CARGO_BIN_EXE_canon");
const BLESS_ENV: &str = "CANON_CONFORMANCE_BLESS";
const BLESS_COMMAND: &str = "conformance/regenerate.sh";
/// The D6 live-stamp case: `canon --version` pinned to the workspace version.
const VERSION_CASE: &str = "version";

/// Lines of unchanged context around each hunk of a mismatch diff.
const DIFF_CONTEXT: usize = 3;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseSpec {
    description: String,
    #[serde(default)]
    env: BTreeMap<String, String>,
    steps: Vec<StepSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepSpec {
    args: Vec<String>,
    exit: i32,
}

struct Case {
    name: String,
    dir: PathBuf,
    spec: CaseSpec,
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

fn cases_dir() -> PathBuf {
    corpus_dir().join("cases")
}

fn bless_mode() -> bool {
    std::env::var_os(BLESS_ENV).is_some_and(|v| v == "1")
}

/// Every `cases/*/case.yaml`, name-sorted. A missing `cases/` directory and
/// an empty one both yield zero cases, which the caller turns into a failure.
/// A case directory without a `case.yaml` is an error, not a skip: a
/// half-added case must not vanish from the run.
fn discover_cases() -> Result<Vec<Case>, String> {
    let root = cases_dir();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut cases = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let yaml_path = dir.join("case.yaml");
        let text = std::fs::read_to_string(&yaml_path)
            .map_err(|e| format!("case `{name}`: cannot read {}: {e}", yaml_path.display()))?;
        let spec: CaseSpec = serde_yaml::from_str(&text)
            .map_err(|e| format!("case `{name}`: invalid case.yaml: {e}"))?;
        if spec.description.trim().is_empty() {
            return Err(format!(
                "case `{name}`: description must name the contract the case pins"
            ));
        }
        if spec.steps.is_empty() {
            return Err(format!("case `{name}`: steps must not be empty"));
        }
        cases.push(Case { name, dir, spec });
    }
    Ok(cases)
}

/// Recursively copy `src` into `dst` (files and directories only).
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// The fixed base environment every step runs with; the case's `env` is
/// merged over it. Nothing else is inherited — in particular no `CANON_*`.
fn base_env(home: &Path) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_default(),
    );
    env.insert("HOME".to_string(), home.to_string_lossy().into_owned());
    env.insert("LC_ALL".to_string(), "C".to_string());
    env.insert("TZ".to_string(), "UTC".to_string());
    env
}

// ── Normalization (D2) ──
//
// Applied in this exact order; `conformance/README.md` lists the same rules
// verbatim. Nothing else is rewritten.
//
// 1. The canonicalized temp repo path (macOS: `/private/var/...`) -> `<repo>`.
// 2. The temp repo path as created (macOS: `/var/...`) -> `<repo>`.
// 3. RFC3339 timestamps -> `<ts>`.
// 4. ULIDs -> `<ulid>`.
// 5. 12-hex record digests in ledger record paths
//    (`kind=<kind>/[area=<area>/]<natural_key>__<12 hex>.json`) ->
//    `__<digest12>.json`.

fn normalize(text: &str, repo_paths: &[String]) -> String {
    let mut out = text.to_string();
    for path in repo_paths {
        out = out.replace(path.as_str(), "<repo>");
    }
    let out = replace_rfc3339(&out);
    let out = replace_ulids(&out);
    replace_ledger_digests(&out)
}

/// The repo path spellings to replace, longest first so the canonical
/// `/private/var/...` form is consumed before its `/var/...` suffix.
fn repo_path_spellings(repo: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(canonical) = repo.canonicalize() {
        paths.push(canonical.to_string_lossy().into_owned());
    }
    let created = repo.to_string_lossy().into_owned();
    if !paths.contains(&created) {
        paths.push(created);
    }
    paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
    paths
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn digits(bytes: &[u8], at: usize, n: usize) -> bool {
    bytes.len() >= at + n && bytes[at..at + n].iter().all(u8::is_ascii_digit)
}

/// Length of an RFC3339 timestamp starting at `at`, if one does:
/// `YYYY-MM-DDTHH:MM:SS[.frac](Z|±HH:MM)`.
fn rfc3339_len(bytes: &[u8], at: usize) -> Option<usize> {
    let b = |i: usize| bytes.get(at + i).copied();
    if !(digits(bytes, at, 4)
        && b(4) == Some(b'-')
        && digits(bytes, at + 5, 2)
        && b(7) == Some(b'-')
        && digits(bytes, at + 8, 2)
        && b(10) == Some(b'T')
        && digits(bytes, at + 11, 2)
        && b(13) == Some(b':')
        && digits(bytes, at + 14, 2)
        && b(16) == Some(b':')
        && digits(bytes, at + 17, 2))
    {
        return None;
    }
    let mut i = 19;
    if b(i) == Some(b'.') && b(i + 1).is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
        while b(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
    }
    match b(i) {
        Some(b'Z') => Some(i + 1),
        Some(b'+' | b'-')
            if digits(bytes, at + i + 1, 2)
                && b(i + 3) == Some(b':')
                && digits(bytes, at + i + 4, 2) =>
        {
            Some(i + 6)
        }
        _ => None,
    }
}

fn replace_rfc3339(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i < bytes.len() {
        let boundary = i == 0 || !is_word_byte(bytes[i - 1]);
        if boundary {
            if let Some(len) = rfc3339_len(bytes, i) {
                if bytes.get(i + len).is_none_or(|&c| !is_word_byte(c)) {
                    out.push_str(&text[copied..i]);
                    out.push_str("<ts>");
                    i += len;
                    copied = i;
                    continue;
                }
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

/// Crockford base32 as ULIDs print it: uppercase, no I/L/O/U.
fn is_crockford(b: u8) -> bool {
    b.is_ascii_digit() || (b.is_ascii_uppercase() && !matches!(b, b'I' | b'L' | b'O' | b'U'))
}

/// A ULID is exactly 26 Crockford characters whose first is `0`–`7` (the
/// 128-bit bound), standing alone between non-alphanumeric bytes.
fn replace_ulids(text: &str) -> String {
    const LEN: usize = 26;
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i + LEN <= bytes.len() {
        let boundary_before = i == 0 || !is_word_byte(bytes[i - 1]);
        let boundary_after = bytes.get(i + LEN).is_none_or(|&c| !is_word_byte(c));
        if boundary_before
            && boundary_after
            && (b'0'..=b'7').contains(&bytes[i])
            && bytes[i..i + LEN].iter().all(|&c| is_crockford(c))
        {
            out.push_str(&text[copied..i]);
            out.push_str("<ulid>");
            i += LEN;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

/// The writer's ledger record path (`canon_store::partition::hive_object_key`,
/// `GitTier::write_namespaced`) is
/// `kind=<kind>/[area=<area>/]<natural_key>__<12 lowercase hex>.json`; the
/// digest is over a time-stamped record, so only that 12-hex segment is
/// replaced, and only where the whole path shape is present:
///
/// - `kind=` begins a path segment: text start, or right after `/`,
///   whitespace or a quote (`"`, `'`, `` ` ``), so both a bare
///   ledger-relative location and `.canon/ledger/kind=...` qualify;
/// - `<kind>` is `[a-z0-9_.-]+` (core snake_case or namespaced `ns.kind`);
/// - `<area>` and `<natural_key>` are non-empty path components, and
///   `<natural_key>` does not start with `.`;
/// - `.json` ends the filename token: text end or a byte that cannot
///   continue a filename (not ASCII alphanumeric, `_`, `-`, `.`, `/`, `\`).
///
/// A bare `name__<12 hex>.json` outside a `kind=` directory — a cache file
/// name in a JSON field, a message — is consumer-visible and kept verbatim.
fn replace_ledger_digests(text: &str) -> String {
    const HEX: usize = 12;
    const SUFFIX: usize = 2 + HEX + 5;
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i + SUFFIX <= bytes.len() {
        if bytes[i..].starts_with(b"__")
            && bytes[i + 2..i + 2 + HEX].iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
            && &bytes[i + 2 + HEX..i + SUFFIX] == b".json"
            && bytes.get(i + SUFFIX).is_none_or(|&c| !continues_filename(c))
            && in_ledger_record_path(bytes, i)
        {
            out.push_str(&text[copied..i]);
            out.push_str("__<digest12>.json");
            i += SUFFIX;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

fn continues_filename(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b'\\')
}

/// A byte that can sit inside one path component of a ledger record path.
fn is_component_byte(b: u8) -> bool {
    !(b.is_ascii_whitespace() || matches!(b, b'/' | b'\\' | b'"' | b'\'' | b'`'))
}

/// Start of the run of component bytes ending at `end` (exclusive).
fn component_start(bytes: &[u8], end: usize) -> usize {
    bytes[..end].iter().rposition(|&b| !is_component_byte(b)).map_or(0, |p| p + 1)
}

/// Whether the `__<12 hex>.json` suffix at `suffix` ends a
/// `kind=<kind>/[area=<area>/]<natural_key>` path (see
/// [`replace_ledger_digests`]).
fn in_ledger_record_path(bytes: &[u8], suffix: usize) -> bool {
    let stem = component_start(bytes, suffix);
    if stem == suffix || stem == 0 || bytes[stem - 1] != b'/' || bytes[stem] == b'.' {
        return false;
    }
    let mut dir_end = stem - 1;
    let mut dir = component_start(bytes, dir_end);
    if let Some(area) = bytes[dir..dir_end].strip_prefix(b"area=") {
        if area.is_empty() || dir == 0 || bytes[dir - 1] != b'/' {
            return false;
        }
        dir_end = dir - 1;
        dir = component_start(bytes, dir_end);
    }
    let Some(kind) = bytes[dir..dir_end].strip_prefix(b"kind=") else {
        return false;
    };
    !kind.is_empty()
        && kind.iter().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
        && (dir == 0 || bytes[dir - 1] != b'\\')
}

// ── Diff ──

/// A unified-style line diff (`-` expected, `+` actual) with
/// `DIFF_CONTEXT` lines of context around each hunk.
fn unified_diff(expected: &str, actual: &str) -> String {
    let a: Vec<&str> = expected.split_inclusive('\n').collect();
    let b: Vec<&str> = actual.split_inclusive('\n').collect();
    // LCS table, suffix form: lcs[i][j] = LCS length of a[i..], b[j..].
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Edit script: (tag, a_line_no, b_line_no, text).
    let mut ops: Vec<(char, usize, usize, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', i, j, a[i]));
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(('-', i, j, a[i]));
            i += 1;
        } else {
            ops.push(('+', i, j, b[j]));
            j += 1;
        }
    }

    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| op.0 != ' ')
        .map(|(k, _)| k)
        .collect();
    let mut out = String::from("--- expected\n+++ actual\n");
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(DIFF_CONTEXT);
        let mut end = (changed[k] + DIFF_CONTEXT + 1).min(ops.len());
        k += 1;
        while k < changed.len() && changed[k] <= end + DIFF_CONTEXT {
            end = (changed[k] + DIFF_CONTEXT + 1).min(ops.len());
            k += 1;
        }
        let hunk = &ops[start..end];
        let a_len = hunk.iter().filter(|op| op.0 != '+').count();
        let b_len = hunk.iter().filter(|op| op.0 != '-').count();
        let _ = writeln!(
            out,
            "@@ -{},{a_len} +{},{b_len} @@",
            hunk[0].1 + 1,
            hunk[0].2 + 1
        );
        for (tag, _, _, line) in hunk {
            out.push(*tag);
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    out
}

// ── Replay ──

fn run_case(case: &Case, bless: bool) -> Result<(), String> {
    let scratch = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let repo = scratch.path().join("repo");
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).map_err(|e| format!("create home: {e}"))?;
    let fixture = case.dir.join("repo");
    if fixture.is_dir() {
        copy_tree(&fixture, &repo).map_err(|e| format!("copy repo/ fixture: {e}"))?;
    } else {
        std::fs::create_dir_all(&repo).map_err(|e| format!("create repo: {e}"))?;
    }

    let mut env = base_env(&home);
    env.extend(case.spec.env.clone());

    let git = Command::new("git")
        .args(["-c", "init.defaultBranch=main", "init", "-q"])
        .current_dir(&repo)
        .env_clear()
        .envs(&env)
        .output()
        .map_err(|e| format!("spawn git init: {e}"))?;
    if !git.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&git.stderr)
        ));
    }

    let repo_paths = repo_path_spellings(&repo);
    let expected_dir = case.dir.join("expected");
    if bless {
        std::fs::create_dir_all(&expected_dir).map_err(|e| format!("create expected/: {e}"))?;
        clear_expectations(&expected_dir)?;
    } else {
        check_no_orphan_expectations(&expected_dir, case.spec.steps.len())?;
    }

    let mut failures = String::new();
    for (index, step) in case.spec.steps.iter().enumerate() {
        let n = index + 1;
        let output = Command::new(BIN)
            .args(&step.args)
            .current_dir(&repo)
            .env_clear()
            .envs(&env)
            .output()
            .map_err(|e| format!("step {n}: spawn canon: {e}"))?;
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| format!("step {n}: stdout is not UTF-8"))?;
        let stderr = String::from_utf8(output.stderr)
            .map_err(|_| format!("step {n}: stderr is not UTF-8"))?;
        let stdout = normalize(&stdout, &repo_paths);
        let stderr = normalize(&stderr, &repo_paths);
        let invocation = format!("canon {}", step.args.join(" "));

        match output.status.code() {
            Some(code) if code == step.exit => {}
            code => {
                let _ = writeln!(
                    failures,
                    "step {n} (`{invocation}`): exit code {code:?}, case.yaml expects {} \
                     (exit codes are hand-authored in case.yaml; bless never rewrites them)\n\
                     stdout:\n{stdout}stderr:\n{stderr}",
                    step.exit
                );
            }
        }

        for (stream, actual) in [("out", &stdout), ("err", &stderr)] {
            let path = expected_dir.join(format!("{n}.{stream}"));
            if bless {
                std::fs::write(&path, actual)
                    .map_err(|e| format!("write {}: {e}", path.display()))?;
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(expected) if &expected == actual => {}
                Ok(expected) => {
                    let _ = writeln!(failures, "step {n} (`{invocation}`): std{stream} differs from expected/{n}.{stream}\n{}", unified_diff(&expected, actual));
                }
                Err(e) => {
                    let _ = writeln!(failures, "step {n} (`{invocation}`): cannot read expected/{n}.{stream}: {e}\nactual std{stream}:\n{actual}");
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn expectation_files(expected_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(expected_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "out" || ext == "err")
        })
        .collect())
}

fn clear_expectations(expected_dir: &Path) -> Result<(), String> {
    for path in expectation_files(expected_dir)? {
        std::fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
    }
    Ok(())
}

/// An `expected/<n>.*` for a step that no longer exists is stale contract
/// text; fail rather than leave it lying around unchecked.
fn check_no_orphan_expectations(expected_dir: &Path, steps: usize) -> Result<(), String> {
    for path in expectation_files(expected_dir)? {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let in_range = stem
            .parse::<usize>()
            .is_ok_and(|n| (1..=steps).contains(&n));
        if !in_range {
            return Err(format!(
                "orphan expectation {} matches no step (case has {steps} steps)",
                path.display()
            ));
        }
    }
    Ok(())
}

#[test]
fn conformance_corpus_replays_byte_exact() {
    let cases = discover_cases().unwrap_or_else(|e| panic!("conformance: {e}"));
    assert!(
        !cases.is_empty(),
        "conformance: discovered zero cases under {} — the corpus moved or was deleted; \
         a run that checks nothing must not pass",
        cases_dir().display()
    );

    let bless = bless_mode();
    let mut report = String::new();
    let mut failed = 0;
    for case in &cases {
        if let Err(failure) = run_case(case, bless) {
            failed += 1;
            let _ = writeln!(
                report,
                "── case `{}`: {}\n{failure}",
                case.name, case.spec.description
            );
        }
    }
    // After the replay, so in bless mode it reads what was just written.
    if let Err(failure) = check_version_stamp(&cases) {
        failed += 1;
        let _ = writeln!(report, "── live stamp (case `{VERSION_CASE}`)\n{failure}");
    }
    assert!(
        report.is_empty(),
        "conformance: {failed} failure(s) across {} case(s)\n\n{report}\nIf the new output is an intended contract change, \
         re-bless with `{BLESS_COMMAND}` and name the change in the release notes.",
        cases.len()
    );
}

/// D6 live stamp: the blessed `canon --version` output must be today's
/// workspace version, so a release bump without a re-bless fails with a
/// pointed message rather than only as a replay diff.
fn check_version_stamp(cases: &[Case]) -> Result<(), String> {
    let case = cases
        .iter()
        .find(|c| c.name == VERSION_CASE)
        .ok_or_else(|| format!("the `{VERSION_CASE}` case is missing from the corpus"))?;
    if case.spec.steps.len() != 1 || case.spec.steps[0].args != ["--version"] {
        return Err(
            "the version case must be exactly one step running `canon --version`".to_string(),
        );
    }
    let path = case.dir.join("expected/1.out");
    let blessed = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let version = env!("CARGO_PKG_VERSION");
    if blessed != format!("canon {version}\n") {
        return Err(format!(
            "the corpus is stamped `{}` but the workspace version is {version}: re-bless with `{BLESS_COMMAND}` after a version bump",
            blessed.trim_end()
        ));
    }
    Ok(())
}

#[test]
fn normalization_rules_rewrite_only_the_documented_values() {
    let repo = "/private/var/folders/x/T/.tmpAbC/repo".to_string();
    let short = "/var/folders/x/T/.tmpAbC/repo".to_string();
    let text = "at /private/var/folders/x/T/.tmpAbC/repo/a and /var/folders/x/T/.tmpAbC/repo/b\n\
                id 01J9Z3Q4W5E6R7T8Y9V0P1A2SX stamped 2026-10-09T12:34:56.123456789Z / 2026-10-09T12:34:56+09:00\n\
                file kind=evidence_record/01J9Z3Q4W5E6R7T8Y9V0P1A2SD__0123456789ab.json\n\
                kept: not-a-ulid 01J9Z3Q4W5E6R7T8Y9U0P1A2SX sha 0123456789abcdef0123 task seed-change#1 2026-10-09\n";
    let got = normalize(text, &[repo, short]);
    assert_eq!(
        got,
        "at <repo>/a and <repo>/b\n\
         id <ulid> stamped <ts> / <ts>\n\
         file kind=evidence_record/<ulid>__<digest12>.json\n\
         kept: not-a-ulid 01J9Z3Q4W5E6R7T8Y9U0P1A2SX sha 0123456789abcdef0123 task seed-change#1 2026-10-09\n"
    );
}

#[test]
fn ledger_digest_rule_rewrites_only_ledger_record_paths() {
    let text = "wrote .canon/ledger/kind=task/seed-change#1__0123456789ab.json\n\
                {\"location\":\"kind=review/area=world/root__world.hotdeal.01__abc__fedcba987654.json\"}\n\
                {\"cache\":\"cache__0123456789ab.json\"}\n\
                plain cache__0123456789ab.json and tmp/cache__0123456789ab.json\n\
                not a filename end: kind=task/x__0123456789ab.json.bak kind=Task/x__0123456789ab.json\n";
    assert_eq!(
        replace_ledger_digests(text),
        "wrote .canon/ledger/kind=task/seed-change#1__<digest12>.json\n\
         {\"location\":\"kind=review/area=world/root__world.hotdeal.01__abc__<digest12>.json\"}\n\
         {\"cache\":\"cache__0123456789ab.json\"}\n\
         plain cache__0123456789ab.json and tmp/cache__0123456789ab.json\n\
         not a filename end: kind=task/x__0123456789ab.json.bak kind=Task/x__0123456789ab.json\n"
    );
}

#[test]
fn mismatch_diff_is_unified_with_context() {
    let expected = "a\nb\nc\nd\ne\nf\ng\nh\n";
    let actual = "a\nb\nc\nd\nE\nf\ng\nh\n";
    assert_eq!(
        unified_diff(expected, actual),
        "--- expected\n+++ actual\n@@ -2,7 +2,7 @@\n b\n c\n d\n-e\n+E\n f\n g\n h\n"
    );
}
