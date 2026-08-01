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
//!
//! # One report, one corpus
//! Two reads of a live ledger are two different corpora, so a surface
//! rendering several numbers a reader will compare against each other
//! runs ONE pinned batch ([`run_pinned_queries`] /
//! [`run_pinned_command`]) rather than one process per number. See
//! [`PIN_CORPUS_SQL`] for what pinning does and why one process alone
//! would not be enough.

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

/// The DDL every PINNED invocation ([`run_pinned_queries`],
/// [`run_pinned_command`]) appends to the `-init` file, after
/// `canon_store::VIEWS_SQL`'s own definitions.
///
/// `views.sql` reads the filesystem in exactly FOUR places — the
/// `read_text`/`read_parquet` calls in `stg_git_records`,
/// `stg_r2_records`, `stg_strategy_items` and `stg_trajectories`
/// (verified by grepping every `read_*(`/`glob(` in that file, and the
/// only reason this list can be short enough to be trustworthy). Every
/// `int_*`/`mart_*` view is a pure derivation over those four. So
/// materializing the four into temp tables and repointing the four
/// staging views at them PINS the whole corpus: every mart the batch
/// goes on to compute reads the same rows, and a record written to the
/// ledger after this DDL runs reaches NONE of them.
///
/// Why this is needed at all: a DuckDB glob is re-expanded and the
/// files re-read for every statement that touches a staging view, so
/// "one process" alone would not make two marts agree — nor would one
/// UNION-ALL statement, whose per-scan binds are separate reads of the
/// same directory. Pinning is the step that makes the panels one
/// computation over one input; the process boundary is only what makes
/// pinning possible, since a temp table dies with its process.
///
/// `CREATE OR REPLACE VIEW` over a view other views already select
/// from re-binds those dependents by NAME at query time — verified
/// against `duckdb` v1.5.4 directly: after this DDL, deleting a row
/// from `_canon_pinned_git_records` changes `mart_review_rounds`,
/// which is only possible if the mart is reading the temp table rather
/// than re-globbing the ledger.
const PIN_CORPUS_SQL: &str = "
CREATE TEMP TABLE _canon_pinned_git_records   AS SELECT * FROM stg_git_records;
CREATE TEMP TABLE _canon_pinned_r2_records    AS SELECT * FROM stg_r2_records;
CREATE TEMP TABLE _canon_pinned_strategies    AS SELECT * FROM stg_strategy_items;
CREATE TEMP TABLE _canon_pinned_trajectories  AS SELECT * FROM stg_trajectories;
CREATE OR REPLACE VIEW stg_git_records   AS SELECT * FROM _canon_pinned_git_records;
CREATE OR REPLACE VIEW stg_r2_records    AS SELECT * FROM _canon_pinned_r2_records;
CREATE OR REPLACE VIEW stg_strategy_items AS SELECT * FROM _canon_pinned_strategies;
CREATE OR REPLACE VIEW stg_trajectories  AS SELECT * FROM _canon_pinned_trajectories;
";

/// Builds the base `duckdb -init <views.sql> [-json]` `Command` every
/// caller needs — env vars set, `roots` seeded first
/// ([`Roots::ensure_seeded`]) so an empty r2/learn source never aborts
/// an unrelated query (module doc of [`crate::roots`]). Returns the
/// backing [`tempfile::NamedTempFile`] alongside the `Command` — it
/// must outlive the `duckdb` invocation, since `-init` takes a
/// filename, never inline SQL.
///
/// `pin_corpus` appends [`PIN_CORPUS_SQL`] to that same init file. A
/// failing statement in an init file is fatal to `duckdb` (non-zero
/// exit + stderr, verified directly), so a pin that could not be taken
/// surfaces as a [`ReportError::QueryFailed`] rather than as a silent
/// fall-back to unpinned reads.
fn base_command(roots: &Roots, json: bool, pin_corpus: bool) -> Result<(Command, tempfile::NamedTempFile), ReportError> {
    if !duckdb_available() {
        return Err(ReportError::DuckDbMissing);
    }
    roots.ensure_seeded()?;

    let mut init_file = tempfile::Builder::new().prefix("canon-report-views-").suffix(".sql").tempfile()?;
    init_file.write_all(canon_store::VIEWS_SQL.as_bytes())?;
    if pin_corpus {
        init_file.write_all(PIN_CORPUS_SQL.as_bytes())?;
    }
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

fn parse_rows(stdout: &str) -> Result<Vec<Row>, ReportError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(trimmed)?)
}

