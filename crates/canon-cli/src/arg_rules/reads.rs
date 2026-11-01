//! Rows of the commands that read, retain and maintain: query, export, purge, retrieve, ingest, migrate, init, skills.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, CALENDAR, NOT_BLANK};

/// White space as `str::trim` and `char::is_whitespace` read it (the macro of the parent module is declared after this module).
macro_rules! white_space {
    () => {
        " \t\n\u{b}\u{c}\r\u{85}\u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}"
    };
}

/// `init --language`: trimmed and lowercased (ASCII), then a primary subtag of two or three letters and `-` subtags of one to
/// eight letters or digits (`ko`, `en-US`, `zh-Hant`).
const LANGUAGE: &str = concat!("^[", white_space!(), "]*[A-Za-z]{2,3}(-[A-Za-z0-9]{1,8})*[", white_space!(), "]*$");

/// `--providers`: blank or `none` (any case) for no provider, else a comma separated list of `claude`, `codex`, `omp`, `pi` (any
/// case, white space around each, blank items skipped, at least one name).
const PROVIDERS: &str = concat!(
    "^([", white_space!(), "]*",
    "|[", white_space!(), "]*[Nn][Oo][Nn][Ee][", white_space!(), "]*",
    "|([", white_space!(), "]*,)*[", white_space!(), "]*([Cc][Ll][Aa][Uu][Dd][Ee]|[Cc][Oo][Dd][Ee][Xx]|[Oo][Mm][Pp]|[Pp][Ii])[", white_space!(), "]*",
    "(,[", white_space!(), "]*(([Cc][Ll][Aa][Uu][Dd][Ee]|[Cc][Oo][Dd][Ee][Xx]|[Oo][Mm][Pp]|[Pp][Ii])[", white_space!(), "]*)?)*",
    ")$"
);

/// `--regime`: four `/` separated segments, the first three without white space, the last 6 to 64 lowercase hex digits.
const REGIME_KEY: &str = concat!("^[^/", white_space!(), "]+/[^/", white_space!(), "]+/[^/", white_space!(), "]+/[0-9a-f]{6,64}$");

/// A `regime show` segment: at least one character that canonicalizing (trim, white space and `/` to `-`) keeps.
const REGIME_SEGMENT: &str = concat!("^[", white_space!(), "/]*[^", white_space!(), "/][^\u{0}]*$");

/// The `regime show --hash`: trimmed and lowercased it is 6 to 64 hex digits.
const REGIME_HASH: &str = concat!("^[", white_space!(), "]*[0-9A-Fa-f]{6,64}[", white_space!(), "]*$");

/// Every kind but the ones a filter of `query` applies to.
const NOT_CHANGE_KINDS: &[&str] = &["scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "subject", "finding"];
const NOT_STATUS_KINDS: &[&str] = &["scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "finding"];
const NOT_DOMAIN_KINDS: &[&str] = &["change", "task", "scenario", "session", "run", "event", "handoff", "review", "divergence", "trajectory", "strategy_item", "evidence", "finding"];

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("ingest plans", &[Rule::requires("dialect", &["plan-source"]), Rule::requires("plan-source", &["dialect"])]),
        ("migrate", &[Rule::requires("backup", &["revert"]), Rule::requires("relayed-via", &["commit"]), Rule::requires("actor", &["relayed-via"]), Rule::requires("approver", &["relayed-via"]), Rule::requires("note", &["relayed-via"])]),
        (
            "query",
            &[
                Rule::forbids_in("kind", NOT_CHANGE_KINDS, &["change"]).code("QUERY_INVALID_ARGUMENT").help("`--change` applies only to `--kind change` and `--kind task`."),
                Rule::forbids_in("kind", NOT_STATUS_KINDS, &["status"]).code("QUERY_INVALID_ARGUMENT").help("`--status` applies only to `--kind change`, `--kind task` and `--kind subject`."),
                Rule::forbids_in("kind", NOT_DOMAIN_KINDS, &["domain"]).code("QUERY_INVALID_ARGUMENT").help("`--domain` applies only to `--kind subject`."),
            ],
        ),
        ("retrieve", &[Rule::exactly_one(&["regime", "domain"]), Rule::requires("subject", &["domain"])]),
    ],
    validators: &[
        (
            "query",
            &[
                Validator {
                    arguments: &["kind", "status"],
                    code: "QUERY_INVALID_ARGUMENT",
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
        ("purge", &[Validator { arguments: &["before"], code: "USAGE", description: CALENDAR, rejects: &["canon purge --kind session --before 2026-02-30 --json"] }]),
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
        // The text is not blank once trimmed; the handler refuses it before it opens the index.
        DomainRow::new(Scope::Arguments(&["search"], "text"), Domain::matching(NOT_BLANK), "SEARCH_QUERY_EMPTY").inside(&["timeout", " a "]).outside(&["", "   "]),
        DomainRow::new(Scope::Arguments(&["init"], "language"), Domain::matching(LANGUAGE), "INIT_INVALID_ARGUMENT").inside(&["ko", "en-US", " zh-Hant ", "KO"]).outside(&["", "e", "eng1", "en-", "en--us", "한국어", "en-abcdefghi"]),
        DomainRow::new(Scope::Arguments(&["skills install", "skills check", "skills doctor"], "providers"), Domain::matching(PROVIDERS), "SKILLS_INVALID_ARGUMENT").inside(&["claude", "none", "NONE", "", " Claude , codex,,pi ", ",omp"]).outside(&[",", "gemini", "none,claude", "claude,gemini", " , "]),
        DomainRow::new(Scope::Arguments(&["retrieve"], "regime"), Domain::matching(REGIME_KEY), "USAGE").inside(&["reviewer/canon/join-spine/9c93d024b1a2", "reviewer/b/c/abcdef"]).outside(&["reviewer/b/c", "reviewer/b/c/d/abcdef", "reviewer//c/abcdef", "reviewer/b/c/ABCDEF", "reviewer/b/c/abcde", "reviewer/b c/c/abcdef"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "role-segment"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["reviewer", " a b ", "/x/"]).outside(&["", "  ", "/", " / "]),
        DomainRow::new(Scope::Arguments(&["regime show"], "repo-name"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["my-repo", "A B"]).outside(&["", "  ", "//"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "area"), Domain::matching(REGIME_SEGMENT), "REGIME_KEY_INVALID").inside(&["api", "a/b"]).outside(&["", "\t", "/ /"]),
        DomainRow::new(Scope::Arguments(&["regime show"], "hash"), Domain::matching(REGIME_HASH), "REGIME_KEY_INVALID").inside(&["abcdef", " ABCDEF0123 ", "0123456789abcdef"]).outside(&["", "abcde", "abcdeg", "ab cdef", "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789a"]),
    ],
};
