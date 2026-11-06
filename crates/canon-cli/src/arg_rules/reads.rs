//! Rows of the commands that read, retain and maintain: query, export, purge, retrieve, ingest, migrate, init, skills.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, ACTOR_DESCRIPTION, ANSWER_RELAYED, CALENDAR, NOT_BLANK, ROLE_DESCRIPTION, VIA_DESCRIPTION};

/// `init --language`: trimmed and lowercased (ASCII), then a primary subtag of two or three letters and `-` subtags of one to
/// eight letters or digits (`ko`, `en-US`, `zh-Hant`).
const LANGUAGE: &str = concat!("^", white_space!(), "*[A-Za-z]{2,3}(-[A-Za-z0-9]{1,8})*", white_space!(), "*$");

/// A `regime show` segment: at least one character that canonicalizing (trim, white space and `/` to `-`) keeps.
const REGIME_SEGMENT: &str = concat!("^[", white_space!(), "/]*[^", white_space!(), "/][^\u{0}]*$");

/// The `regime show --hash`: trimmed and lowercased it is 6 to 64 hex digits.
const REGIME_HASH: &str = concat!("^", white_space!(), "*[0-9A-Fa-f]{6,64}", white_space!(), "*$");

/// Every kind but the ones a filter of `query` applies to.
/// What `canon trace` reads an operand as, after trimming (`search/trace.rs`): a review id (26 characters of Crockford base32),
/// a commit sha (7 to 40 hex digits) or a finding key `<change>_<round>_<seq>` with a non-empty change. It is judged from the text
/// alone, before any read; a pattern is a necessary condition: it admits some text (` _1_2`) that the parser then calls unrecognised.
const TRACE_OPERAND: &str = concat!("^", white_space!(), "*([0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}|[0-9A-Fa-f]{7,40}|", any_char!(), "+_[0-9]+_[0-9]+)", white_space!(), "*$");

