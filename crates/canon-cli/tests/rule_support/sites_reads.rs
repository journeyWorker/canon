//! The classified sites of this group (see `sites.rs`).

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    site("dashboard_briefs.rs:status_of:APPROVAL_INVALID#1", Kind::Other("the HTTP status the dashboard gives a refusal code (a table of codes to 400), not a check of an argument")),
    site("init.rs:run_init_in:INIT_ARGUMENT_INVALID#1", Kind::Domain { command: "init", argument: "language" }),
    site("input.rs:expand:USAGE#1", Kind::Other("`--input` is expanded to argv before clap runs, for every command that has it: a second `--input` is a fault of the expansion's own syntax")),
    site("input.rs:expand:USAGE#2", Kind::Other("`--input` is expanded to argv before clap runs: an `--input` without a value is a fault of the expansion's own syntax")),
    site("input.rs:run_batch:USAGE#1", Kind::Other("`--fields` with a batch: the output of a batch declares no `fields` (spec 6.10, 10.9.1), which is where the contract says it; the paging change's")),
    site("input.rs:expand:USAGE#3", Kind::Other("`--input -` beside another stdin reader: a violation of 5.5 (one stdin, two readers), `USAGE` naming the option given later; no document declares it (the ATIP author's ruling: not a validator)")),
    site("fields.rs:refusal:USAGE#1", Kind::Other("the paging change's: `--fields` takes the paths a command declares under `outputs[].fields.paths` (spec 10.9.1), a closed set that belongs to the output, not a domain, a constraint or a validator of the argument")),
    site("query.rs:to_cli_error:QUERY_ARGUMENT_INVALID#1", Kind::Rule { command: "query", rule: "forbids", arguments: &["kind", "change"] }),
    site("query.rs:to_cli_error:QUERY_ARGUMENT_INVALID#2", Kind::Validator { command: "query", arguments: &["kind", "status"] }),
    site("retention.rs:check_window:USAGE#1", Kind::Validator { command: "export", arguments: &["after", "before"] }),
    site("run/entry.rs:usage_error:USAGE#1", Kind::Other("the refusal of a removed spelling of an option: it maps a fault, it does not check an argument of a command")),
    site("run/entry.rs:help_json_refusal:USAGE#1", Kind::Other("the refusal of `--help` or `--version` with `--json` (spec 4.4.1): it maps a request for text to the envelope, it does not check an argument of a command")),
    site("run/entry.rs:usage_parts:USAGE#1", Kind::Other("the mapping of clap's own parse error to the USAGE refusal")),
    site("run/entry.rs:usage_parts:USAGE#2", Kind::Other("the mapping of clap's own parse error (a bare group of commands) to the USAGE refusal")),
    site("run/entry.rs:usage_parts:USAGE#3", Kind::Other("the mapping of clap's own 'required arguments were not provided' error to the USAGE refusal")),
    site("run/skills.rs:skills_error:SKILLS_ARGUMENT_INVALID#1", Kind::Precondition("the install lock names a provider canon does not know: reads the install lock, so it is no argument of the command line")),
    site("run/top.rs:run_query:QUERY_ARGUMENT_INVALID#1", Kind::Precondition("`--include-sensitive` without `query.allow_sensitive: true` in .canon/policy.yaml: reads the policy file")),
    site("run/top.rs:run_regime_show:REGIME_KEY_INVALID#1", Kind::Domain { command: "regime show", argument: "hash" }),
    site("run/top.rs:run_retrieve:USAGE#1", Kind::Rule { command: "retrieve", rule: "exactlyOne", arguments: &["regime", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#2", Kind::Rule { command: "retrieve", rule: "exactlyOne", arguments: &["regime", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#3", Kind::Rule { command: "retrieve", rule: "requires", arguments: &["subject", "domain"] }),
    site("run/top.rs:run_retrieve:USAGE#4", Kind::Validator { command: "retrieve", arguments: &["for-role", "regime"] }),
    site("search/mod.rs:run_search:SEARCH_QUERY_REQUIRED#1", Kind::Domain { command: "search", argument: "text" }),
    site("search/mod.rs:run_trace:TRACE_OPERAND_INVALID#1", Kind::Domain { command: "trace", argument: "trace" }),
];
