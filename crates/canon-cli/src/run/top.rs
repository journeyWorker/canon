//! Handlers of the top-level leaf commands.

use canon_model::envelope::RecordKind;
use canon_model::{ChangeId, RegimeKey, RoleId, SubjectId, regime_key};
use chrono::{DateTime, Utc};
use crate::cli_error::CliError;
use std::process::ExitCode;

/// `plugin: None` calls the EXACT SAME [`crate::query::run`] path this
/// function used before sensitive query output was added. The selected
/// formatter receives the already-policy-validated `include_sensitive` bit,
/// while the default formatter wrappers remain redacted. `plugin: Some(id)`
/// calls [`crate::query::run_with_plugin`] instead; every diagnostic it
/// returns is printed to stderr regardless of whether a projection actually
/// resolved, and stdout uses the matching core/overlay formatter with the
/// same sensitive-output authorization.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_query(
    repo: &std::path::Path,
    kind: RecordKind,
    since: Option<DateTime<Utc>>,
    json: bool,
    plugin: Option<String>,
    history: bool,
    change_id: Option<ChangeId>,
    status: Option<String>,
    domain: Option<String>,
    include_sensitive: bool,
    page: crate::paging::Page,
    fields: Option<Vec<String>>,
) -> ExitCode {
    // s19 `query-scope-filters` design D5: kind-gating + status-domain
    // validation runs BEFORE any tier read (task 3.2/3.3) -- a usage
    // fault here is a clean, nothing-read `2`, never a store error.
    if let Err(e) = crate::query::validate_scope(kind, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        e.to_cli_error().emit("canon query", json);
        return ExitCode::from(2);
    }

    if let Err(err) = crate::retention::validate_sensitive(repo, include_sensitive) {
        CliError::refused("QUERY_INVALID_ARGUMENT", err.to_string()).hint("`canon schema query` lists the accepted arguments and their forms").emit("canon query", json);
        return ExitCode::from(2);
    }
    let Some(plugin_id) = plugin else {
        return match crate::query::run(repo, kind, since, change_id.as_ref(), status.as_deref(), domain.as_deref(), history) {
            Ok(outcome) => {
                let outcome = match crate::query::page_outcome(outcome, &page, fields, false) {
                    Ok(outcome) => outcome,
                    Err(err) => return ExitCode::from(err.emit("canon query", json) as u8),
                };
                let _ = crate::retention::audit_query(repo, kind, since, include_sensitive, outcome.records.len());
                if json {
                    print_query(&crate::query::report(&outcome, include_sensitive), outcome.page.as_ref(), &[]);
                } else {
                    print!("{}", crate::query::format_human_with_sensitive(&outcome, include_sensitive));
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                ExitCode::from(CliError::failed("QUERY_FAILED", err.to_string()).hint("run `canon inventory sync` to rebuild the index, then run the query again").emit("canon query", json) as u8)
            }
        };
    };

    match crate::query::run_with_plugin(repo, kind, since, &plugin_id, change_id.as_ref(), status.as_deref(), domain.as_deref()) {
        Ok((outcome, plugin_outcome)) => {
            let outcome = match crate::query::page_outcome(outcome, &page, fields, true) {
                Ok(outcome) => outcome,
                Err(err) => return ExitCode::from(err.emit("canon query", json) as u8),
            };
            let _ = crate::retention::audit_query(repo, kind, since, include_sensitive, outcome.records.len());
            for msg in &plugin_outcome.diagnostics {
                crate::say_err!(json, "canon query --plugin {plugin_id}: {msg}");
            }
            let diagnostics: Vec<String> = plugin_outcome.diagnostics.iter().map(|msg| format!("canon query --plugin {plugin_id}: {msg}")).collect();
            if plugin_outcome.projections.is_empty() {
                if json {
                    print_query(&crate::query::report(&outcome, include_sensitive), outcome.page.as_ref(), &diagnostics);
                } else {
                    print!("{}", crate::query::format_human_with_sensitive(&outcome, include_sensitive));
                }
            } else if json {
                print_query(&crate::query::report_with_overlay(&outcome, &plugin_id, &plugin_outcome.projections, include_sensitive), outcome.page.as_ref(), &diagnostics);
            } else {
                print!("{}", crate::query::format_human_with_overlay_and_sensitive(&outcome, &plugin_id, &plugin_outcome.projections, include_sensitive));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            ExitCode::from(CliError::failed("QUERY_FAILED", err.to_string()).hint("run `canon inventory sync` to rebuild the index, then run the query again").emit("canon query", json) as u8)
        }
    }
}

/// `base`, plus how to repair a `missing-provenance` violation when one is
/// among them (#64: editing a generated `.feature` drops the line).
pub(super) fn with_provenance_fix(base: &str, missing_provenance: bool) -> String {
    if missing_provenance {
        format!("{base}; for missing-provenance, {}", canon_fmt::PROVENANCE_FIX)
    } else {
        base.to_string()
    }
}

/// `canon format [<DIR>]`: validates the corpus under `DIR`, or under every
/// configured `specs.roots[]` entry when none is given. A relative `DIR`
/// resolves under `--repo` when that is given, else the current directory.
/// Exit 1 = violations, or a corpus with no file to check (`unverifiable`);
/// exit 2 = `DIR` is not a directory, or the configured roots cannot be read.
pub(super) fn run_fmt(dir: Option<&std::path::Path>, repo: &crate::cli::RepoArgs, json: bool) -> ExitCode {
    let mut coverage = crate::coverage::Coverage::new();
    let mut report = canon_fmt::FmtReport::default();
    let mut check = |id: &str, shown: String, root: &std::path::Path, coverage: &mut crate::coverage::Coverage| {
        let one = crate::fmt::run(root);
        coverage.root(id, shown, one.files_checked);
        report.files_checked += one.files_checked;
        report.violations.extend(one.violations);
    };
    match dir {
        Some(dir) => {
            let resolved = match repo.given() {
                Some(_) if !dir.is_absolute() => repo.join(dir),
                _ => dir.to_path_buf(),
            };
            // A corpus root that is not a directory holds no files to find wrong: it must not read as clean.
            if !resolved.is_dir() {
                let error = CliError::refused("REPO_UNREADABLE", format!("`{}` is not a directory, so there is no corpus to check", resolved.display()))
                    .field("dir")
                    .hint("pass the corpus directory, e.g. `canon format .canon/specs`, relative to the current directory or to --repo");
                return ExitCode::from(error.emit("canon format", json) as u8);
            }
            check("dir", dir.display().to_string(), &resolved, &mut coverage);
        }
        None => {
            let roots = match crate::inventory::load_spec_roots(&repo.join("canon.yaml")) {
                Ok(roots) => roots,
                Err(err) => {
                    let error = CliError::refused("CONFIG_UNREADABLE", err.to_string()).hint("fix `specs.roots` in canon.yaml (`canon init --check` reports what is wrong), or pass the corpus directory: `canon format <DIR>`");
                    return ExitCode::from(error.emit("canon format", json) as u8);
                }
            };
            for root in roots {
                let shown = root.root.strip_prefix(&**repo).unwrap_or(&root.root).display().to_string();
                if root.root.is_dir() {
                    check(root.id.as_str(), shown, &root.root, &mut coverage);
                } else {
                    coverage.skip(shown, format!("spec root `{}` is not a directory", root.id.as_str()));
                }
            }
        }
    }
    coverage.count("files", report.files_checked);
    crate::say_raw!(json, "{}", crate::fmt::format_human(&report));
    let unverifiable = report.is_clean() && coverage.examined_nothing();
    let data = crate::fmt::format_json(&report, unverifiable);
    if unverifiable {
        let error = crate::coverage::unverifiable_error("canon format", "0 files", "author a spec with `canon feature new`, or pass the corpus directory: `canon format <DIR>`");
        return ExitCode::from(error.emit_with_data_and_meta("canon format", json, &data, coverage.meta()) as u8);
    }
    if report.is_clean() {
        if json {
            crate::json_out::data_with_meta("format", &data, coverage.meta());
        }
        ExitCode::SUCCESS
    } else {
        let missing_provenance = report.violations.iter().any(|v| v.class == canon_fmt::FmtFailureClass::MissingProvenance);
        let hint = with_provenance_fix("fix each violation named in data.violations, then run `canon format` again", missing_provenance);
        let error = CliError::failed("FORMAT_VIOLATIONS", format!("{} violation(s) in {} checked file(s)", report.violations.len(), report.files_checked)).hint(hint);
        ExitCode::from(error.emit_with_data_and_meta("canon format", json, &data, coverage.meta()) as u8)
    }
}

/// `canon init [--repo <dir>] [--no-agents-md] [--policy <level> | --no-policy]` / `canon
/// init --check` (s19 P4, `crate::init`'s module doc):
/// `check_config: false` scaffolds a fresh repo, or on an existing
/// `canon.yaml` refreshes only the AGENTS.md block (see
/// `crate::init::run_init` for the exit codes); `check_config: true`
/// READ-ONLY validates an existing config instead (`2` on a missing
/// file, `0` when every present section parses clean, `1` when a
/// present section fails).
pub(super) fn run_init(repo: &std::path::Path, check_config: bool, options: crate::init::InitOptions, json: bool) -> ExitCode {
    let code = if check_config { crate::init::run_check_config(repo, json) } else { crate::init::run_init(repo, options, json) };
    ExitCode::from(code as u8)
}

/// A capability query, never validation (invariant 1): always
/// `ExitCode::SUCCESS`, mirroring `resolve_surface`'s own infallibility —
/// there is no corpus check here to fail against. `repo` is first resolved
/// via [`crate::context::resolve_repo_root`] (design D7, task 1.4) —
/// the `--repo`-omitted/`--repo .` nearest-`canon.yaml` ancestor walk —
/// before [`crate::context::resolve_surface`] ever reads it.
pub(super) fn run_context(repo: &std::path::Path, json: bool, fields: Option<Vec<String>>, role: Option<crate::role::Role>) -> ExitCode {
    if let Some(requested) = &fields {
        if let Err(e) = crate::paging::check_known("canon context", requested, &crate::context::SURFACE_FIELDS) {
            return ExitCode::from(e.emit("canon context", json) as u8);
        }
    }
    let repo = crate::context::resolve_repo_root(repo);
    let surface = crate::context::resolve_surface(&repo, crate::context::ContextOptions { role });
    if json {
        match &fields {
            Some(requested) => {
                let projected = crate::paging::project(&serde_json::to_value(&surface).expect("the surface serializes"), requested);
                crate::json_out::data("context", &crate::json_out::Projected::<crate::context::ContextReport>::new(projected));
            }
            None => crate::json_out::data("context", &surface),
        }
    } else {
        print!("{}", crate::context::render_outline(&surface, fields.as_deref()));
    }
    ExitCode::SUCCESS
}

/// `canon retrieve` (S8 part2, design.md decision 3): once
/// `crate::retrieve::run` clears the `--role`/`--regime` usage
/// precondition, this ALWAYS exits `0` — a store outage or malformed
/// row degrades to empty guidance internally (`retrieve_guidance`'s own
/// fail-soft contract), never surfaced as a nonzero exit here. The
/// ONLY nonzero exit is the usage precondition itself (`--role`
/// disagreeing with `--regime`'s own leading segment), reported and
/// exiting `2` (mirrors `canon gate check`'s own 0-clean/1-red/2-usage
/// convention) — never reachable via `retrieve_guidance`, which has no
/// error channel at all (`crate::retrieve`'s own module doc).
pub(super) fn run_retrieve(repo: &std::path::Path, role: &RoleId, regime: Option<&RegimeKey>, domain: Option<&str>, subject: Option<&SubjectId>, k: Option<usize>, json: bool) -> ExitCode {
    match crate::retrieve::run_scoped(repo, role, regime, domain, subject, k) {
        Ok(o) => {
            if json {
                let warnings = crate::retrieve::serving_note(&o).map(|note| vec![note]).unwrap_or_default();
                crate::json_out::data_with_meta("retrieve", &crate::retrieve::RetrieveReport(&o.guidance), crate::json_out::warnings_meta(&warnings));
            } else {
                print!("{}", crate::retrieve::format_human_scoped(role, &o));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            ExitCode::from(CliError::refused("RETRIEVE_FAILED", e.to_string()).hint("check --role and --regime (`canon regime show` prints a valid key) and that the ledger is readable").emit("canon retrieve", json) as u8)
        }
    }
}

/// `canon regime show` (S8 `s8-retrieve-before-task` whole-branch-review
/// fix): serialize + VALIDATE one canonical `regime_key` so shell hooks
/// route through the identical `canon_model::ids::regime_key`
/// normalizer the Rust write path uses, never a second derivation
/// (design decision 1). Prints the validated key and exits `0`; on a
/// malformed result (empty segment / bad `<hash>`, which `regime_key`
/// can still produce — see its doc) reports to stderr and exits `2`
/// with nothing on stdout, so the hook's own `|| exit 0` degrades it to
/// a silent no-op rather than passing a malformed `--regime` on.
pub(super) fn run_regime_show(role: &str, repo: &str, area: &str, hash: &str, json: bool) -> ExitCode {
    match RegimeKey::parse(regime_key(role, repo, area, hash)) {
        Ok(valid) => {
            if json {
                crate::json_out::data("regime show", &crate::retrieve::RegimeShowReport { regime_key: valid.as_str() });
            } else {
                println!("{}", valid.as_str());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            ExitCode::from(CliError::refused("REGIME_KEY_INVALID", err.to_string()).hint("a regime key is <role>/<repo>/<area>/<hash>; `canon regime show --help` lists the inputs").emit("canon regime show", json) as u8)
        }
    }
}

/// `canon report` (S9 part2, tasks.md 3.1): resolves `--repo` +
/// `canon-report`'s `Roots` (`crate::report::resolve_inputs`) then
/// dispatches to exactly one of `canon-report`'s three library entry
/// points — never reimplements any of them (module doc of
/// `crate::report`). `--snapshot <dir>` takes priority when given
/// (module doc of the `Report` clap variant); otherwise `--check`
/// surfaces `canon_report::CheckOutcome::exit_code()` UNCHANGED (`0`
/// no-drift / `1` `MISSING`/`DRIFT`, design D2); the flagless default
/// writes the report. Before any of those three modes, prints a
/// one-line stderr `canon report: WARN …` naming any record kind
/// routed to a backend that is not read directly by the report (s25
/// `report-pg-tier-boundary` design D3/D4, s27 `tier-role-backend-
/// split` design D2, s28 `rung-backend-capability` design D2/D3) —
/// computed via the SAME
/// `canon_report::tier_boundary::kinds_not_read_directly`
/// derivation the written report's own `## Kinds not read directly`
/// section reads, so the two can never disagree; silent for a repo
/// with nothing routed to a backend that is not read directly.
pub(super) fn run_report(repo: &std::path::Path, check: bool, snapshot_dir: Option<&std::path::Path>, json: bool) -> ExitCode {
    let command = if check {
        "canon report check"
    } else if snapshot_dir.is_some() {
        "canon report snapshot"
    } else {
        "canon report build"
    };
    // A present but unusable `canon.yaml` is a usage error (exit 2),
    // the same refusal `canon gate check` makes — never a report of
    // the default ledger.
    let (repo, inputs) = match crate::report::resolve_inputs(repo) {
        Ok(resolved) => resolved,
        Err(err) => {
            return ExitCode::from(CliError::refused("CONFIG_UNREADABLE", err.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit(command, json) as u8);
        }
    };

    let kinds_not_read_directly = canon_report::tier_boundary::kinds_not_read_directly(&repo);
    let mut report_warnings = Vec::new();
    if let Some(msg) = canon_report::tier_boundary::warn_line(&kinds_not_read_directly) {
        crate::say_err!(json, "canon report: WARN {msg}");
        report_warnings.push(format!("canon report: WARN {msg}"));
    }

    if let Some(dir) = snapshot_dir {
        return match canon_report::snapshot(&inputs, dir) {
            Ok(manifest) => {
                crate::say!(json, "canon report snapshot: wrote {} table(s) to {}", manifest.tables.len(), dir.display());
                if json {
                    let tables: Vec<_> = manifest.tables.iter().map(|t| crate::report::SnapshotTable { table: &t.table, file: &t.file }).collect();
                    crate::json_out::data_with_meta("report snapshot", &crate::report::ReportSnapshotReport { directory: dir.display().to_string(), tables }, crate::json_out::warnings_meta(&report_warnings));
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                ExitCode::from(CliError::failed("REPORT_FAILED", err.to_string()).hint("check that the ledger is readable (`canon gate check` shows its state), then run the command again").emit("canon report snapshot", json) as u8)
            }
        };
    }

    let report_path = crate::report::default_report_path(&repo);

    if check {
        return match canon_report::check_report(&inputs, &report_path) {
            Ok(outcome) => {
                crate::say_err!(json, "{}", outcome.message(&report_path));
                let state = match outcome {
                    canon_report::CheckOutcome::NoDrift => "fresh",
                    canon_report::CheckOutcome::Missing => "missing",
                    canon_report::CheckOutcome::Drift => "drift",
                };
                let data = crate::report::ReportCheckReport { state, path: report_path.display().to_string() };
                if outcome.exit_code() == 0 {
                    if json {
                        crate::json_out::data_with_meta("report check", &data, crate::json_out::warnings_meta(&report_warnings));
                    }
                    ExitCode::SUCCESS
                } else {
                    let error = CliError::failed("REPORT_STALE", format!("the report is {state}")).hint("regenerate it with `canon report build`");
                    ExitCode::from(error.emit_with_data("canon report check", json, &data) as u8)
                }
            }
            Err(err) => {
                ExitCode::from(CliError::failed("REPORT_FAILED", err.to_string()).hint("check that the ledger is readable (`canon gate check` shows its state), then run the command again").emit("canon report check", json) as u8)
            }
        };
    }

    match canon_report::write_report(&inputs, &report_path) {
        Ok(_content) => {
            crate::say!(json, "canon report build: wrote {}", report_path.display());
            if json {
                crate::json_out::data_with_meta("report build", &crate::report::ReportBuildReport { path: report_path.display().to_string() }, crate::json_out::warnings_meta(&report_warnings));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            ExitCode::from(CliError::failed("REPORT_FAILED", err.to_string()).hint("check that the ledger is readable (`canon gate check` shows its state), then run the command again").emit("canon report build", json) as u8)
        }
    }
}

/// `canon dashboard` (S9 part3, tasks.md 6.1): resolves + (re)generates
/// the snapshot and binds the static server (`crate::dashboard::prepare`,
/// module doc for the default-vs-explicit `--snapshot` rule), then serves
/// forever — this subcommand never returns on success; the process exits
/// only via the standard SIGINT/SIGTERM default handler (no signal
/// handling installed, matching every other local dev-server tool).
pub(super) fn run_dashboard(repo: &std::path::Path, snapshot: Option<&std::path::Path>, port: u16, json: bool) -> ExitCode {
    match crate::dashboard::prepare(repo, snapshot, port) {
        Ok(bound) => {
            crate::say!(json, "canon dashboard: app       = {}", bound.app.describe());
            crate::say!(json, "canon dashboard: snapshot  = {}", bound.snapshot_dir.display());
            let mut warnings = Vec::new();
            if let crate::dashboard::SnapshotState::Unavailable { reason, fix } = &bound.snapshot {
                let warning = format!("{reason}; the panels page says so instead of showing panels; to fix it: {fix}");
                crate::say_err!(json, "canon dashboard: warning: {warning}");
                warnings.push(warning);
            }
            crate::say!(json, "canon dashboard: briefs    = {}", bound.briefs_url());
            crate::say!(json, "canon dashboard: serving {}", bound.url());
            if json {
                // The one document: where it serves. The server then runs until the process is stopped.
                let data = crate::dashboard::DashboardReport {
                    url: bound.url(),
                    briefs_url: bound.briefs_url(),
                    app: bound.app.describe(),
                    snapshot: bound.snapshot_dir.display().to_string(),
                    snapshot_available: bound.snapshot == crate::dashboard::SnapshotState::Ready,
                };
                crate::json_out::data_with_meta("dashboard", &data, crate::json_out::warnings_meta(&warnings));
            }
            bound.serve_forever();
            ExitCode::SUCCESS
        }
        Err(err) => ExitCode::from(err.to_cli_error().emit("canon dashboard", json) as u8),
    }
}

/// Prints one query document as the envelope: the page state goes into `meta`,
/// beside the command's `warnings`.
fn print_query(report: &crate::query::QueryReport, page: Option<&crate::paging::PageMeta>, warnings: &[String]) {
    let mut meta = serde_json::Map::new();
    if let Some(page) = page {
        page.insert_into(&mut meta);
    }
    if !warnings.is_empty() {
        meta.insert("warnings".to_string(), serde_json::to_value(warnings).expect("strings serialize"));
    }
    let meta = if meta.is_empty() { serde_json::Value::Null } else { serde_json::Value::Object(meta) };
    crate::json_out::data_with_meta("query", report, meta);
}

/// `canon completion <SHELL>`: text mode prints the script itself, so
/// `canon completion zsh > _canon` is the whole installation; `--json`
/// envelopes it as `{shell, script}`.
pub(super) fn run_completion(shell: crate::cli::CompletionShell, json: bool) -> ExitCode {
    let report = crate::completion::script(shell);
    if json {
        crate::json_out::data("completion", &report);
    } else {
        print!("{}", report.script);
    }
    ExitCode::SUCCESS
}
