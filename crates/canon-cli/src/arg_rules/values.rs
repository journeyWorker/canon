//! The three value rules of 0.37 (`canon-0-37-option-values`): an option of the same name has the same domain in every command that has it.
//!
//! - an id (`--project`, `--role`, `--domain`): a kebab slug, [`super::KEBAB`]. `--project` and `--role` get it from the shared rows by value name
//!   (`mod.rs`); `--domain` is below, with the code each command refuses it with;
//! - a name (`--actor`, `--approver`, `--relayed-via`) and a sentence (`--reason`, `--summary`, `--override-reason`): one line of text,
//!   [`super::LINE_TEXT`]. The white space around it is taken and trimmed (the value parser of the option, `cli::trimmed`), a value that is
//!   blank once trimmed and one with a control character, a line separator or a paragraph separator inside it is refused;
//! - the bounds: `--round` starts at 1 wherever it is, `--limit` too (`mod.rs` for the listings, below for `retrieve`), and the key of
//!   `--regime` is the same for the two commands that take it.
//!
//! The code of a refusal is the one the command already refused that option with; a command that had no check says its own subject's
//! argument code (`DIVERGENCE_ARGUMENT_INVALID`, `SCAFFOLD_ARGUMENT_INVALID`, ...). The brief names the one domain, not the code.

use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, KEBAB, LINE_TEXT, REGIME_KEY};

/// Values the one-line text domain takes: the white space around is the parser's to trim, the middle keeps its spaces and its no-break
/// spaces, and any script is text.
const TEXT_INSIDE: &[&str] = &["alice", " alice ", "\talice\n", "a b", "a\u{a0}b", "a\u{3000}b", "알리스", "a\u{2028}", "\u{85}a"];
/// Values it refuses: nothing, only white space, and a control character, a line separator or a paragraph separator inside the text.
const TEXT_OUTSIDE: &[&str] = &["", " ", "\t", "\n", "\u{85}", "\u{2028}", "a\nb", "a\rb", "a\tb", "a\u{b}b", "a\u{c}b", "a\u{1b}b", "a\u{7f}b", "a\u{85}b", "a\u{2028}b", "a\u{2029}b"];
/// The sentence rows' own: a sentence with spaces in it.
const SENTENCE_INSIDE: &[&str] = &["the engine drops the event", " a ", "\ta\n", "a\u{a0}b", "한 줄의 이유", "a\u{2029}"];

/// A name option's row: [`LINE_TEXT`] with the code its commands refuse it with.
const fn name(commands: &'static [&'static str], option: &'static str, code: &'static str) -> DomainRow {
    DomainRow::new(Scope::Arguments(commands, option), Domain::matching(LINE_TEXT), code).inside(TEXT_INSIDE).outside(TEXT_OUTSIDE)
}

/// A sentence option's row: [`LINE_TEXT`] with the code its commands refuse it with.
const fn sentence(commands: &'static [&'static str], option: &'static str, code: &'static str) -> DomainRow {
    DomainRow::new(Scope::Arguments(commands, option), Domain::matching(LINE_TEXT), code).inside(SENTENCE_INSIDE).outside(TEXT_OUTSIDE)
}

