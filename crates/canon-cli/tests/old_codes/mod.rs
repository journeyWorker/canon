//! The error codes of 0.35 that 0.36 removed from the registry (`canon-0-36-codes-table`).
//!
//! The 0.35 registry is data: `tests/fixtures/codes-0.35.tsv` holds its 179 names, taken once from the 0.35 release, because the old names
//! are in no registry view any more and cannot be derived. A name is *gone* when the error registry no longer holds it. A name that a
//! warning of the warning registry still carries is not searched for (spec 7.2.1: a condition applies only to `error.code`), and a name
//! the error registry still holds is a name that stayed. The CHANGELOG's replacement table is the one place that spells the gone names.

#![allow(dead_code, reason = "each test crate that includes this module uses a part of it")]
use std::sync::LazyLock;

/// The 0.35 registry: `(code, exit, retryable)` of each of its 179 codes, from `tests/fixtures/codes-0.35.tsv`.
static REGISTRY_0_35: LazyLock<Vec<(&'static str, u8, bool)>> = LazyLock::new(|| {
    include_str!("../fixtures/codes-0.35.tsv")
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            let [code, exit, retryable] = cells.as_slice() else { panic!("a line of codes-0.35.tsv has {} cells: {line}", cells.len()) };
            (*code, exit.parse().expect("an exit"), *retryable == "yes")
        })
        .collect()
});

static CODES_0_35: LazyLock<Vec<&'static str>> = LazyLock::new(|| REGISTRY_0_35.iter().map(|(code, ..)| *code).collect());

static GONE: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| CODES_0_35.iter().copied().filter(|code| canon_cli::cli_error::code_info(code).is_none() && canon_cli::cli_error::warning_info(code).is_none()).collect());

/// The 0.35 registry: `(code, exit, retryable)`, in the order of the fixture.
pub fn registry_0_35() -> &'static [(&'static str, u8, bool)] {
    &REGISTRY_0_35
}

/// The 179 error codes of the 0.35 registry, in the order of the fixture.
pub fn codes_0_35() -> &'static [&'static str] {
    &CODES_0_35
}

/// The 0.35 codes that no longer exist as an error code and are no warning's name either: the names no consumer may spell.
pub fn gone_codes() -> &'static [&'static str] {
    &GONE
}

/// `word` in `text` as a whole identifier, so `CONTEXT_INPUT_NOT_FOUND` is not `INPUT_MISSING`.
pub fn word_in(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

/// Every gone code `text` spells, in the order of the fixture.
pub fn gone_codes_in(text: &str) -> Vec<&'static str> {
    gone_codes().iter().copied().filter(|code| word_in(text, code)).collect()
}

/// One row of the CHANGELOG's replacement table: a code of the 0.35 registry and what it is in 0.36.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The 0.35 name.
    pub old: String,
    /// Every code of 0.36 that takes its place, in the order of the cell (the old name itself for `kept`).
    pub new: Vec<String>,
    /// `kept`, `renamed`, `split`, `merged` or `gone`.
    pub kind: String,
    pub exit_before: u8,
    /// Every exit the new codes have, in the order of the cell.
    pub exit_after: Vec<u8>,
    pub retryable_before: bool,
    pub retryable_after: Vec<bool>,
    /// The `error.reason` values the row names.
    pub reasons: Vec<String>,
    pub why: String,
}

/// The backticked names of `cell`.
fn ticked(cell: &str) -> Vec<String> {
    cell.split('`').skip(1).step_by(2).map(str::to_string).collect()
}

fn yes_no(word: &str) -> Result<bool, String> {
    match word {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => Err(format!("`{other}` is neither yes nor no")),
    }
}

/// The header of the replacement table.
pub const TABLE_HEADER: &str = "| 0.35 | 0.36 | kind | exit | retryable | `reason` | why |";

