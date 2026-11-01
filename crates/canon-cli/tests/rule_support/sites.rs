//! The survey of the sources (0.34 `canon-0-34-atip-constraints`, F2 and F4): every place a handler or a parser refuses an
//! argument with a code that says "this argument is wrong", found by reading `crates/canon-cli/src`, and what each is.
//!
//! A site is a literal of one of [`ARGUMENT_CODES`] in the code of a function (test modules are not read), named by
//! its file, the function it is in, its code and its place among that function's sites of that code. Every site must
//! be classified in a group's table ([`super::sites_evidence`], [`super::sites_authoring`], [`super::sites_reads`]) as
//! one of [`Kind`]: a rule, a domain or a validator that the command document declares (and the sweeps of
//! `tests/arg_rules.rs` drive through the binary), a precondition (it reads state, so it is not declared), a check that
//! cannot be reached from the command line, or a check the contract cannot declare (reported as ATIP feedback). A site the tables
//! do not know, and a row that finds no site, fail the survey: adding a refusal that no document declares, or deleting
//! one the document still declares, cannot go unseen.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

/// The codes that mean an argument is wrong (or two are wrong together). `USAGE` is included: a handler that raises it by
/// itself checks an argument the parser did not.
pub const ARGUMENT_CODES: &[&str] = &[
    "USAGE",
    "APPROVAL_INCOMPLETE",
    "CHANGE_INVALID_ARGUMENT",
    "DISPATCH_INVALID_ARGUMENT",
    "DISPATCH_ROLE_REGIME_MISMATCH",
    "DIVERGENCE_ACCEPT_REFUSED",
    "DIVERGENCE_INVALID_ARGUMENT",
    "EVIDENCE_APPROVAL_INVALID",
    "EVIDENCE_FABRICATED",
    "EVIDENCE_INVALID_ARGUMENT",
    "EVIDENCE_REFUSED",
    "FINDING_INTRODUCED_REQUIRED",
    "FINDING_INVALID_ARGUMENT",
    "FINDING_REASON_REQUIRED",
    "FINDING_RESOLUTION_REV_UNEXPECTED",
    "GATE_TASK_INVALID_ARGUMENT",
    "INIT_INVALID_ARGUMENT",
    "LEARN_INVALID_ARGUMENT",
    "POLICY_APPROVAL_REFUSED",
    "QUERY_INVALID_ARGUMENT",
    "REGIME_KEY_INVALID",
    "REVIEW_INVALID_ARGUMENT",
    "SCAFFOLD_INVALID_ARGUMENT",
    "SEARCH_QUERY_EMPTY",
    "SKILLS_INVALID_ARGUMENT",
    "SUBJECT_INVALID_ARGUMENT",
    "SUBJECT_REASON_REQUIRED",
    "TRACE_OPERAND_INVALID",
];

/// Files that hold the codes themselves or the tables of this change: not sites.
const NOT_SITES: &[&str] = &["cli_error.rs", "schema_codes.rs", "spec.rs", "schema.rs", "arg_rules/"];