pub(crate) static TABLES: Tables = Tables {
    rules: &[],
    validators: &[],
    domains: &[
        // ---------------------------------------------------------------------------------------------- a name: --actor
        // The relay's actor is also one word: that is the validator of the four relayed commands, not this domain, so its inside values are one word.
        DomainRow::new(Scope::Arguments(&["brief approve", "policy approve", "migrate"], "actor"), Domain::matching(LINE_TEXT), "USAGE").inside(&["impl", " impl ", "impl\n", "\timpl"]).outside(TEXT_OUTSIDE),
        name(&["dispatch begin"], "actor", "DISPATCH_ARGUMENT_INVALID"),
        name(&["divergence add", "divergence defer", "divergence resolve", "divergence accept"], "actor", "DIVERGENCE_ARGUMENT_INVALID"),
        name(&["evidence add", "evidence audit"], "actor", "EVIDENCE_ARGUMENT_INVALID"),
        name(&["feature new", "scenario new"], "actor", "SCAFFOLD_ARGUMENT_INVALID"),
        name(&["finding add", "finding close"], "actor", "FINDING_ARGUMENT_INVALID"),
        name(&["review change", "review scenario"], "actor", "REVIEW_ARGUMENT_INVALID"),
        name(&["strategy rollback"], "actor", "STRATEGY_ARGUMENT_INVALID"),
        name(&["subject move", "subject new"], "actor", "SUBJECT_ARGUMENT_INVALID"),
        // ------------------------------------------------------------------------------------------- a name: --approver
        name(&["brief approve", "policy approve"], "approver", "APPROVAL_INVALID"),
        name(&["divergence accept"], "approver", "DIVERGENCE_ARGUMENT_INVALID"),
        name(&["migrate"], "approver", "USAGE"),
        name(&["evidence approval-payload"], "approver", "EVIDENCE_ARGUMENT_INVALID"),
        name(&["strategy approval-payload"], "approver", "STRATEGY_ARGUMENT_INVALID"),
        name(&["evidence add"], "approver", "EVIDENCE_APPROVAL_INVALID").reason("MALFORMED"),
        // --------------------------------------------------------------------------------------- a name: --relayed-via
        // That it is one word (no white space, parenthesis or quote) is the validator of the four commands.
        DomainRow::new(Scope::Arguments(&["brief approve", "policy approve", "divergence accept", "migrate"], "relayed-via"), Domain::matching(LINE_TEXT), "USAGE").inside(&["ask", "AskUserQuestion", " chat "]).outside(TEXT_OUTSIDE),
        // ------------------------------------------------------------------------------------------ a sentence: --reason
        sentence(&["divergence add", "divergence defer", "divergence accept"], "reason", "DIVERGENCE_ARGUMENT_INVALID"),
        sentence(&["finding close"], "reason", "FINDING_REASON_REQUIRED"),
        sentence(&["strategy rollback"], "reason", "STRATEGY_ARGUMENT_INVALID"),
        sentence(&["subject move"], "reason", "SUBJECT_ARGUMENT_INVALID"),
        // ---------------------------------------------------------------------------------------- a sentence: --summary
        sentence(&["evidence add", "evidence audit"], "summary", "EVIDENCE_ARGUMENT_INVALID"),
        sentence(&["finding add"], "summary", "FINDING_ARGUMENT_INVALID"),
        sentence(&["subject new"], "summary", "SUBJECT_ARGUMENT_INVALID"),
        // ------------------------------------------------------------------------------------ a sentence: --override-reason
        sentence(&["evidence add", "evidence audit"], "override-reason", "EVIDENCE_ARGUMENT_INVALID"),
        sentence(&["subject move"], "override-reason", "SUBJECT_ARGUMENT_INVALID"),
        // ---------------------------------------------------------------------------------------------- an id: --domain
        // The subject's domain is a kebab slug wherever it is given; whether it is in the repo's vocabulary is state.
        DomainRow::new(Scope::Arguments(&["subject new"], "domain"), Domain::matching(KEBAB), "SUBJECT_ARGUMENT_INVALID").inside(&["dev", "a-1"]).outside(&["Dev", "a_b", "-a", "a-", "a--b", "d e", ""]),
        DomainRow::new(Scope::Arguments(&["query"], "domain"), Domain::matching(KEBAB), "QUERY_ARGUMENT_INVALID").inside(&["dev", "a-1"]).outside(&["Dev", "a_b", "-a", "a-", "a--b", "d e", ""]),
        DomainRow::new(Scope::Arguments(&["retrieve"], "domain"), Domain::matching(KEBAB), "USAGE").inside(&["join-spine", "a-1"]).outside(&["Join", "a_b", "-a", "a-", "a--b", "d e", ""]),
        // ----------------------------------------------------------------------------------------------- the bounds
        // `--round` is 1-based wherever it is given; `finding add` and `finding close` hold it in `evidence.rs`.
        DomainRow::new(Scope::Arguments(&["divergence add", "divergence defer", "divergence resolve"], "round"), Domain::at_least(1), "DIVERGENCE_ARGUMENT_INVALID"),
        // The regime key is one rule for both commands that take it; `dispatch begin` also holds `--role` to its first segment (its validator).
        DomainRow::new(Scope::Arguments(&["retrieve", "dispatch begin"], "regime"), Domain::matching(REGIME_KEY), "USAGE")
            .inside(&["reviewer/canon/join-spine/9c93d024b1a2", "reviewer/b/c/abcdef"])
            .outside(&["reviewer/b/c", "reviewer/b/c/d/abcdef", "reviewer//c/abcdef", "reviewer/b/c/ABCDEF", "reviewer/b/c/abcde", "reviewer/b c/c/abcdef"]),
    ],
};