/// The rows of the replacement table of the `## 0.36.0` section of `changelog`. A row that does not read is an error naming it.
pub fn replacement_table(changelog: &str) -> Result<Vec<Row>, String> {
    let start = changelog.find("\n## 0.36.0").ok_or("the CHANGELOG has no Unreleased section")?;
    let rest = &changelog[start + 1..];
    let section = &rest[..rest[3..].find("\n## ").map_or(rest.len(), |at| at + 3)];
    let mut lines = section.lines().skip_while(|line| *line != TABLE_HEADER);
    lines.next().ok_or("the Unreleased section has no replacement table")?;
    lines.next().filter(|line| line.starts_with("|---")).ok_or("the replacement table has no separator line")?;
    let mut rows = Vec::new();
    for line in lines.take_while(|line| line.starts_with('|')) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let [_, old, new, kind, exit, retryable, reason, why, _] = cells.as_slice() else { return Err(format!("the row has {} cells, not 7: {line}", cells.len().saturating_sub(2))) };
        let (old, new_codes) = (ticked(old), ticked(new));
        if old.len() != 1 || new_codes.is_empty() {
            return Err(format!("the row names {} old and {} new codes: {line}", old.len(), new_codes.len()));
        }
        let (exit_before, exit_after) = exit.split_once(" → ").ok_or_else(|| format!("the exit cell is not `<n> → <m>`: {line}"))?;
        let (retry_before, retry_after) = retryable.split_once(" → ").ok_or_else(|| format!("the retryable cell is not `<a> → <b>`: {line}"))?;
        let number = |text: &str| text.trim().parse::<u8>().map_err(|e| format!("`{text}` is no exit ({e}): {line}"));
        rows.push(Row {
            old: old[0].clone(),
            new: new_codes,
            kind: (*kind).to_string(),
            exit_before: number(exit_before)?,
            exit_after: exit_after.split(',').map(number).collect::<Result<_, _>>()?,
            retryable_before: yes_no(retry_before.trim()).map_err(|e| format!("{e}: {line}"))?,
            retryable_after: retry_after.split(',').map(|word| yes_no(word.trim())).collect::<Result<_, _>>().map_err(|e| format!("{e}: {line}"))?,
            reasons: ticked(reason),
            why: (*why).to_string(),
        });
    }
    Ok(rows)
}

/// The CHANGELOG of the repository.
pub fn changelog() -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the CHANGELOG")
}

/// The condition words a code's name may end in (spec 7.2.1).
const CONDITIONS: &[&str] = &["INVALID", "REQUIRED", "NOT_FOUND", "EXISTS", "CONFLICT", "STALE", "EXPIRED", "DENIED", "UNSUPPORTED", "TOO_LARGE", "CORRUPT", "UNAVAILABLE", "FAILED", "INCOMPLETE", "TIMEOUT"];

/// Every whole identifier of `text` that reads as a code name of the rule (`<SUBJECT>_<CONDITION>`) and that neither the error registry, the warning
/// registry nor a reason of either holds: a code a consumer says that canon does not have (a typo included).
pub fn unregistered_codes_in(text: &str, registered: &std::collections::BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, found: &mut Vec<String>| {
        let candidate = word.trim_matches('_').to_string();
        let upper = candidate.contains('_') && candidate.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && candidate.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        // A name of the rule's shape, or a name one or two letters from a registered code (a typo).
        let looks_like_code = upper && (CONDITIONS.iter().any(|c| candidate.len() > c.len() + 1 && candidate.ends_with(&format!("_{c}"))) || (candidate.len() >= 8 && registered.iter().any(|code| code.len() >= 8 && edit_distance(code, &candidate) <= 2)));
        if looks_like_code && !registered.contains(&candidate) && !found.contains(&candidate) {
            found.push(candidate);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut found);
        }
    }
    flush(&mut word, &mut found);
    found
}

/// The Levenshtein distance of two names.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (row[j] + 1).min(above + 1).min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Every code, warning and reason canon registers, and the names a consumer may spell that are not codes (environment variables and the like).
pub fn registered_names() -> std::collections::BTreeSet<String> {
    let mut names: std::collections::BTreeSet<String> = canon_cli::cli_error::all_codes().iter().flat_map(|info| std::iter::once(info.code.to_string()).chain(info.reasons.iter().map(|r| r.reason.to_string()))).collect();
    names.extend(canon_cli::cli_error::all_warnings().iter().map(|info| info.code.to_string()));
    names
}
