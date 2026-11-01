//! Rows of the commands that record: evidence, reviews, divergences, findings, dispatch.

use atip::constraint::Rule;

use super::Tables;

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("divergence accept", &[Rule::requires("approver", &["relayed-via"]), Rule::requires("note", &["relayed-via"])]),
        ("evidence add", &[Rule::at_least_one(&["class", "report"]), Rule::at_least_one(&["ref", "report"])]),
    ],
    validators: &[],
    domains: &[],
};
