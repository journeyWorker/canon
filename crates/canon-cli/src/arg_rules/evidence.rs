//! Rows of the commands that record: evidence, reviews, divergences, findings, dispatch.

use atip::constraint::{Rule, Validator};
use atip::domain::Domain;

use super::{DomainRow, Scope, Tables, ANSWER_RELAYED, STDIN_DESCRIPTION, CALENDAR, ROLE_DESCRIPTION, NOT_BLANK, ONE_LINE, ONE_LINE_NOT_BLANK, SESSION_ID, SHA256_HEX, TRIMMED_ONE_LINE};

/// A text that neither starts nor ends with white space, is not empty, and carries no `\n` and no `\r`: the approver of
/// `evidence add` (the handler refuses only those two line breaks, unlike the shared `TRIMMED_ONE_LINE`).
const APPROVER: &str = concat!("^[^", white_space!(), "]([^\\n\\r]*[^", white_space!(), "])?$");
/// A risk surface that is not empty, is trimmed and carries no `\n`, `\r` or NUL (the shape every `--surface-ref` has; the
/// `effect:` form and the path rules are the validator's).
const SURFACE_REF: &str = concat!("^[^", white_space!(), "]([^\\n\\r\u{0}]*[^", white_space!(), "])?$");
/// A `--report` is `<junit|cucumber|swift-testing>:<path>` with a path.
const REPORT: &str = "^(junit|cucumber|swift-testing):[^\u{0}]+$";
/// A `--prompt-bundle` is `NAME@VERSION`: a name and a version, neither empty, the version without a second `@`.
const PROMPT_BUNDLE: &str = "^[^@]+@[^@]+$";
/// A runtime label (`--reviewer-provider`, `--reviewer-model`, `--reviewer-harness`): not empty, trimmed, no control character.
const RUNTIME_LABEL: &str = SESSION_ID;

