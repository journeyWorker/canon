//! Rows of the commands that author and approve: briefs, subjects, scenarios, policy, strategies, prompts, context packs.

use atip::constraint::Rule;

use super::Tables;

pub(super) static TABLES: Tables = Tables {
    rules: &[
        ("brief approve", &[Rule::requires("actor", &["relayed-via"]), Rule::requires("digest", &["relayed-via"])]),
        ("policy approve", &[Rule::requires("actor", &["relayed-via"])]),
        ("scenario new", &[Rule::requires("change", &["flow"])]),
        ("strategy promote", &[Rule::requires("evaluation", &["approval"]), Rule::requires("approval", &["evaluation"]), Rule::requires("signature-file", &["approval"])]),
    ],
    validators: &[],
    domains: &[],
};