/// Runs ONE `sql` statement against `canon_store::VIEWS_SQL` opened
/// over `roots`, returning every row `-json` mode printed. `roots` is
/// seeded first ([`Roots::ensure_seeded`]) so an empty r2/learn source
/// never aborts an unrelated query (module doc of [`crate::roots`]).
///
/// Deliberately UNPINNED: a single statement has nothing to be
/// inconsistent with, and pinning would materialize the whole corpus
/// to serve one read. Two numbers that a reader will compare must come
/// from [`run_pinned_queries`] instead.
pub fn run_query(roots: &Roots, sql: &str) -> Result<Vec<Row>, ReportError> {
    let (mut cmd, _init_file) = base_command(roots, true, false)?;
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

    parse_rows(&String::from_utf8_lossy(&output.stdout))
}

/// Runs `statements` — one row-returning statement per element — in
/// ONE `duckdb` process over ONE materialized read of the corpus
/// ([`PIN_CORPUS_SQL`]), returning one row set per statement, in
/// order.
///
/// This is the read every multi-number surface uses. `-json` mode
/// prints one JSON array per result-producing statement, including an
/// empty `[]` for a statement that matched nothing (verified against
/// `duckdb` v1.5.4), so the arrays align one-to-one with `statements`
/// and a mismatch is a hard [`ReportError::ResultSetArity`] rather
/// than a silently shifted mart. Every element must therefore produce
/// EXACTLY one result set: a `COPY`/DDL statement prints nothing and
/// would shift the whole batch.
pub fn run_pinned_queries<S: AsRef<str>>(roots: &Roots, statements: &[S]) -> Result<Vec<Vec<Row>>, ReportError> {
    let (mut cmd, _init_file) = base_command(roots, true, true)?;
    let script = statements.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("\n");
    cmd.arg("-c").arg(script);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(ReportError::QueryFailed { stderr: String::from_utf8_lossy(&output.stderr).into_owned() });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sets = Vec::with_capacity(statements.len());
    for set in serde_json::Deserializer::from_str(stdout.trim()).into_iter::<Vec<Row>>() {
        sets.push(set?);
    }
    if sets.len() != statements.len() {
        return Err(ReportError::ResultSetArity { want: statements.len(), got: sets.len() });
    }
    Ok(sets)
}

/// Runs one or more `;`-separated SQL statements against
/// `canon_store::VIEWS_SQL` opened over `roots`, discarding any
/// stdout (never `-json` mode — callers that need rows use
/// [`run_query`] instead).
///
/// UNPINNED, like [`run_query`] and for the same reason: a caller
/// whose statements must all see one corpus uses
/// [`run_pinned_command`].
pub fn run_command(roots: &Roots, sql: &str) -> Result<(), ReportError> {
    run(roots, sql, false)
}

/// [`run_command`] over ONE materialized read of the corpus
/// ([`PIN_CORPUS_SQL`]) — [`crate::snapshot`]'s nine `COPY "<table>"
/// TO '<file>' (FORMAT parquet)` statements are the caller: nine
/// parquet files exported from one input, so no two of them can
/// disagree the way nine separately-globbed exports could. A `COPY`
/// prints nothing on success, only a non-zero exit + stderr on
/// failure, which is why this returns no rows.
pub fn run_pinned_command(roots: &Roots, sql: &str) -> Result<(), ReportError> {
    run(roots, sql, true)
}

fn run(roots: &Roots, sql: &str, pin_corpus: bool) -> Result<(), ReportError> {
    let (mut cmd, _init_file) = base_command(roots, false, pin_corpus)?;
    cmd.arg("-c").arg(sql);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(ReportError::QueryFailed { stderr: String::from_utf8_lossy(&output.stderr).into_owned() });
    }
    Ok(())
}