/// The message the commands that take `--approver` give for a partial approval.
const APPROVAL_TOGETHER: &str = "The four approval options (`--approver`, `--approval-role`, `--signature-file`, `--approval-at`) go together: giving one needs the other three.";

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("divergence accept", &[Rule::requires("approver", &["relayed-via"]), Rule::requires("note", &["relayed-via"])]),
        ("divergence add", &[Rule::requires_in("state", &["deferred"], &["reason", "expiry"]).code("DIVERGENCE_INVALID_ARGUMENT").help("A deferred divergence records why it is deferred and when the deferral lapses.")]),
        (
            "evidence add",
            &[
                Rule::at_least_one(&["class", "report"]),
                Rule::at_least_one(&["ref", "report"]),
                Rule::at_least_one(&["task", "scenario", "all-scenarios"]).code("EVIDENCE_INVALID_ARGUMENT").help("A record needs a coverage subject: a task, a scenario, or one record per scenario of a report."),
                Rule::forbids("all-scenarios", &["scenario", "task", "report-case", "approver", "approval-role", "signature-file", "approval-at"])
                    .code("EVIDENCE_INVALID_ARGUMENT")
                    .help("--all-scenarios writes one record per scenario its report owns, so it names no single subject."),
                Rule::requires("all-scenarios", &["report"]).code("EVIDENCE_INVALID_ARGUMENT").help("--all-scenarios splits a --report by the scenario each case belongs to."),
                Rule::requires("report-case", &["report"]).code("EVIDENCE_INVALID_ARGUMENT").help("--report-case names a case of a --report."),
                Rule::requires("command-result", &["summary"]).code("EVIDENCE_INVALID_ARGUMENT").help("The captured output backs a --summary."),
                Rule::requires("approver", &["approval-role", "signature-file", "approval-at"]).code("EVIDENCE_APPROVAL_INVALID").help(APPROVAL_TOGETHER),
                Rule::requires("approval-role", &["approver", "signature-file", "approval-at"]).code("EVIDENCE_APPROVAL_INVALID").help(APPROVAL_TOGETHER),
                Rule::requires("signature-file", &["approver", "approval-role", "approval-at"]).code("EVIDENCE_APPROVAL_INVALID").help(APPROVAL_TOGETHER),
                Rule::requires("approval-at", &["approver", "approval-role", "signature-file"]).code("EVIDENCE_APPROVAL_INVALID").help(APPROVAL_TOGETHER),
            ],
        ),
        (
            "evidence audit",
            &[
                Rule::requires("report-case", &["report"]).code("EVIDENCE_INVALID_ARGUMENT").help("--report-case names a case of a --report."),
                Rule::requires("command-result", &["summary"]).code("EVIDENCE_INVALID_ARGUMENT").help("The captured output backs a --summary."),
            ],
        ),
        ("evidence approval-payload", &[Rule::at_least_one(&["task", "scenario"]).code("EVIDENCE_REFUSED").help("An approval is for a task or a scenario.")]),
        (
            "finding add",
            &[
                Rule::at_most_one(&["introduced-rev", "introduced-unknown"]).code("FINDING_INVALID_ARGUMENT").help("A finding names the commit that introduced the defect or says it is not known, never both."),
                Rule::requires("resolution-rev", &["disposition"]).code("FINDING_RESOLUTION_REV_UNEXPECTED").help("Only a `fixed` finding was closed by a commit."),
                Rule::forbids_in("disposition", &["open", "rejected", "deferred"], &["resolution-rev"]).code("FINDING_RESOLUTION_REV_UNEXPECTED").help("Only a `fixed` finding was closed by a commit."),
            ],
        ),
        (
            "finding close",
            &[
                Rule::requires_in("disposition", &["rejected", "deferred"], &["reason"]).code("FINDING_REASON_REQUIRED").help("A rejected or deferred finding records why."),
                Rule::forbids_in("disposition", &["open", "rejected", "deferred"], &["resolution-rev"]).code("FINDING_RESOLUTION_REV_UNEXPECTED").help("Only a `fixed` finding was closed by a commit."),
            ],
        ),
        ("dispatch begin", &[Rule::requires("skill-digest", &["skill"]).code("DISPATCH_INVALID_ARGUMENT").help("A skill digest is the digest of a named skill.")]),
    ],
    validators: &[
        (
            "divergence add",
            &[Validator {
                arguments: &["expiry"],
                code: "USAGE",
                description: CALENDAR,
                rejects: &["canon divergence add auth.login.01 --actor alice --role reviewer --expiry 2026-02-30"],
            }],
        ),
        (
            "divergence defer",
            &[Validator {
                arguments: &["expiry"],
                code: "USAGE",
                description: CALENDAR,
                rejects: &["canon divergence defer auth.login.01 --actor alice --reason \"waiting on the vendor\" --expiry 2026-02-30 --role reviewer"],
            }],
        ),
        (
            "evidence add",
            &[Validator {
                arguments: &["input", "report", "artifact"],
                code: "USAGE",
                description: STDIN_DESCRIPTION,
                rejects: &[
                    "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --report junit:- --input -",
                    "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --artifact - --input -",
                ],
            }],
        ),
        (
            "evidence audit",
            &[Validator {
                arguments: &["input", "artifact"],
                code: "USAGE",
                description: STDIN_DESCRIPTION,
                rejects: &["canon evidence audit auth.login.01 --ref audit-report.json --evaluator eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed --role implementer --artifact - --input -"],
            }],
        ),
        (
            "divergence accept",
            &[
                Validator {
                    arguments: &["expiry"],
                    code: "USAGE",
                    description: CALENDAR,
                    rejects: &["canon divergence accept auth.login.01 --actor alice --reason \"the engine drops the event\" --tracking https://example.com/engine/issues/118 --expiry 2026-02-30 --role reviewer"],
                },
                Validator {
                    arguments: &["relayed-via", "as"],
                    code: "USAGE",
                    description: ROLE_DESCRIPTION,
                    rejects: &["canon divergence accept auth.login.01 --actor alice --reason \"the engine drops the event\" --tracking https://example.com/engine/issues/118 --role reviewer --relayed-via ask --approver alice --note ok"],
                },
                Validator {
                    arguments: &["relayed-via", "approver", "note"],
                    code: "USAGE",
                    description: ANSWER_RELAYED,
                    rejects: &[
                        "canon divergence accept auth.login.01 --actor alice --reason \"the engine drops the event\" --tracking https://example.com/engine/issues/118 --role reviewer --relayed-via ask --as implementer --approver \" \" --note ok",
                        "canon divergence accept auth.login.01 --actor alice --reason \"the engine drops the event\" --tracking https://example.com/engine/issues/118 --role reviewer --relayed-via ask --as implementer --approver alice --note \" \"",
                    ],
                },
            ],
        ),
        (
            "divergence list",
            &[Validator { arguments: &["as-of"], code: "USAGE", description: CALENDAR, rejects: &["canon divergence list --as-of 2026-02-30"] }],
        ),
        (
            "evidence add",
            &[
                Validator {
                    arguments: &["approval-at"],
                    code: "USAGE",
                    description: CALENDAR,
                    rejects: &["canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --approver alice --approval-role human --signature-file approval.sig --approval-at 2026-02-30"],
                },
                Validator {
                    arguments: &["surface-ref"],
                    code: "USAGE",
                    description: "A risk surface is `effect:<kebab-slug>` (lowercase letters and digits in hyphen-separated parts) or a repository-relative path: not absolute and with no `..` component.",
                    rejects: &[
                        "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --surface-ref /etc/passwd",
                        "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --surface-ref ../outside",
                        "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --surface-ref effect:Bad",
                    ],
                },
                Validator {
                    arguments: &["class"],
                    code: "EVIDENCE_INVALID_ARGUMENT",
                    description: "`--class audit` is refused: an external evaluator's re-audit is `canon evidence audit`.",
                    rejects: &["canon evidence add --scenario auth.login.01 --class audit --ref \"npm test\" --role implementer"],
                },
                Validator {
                    arguments: &["artifact", "scenario", "all-scenarios"],
                    code: "EVIDENCE_INVALID_ARGUMENT",
                    description: "An `--artifact` binds to one record. Without `--all-scenarios` a scoped `--artifact <scenario-id>=<path>` must name the record's own `--scenario`; with `--all-scenarios` every `--artifact` must be scoped (a bare path would bind to every record of the batch). The part before the first `=` is a scope only when it is shaped like a scenario id.",
                    rejects: &[
                        "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --artifact auth.login.02=shot.png",
                        "canon evidence add --all-scenarios --report junit:report.xml --role implementer --artifact shot.png",
                    ],
                },
            ],
        ),
        (
            "evidence audit",
            &[Validator {
                arguments: &["artifact", "scenario"],
                code: "EVIDENCE_INVALID_ARGUMENT",
                description: "An `--artifact` binds to one record: a scoped `--artifact <scenario-id>=<path>` must name the audited scenario. The part before the first `=` is a scope only when it is shaped like a scenario id.",
                rejects: &["canon evidence audit auth.login.01 --ref audit-report.json --evaluator eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed --role implementer --artifact auth.login.02=shot.png"],
            }],
        ),
        (
            "evidence approval-payload",
            &[
                Validator {
                    arguments: &["approval-at"],
                    code: "USAGE",
                    description: CALENDAR,
                    rejects: &["canon evidence approval-payload --scenario auth.login.01 --project root --approver alice --approval-at 2026-02-30"],
                },
                Validator {
                    arguments: &["surface-ref"],
                    code: "USAGE",
                    description: "A risk surface is `effect:<kebab-slug>` (lowercase letters and digits in hyphen-separated parts) or a repository-relative path: not absolute and with no `..` component.",
                    rejects: &[
                        "canon evidence approval-payload --scenario auth.login.01 --project root --approver alice --approval-at 2030-01-01T00:00:00Z --surface-ref /etc/passwd",
                        "canon evidence approval-payload --scenario auth.login.01 --project root --approver alice --approval-at 2030-01-01T00:00:00Z --surface-ref ../outside",
                        "canon evidence approval-payload --scenario auth.login.01 --project root --approver alice --approval-at 2030-01-01T00:00:00Z --surface-ref effect:Bad",
                    ],
                },
            ],
        ),
        (
            "finding add",
            &[
                Validator {
                    arguments: &["severity", "introduced-rev", "introduced-unknown"],
                    code: "FINDING_INTRODUCED_REQUIRED",
                    description: "A new `blocker` or `should-fix` finding needs `--introduced-rev` (the commit that introduced the defect) or `--introduced-unknown` (it is not known); a `note` needs neither.",
                    rejects: &["canon finding add --change add-login --round 1 --seq 2 --severity blocker --actor alice --summary \"Token is logged\""],
                },
                Validator {
                    arguments: &["introduced-rev"],
                    code: "USAGE",
                    description: "`--introduced-rev` names the commit that introduced the defect: the value `self` is refused (to record a defect found in one's own work, set `--actor`).",
                    rejects: &["canon finding add --change add-login --round 1 --seq 2 --severity note --actor alice --summary \"Token is logged\" --introduced-rev self"],
                },
            ],
        ),
        (
            "dispatch begin",
            &[Validator {
                arguments: &["role", "regime"],
                code: "DISPATCH_ROLE_REGIME_MISMATCH",
                description: "`--role` must equal the first segment of `--regime` (`<role>/<repo>/<area>/<hash>`).",
                rejects: &["canon dispatch begin --regime implementer/my-repo/api/abcdef012345 --role reviewer"],
            }],
        ),
    ],
    domains: &[
        // evidence add / audit: the checks run_add makes on the strings it records.
        DomainRow::new(Scope::Arguments(&["evidence add"], "class"), Domain::matching(NOT_BLANK), "EVIDENCE_INVALID_ARGUMENT").inside(&["test-run", " a "]).outside(&["", " ", "\t"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence audit"], "ref"), Domain::matching(NOT_BLANK), "EVIDENCE_INVALID_ARGUMENT").inside(&["npm test", " a "]).outside(&["", " ", "\t"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence audit"], "summary"), Domain::matching(ONE_LINE), "EVIDENCE_INVALID_ARGUMENT").inside(&["Tests pass", ""]).outside(&["a\nb", "a\rb", "a\u{2028}b", "a\u{85}b"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence audit"], "actor"), Domain::matching(ONE_LINE), "EVIDENCE_INVALID_ARGUMENT").inside(&["alice", ""]).outside(&["a\nb", "a\u{b}b", "a\u{2029}b"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence audit"], "override-reason"), Domain::matching(ONE_LINE_NOT_BLANK), "EVIDENCE_INVALID_ARGUMENT").inside(&["the human is away", " a "]).outside(&["", " ", "a\nb"]),
        DomainRow::new(Scope::Arguments(&["evidence audit"], "evaluator"), Domain::matching(TRIMMED_ONE_LINE), "EVIDENCE_INVALID_ARGUMENT").inside(&["eval-1", "a b"]).outside(&["", " a", "a ", "a\nb"]),
        DomainRow::new(Scope::Arguments(&["evidence audit"], "evaluator-version"), Domain::matching(TRIMMED_ONE_LINE), "EVIDENCE_INVALID_ARGUMENT").inside(&["2.1", "a b"]).outside(&["", " 1", "1 ", "1\n2"]),
        DomainRow::new(Scope::Arguments(&["evidence audit"], "evaluator-digest"), Domain::matching(SHA256_HEX), "EVIDENCE_INVALID_ARGUMENT")
            .inside(&["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"])
            .outside(&["", "0123456789abcdef", "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef", "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0"]),
        DomainRow::new(Scope::Arguments(&["evidence add"], "approver"), Domain::matching(APPROVER), "EVIDENCE_APPROVAL_INVALID").inside(&["alice", "a b", "a\u{2028}b"]).outside(&["", " a", "a ", "a\nb", "a\rb"]),
        DomainRow::new(Scope::Arguments(&["evidence approval-payload"], "approver"), Domain::matching(TRIMMED_ONE_LINE), "EVIDENCE_REFUSED").inside(&["alice", "a b"]).outside(&["", " a", "a ", "a\nb", "a\u{2028}b"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence approval-payload"], "surface-ref"), Domain::matching(SURFACE_REF), "USAGE").inside(&["src/lib.rs", "effect:payments", "a b"]).outside(&["", " a", "a ", "a\nb", "a\rb"]),
        DomainRow::new(Scope::Arguments(&["evidence add", "evidence audit"], "report"), Domain::matching(REPORT), "USAGE").inside(&["junit:r.xml", "cucumber:r.json", "swift-testing:e.jsonl", "junit:a:b"]).outside(&["r.xml", "junit:", "mocha:r.xml", ":r.xml", "JUnit:r.xml"]),
        // review: the runtime labels clap's value parser reads.
        DomainRow::new(Scope::Arguments(&["review scenario", "review change"], "reviewer-provider"), Domain::matching(RUNTIME_LABEL), "USAGE").inside(&["anthropic", "a b"]).outside(&["", " a", "a ", "a\tb", "a\nb"]),
        DomainRow::new(Scope::Arguments(&["review scenario", "review change"], "reviewer-model"), Domain::matching(RUNTIME_LABEL), "USAGE").inside(&["claude-sonnet-5", "a b"]).outside(&["", " a", "a ", "a\tb", "a\nb"]),
        DomainRow::new(Scope::Arguments(&["review scenario", "review change"], "reviewer-harness"), Domain::matching(RUNTIME_LABEL), "USAGE").inside(&["omp", "a b"]).outside(&["", " a", "a ", "a\tb", "a\nb"]),
        // divergence accept.
        DomainRow::new(Scope::Arguments(&["divergence accept"], "reason"), Domain::matching(NOT_BLANK), "DIVERGENCE_INVALID_ARGUMENT").inside(&["the engine drops the event", " a "]).outside(&["", " ", "\t"]),
        DomainRow::new(Scope::Arguments(&["divergence accept"], "tracking"), Domain::matching(NOT_BLANK), "DIVERGENCE_INVALID_ARGUMENT").inside(&["https://example.com/issues/1", " a "]).outside(&["", " ", "\t"]),
        DomainRow::new(Scope::Arguments(&["divergence accept"], "role"), Domain::matching("^reviewer$"), "DIVERGENCE_ACCEPT_REFUSED").inside(&["reviewer"]).outside(&["implementer", "planner"]),
        // finding add / close.
        DomainRow::new(Scope::Arguments(&["finding add", "finding close"], "round"), Domain::at_least(1), "FINDING_INVALID_ARGUMENT"),
        DomainRow::new(Scope::Arguments(&["finding add", "finding close"], "seq"), Domain::at_least(1), "FINDING_INVALID_ARGUMENT"),
        DomainRow::new(Scope::Arguments(&["finding add"], "actor"), Domain::matching(ONE_LINE_NOT_BLANK), "FINDING_INVALID_ARGUMENT").inside(&["alice", " a "]).outside(&["", " ", "a\nb", "a\u{2028}b"]),
        DomainRow::new(Scope::Arguments(&["finding add"], "summary"), Domain::matching(ONE_LINE_NOT_BLANK), "FINDING_INVALID_ARGUMENT").inside(&["Token is logged", " a "]).outside(&["", " ", "a\nb", "a\u{2029}b"]),
        DomainRow::new(Scope::Arguments(&["finding add"], "file-ref"), Domain::matching(ONE_LINE), "FINDING_INVALID_ARGUMENT").inside(&["src/main.rs:120-134", ""]).outside(&["a\nb", "a\u{85}b"]),
        DomainRow::new(Scope::Arguments(&["finding add"], "introduced-rev"), Domain::matching(NOT_BLANK), "USAGE").inside(&["HEAD~1", " a "]).outside(&["", " ", "\t"]),
        DomainRow::new(Scope::Arguments(&["finding close"], "reason"), Domain::matching(ONE_LINE_NOT_BLANK), "FINDING_REASON_REQUIRED").inside(&["works as designed", " a "]).outside(&["", " ", "a\nb", "a\u{2028}b"]),
        // dispatch begin.
        DomainRow::new(Scope::Arguments(&["dispatch begin"], "prompt-bundle"), Domain::matching(PROMPT_BUNDLE), "USAGE").inside(&["bundle@1", "a@b.c"]).outside(&["bundle", "@1", "bundle@", "a@b@c", ""]),
    ],
};
