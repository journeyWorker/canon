//! Rows of the commands that author and approve: briefs, subjects, scenarios, policy, strategies, prompts, context packs.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, KEBAB, NOT_BLANK, TASK_ID};

/// A scenario tag as `scenario new` reads it: `<area>.<surface>.<nn>` with at most one leading `@` (the form a scenario
/// body writes it in).
const SCENARIO_TAG: &str = "^@?[a-z0-9-]+\\.[a-z0-9-]+\\.[0-9]{2,}$";
/// `<area>.<surface>`: exactly one `.`, two `[a-z0-9-]+` words.
const SURFACE: &str = "^[a-z0-9-]+\\.[a-z0-9-]+$";
/// `NAME@VERSION`: both parts non-empty, no second `@`.
const NAME_AT_VERSION: &str = "^[^@]+@[^@]+$";
/// A context pack id: `sha256:` and 64 lower-case hex digits.
const PACK_ID: &str = "^sha256:[0-9a-f]{64}$";
/// A line of text that is not blank and holds no CR or LF (`subject move --reason`).
const REASON: &str = concat!("^[^\\n\\r]*[^", white_space!(), "][^\\n\\r]*$");
/// The human's answer (`--approver`, `--note` of an approval): once trimmed, not empty and one line.
const ANSWER: &str = concat!("^[", white_space!(), "]*[^", white_space!(), "]([^\\n\\r]*[^", white_space!(), "])?[", white_space!(), "]*$");
/// The tool a relayed approval names (`--relayed-via`): one token, no white space, parenthesis or quote.
const RELAY_TOKEN: &str = concat!("^[^", white_space!(), "()\"']+$");
/// The relaying agent's actor id (`--actor` with `--relayed-via`): once trimmed, one such token.
const RELAY_ACTOR: &str = concat!("^[", white_space!(), "]*[^", white_space!(), "()\"']+[", white_space!(), "]*$");
/// One `--digest` entry: `<change>=sha256:<digest>`, the change not empty.
const DIGEST: &str = "^[^=]+=sha256:[^\u{0}]*$";

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("brief approve", &[Rule::requires("actor", &["relayed-via"]), Rule::requires("digest", &["relayed-via"])]),
        ("policy approve", &[Rule::requires("actor", &["relayed-via"]), Rule::requires("relayed-via", &["note"])]),
        ("scenario new", &[Rule::requires("change", &["flow"])]),
        ("strategy promote", &[Rule::requires("evaluation", &["approval"]), Rule::requires("approval", &["evaluation"]), Rule::requires("signature-file", &["approval"])]),
    ],
    validators: &[
        (
            "brief approve",
            &[
                Validator {
                    arguments: &["relayed-via", "digest", "change"],
                    code: "USAGE",
                    description: "A relayed approval names, for every change it approves, the digest the human was shown (`--digest <change>=sha256:<digest>`, once per change) and no digest of a change it does not approve. With no change operand the repo's only live change is read, and its digest is checked then.",
                    rejects: &[
                        "canon brief approve add-login --approver alice --note ok --relayed-via ask --actor impl --json",
                        "canon brief approve add-login --approver alice --note ok --relayed-via ask --actor impl --digest add-login=sha256:0123456789abcdef --digest other=sha256:0123456789abcdef --json",
                    ],
                },
                Validator {
                    arguments: &["digest"],
                    code: "USAGE",
                    description: "`--digest` names a change once: two entries for the same change must carry the same digest.",
                    rejects: &["canon brief approve add-login --approver alice --note ok --relayed-via ask --actor impl --digest add-login=sha256:aa --digest add-login=sha256:bb --json"],
                },
            ],
        ),
        (
            "policy approve",
            &[Validator {
                arguments: &["relayed-via", "note"],
                code: "USAGE",
                description: "A relayed approval carries the human's answer word for word: with `--relayed-via`, `--note` is not blank.",
                rejects: &["canon policy approve --approver alice --relayed-via ask --actor impl --note \" \" --json"],
            }],
        ),
        (
            "strategy approval-payload",
            &[Validator {
                arguments: &["approval-at"],
                code: "USAGE",
                description: super::CALENDAR,
                rejects: &["canon strategy approval-payload 01ARZ3NDEKTSV4RRFFQ69G5FAV --evaluation evaluation.json --approver alice --approval-at 2026-02-30 --json"],
            }],
        ),
        (
            "strategy rollback",
            &[Validator {
                arguments: &["approval-at"],
                code: "USAGE",
                description: super::CALENDAR,
                rejects: &["canon strategy rollback 01ARZ3NDEKTSV4RRFFQ69G5FAV --reason \"regressed in production\" --actor alice --signature-file approval.sig --approval-at 2026-02-30"],
            }],
        ),
    ],
    domains: &[
        // The id of a task, read by the handler (the operand has no clap parser).
        DomainRow::new(Scope::Arguments(&["task done"], "task"), Domain::matching(TASK_ID), "GATE_TASK_INVALID_ARGUMENT").inside(&["add-login#1", "add-login#1.2.3", "a#007"]).outside(&["Add#1", "add-login", "add-login#", "add-login#1.", "add-login#a", "#1"]),
        // The subject's domain: a kebab slug (its membership of the repo's vocabulary is state).
        DomainRow::new(Scope::Arguments(&["subject new"], "domain"), Domain::matching(KEBAB), "SUBJECT_INVALID_ARGUMENT").inside(&["dev"]).outside(&["Dev", "a_b", "-a", "a-", "a--b", "d e", ""]),
        // A reason is one non-blank line.
        DomainRow::new(Scope::Arguments(&["subject move"], "reason"), Domain::matching(REASON), "SUBJECT_INVALID_ARGUMENT").inside(&["ok", "a\u{85}b", "a\u{2028}b", " a "]).outside(&["", " ", "\t", "a\nb", "a\rb", "a\n", "\u{85}"]),
        DomainRow::new(Scope::Arguments(&["subject move"], "override-reason"), Domain::matching(REASON), "SUBJECT_INVALID_ARGUMENT").inside(&["ok", "a\u{85}b", " a "]).outside(&["", " ", "\t", "a\nb", "a\rb", "a\n", "\u{85}"]),
        // The tag of a scenario may carry the `@` the scenario bodies write it with; it is the parser's, not the shared grammar's.
        DomainRow::new(Scope::Arguments(&["scenario new"], "tag"), Domain::matching(SCENARIO_TAG), "USAGE").inside(&["auth.login.01", "@auth.login.01", "a-.-b.123"]).outside(&["auth.login.1", "@@auth.login.01", "Auth.login.01", "auth.login", "a.b.c.01", "@"]),
        DomainRow::new(Scope::Arguments(&["scenario new"], "lane"), Domain::matching(KEBAB), "USAGE").inside(&["a", "a-1"]).outside(&["A", "a_b", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::Arguments(&["scenario new"], "case"), Domain::matching(KEBAB), "USAGE").inside(&["a", "a-1"]).outside(&["A", "a_b", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::Arguments(&["feature new"], "surface"), Domain::matching(SURFACE), "USAGE").inside(&["auth.login", "a-.-b", "1.2"]).outside(&["auth", "auth.login.01", "Auth.login", ".login", "auth.", "a_b.c", ""]),
        // The answer of an approval: the human's own words, one line once trimmed.
        DomainRow::new(Scope::Arguments(&["brief approve"], "approver"), Domain::matching(ANSWER), "APPROVAL_INCOMPLETE").inside(&["Ana", " Ana ", "Ana\n", "a\u{85}b"]).outside(&["", " ", "\t", "\n", "a\nb", "a\rb", " a\nb "]),
        DomainRow::new(Scope::Arguments(&["brief approve"], "note"), Domain::matching(ANSWER), "APPROVAL_INCOMPLETE").inside(&["Go.", " go ", "go\n", "a\u{85}b"]).outside(&["", " ", "\t", "\n", "a\nb", "a\rb", " a\nb "]),
        DomainRow::new(Scope::Arguments(&["policy approve"], "approver"), Domain::matching(NOT_BLANK), "POLICY_APPROVAL_REFUSED").inside(&["Ana", " a ", "a\nb"]).outside(&["", " ", "\t", "\n", "\u{85}"]),
        // The tool and the actor of a relayed approval.
        DomainRow::new(Scope::Arguments(&["brief approve", "policy approve"], "relayed-via"), Domain::matching(RELAY_TOKEN), "USAGE").inside(&["ask", "AskUserQuestion", "chat"]).outside(&["", " ", "a b", "a(b", "a)b", "a\"b", "a'b", "a\tb", "a\nb"]),
        DomainRow::new(Scope::Arguments(&["brief approve", "policy approve"], "actor"), Domain::matching(RELAY_ACTOR), "USAGE").inside(&["impl", " impl ", "impl\n"]).outside(&["", " ", "\t", "a b", "a(b", "a)b", "a\"b", "a'b", " a b "]),
        // A shown digest: `<change>=sha256:<digest>`.
        DomainRow::new(Scope::Arguments(&["brief approve"], "digest"), Domain::matching(DIGEST), "USAGE").inside(&["add-login=sha256:0123456789abcdef", "add-login=sha256:"]).outside(&["add-login", "=sha256:aa", "add-login=aa", "add-login=SHA256:aa", "add-login=sha256", ""]),
        // `NAME@VERSION`, by clap's parser.
        DomainRow::new(Scope::Arguments(&["prompt add", "prompt show"], "bundle"), Domain::matching(NAME_AT_VERSION), "USAGE").inside(&["reviewer@v1", "a@b", "a b@c d"]).outside(&["reviewer", "@v1", "reviewer@", "a@b@c", "@", ""]),
        // The id of a context pack, read by the handler.
        DomainRow::new(Scope::Arguments(&["context-pack show", "context-pack verify"], "id"), Domain::matching(PACK_ID), "USAGE")
            .inside(&["sha256:0000000000000000000000000000000000000000000000000000000000000000", "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"])
            .outside(&["", "sha256:", "sha256:abc", "sha256:000000000000000000000000000000000000000000000000000000000000000", "sha256:00000000000000000000000000000000000000000000000000000000000000000", "sha256:ABCDEF0000000000000000000000000000000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "sha256:g000000000000000000000000000000000000000000000000000000000000000"]),
    ],
};
