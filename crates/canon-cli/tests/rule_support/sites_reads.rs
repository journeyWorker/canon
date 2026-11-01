//! The classified sites of this group (see `sites.rs`).

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    site("dashboard_briefs.rs:status_of:APPROVAL_INCOMPLETE#1", Kind::Other("the HTTP status the dashboard gives a refusal code (a table of codes to 400), not a check of an argument")),
    site("init.rs:run_init_in:INIT_INVALID_ARGUMENT#1", Kind::Domain { command: "init", argument: "language" }),
    site("init.rs:run_init_in:INIT_INVALID_ARGUMENT#2", Kind::Precondition("`--providers` is parsed only when canon.yaml does not exist yet (choose_skills); with a canon.yaml it is ignored with a note, and the prompt path reads the terminal")),
    site("input.rs:expand:USAGE#1", Kind::Other("`--input` is expanded to argv before clap runs, for every command that has it: a second `--input` is a fault of the expansion's own syntax")),
    site("input.rs:expand:USAGE#2", Kind::Other("`--input` is expanded to argv before clap runs: an `--input` without a value is a fault of the expansion's own syntax")),
    site("input.rs:run_batch:USAGE#1", Kind::Undeclared("`--fields` with a batch: the refusal depends on the SHAPE of the `--input` value (an array or several lines is a batch, one object is not), which no rule or domain of an option says; the paging change's (spec 6.10); ATIP feedback: a `forbids` that reads the shape of a value")),
    site("input.rs:expand:USAGE#3", Kind::Undeclared("`--input -` and another stdin-reading option cannot both read stdin: a relation between `--input` and an option that differs per command, found while expanding argv before clap; ATIP feedback: no rule says 'these two read the same stream'")),
    site("fields.rs:refusal:USAGE#1", Kind::Other("the paging change's: `--fields` takes the paths a command declares under `outputs[].fields.paths` (spec 10.9.1), a closed set that belongs to the output, not a domain, a constraint or a validator of the argument")),
    site("query.rs:to_cli_error:QUERY_INVALID_ARGUMENT#1", Kind::Rule { command: "query", rule: "forbids", arguments: &["kind", "change"] }),
    site("query.rs:to_cli_error:QUERY_INVALID_ARGUMENT#2", Kind::Validator { command: "query", arguments: &["kind", "status"] }),
    site("retention.rs:check_window:USAGE#1", Kind::Validator { command: "export", arguments: &["after", "before"] }),
    site("run/entry.rs:usage_error:USAGE#1", Kind::Other("the refusal of a removed spelling of an option: it maps a fault, it does not check an argument of a command")),
    site("run/entry.rs:usage_parts:USAGE#1", Kind::Other("the mapping of clap's own parse error to the USAGE refusal")),
    site("run/entry.rs:usage_parts:USAGE#2", Kind::Other("the mapping of clap's own parse error (a bare group of commands) to the USAGE refusal")),
    site("run/entry.rs:usage_parts:USAGE#3", Kind::Other("the mapping of clap's own 'required arguments were not provided' error to the USAGE refusal")),
    site("run/skills.rs:skills_error:SKILLS_INVALID_ARGUMENT#1", Kind::Domain { command: "skills install", argument: "providers" }),
    site("run/skills.rs:skills_error:SKILLS_INVALID_ARGUMENT#2", Kind::Precondition("a malformed `skills:` section of canon.yaml")),
    site("run/skills.rs:skills_error:SKILLS_INVALID_ARGUMENT#3", Kind::Precondition("`--skills-source` is not a directory, or the directory holds no SKILL.src.md: reads the file system")),
    site("run/top.rs:run_query:QUERY_INVALID_ARGUMENT#1", Kind::Precondition("`--include-sensitive` without `query.allow_sensitive: true` in .canon/policy.yaml: reads the policy file")),
    site("run/top.rs:run_regime_show:REGIME_KEY_INVALID#1", Kind::Domain { command: "regime show", argument: "hash" }),
    site("run/top.rs:run_retrieve:USAGE#1", Kind::Rule { command: "retrieve", rule: "exactlyOne", arguments: &["regime", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#2", Kind::Rule { command: "retrieve", rule: "exactlyOne", arguments: &["regime", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#3", Kind::Rule { command: "retrieve", rule: "requires", arguments: &["subject", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#4", Kind::Validator { command: "retrieve", arguments: &["for-role", "regime"] }),
    site("search/mod.rs:run_search:SEARCH_QUERY_EMPTY#1", Kind::Domain { command: "search", argument: "text" }),
    site("search/mod.rs:run_trace:TRACE_OPERAND_INVALID#1", Kind::Domain { command: "trace", argument: "id" }),
];