/// What a site is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A constraint of the command's document: the rule word and arguments it names (at least these).
    Rule { command: &'static str, rule: &'static str, arguments: &'static [&'static str] },
    /// The domain of one argument of a command.
    Domain { command: &'static str, argument: &'static str },
    /// A validator of the command that reads these arguments.
    Validator { command: &'static str, arguments: &'static [&'static str] },
    /// It reads state (the repo, the ledger, the environment, a file): not a constraint, a domain or a validator.
    Precondition(&'static str),
    /// No command line reaches it, or another check answers first (and is classified).
    Unreachable(&'static str),
    /// Stateless, but outside what the contract can declare (an exit 1 code, a check of the environment); reported as ATIP feedback.
    Undeclared(&'static str),
    /// Not a check of an argument of a command (the mapping of clap's own errors, an internal helper).
    Other(&'static str),
}

/// One classified site: `file:function:CODE#n`, with `file` relative to `crates/canon-cli/src`.
#[derive(Debug, Clone, Copy)]
pub struct Site {
    pub key: &'static str,
    pub kind: Kind,
}

pub const fn site(key: &'static str, kind: Kind) -> Site {
    Site { key, kind }
}

/// Every classified site of every group.
pub fn classified() -> Vec<Site> {
    super::sites_authoring::SITES.iter().chain(super::sites_evidence::SITES).chain(super::sites_reads::SITES).copied().collect()
}

fn skip(file: &str) -> bool {
    NOT_SITES.iter().any(|part| file.contains(part))
}

/// The sites of one source text: `(key, line)`. Test modules (from `#[cfg(test)]` on) are not read.
pub fn sites_in(file: &str, text: &str) -> Vec<(String, usize)> {
    if skip(file) {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let end = lines.iter().position(|line| line.trim() == "#[cfg(test)]").unwrap_or(lines.len());
    let mut counts: BTreeMap<(String, &str), usize> = BTreeMap::new();
    let mut found = Vec::new();
    let mut function = String::from("-");
    for (at, line) in lines[..end].iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub(crate) fn ").or_else(|| trimmed.strip_prefix("pub fn ")).or_else(|| trimmed.strip_prefix("fn ")).or_else(|| trimmed.strip_prefix("pub(super) fn ")).or_else(|| trimmed.strip_prefix("async fn ")) {
            function = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("-").to_string();
        }
        if trimmed.starts_with("//") {
            continue;
        }
        for code in ARGUMENT_CODES {
            let literal = format!("\"{code}\"");
            let mut from = 0;
            while let Some(found_at) = line[from..].find(&literal) {
                from += found_at + literal.len();
                let n = counts.entry((function.clone(), code)).and_modify(|n| *n += 1).or_insert(1);
                found.push((format!("{file}:{function}:{code}#{n}"), at + 1));
            }
        }
    }
    found
}

/// Every site of `crates/canon-cli/src` (and `extra`, a source the caller adds: how a test seeds a new check).
pub fn sites_of(src: &Path, extra: &[(&str, &str)]) -> Vec<(String, usize)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).expect("a source directory").map(|entry| entry.expect("an entry").path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let relative = path.strip_prefix(root).expect("below the root").to_string_lossy().into_owned();
                out.push((relative, std::fs::read_to_string(&path).expect("a source file")));
            }
        }
    }
    let mut files = Vec::new();
    walk(src, src, &mut files);
    files.extend(extra.iter().map(|(file, text)| ((*file).to_string(), (*text).to_string())));
    files.iter().flat_map(|(file, text)| sites_in(file, text)).collect()
}

/// The survey: what the sources have that the tables do not classify, what the tables classify that the sources lack, and
/// every `Rule`, `Domain` and `Validator` row that the command documents do not declare. Empty when the survey holds.
pub fn problems(found: &[(String, usize)], classified: &[Site], docs: &[&Value]) -> Vec<String> {
    let mut problems = Vec::new();
    let known: BTreeMap<&str, Kind> = classified.iter().map(|site| (site.key, site.kind)).collect();
    if known.len() != classified.len() {
        problems.push("a site is classified twice".to_string());
    }
    for (key, line) in found {
        if !known.contains_key(key.as_str()) {
            problems.push(format!("{key} (line {line}) refuses an argument and no table classifies it: declare it (a rule, a domain or a validator) or say why it is not one"));
        }
    }
    for site in classified {
        if !found.iter().any(|(key, _)| key == site.key) {
            problems.push(format!("{} is classified and the sources have no such site", site.key));
        }
        let doc = |command: &str| docs.iter().find(|doc| doc["command"] == command);
        match site.kind {
            Kind::Rule { command, rule, arguments } => {
                let declared = doc(command).is_some_and(|doc| {
                    super::constraints(doc).iter().any(|constraint| constraint.rule.as_str() == rule && arguments.iter().all(|argument| atip::constraint::members(constraint).iter().any(|member| member == argument)))
                });
                if !declared {
                    problems.push(format!("{}: `{command}` declares no `{rule}` rule over {arguments:?}", site.key));
                }
            }
            Kind::Domain { command, argument } => {
                if !doc(command).is_some_and(|doc| super::argument(doc, argument).is_some_and(|argument| argument.get("domain").is_some())) {
                    problems.push(format!("{}: `{command}` declares no domain for `{argument}`", site.key));
                }
            }
            Kind::Validator { command, arguments } => {
                let declared = doc(command).is_some_and(|doc| doc["validators"].as_array().into_iter().flatten().any(|validator| arguments.iter().all(|argument| validator["arguments"].as_array().into_iter().flatten().any(|name| name == argument))));
                if !declared {
                    problems.push(format!("{}: `{command}` declares no validator over {arguments:?}", site.key));
                }
            }
            Kind::Precondition(_) | Kind::Unreachable(_) | Kind::Undeclared(_) | Kind::Other(_) => {}
        }
    }
    problems.sort();
    problems
}
