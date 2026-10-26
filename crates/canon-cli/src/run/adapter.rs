//! `canon adapter` — the handlers of the group.

use canon_model::envelope::RecordKind;
use crate::cli::adapter::{AdapterCommand, AdapterValidateArgs, AdapterAuthorizeArgs};
use crate::cli_error::CliError;
use std::process::ExitCode;

pub fn run(cmd: AdapterCommand) -> ExitCode {
    match cmd {
        AdapterCommand::Validate(AdapterValidateArgs { response, repo, json }) => run_adapter_validate(&response, &repo, json),
        AdapterCommand::Authorize(AdapterAuthorizeArgs { response, repo, json }) => run_adapter_authorize(&response, &repo, json),
    }
}

/// `canon adapter validate` is deliberately read-only: it parses one response
/// envelope and prints a redacted summary. Provider execution and capability
/// enforcement remain outside canon.
fn run_adapter_validate(response: &std::path::Path, repo: &std::path::Path, json: bool) -> ExitCode {
    let mut summary = match crate::adapter::validate_response(response) {
        Ok(summary) => summary,
        Err(error) => {
            // A missing or unreadable file is a refusal (2); an envelope that does not validate is the check's finding (1).
            let refused = matches!(error, crate::adapter::AdapterError::Io { .. });
            let message = error.to_string();
            let error = if refused {
                CliError::refused("ADAPTER_REFUSED", message).with_exit(crate::cli_error::EXIT_REFUSED).hint("check that the --response path names a readable file; `canon adapter validate --response <file>` re-checks it")
            } else {
                CliError::failed("ADAPTER_REFUSED", message).hint("fix the response envelope the message names; `canon adapter validate --response <file>` re-checks it")
            };
            return ExitCode::from(error.emit("canon adapter validate", json) as u8);
        }
    };

    // The context-pack join needs canon state: it runs when a canon.yaml is found (given or discovered) and is skipped otherwise.
    if repo.join("canon.yaml").is_file() {
        summary.context_join_verified = Some(verify_adapter_context_join(repo, &summary));
    }

    if json {
        crate::json_out::data("adapter validate", &summary);
    } else {
        println!("adapter response: valid");
        println!("  protocol_version: {}", summary.protocol_version);
        println!("  run_id: {}", summary.run_id);
        println!("  provider: {}", summary.provider);
        println!("  model: {}", summary.model);
        println!("  context_pack_id: {}", summary.context_pack_id);
        println!("  status: {}", format!("{:?}", summary.status).to_lowercase());
        println!("  evidence_refs: {}", summary.evidence_count);
        println!("  extensions: {}", if summary.extension_keys.is_empty() {
            "(none)".to_string()
        } else {
            summary.extension_keys.join(", ")
        });
        if let Some(verified) = summary.context_join_verified {
            println!("  context_join_verified: {verified}");
        }
    }
    ExitCode::SUCCESS
}

fn run_adapter_authorize(response: &std::path::Path, repo: &std::path::Path, json: bool) -> ExitCode {
    let result = match crate::adapter::authorize_response(response, repo) {
        Ok(result) => result,
        Err(error) => {
            // A missing or unreadable file is a refusal (2); an envelope that does not validate is the check's finding (1).
            let refused = matches!(error, crate::adapter::AdapterError::Io { .. });
            let message = error.to_string();
            let error = if refused {
                CliError::refused("ADAPTER_REFUSED", message).with_exit(crate::cli_error::EXIT_REFUSED).hint("check that the --response path names a readable file; `canon adapter validate --response <file>` re-checks it")
            } else {
                CliError::failed("ADAPTER_REFUSED", message).hint("fix the response envelope the message names; `canon adapter validate --response <file>` re-checks it")
            };
            return ExitCode::from(error.emit("canon adapter authorize", json) as u8);
        }
    };
    if json && result.execution_authorized {
        crate::json_out::data("adapter authorize", &result);
    } else if json {
        // The failure keeps the result in `data`.
        let error = CliError::failed("ADAPTER_EXECUTION_NOT_AUTHORIZED", "execution_not_authorized").hint("declare the capability in .canon/policy.yaml and approve it with `canon policy approve --approver <name>`, or drop it from the response");
        return ExitCode::from(error.emit_with_data("canon adapter authorize", true, &result) as u8);
    } else {
        println!("execution_authorized: {}", result.execution_authorized);
        println!("execution_performed: false");
        println!("sandbox_enforced: false");
        for reason in &result.reasons { println!("reason: {reason}"); }
    }
    if result.execution_authorized {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(CliError::failed("ADAPTER_EXECUTION_NOT_AUTHORIZED", "execution_not_authorized").hint("declare the capability in .canon/policy.yaml and approve it with `canon policy approve --approver <name>`, or drop it from the response").emit("canon adapter authorize", false) as u8)
    }
}

fn verify_adapter_context_join(repo: &std::path::Path, summary: &crate::adapter::ValidationSummary) -> bool {
    if crate::context_pack::verify(repo, &summary.context_pack_id).is_err() {
        return false;
    }
    let Ok(outcome) = crate::query::run(repo, RecordKind::Run, None, None, None, None, false) else {
        return false;
    };
    outcome.records.iter().any(|record| {
        record.0.get("run_id").and_then(serde_json::Value::as_str) == Some(summary.run_id.as_str())
            && record
                .0
                .get("lineage")
                .and_then(|lineage| lineage.get("context"))
                .and_then(|context| context.get("pack_id"))
                .and_then(serde_json::Value::as_str)
                == Some(summary.context_pack_id.as_str())
    })
}