const NOT_CHANGE_KINDS: &[&str] = &["scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "subject", "finding"];
const NOT_STATUS_KINDS: &[&str] = &["scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "finding"];
const NOT_DOMAIN_KINDS: &[&str] = &["change", "task", "scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "finding"];

pub(crate) static TABLES: Tables = Tables {
    rules: &[
        ("ingest plans", &[Rule::requires("dialect", &["plan-source"]), Rule::requires("plan-source", &["dialect"])]),
        ("migrate", &[Rule::requires("backup", &["revert"]), Rule::requires("relayed-via", &["commit"]), Rule::requires("actor", &["relayed-via"]), Rule::requires("approver", &["relayed-via"]), Rule::requires("note", &["relayed-via"])]),
        (
            "query",
            &[
                Rule::forbids_in("kind", NOT_CHANGE_KINDS, &["change"]).code("QUERY_ARGUMENT_INVALID").help("`--change` applies only to `--kind change` and `--kind task`."),
                Rule::forbids_in("kind", NOT_STATUS_KINDS, &["status"]).code("QUERY_ARGUMENT_INVALID").help("`--status` applies only to `--kind change`, `--kind task` and `--kind subject`."),
                Rule::forbids_in("kind", NOT_DOMAIN_KINDS, &["domain"]).code("QUERY_ARGUMENT_INVALID").help("`--domain` applies only to `--kind subject`."),
            ],
        ),
        ("skills install", &[Rule::at_most_one(&["harness", "no-harness"])]),
        ("skills check", &[Rule::at_most_one(&["harness", "no-harness"])]),
        ("skills doctor", &[Rule::at_most_one(&["harness", "no-harness"])]),
        ("retrieve", &[Rule::exactly_one(&["regime", "domain"]).hint("canon retrieve --for-role <ROLE> --domain <DOMAIN>"), Rule::requires("subject", &["domain"])]),
    ],
    validators: &[
        (
            "query",
            &[
                Validator {
                    arguments: &["kind", "status"],
                    code: "QUERY_ARGUMENT_INVALID",
                    description: "`--status` is one of the statuses of the queried kind: `open`, `done` for `task`; `proposed`, `in_progress`, `completed`, `archived` for `change`; `proposed`, `specced`, `building`, `verifying`, `shipped`, `retired` for `subject`.",
                    rejects: &["canon query --kind task --status proposed --json"],
                },
                Validator { arguments: &["since"], code: "USAGE", description: CALENDAR, rejects: &["canon query --kind evidence --since 2026-02-30 --json"] },
            ],
        ),
        (
            "export",
            &[
                Validator { arguments: &["before", "after"], code: "USAGE", description: CALENDAR, rejects: &["canon export --kind change --before 2026-02-30 --json", "canon export --kind change --after 2026-02-30 --json"] },
                Validator {
                    arguments: &["after", "before"],
                    code: "USAGE",
                    description: "`--after` (inclusive) is not later than `--before` (exclusive); an equal pair is an empty window, a reversed pair is refused before anything is read or written, with `--dry-run` and `--output` too.",
                    rejects: &["canon export --kind change --before 2026-01-02 --after 2026-02-01 --json"],
                },
            ],
        ),
        (
            "migrate",
            &[
                Validator {
                    arguments: &["relayed-via", "as"],
                    code: "USAGE",
                    description: ROLE_DESCRIPTION,
                    rejects: &["canon migrate --commit --plan-digest abc --relayed-via ask --actor impl --approver alice --note ok"],
                },
                Validator {
                    arguments: &["relayed-via"],
                    code: "USAGE",
                    description: VIA_DESCRIPTION,
                    rejects: &["canon migrate --commit --plan-digest abc --relayed-via \"a b\" --as implementer --actor impl --approver alice --note ok"],
                },
                Validator {
                    arguments: &["relayed-via", "actor"],
                    code: "USAGE",
                    description: ACTOR_DESCRIPTION,
                    rejects: &["canon migrate --commit --plan-digest abc --relayed-via ask --as implementer --approver alice --note ok"],
                },
                Validator {
                    arguments: &["relayed-via", "approver", "note"],
                    code: "USAGE",
                    description: ANSWER_RELAYED,
                    rejects: &["canon migrate --commit --plan-digest abc --relayed-via ask --as implementer --actor impl --approver alice"],
                },
            ],
        ),
        ("purge", &[Validator { arguments: &["before"], code: "USAGE", description: CALENDAR, rejects: &["canon purge --kind session --before 2026-02-30 --json"] }]),
        (
            "schema",
            &[Validator {
                arguments: &["schemas", "path"],
                code: "USAGE",
                description: "`--schemas` answers for one command: `path` names a command, not a group and not nothing (the tree).",
                rejects: &["canon schema --schemas --json", "canon schema finding --schemas --json"],
            }],
        ),
        (
            "retrieve",
            &[Validator {
                arguments: &["for-role", "regime"],
                code: "USAGE",
                description: "`--for-role` equals the first segment of `--regime` (the role of the regime key).",
                rejects: &["canon retrieve --for-role reviewer --regime dev/canon/join-spine/9c93d024b1a2 --json"],
            }],
        ),
    ],
    domains: &[
        DomainRow::new(Scope::Arguments(&["trace"], "id"), Domain::matching(TRACE_OPERAND), "TRACE_OPERAND_INVALID").inside(&["01ARZ3NDEKTSV4RRFFQ69G5FAV", "abcdef0", "loop-change_9_9", " a_1_2 "]).says("is none of the three shapes a trace operand takes: a review id (26 base32 characters), a commit sha (7 to 40 hex digits) or a finding key (<change>_<round>_<seq>)").outside(&["abc", "not a shape!", "abcdef", "a_1", "a__2", "a_1_x", "a_1_2_"]),
        // The text is not blank once trimmed; the handler refuses it before it opens the index.
        DomainRow::new(Scope::Arguments(&["search"], "text"), Domain::matching(NOT_BLANK), "SEARCH_QUERY_REQUIRED").inside(&["timeout", " a "]).outside(&["", "   "]),
        DomainRow::new(Scope::Arguments(&["init"], "language"), Domain::matching(LANGUAGE), "INIT_ARGUMENT_INVALID").inside(&["ko", "en-US", " zh-Hant ", "KO"]).outside(&["", "e", "eng1", "en-", "en--us", "한국어", "en-abcdefghi"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "role-segment"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["reviewer", " a b ", "/x/"]).outside(&["", "  ", "/", " / "]),
        DomainRow::new(Scope::Arguments(&["regime show"], "repo-name"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["my-repo", "A B"]).outside(&["", "  ", "//"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "area"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["api", "a/b"]).outside(&["", "\t", "/ /"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "hash"), Domain::matching(REGIME_HASH), "REGIME_KEY_INVALID").inside(&["abcdef", " ABCDEF0123 ", "0123456789abcdef"]).outside(&["", "abcde", "abcdeg", "ab cdef", "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789a"]),
    ],
};
