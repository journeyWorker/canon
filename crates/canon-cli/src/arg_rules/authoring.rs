//! Rows of the commands that author and approve: briefs, subjects, scenarios, policy, strategies, prompts, context packs.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, ACTOR_DESCRIPTION, ANSWER_RELAYED, KEBAB, ROLE_DESCRIPTION, TASK_ID, VIA_DESCRIPTION};

/// A scenario tag as `scenario new` reads it: `<area>.<surface>.<nn>` with at most one leading `@` (the form a scenario
/// body writes it in).
const SCENARIO_TAG: &str = "^@?[a-z0-9-]+\\.[a-z0-9-]+\\.[0-9]{2,}$";
/// `<area>.<surface>`: exactly one `.`, two `[a-z0-9-]+` words.
const SURFACE: &str = "^[a-z0-9-]+\\.[a-z0-9-]+$";
/// `NAME@VERSION`: both parts non-empty, no second `@`.
const NAME_AT_VERSION: &str = "^[^@]+@[^@]+$";
/// A context pack id: `sha256:` and 64 lower-case hex digits.
const PACK_ID: &str = "^sha256:[0-9a-f]{64}$";
/// One `--digest` entry: `<change>=sha256:<digest>`, the change not empty.
const DIGEST: &str = "^[^=]+=sha256:[^\u{0}]*$";

pub(crate) static TABLES: Tables = Tables {
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
                    arguments: &["relayed-via", "as"],
                    code: "USAGE",
                    description: ROLE_DESCRIPTION,
                    rejects: &["canon brief approve add-login --approver alice --note ok --relayed-via ask --actor impl --digest add-login=sha256:0123456789abcdef"],
                },
                Validator {
                    arguments: &["relayed-via"],
                    code: "USAGE",
                    description: VIA_DESCRIPTION,
                    rejects: &["canon brief approve add-login --approver alice --note ok --relayed-via \"a b\" --as implementer --actor impl --digest add-login=sha256:0123456789abcdef", "canon brief approve add-login --approver alice --note ok --relayed-via \"a(b\" --as implementer --actor impl --digest add-login=sha256:0123456789abcdef"],
                },
                Validator {
                    arguments: &["relayed-via", "actor"],
                    code: "USAGE",
                    description: ACTOR_DESCRIPTION,
                    rejects: &["canon brief approve add-login --approver alice --note ok --relayed-via ask --as implementer --digest add-login=sha256:0123456789abcdef", "canon brief approve add-login --approver alice --note ok --relayed-via ask --as implementer --actor \"a b\" --digest add-login=sha256:0123456789abcdef"],
                },
                Validator {
                    arguments: &["relayed-via", "approver", "note"],
                    code: "USAGE",
                    description: ANSWER_RELAYED,
                    rejects: &["canon brief approve add-login --approver alice --note \" \" --relayed-via ask --as implementer --actor impl --digest add-login=sha256:0123456789abcdef"],
                },
                Validator {
                    arguments: &["note"],
                    code: "APPROVAL_INVALID",
                    description: "The human's own words (`--note`) are one line and, written by the human without `--relayed-via`, not blank once trimmed (a blank answer under `--relayed-via` is the relayed validator's, `USAGE`; the name, `--approver`, is the domain of its own).",
                    rejects: &["canon brief approve add-login --approver alice --note \" \""],
                },
                Validator {
                    arguments: &["relayed-via", "digest", "change"],
                    code: "USAGE",
                    description: "A relayed approval names, for every change it approves, the digest the human was shown (`--digest <change>=sha256:<digest>`, once per change) and no digest of a change it does not approve. With no change operand the repo's only live change is read, and its digest is checked then.",
                    rejects: &[
                        "canon brief approve add-login --approver alice --note ok --relayed-via ask --as implementer --actor impl",
                        "canon brief approve add-login --approver alice --note ok --relayed-via ask --as implementer --actor impl --digest add-login=sha256:0123456789abcdef --digest other=sha256:0123456789abcdef",
                    ],
                },
                Validator {
                    arguments: &["digest"],
                    code: "USAGE",
                    description: "`--digest` names a change once: two entries for the same change must carry the same digest.",
                    rejects: &["canon brief approve add-login --approver alice --note ok --relayed-via ask --as implementer --actor impl --digest add-login=sha256:aa --digest add-login=sha256:bb"],
                },
            ],
        ),
        (
            "policy approve",
            &[
                Validator {
                    arguments: &["relayed-via", "as"],
                    code: "USAGE",
                    description: ROLE_DESCRIPTION,
                    rejects: &["canon policy approve --approver alice --note ok --relayed-via ask --actor impl"],
                },
                Validator {
                    arguments: &["relayed-via"],
                    code: "USAGE",
                    description: VIA_DESCRIPTION,
                    rejects: &["canon policy approve --approver alice --note ok --relayed-via \"a b\" --as implementer --actor impl", "canon policy approve --approver alice --note ok --relayed-via \"a(b\" --as implementer --actor impl"],
                },
                Validator {
                    arguments: &["relayed-via", "actor"],
                    code: "USAGE",
                    description: ACTOR_DESCRIPTION,
                    rejects: &["canon policy approve --approver alice --note ok --relayed-via ask --as implementer", "canon policy approve --approver alice --note ok --relayed-via ask --as implementer --actor \"a b\""],
                },
                Validator {
                    arguments: &["relayed-via", "approver", "note"],
                    code: "USAGE",
                    description: ANSWER_RELAYED,
                    rejects: &["canon policy approve --approver alice --relayed-via ask --as implementer --actor impl --note \" \""],
                },
            ],
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
        DomainRow::new(Scope::Arguments(&["task done"], "task"), Domain::matching(TASK_ID), "TASK_ID_INVALID").inside(&["add-login#1", "add-login#1.2.3", "a#007"]).outside(&["Add#1", "add-login", "add-login#", "add-login#1.", "add-login#a", "#1"]),
        // The tag of a scenario may carry the `@` the scenario bodies write it with; it is the parser's, not the shared grammar's.
        DomainRow::new(Scope::Arguments(&["scenario new"], "tag"), Domain::matching(SCENARIO_TAG), "USAGE").inside(&["auth.login.01", "@auth.login.01", "a-.-b.123"]).outside(&["auth.login.1", "@@auth.login.01", "Auth.login.01", "auth.login", "a.b.c.01", "@"]),
        DomainRow::new(Scope::Arguments(&["scenario new"], "lane"), Domain::matching(KEBAB), "USAGE").inside(&["a", "a-1"]).outside(&["A", "a_b", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::Arguments(&["scenario new"], "case"), Domain::matching(KEBAB), "USAGE").inside(&["a", "a-1"]).outside(&["A", "a_b", "-a", "a-", "a--b", ""]),
        DomainRow::new(Scope::Arguments(&["feature new"], "surface"), Domain::matching(SURFACE), "USAGE").inside(&["auth.login", "a-.-b", "1.2"]).outside(&["auth", "auth.login.01", "Auth.login", ".login", "auth.", "a_b.c", ""]),
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
