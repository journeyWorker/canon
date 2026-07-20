//! The DuckDB query driver: shells out to the real `duckdb` CLI with
//! `canon_store::VIEWS_SQL` as its `-init` file — the identical
//! `duckdb -init sql/views.sql` invocation
//! `crates/canon-store/tests/e2e_write_age_query_duckdb.rs` already
//! established (module doc there: "Open the DuckDB views against the
//! SAME fixture roots"). `canon-report` never links a `duckdb` Rust
//! crate (none exists as a workspace dependency, verified across every
//! `Cargo.toml` in this workspace, 2026-07-11) — the CLI subprocess IS
//! the query surface S2 built and tested against, so this driver
//! reuses it rather than introducing a second, untested DuckDB
//! binding.

use std::io::Write as _;
use std::process::Command;

use crate::error::ReportError;
use crate::roots::Roots;

/// One row of a query result — `-json` output mode (verified against a
/// real `duckdb -json` run, 2026-07-11) parses directly into
/// `serde_json::Value`, so callers extract whatever columns their mart
/// query selected without a second typed-row layer here.
pub type Row = serde_json::Map<String, serde_json::Value>;

fn duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

/// Builds the base `duckdb -init <views.sql> [-json]` `Command` every
/// caller needs — env vars set, `roots` seeded first
/// ([`Roots::ensure_seeded`]) so an empty r2/learn source never aborts
/// an unrelated query (module doc of [`crate::roots`]). Returns the
/// backing [`tempfile::NamedTempFile`] alongside the `Command` — it
/// must outlive the `duckdb` invocation, since `-init` takes a
/// filename, never inline SQL.
fn base_command(roots: &Roots, json: bool) -> Result<(Command, tempfile::NamedTempFile), ReportError> {
    if !duckdb_available() {
        return Err(ReportError::DuckDbMissing);
    }
    roots.ensure_seeded()?;

    let mut init_file = tempfile::Builder::new().prefix("canon-report-views-").suffix(".sql").tempfile()?;
    init_file.write_all(canon_store::VIEWS_SQL.as_bytes())?;
    init_file.flush()?;

    let mut cmd = Command::new("duckdb");
    cmd.arg("-init").arg(init_file.path());
    if json {
        cmd.arg("-json");
    }
    for (key, value) in roots.env_pairs() {
        cmd.env(key, value);
    }
    Ok((cmd, init_file))
}

/// Runs `sql` against `canon_store::VIEWS_SQL` opened over `roots`,
/// returning every row `-json` mode printed. `roots` is seeded first
/// ([`Roots::ensure_seeded`]) so an empty r2/learn source never aborts
/// an unrelated query (module doc of [`crate::roots`]).
pub fn run_query(roots: &Roots, sql: &str) -> Result<Vec<Row>, ReportError> {
    let (mut cmd, _init_file) = base_command(roots, true)?;
    // `-init` takes a FILENAME, never inline SQL — the embedded view
    // layer is written to a temp file per call rather than assuming a
    // caller-relative `sql/views.sql` path exists on disk (this crate
    // is invoked from an arbitrary cwd, unlike the canon-store test
    // that already runs from its own `CARGO_MANIFEST_DIR`).
    cmd.arg("-c").arg(sql);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(ReportError::QueryFailed { stderr: String::from_utf8_lossy(&output.stderr).into_owned() });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<Row> = serde_json::from_str(trimmed)?;
    Ok(rows)
}

/// Runs one or more `;`-separated SQL statements against
/// `canon_store::VIEWS_SQL` opened over `roots`, discarding any
/// stdout (never `-json` mode — callers that need rows use
/// [`run_query`] instead). [`crate::snapshot`]'s `COPY "<table>" TO
/// '<file>' (FORMAT parquet)` statements are the primary caller: a
/// `COPY` prints nothing on success, only a non-zero exit + stderr on
/// failure.
pub fn run_command(roots: &Roots, sql: &str) -> Result<(), ReportError> {
    let (mut cmd, _init_file) = base_command(roots, false)?;
    cmd.arg("-c").arg(sql);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(ReportError::QueryFailed { stderr: String::from_utf8_lossy(&output.stderr).into_owned() });
    }
    Ok(())
}
