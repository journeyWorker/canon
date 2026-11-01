//! Rows of the commands that read, retain and maintain: query, export, purge, retrieve, ingest, migrate, init, skills.

use atip::constraint::Rule;

use super::Tables;

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("ingest plans", &[Rule::requires("dialect", &["plan-source"]), Rule::requires("plan-source", &["dialect"])]),
        ("migrate", &[Rule::requires("backup", &["revert"]), Rule::requires("relayed-via", &["commit"]), Rule::requires("actor", &["relayed-via"]), Rule::requires("approver", &["relayed-via"]), Rule::requires("note", &["relayed-via"])]),
    ],
    validators: &[],
    domains: &[],
};
