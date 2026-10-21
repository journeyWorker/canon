//! `canon dashboard [--repo <dir>] [--snapshot <dir>] [--port <n>]` (S9
//! part3, `s9-unified-surface` tasks.md 6.1; 0.25 D2/D3): serves the
//! dashboard app locally, in any repo, pointed at a real snapshot via the
//! app's own `?snapshot=` override (`packages/dashboard/src/main.ts`), and
//! serves the briefs page's two endpoints ([`crate::dashboard_briefs`]).
//! This module never re-implements snapshot/report generation (design D1) —
//! it calls straight into [`canon_report::snapshot`], the exact library
//! entry point [`crate::report`]'s `canon report --snapshot` arm wires.
//!
//! # Where the app comes from ([`resolve_app`])
//! 1. `<repo>/packages/dashboard/dist` when `<repo>/packages/dashboard/
//!    package.json` names `@canon-dev/dashboard` and the build is there:
//!    the development override. Never another project's `packages/dashboard`.
//! 2. The app embedded in the binary ([`EMBEDDED_APP`], from
//!    `CANON_DASHBOARD_DIST` at build time, `build.rs`), stored gzip-compressed:
//!    sent as is to a browser that accepts gzip, decompressed for one that
//!    does not.
//! 3. Neither: `DASHBOARD_NOT_BUILT`.
//!
//! Neither source serves `/snapshot/`: the app is built with a demo fixture
//! there, and a served page must never show demo data as the repo's.
//!
//! # Default vs. explicit `--snapshot`
//! `--snapshot` omitted: the snapshot is regenerated fresh, every run, at
//! [`DEFAULT_SNAPSHOT_DIR`]. `--snapshot <dir>` given: served AS-IS when
//! `<dir>` already has a `manifest.json`, else generated there once. A
//! snapshot that cannot be generated (no `duckdb` on `PATH`, say) is not
//! fatal: the server runs, `GET /api/status` says why there is no snapshot
//! and how to get one, and the panels page shows that instead of panels.
//!
//! # Server shape
//! A minimal, dependency-free [`std::net::TcpListener`] server bound to
//! `127.0.0.1` only. One spawned thread per connection, `Connection: close`:
//!
//! - `GET /api/status`, `GET /api/briefs`: JSON in the CLI envelope.
//! - `POST /api/briefs/approve`: records a brief's approval ([`handle_approve`]
//!   lists the guards that run before the body is read).
//! - [`LIVE_SNAPSHOT_ROUTE`] → the snapshot directory's own files.
//! - everything else, `GET` only → the app.
//!
//! # Path resolution is bounded to the served root
//! A file from a directory is canonicalized and REQUIRED to stay under its
//! canonical root ([`bounded`]) before it is read — closing `..` segments, an
//! absolute remainder (which would make `Path::join` discard the root), a
//! leading `//`, and a symlink inside the root that points outside it. A
//! request that resolves outside its root is a 404, like any other miss.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use canon_model::paths;
use serde_json::{json, Value};

use crate::cli_error::CliError;
use crate::dashboard_briefs::{bad_request, Reply};
use crate::report::resolve_inputs;

/// Conventional default snapshot scratch dir (module doc).
pub const DEFAULT_SNAPSHOT_DIR: &str = paths::DASHBOARD_SNAPSHOT_DIR;

/// The built `packages/dashboard` static app, relative to a resolved
/// repo root — `packages/dashboard`'s own `vite build` output
/// (`package.json`'s `build` script, `outDir: "dist"` in
/// `vite.config.ts`).
pub const DASHBOARD_DIST_DIR: &str = "packages/dashboard/dist";

/// The `name` of canon's own dashboard package: only a checkout whose
/// `packages/dashboard/package.json` carries it is served from disk.
pub const DEV_PACKAGE_NAME: &str = "@canon-dev/dashboard";

/// The URL path prefix the live snapshot is served under (module doc).
pub const LIVE_SNAPSHOT_ROUTE: &str = "/live-snapshot/";

/// The demo fixture at the root of the built app: never served.
const DEMO_FIXTURE: &str = "snapshot";

/// The write endpoint's body limit.
const MAX_BODY: usize = 64 * 1024;
/// Longest request line or header line read.
const MAX_LINE: u64 = 16 * 1024;
/// Most header lines read.
const MAX_HEADERS: usize = 100;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// One file of the app embedded in the binary: its path under the app's
/// root and its gzip-compressed bytes.
pub struct EmbeddedFile {
    pub path: &'static str,
    pub gz: &'static [u8],
}

// `pub static EMBEDDED_APP: &[EmbeddedFile]`, written by `build.rs`: empty
// unless `CANON_DASHBOARD_DIST` named a built app at build time.
include!(concat!(env!("OUT_DIR"), "/dashboard_embedded.rs"));

#[derive(Debug, thiserror::Error)]
pub enum DashboardError {
    /// A present but unusable `canon.yaml` (`crate::report`'s refusal).
    #[error(transparent)]
    Config(#[from] crate::report::ReportConfigError),
    #[error("the dashboard app is not built: this canon binary embeds none, and `{DASHBOARD_DIST_DIR}` under {0} is not a build of canon's dashboard")]
    NotBuilt(PathBuf),
    #[error("the repository {0} does not exist")]
    RepoMissing(PathBuf),
    #[error("port {0} is already in use")]
    PortInUse(u16),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl DashboardError {
    /// The refusal `canon dashboard` prints for this failure.
    pub fn to_cli_error(&self) -> CliError {
        match self {
            DashboardError::Config(_) => CliError::refused("DASHBOARD_CONFIG_INVALID", self.to_string()).hint("fix the dashboard settings in canon.yaml (`canon init --check-config` reports what is wrong)"),
            DashboardError::NotBuilt(_) => CliError::failed("DASHBOARD_NOT_BUILT", self.to_string())
                .hint("install the release build (`bun add -g @journeykit/canon`), which carries the app; or, in a canon checkout, build it (`bun install --frozen-lockfile && bun run --cwd packages/dashboard build`) and run `canon dashboard` from that checkout"),
            DashboardError::RepoMissing(_) => CliError::refused("REPO_UNREADABLE", self.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root"),
            DashboardError::PortInUse(_) => CliError::failed("DASHBOARD_PORT_IN_USE", self.to_string()).hint("pass `--port <n>` to serve on another port, or `--port 0` for any free one"),
            DashboardError::Io(_) => CliError::failed("DASHBOARD_FAILED", self.to_string()).hint("the message names the cause: fix it (check that the repository is readable), then run `canon dashboard` again"),
        }
    }
}

/// Resolves the snapshot directory `canon dashboard` will serve:
/// `--snapshot` verbatim when given, else the conventional
/// [`DEFAULT_SNAPSHOT_DIR`] under the resolved repo root (module doc).
pub fn resolve_snapshot_dir(repo: &Path, snapshot: Option<&Path>) -> PathBuf {
    snapshot.map(Path::to_path_buf).unwrap_or_else(|| repo.join(DEFAULT_SNAPSHOT_DIR))
}

/// Whether the served snapshot can back the panels page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotState {
    Ready,
    /// Why there is none, and what to do about it.
    Unavailable { reason: String, fix: String },
}

impl SnapshotState {
    fn to_json(&self) -> Value {
        match self {
            SnapshotState::Ready => json!({ "available": true, "reason": null, "fix": null }),
            SnapshotState::Unavailable { reason, fix } => json!({ "available": false, "reason": reason, "fix": fix }),
        }
    }
}

/// Regenerates `dir`'s snapshot unconditionally when `explicit` is
/// false (the default scratch dir — module doc), else only when `dir`
/// has no `manifest.json` yet (an explicitly-given dir is the caller's
/// own; served as-is when it already has content, never stomped). A
/// failed generation is the returned state, not an error.
fn ensure_snapshot(inputs: &canon_report::ReportInputs, dir: &Path, explicit: bool) -> SnapshotState {
    if explicit && dir.join("manifest.json").is_file() {
        return SnapshotState::Ready;
    }
    match canon_report::snapshot(inputs, dir) {
        Ok(_) => SnapshotState::Ready,
        Err(e) => {
            let fix = match e {
                canon_report::ReportError::DuckDbMissing => "install DuckDB (https://duckdb.org/docs/installation) so the `duckdb` command is on PATH, then run `canon dashboard` again".to_string(),
                _ => "run `canon report --snapshot <dir>` to see the full error, fix it, then run `canon dashboard` again".to_string(),
            };
            SnapshotState::Unavailable { reason: format!("the snapshot could not be generated: {e}"), fix }
        }
    }
}

/// Where the served app's files come from (module doc).
#[derive(Clone)]
pub enum AppSource {
    /// A built `dist/` directory: the development override.
    Dir(PathBuf),
    /// The table embedded in the binary, gzip-compressed.
    Embedded(&'static [EmbeddedFile]),
}

/// One file of the app, ready to send.
pub struct AppFile {
    pub content_type: &'static str,
    /// Gzip-compressed when `gzipped`, else the file's bytes.
    pub body: Cow<'static, [u8]>,
    pub gzipped: bool,
}

impl AppSource {
    /// Where the app is served from, for the startup lines.
    pub fn describe(&self) -> String {
        match self {
            AppSource::Dir(dir) => dir.display().to_string(),
            AppSource::Embedded(_) => "embedded in the canon binary".to_string(),
        }
    }

    /// The app file a request path names; `None` is a 404. Never the demo
    /// fixture, whichever source.
    pub fn read(&self, path: &str) -> Option<AppFile> {
        let relative = app_path(path)?;
        match self {
            AppSource::Dir(dist) => {
                let file = bounded(dist, &relative)?;
                let from_root = file.strip_prefix(dist.canonicalize().ok()?).ok()?;
                if from_root.components().next().is_some_and(|c| c.as_os_str() == DEMO_FIXTURE) {
                    return None;
                }
                let bytes = std::fs::read(&file).ok()?;
                Some(AppFile { content_type: content_type(&file), body: Cow::Owned(bytes), gzipped: false })
            }
            AppSource::Embedded(table) => {
                let file = table.iter().find(|f| f.path == relative)?;
                Some(AppFile { content_type: content_type(Path::new(file.path)), body: Cow::Borrowed(file.gz), gzipped: true })
            }
        }
    }
}

/// Whether `<dir>/package.json` is canon's own dashboard package.
fn is_canon_dashboard_package(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("package.json")).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()).is_some_and(|package| package["name"] == DEV_PACKAGE_NAME)
}

/// Picks the app to serve (module doc): `repo`'s own build when `repo` is
/// canon's checkout, else `embedded`, else [`DashboardError::NotBuilt`].
pub fn resolve_app(repo: &Path, embedded: &'static [EmbeddedFile]) -> Result<AppSource, DashboardError> {
    let dist = repo.join(DASHBOARD_DIST_DIR);
    if is_canon_dashboard_package(&repo.join("packages/dashboard")) && dist.join("index.html").is_file() {
        return Ok(AppSource::Dir(dist));
    }
    if embedded.iter().any(|f| f.path == "index.html") {
        return Ok(AppSource::Embedded(embedded));
    }
    Err(DashboardError::NotBuilt(repo.to_path_buf()))
}

/// What the handlers share.
struct Server {
    repo: PathBuf,
    app: AppSource,
    snapshot_dir: PathBuf,
    snapshot: SnapshotState,
    port: u16,
    /// One approval at a time: each reads the brief, then rewrites it.
    approvals: Mutex<()>,
}

/// One bound, ready-to-serve dashboard: the resolved app/snapshot (for
/// logging) plus the live [`TcpListener`], already bound — so a caller (or
/// a test) can read the actually-assigned port ([`BoundDashboard::port`])
/// before handing off to [`BoundDashboard::serve_forever`].
pub struct BoundDashboard {
    pub listener: TcpListener,
    pub repo: PathBuf,
    pub app: AppSource,
    pub snapshot_dir: PathBuf,
    pub snapshot: SnapshotState,
}

/// Resolves `--repo`/`--snapshot`, finds the app, (re)generates the
/// snapshot per [`ensure_snapshot`], and binds the listener — everything
/// [`BoundDashboard::serve_forever`] needs, split out so callers (and
/// tests) can bind + query before the infinite serve loop starts.
pub fn prepare(repo: &Path, snapshot: Option<&Path>, port: u16) -> Result<BoundDashboard, DashboardError> {
    prepare_with(repo, snapshot, port, EMBEDDED_APP)
}

fn prepare_with(repo: &Path, snapshot: Option<&Path>, port: u16, embedded: &'static [EmbeddedFile]) -> Result<BoundDashboard, DashboardError> {
    if !repo.is_dir() {
        return Err(DashboardError::RepoMissing(repo.to_path_buf()));
    }
    let (repo, inputs) = resolve_inputs(repo)?;
    let app = resolve_app(&repo, embedded)?;
    let snapshot_dir = resolve_snapshot_dir(&repo, snapshot);
    let state = ensure_snapshot(&inputs, &snapshot_dir, snapshot.is_some());
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| if e.kind() == std::io::ErrorKind::AddrInUse { DashboardError::PortInUse(port) } else { e.into() })?;
    Ok(BoundDashboard { listener, repo, app, snapshot_dir, snapshot: state })
}

impl BoundDashboard {
    /// The actually-bound port — meaningful even when `--port 0` asked
    /// the OS to pick any free port.
    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|addr| addr.port()).unwrap_or(0)
    }

    /// The URL to open: the app root with `?snapshot=` already pointed
    /// at [`LIVE_SNAPSHOT_ROUTE`] (module doc).
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?snapshot={LIVE_SNAPSHOT_ROUTE}", self.port())
    }

    /// The briefs page, where the human approves briefs.
    pub fn briefs_url(&self) -> String {
        format!("http://127.0.0.1:{}/briefs.html", self.port())
    }

    /// Accepts connections forever, one spawned thread per connection
    /// (module doc) — `canon dashboard`'s actual run loop. Never
    /// returns; a caller wanting a bounded lifetime (tests) drives the
    /// listener directly instead of calling this.
    pub fn serve_forever(self) {
        let server = Arc::new(Server { port: self.port(), repo: self.repo, app: self.app, snapshot_dir: self.snapshot_dir, snapshot: self.snapshot, approvals: Mutex::new(()) });
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            let server = Arc::clone(&server);
            std::thread::spawn(move || handle_connection(stream, &server));
        }
    }
}

/// The request line and headers of one request. Header names are
/// lower-cased; a header sent twice holds both values, `, `-joined, so a
/// guard comparing it to one exact value refuses the pair.
struct Head {
    method: String,
    path: String,
    headers: HashMap<String, String>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// One line, at most [`MAX_LINE`] bytes. `Ok(None)` at end of input.
fn read_line_bounded(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut line = String::new();
    let read = reader.by_ref().take(MAX_LINE).read_line(&mut line).map_err(|e| e.to_string())?;
    if read == 0 {
        return Ok(None);
    }
    if !line.ends_with('\n') && read as u64 >= MAX_LINE {
        return Err("a request line or header is too long".to_string());
    }
    Ok(Some(line))
}

fn read_head(reader: &mut impl BufRead) -> Result<Option<Head>, String> {
    let Some(request_line) = read_line_bounded(reader)? else { return Ok(None) };
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else { return Err("malformed request line".to_string()) };
    let path = target.split('?').next().unwrap_or(target).to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    for _ in 0..MAX_HEADERS {
        let Some(line) = read_line_bounded(reader)? else { break };
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return Ok(Some(Head { method: method.to_string(), path, headers }));
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            headers.entry(name.trim().to_ascii_lowercase()).and_modify(|v| *v = format!("{v}, {value}")).or_insert_with(|| value.to_string());
        }
    }
    Err("too many headers".to_string())
}

/// Handles one connection: reads the request line + headers and answers
/// (module doc's routes). Never panics the accept loop: every fallible
/// step degrades to an error response instead of propagating.
fn handle_connection(stream: TcpStream, server: &Server) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut stream = stream;
    let Ok(peer) = stream.try_clone() else { return };
    let mut reader = BufReader::new(peer);
    let head = match read_head(&mut reader) {
        Ok(Some(head)) => head,
        Ok(None) => return,
        Err(message) => return write_status(&mut stream, 400, &format!("bad request: {message}")),
    };

    // Every route checks `Host` first, the static app and the snapshot included: a rebinding page must read nothing.
    if let Err(reply) = guard_host(&head, server.port) {
        write_reply(&mut stream, &reply);
        return drain(&mut reader, &head);
    }
    match (head.method.as_str(), head.path.as_str()) {
        ("GET", "/api/status") => {
            let data = json!({ "snapshot": server.snapshot.to_json(), "app": { "source": if matches!(server.app, AppSource::Embedded(_)) { "embedded" } else { "directory" } } });
            let body = crate::json_out::envelope("dashboard status", data);
            write_reply(&mut stream, &Reply { status: 200, body });
        }
        ("GET", "/api/briefs") => write_reply(&mut stream, &crate::dashboard_briefs::list(&server.repo)),
        (_, "/api/briefs/approve") => match approve_guards(&head, server.port) {
            Err(reply) => {
                write_reply(&mut stream, &reply);
                drain(&mut reader, &head);
            }
            Ok(length) => write_reply(&mut stream, &read_and_approve(&mut reader, length, server)),
        },
        ("GET", path) => serve_file(&mut stream, &head, path, server),
        _ => write_status(&mut stream, 400, "bad request: GET only"),
    }
}

/// The `Host` header must name the bound address: a page reached under
/// another name (DNS rebinding) is refused.
fn guard_host(head: &Head, port: u16) -> Result<(), Reply> {
    let allowed = head.header("host").is_some_and(|host| [format!("127.0.0.1:{port}"), format!("localhost:{port}")].iter().any(|own| host.eq_ignore_ascii_case(own)));
    if allowed {
        return Ok(());
    }
    Err(Reply::error(403, &CliError::refused("DASHBOARD_ORIGIN_REFUSED", format!("the request's Host `{}` is not this dashboard's address", head.header("host").unwrap_or("(none)"))).hint(format!("open the dashboard at http://127.0.0.1:{port}/ or http://localhost:{port}/, the address `canon dashboard` printed"))))
}

/// The `Origin` header must be the page's own origin: another site's page
/// cannot make the browser approve a brief here.
fn guard_origin(head: &Head, port: u16) -> Result<(), Reply> {
    let allowed = head.header("origin").is_some_and(|origin| [format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}")].iter().any(|own| origin.eq_ignore_ascii_case(own)));
    if allowed {
        return Ok(());
    }
    Err(Reply::error(
        403,
        &CliError::refused("DASHBOARD_ORIGIN_REFUSED", format!("the request's Origin `{}` is not this dashboard's own origin", head.header("origin").unwrap_or("(none)")))
            .hint(format!("approve from the briefs page at http://127.0.0.1:{port}/briefs.html; a request with no Origin or another site's is refused")),
    ))
}

/// The guards of `POST /api/briefs/approve`, run before its body is read,
/// in this order: the method is `POST`; `Host` is the bound address
/// (already checked for every route); `Origin` is present and the
/// page's own; `Content-Type` is `application/json`; `Content-Length` is
/// present and at most [`MAX_BODY`]. Returns the body's length.
fn approve_guards(head: &Head, port: u16) -> Result<usize, Reply> {
    if head.method != "POST" {
        return Err(Reply::error(405, &CliError::refused("DASHBOARD_BAD_REQUEST", format!("{} is not allowed here: approvals are POSTed", head.method)).hint("POST a JSON body from the briefs page")));
    }
    guard_origin(head, port)?;
    let json_type = head.header("content-type").is_some_and(|ct| ct.split(';').next().is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json")));
    if !json_type {
        return Err(bad_request("the Content-Type must be application/json"));
    }
    let Some(length) = head.header("content-length").and_then(|v| v.parse::<usize>().ok()) else {
        return Err(bad_request("the request needs a Content-Length"));
    };
    if length > MAX_BODY {
        return Err(bad_request(format!("the body is {length} bytes; the limit is {MAX_BODY}")));
    }
    Ok(length)
}

/// Reads the `length`-byte body and approves, under the approvals lock.
fn read_and_approve(reader: &mut impl Read, length: usize, server: &Server) -> Reply {
    let mut body = vec![0u8; length];
    if reader.read_exact(&mut body).is_err() {
        return bad_request("the body is shorter than its Content-Length");
    }
    let _approving = server.approvals.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::dashboard_briefs::approve(&server.repo, &body)
}

/// Serves `GET path` from the snapshot or the app.
fn serve_file(stream: &mut TcpStream, head: &Head, path: &str, server: &Server) {
    if let Some(relative) = path.strip_prefix(LIVE_SNAPSHOT_ROUTE) {
        let file = (server.snapshot == SnapshotState::Ready).then(|| snapshot_file(&server.snapshot_dir, relative)).flatten();
        match file.and_then(|f| std::fs::read(&f).ok().map(|bytes| (f, bytes))) {
            Some((file, bytes)) => write_response(stream, 200, content_type(&file), &bytes, &[]),
            None => write_status(stream, 404, "not found"),
        }
        return;
    }
    let Some(file) = server.app.read(path) else { return write_status(stream, 404, "not found") };
    if !file.gzipped {
        return write_response(stream, 200, file.content_type, &file.body, &[]);
    }
    if accepts_gzip(head.header("accept-encoding")) {
        return write_response(stream, 200, file.content_type, &file.body, &[("Content-Encoding", "gzip"), ("Vary", "Accept-Encoding")]);
    }
    let mut plain = Vec::new();
    match flate2::read::GzDecoder::new(&file.body[..]).read_to_end(&mut plain) {
        Ok(_) => write_response(stream, 200, file.content_type, &plain, &[("Vary", "Accept-Encoding")]),
        Err(_) => write_status(stream, 500, "the embedded app file is unreadable"),
    }
}

/// Whether an `Accept-Encoding` value lets the server send gzip.
fn accepts_gzip(header: Option<&str>) -> bool {
    header.is_some_and(|value| {
        value.split(',').any(|item| {
            let mut parts = item.split(';');
            let coding = parts.next().unwrap_or("").trim();
            let refused = parts.any(|p| p.trim().strip_prefix("q=").is_some_and(|q| q.trim().parse::<f32>().is_ok_and(|q| q == 0.0)));
            (coding.eq_ignore_ascii_case("gzip") || coding == "*") && !refused
        })
    })
}

/// The app's relative file for a request path: a directory-shaped path,
/// including bare `/`, is that directory's `index.html`. `None` for a `..`
/// segment and for the demo fixture. Pure path arithmetic; a directory
/// source still proves containment with [`bounded`].
fn app_path(path: &str) -> Option<String> {
    if path.split('/').any(|segment| segment == "..") {
        return None;
    }
    let rest = path.trim_start_matches('/');
    let relative = if rest.is_empty() || rest.ends_with('/') { format!("{rest}index.html") } else { rest.to_string() };
    (relative.split('/').next() != Some(DEMO_FIXTURE)).then_some(relative)
}

/// The snapshot file a [`LIVE_SNAPSHOT_ROUTE`] remainder names, if it stays
/// inside `snapshot_dir`.
fn snapshot_file(snapshot_dir: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || Path::new(relative).is_absolute() || relative.split('/').any(|segment| segment == "..") {
        return None;
    }
    bounded(snapshot_dir, relative)
}

/// The actual security boundary: `root.join(relative)` canonicalized, and
/// REQUIRED to stay under the canonicalized `root` before any caller may
/// read it. `Path::canonicalize` resolves `.`/`..`, repeated separators,
/// and symlinks in one pass, so a symlink inside the root that points
/// outside it is refused with the rest. `None` for a path that does not
/// exist (already a 404) or that resolves outside `root`.
fn bounded(root: &Path, relative: &str) -> Option<PathBuf> {
    let canonical_root = root.canonicalize().ok()?;
    let canonical = root.join(relative).canonicalize().ok()?;
    (canonical.starts_with(&canonical_root) && canonical.is_file()).then_some(canonical)
}

/// Extension → MIME type. Every file type the built dashboard app +
/// its self-hosted DuckDB-Wasm bundle ships (`design.md` D4):
/// `text/javascript` for ES modules and `application/wasm` for the
/// Wasm core module matter most — browsers are strict about both.
fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        Some("parquet") => "application/octet-stream",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

fn reason(code: u16) -> &'static str {
    const REASONS: [(u16, &str); 6] = [(200, "OK"), (400, "Bad Request"), (403, "Forbidden"), (404, "Not Found"), (405, "Method Not Allowed"), (409, "Conflict")];
    REASONS.iter().find(|(c, _)| *c == code).map_or("Internal Server Error", |(_, phrase)| phrase)
}

fn write_response(stream: &mut TcpStream, code: u16, content_type: &str, body: &[u8], extra: &[(&str, &str)]) {
    let mut header = format!("HTTP/1.1 {code} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n", reason(code), body.len());
    for (name, value) in extra {
        header.push_str(&format!("{name}: {value}\r\n"));
    }
    header.push_str("\r\n");
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn write_status(stream: &mut TcpStream, code: u16, body: &str) {
    write_response(stream, code, "text/plain; charset=utf-8", body.as_bytes(), &[]);
}

fn write_reply(stream: &mut TcpStream, reply: &Reply) {
    let body = reply.body.to_string();
    let mut extra = vec![("Cache-Control", "no-store")];
    if reply.status == 405 {
        extra.push(("Allow", "POST"));
    }
    write_response(stream, reply.status, "application/json", body.as_bytes(), &extra);
    let _ = stream.flush();
}

/// Discards the body of a request refused before it was read (at most
/// [`MAX_BODY`] bytes), so closing the socket does not reset the connection
/// under the client before it has read the refusal.
fn drain(reader: &mut impl Read, head: &Head) {
    let unread = head.header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(MAX_BODY);
    let _ = std::io::copy(&mut reader.by_ref().take(unread as u64), &mut std::io::sink());
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    /// A synthetic embedded table: each `(path, content)` gzip-compressed
    /// and leaked, as `build.rs` would have written it.
    fn table(files: &[(&'static str, &str)]) -> &'static [EmbeddedFile] {
        let entries: Vec<EmbeddedFile> = files
            .iter()
            .map(|(path, content)| {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(content.as_bytes()).unwrap();
                EmbeddedFile { path, gz: Box::leak(encoder.finish().unwrap().into_boxed_slice()) }
            })
            .collect();
        Box::leak(entries.into_boxed_slice())
    }

    fn gunzip(bytes: &[u8]) -> String {
        let mut text = String::new();
        flate2::read::GzDecoder::new(bytes).read_to_string(&mut text).unwrap();
        text
    }

    /// What one request got back.
    struct Response {
        code: u16,
        head: String,
        body: Vec<u8>,
    }

    impl Response {
        fn header(&self, name: &str) -> Option<&str> {
            self.head.lines().skip(1).find_map(|l| l.split_once(':').filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim()))
        }
    }

    /// Sends one raw request (built from the bound port) to a one-shot
    /// `handle_connection` on real sockets.
    fn exchange(app: AppSource, snapshot_dir: &Path, snapshot: SnapshotState, request: impl Fn(u16) -> String) -> Response {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Server { repo: PathBuf::from("/nonexistent-repo"), app, snapshot_dir: snapshot_dir.to_path_buf(), snapshot, port, approvals: Mutex::new(()) };
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &server);
        });
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request(port).as_bytes()).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        handle.join().unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("a header/body separator");
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let code = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).expect("a status code");
        Response { code, head, body: raw[split + 4..].to_vec() }
    }

    fn get(app: AppSource, path: &str, extra_headers: &str) -> Response {
        let path = path.to_string();
        let extra = extra_headers.to_string();
        exchange(app, Path::new("/nonexistent-snapshot"), SnapshotState::Ready, move |port| format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}\r\n"))
    }

    /// Builds a real `dist_dir` and `snapshot_dir` on disk.
    fn served_roots() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dist_dir = root.path().join("dist");
        let snapshot_dir = root.path().join("snapshot");
        std::fs::create_dir_all(dist_dir.join("snapshot")).unwrap();
        std::fs::create_dir_all(&snapshot_dir).unwrap();
        std::fs::write(dist_dir.join("index.html"), b"<html>dashboard</html>").unwrap();
        std::fs::write(dist_dir.join("snapshot/manifest.json"), b"{\"demo\":true}").unwrap();
        std::fs::write(snapshot_dir.join("manifest.json"), b"{\"ok\":true}").unwrap();
        (root, dist_dir, snapshot_dir)
    }

    fn get_live(dist_dir: &Path, snapshot_dir: &Path, path: &str) -> Response {
        let path = path.to_string();
        exchange(AppSource::Dir(dist_dir.to_path_buf()), snapshot_dir, SnapshotState::Ready, move |port| format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"))
    }

    #[test]
    fn resolve_snapshot_dir_defaults_to_the_conventional_scratch_dir_under_repo_root() {
        let repo = PathBuf::from("/some/repo");
        assert_eq!(resolve_snapshot_dir(&repo, None), PathBuf::from("/some/repo/.canon/dashboard-snapshot"));
    }

    #[test]
    fn resolve_snapshot_dir_honors_an_explicit_dir_verbatim() {
        let repo = PathBuf::from("/some/repo");
        let explicit = PathBuf::from("/elsewhere/snap");
        assert_eq!(resolve_snapshot_dir(&repo, Some(&explicit)), explicit);
    }

    #[test]
    fn app_path_maps_directories_to_index_html_and_refuses_traversal_and_the_demo_fixture() {
        assert_eq!(app_path("/").as_deref(), Some("index.html"));
        assert_eq!(app_path("/briefs.html").as_deref(), Some("briefs.html"));
        assert_eq!(app_path("/assets/duckdb-mvp.wasm").as_deref(), Some("assets/duckdb-mvp.wasm"));
        assert_eq!(app_path("/../../etc/passwd"), None);
        assert_eq!(app_path("/snapshot/manifest.json"), None);
        assert_eq!(app_path("/snapshot"), None);
        assert_eq!(app_path("/snapshot/"), None);
        assert_eq!(app_path("//snapshot/manifest.json"), None);
    }

    #[test]
    fn snapshot_file_refuses_an_empty_absolute_or_traversing_remainder() {
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(snapshot_file(&snap, ""), None);
        assert_eq!(snapshot_file(&snap, "/etc/passwd"), None);
        assert_eq!(snapshot_file(&snap, "../../../etc/passwd"), None);
    }

    #[test]
    fn content_type_covers_every_dashboard_asset_extension() {
        assert_eq!(content_type(Path::new("index.html")), "text/html; charset=utf-8");
        assert_eq!(content_type(Path::new("app.js")), "text/javascript; charset=utf-8");
        assert_eq!(content_type(Path::new("style.css")), "text/css; charset=utf-8");
        assert_eq!(content_type(Path::new("manifest.json")), "application/json");
        assert_eq!(content_type(Path::new("duckdb-mvp.wasm")), "application/wasm");
        assert_eq!(content_type(Path::new("mart_trust_matrix.parquet")), "application/octet-stream");
        assert_eq!(content_type(Path::new("no-extension")), "application/octet-stream");
    }

    #[test]
    fn accepts_gzip_reads_the_header_like_a_browser_sends_it() {
        assert!(accepts_gzip(Some("gzip, deflate, br")));
        assert!(accepts_gzip(Some("deflate, GZIP;q=0.8")));
        assert!(accepts_gzip(Some("*")));
        assert!(!accepts_gzip(Some("deflate, br")));
        assert!(!accepts_gzip(Some("gzip;q=0")));
        assert!(!accepts_gzip(Some("identity")));
        assert!(!accepts_gzip(None));
    }

    #[test]
    fn bound_dashboard_urls_point_at_the_live_snapshot_and_the_briefs_page() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let bound = BoundDashboard { listener, repo: PathBuf::from("/repo"), app: AppSource::Dir(PathBuf::from("/dist")), snapshot_dir: PathBuf::from("/snap"), snapshot: SnapshotState::Ready };
        assert!(bound.url().starts_with("http://127.0.0.1:"));
        assert!(bound.url().ends_with("/?snapshot=/live-snapshot/"));
        assert!(bound.briefs_url().starts_with("http://127.0.0.1:"));
        assert!(bound.briefs_url().ends_with("/briefs.html"));
    }

    /// report.dashboard.01: the app embedded in the binary is served gzip-compressed to a browser that accepts it.
    #[test]
    fn report_dashboard_01_an_embedded_app_is_served_gzip_to_a_browser_that_accepts_it() {
        let app = AppSource::Embedded(table(&[("index.html", "<html>embedded</html>"), ("assets/app.js", "console.log(1)")]));
        let response = get(app, "/assets/app.js", "Accept-Encoding: gzip, deflate\r\n");
        assert_eq!(response.code, 200);
        assert_eq!(response.header("content-encoding"), Some("gzip"));
        assert_eq!(response.header("content-type"), Some("text/javascript; charset=utf-8"));
        assert_eq!(response.header("content-length"), Some(response.body.len().to_string().as_str()));
        assert_eq!(gunzip(&response.body), "console.log(1)");
    }

    /// report.dashboard.01: the same app is served decompressed to a browser that does not accept gzip.
    #[test]
    fn report_dashboard_01_an_embedded_app_is_served_decompressed_to_a_browser_that_does_not_accept_gzip() {
        let app = AppSource::Embedded(table(&[("index.html", "<html>embedded</html>")]));
        for headers in ["", "Accept-Encoding: identity\r\n", "Accept-Encoding: gzip;q=0\r\n"] {
            let response = get(app.clone(), "/", headers);
            assert_eq!(response.code, 200, "{headers}");
            assert_eq!(response.header("content-encoding"), None, "{headers}");
            assert_eq!(response.body, b"<html>embedded</html>", "{headers}");
        }
    }

    #[test]
    fn an_embedded_app_answers_404_for_a_file_it_does_not_hold() {
        let app = AppSource::Embedded(table(&[("index.html", "x")]));
        assert_eq!(get(app, "/nope.js", "").code, 404);
    }

    /// report.dashboard.04: the demo fixture is never served, whichever source holds it.
    #[test]
    fn report_dashboard_04_the_demo_fixture_is_never_served_from_either_source() {
        // An embedded table that (wrongly) holds the fixture still does not serve it.
        let embedded = AppSource::Embedded(table(&[("index.html", "x"), ("snapshot/manifest.json", "{\"demo\":true}")]));
        for path in ["/snapshot/manifest.json", "/snapshot/", "/snapshot"] {
            assert_eq!(get(embedded.clone(), path, "").code, 404, "embedded {path}");
        }
        let (_root, dist_dir, snapshot_dir) = served_roots();
        for path in ["/snapshot/manifest.json", "/snapshot/", "/./snapshot/manifest.json", "/assets/../snapshot/manifest.json"] {
            let response = get_live(&dist_dir, &snapshot_dir, path);
            assert_eq!(response.code, 404, "directory {path}");
            assert_eq!(response.body, b"not found");
        }
    }

    /// report.dashboard.01: canon's own checkout build overrides the embedded app, and nobody else's `packages/dashboard` is ever served.
    #[test]
    fn report_dashboard_01_resolve_app_serves_the_checkout_build_only_for_canons_own_dashboard_package() {
        let embedded = table(&[("index.html", "embedded")]);
        let repo = tempfile::tempdir().unwrap();
        let package = repo.path().join("packages/dashboard");
        std::fs::create_dir_all(package.join("dist")).unwrap();
        std::fs::write(package.join("dist/index.html"), "built").unwrap();

        // Another project's packages/dashboard (or none naming it) is never served.
        std::fs::write(package.join("package.json"), r#"{"name":"@acme/dashboard"}"#).unwrap();
        assert!(matches!(resolve_app(repo.path(), embedded).unwrap(), AppSource::Embedded(_)));
        assert!(matches!(resolve_app(repo.path(), &[]), Err(DashboardError::NotBuilt(_))));
        std::fs::write(package.join("package.json"), "not json").unwrap();
        assert!(matches!(resolve_app(repo.path(), &[]), Err(DashboardError::NotBuilt(_))));

        // Canon's own checkout, built: the development override beats the embedded app.
        std::fs::write(package.join("package.json"), format!(r#"{{"name":"{DEV_PACKAGE_NAME}"}}"#)).unwrap();
        assert!(matches!(resolve_app(repo.path(), embedded).unwrap(), AppSource::Dir(_)));
        assert!(matches!(resolve_app(repo.path(), &[]).unwrap(), AppSource::Dir(_)));

        // Canon's own checkout, not built: the embedded app, else not built.
        std::fs::remove_file(package.join("dist/index.html")).unwrap();
        assert!(matches!(resolve_app(repo.path(), embedded).unwrap(), AppSource::Embedded(_)));
        assert!(matches!(resolve_app(repo.path(), &[]), Err(DashboardError::NotBuilt(_))));
    }

    #[test]
    fn live_snapshot_route_rejects_an_absolute_remainder_join_reset_bypass() {
        // Pre-fix, `Path::join` on an absolute remainder discards
        // `snapshot_dir` and this request serves the real `/etc/passwd`.
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let response = get_live(&dist_dir, &snapshot_dir, "/live-snapshot//etc/passwd");
        assert_eq!(response.code, 404);
        assert_eq!(response.body, b"not found");
    }

    #[test]
    fn live_snapshot_route_rejects_dotdot_traversal_end_to_end() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        assert_eq!(get_live(&dist_dir, &snapshot_dir, "/live-snapshot/../../../etc/passwd").code, 404);
    }

    #[test]
    fn dist_route_rejects_dotdot_traversal_end_to_end() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        assert_eq!(get_live(&dist_dir, &snapshot_dir, "/../../etc/passwd").code, 404);
    }

    #[test]
    fn dist_route_serves_a_legitimate_in_root_file() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let response = get_live(&dist_dir, &snapshot_dir, "/index.html");
        assert_eq!(response.code, 200);
        assert_eq!(response.body, b"<html>dashboard</html>");
        assert_eq!(response.header("content-encoding"), None);
    }

    #[test]
    fn live_snapshot_route_serves_a_legitimate_in_root_file() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let response = get_live(&dist_dir, &snapshot_dir, "/live-snapshot/manifest.json");
        assert_eq!(response.code, 200);
        assert_eq!(response.body, b"{\"ok\":true}");
    }

    #[test]
    fn live_snapshot_route_is_not_served_when_the_snapshot_is_unavailable() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let unavailable = SnapshotState::Unavailable { reason: "no duckdb".into(), fix: "install it".into() };
        let response = exchange(AppSource::Dir(dist_dir), &snapshot_dir, unavailable, |port| format!("GET /live-snapshot/manifest.json HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert_eq!(response.code, 404);
    }

    #[cfg(unix)]
    #[test]
    fn dist_route_rejects_a_symlink_escaping_the_root() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET OUTSIDE dist_dir").unwrap();
        std::os::unix::fs::symlink(&secret, dist_dir.join("escape.txt")).unwrap();
        assert_eq!(get_live(&dist_dir, &snapshot_dir, "/escape.txt").code, 404);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_into_the_demo_fixture_is_not_served_either() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        std::os::unix::fs::symlink(dist_dir.join("snapshot/manifest.json"), dist_dir.join("alias.json")).unwrap();
        assert_eq!(get_live(&dist_dir, &snapshot_dir, "/alias.json").code, 404);
    }

    #[cfg(unix)]
    #[test]
    fn live_snapshot_route_rejects_a_symlink_escaping_the_root() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET OUTSIDE snapshot_dir").unwrap();
        std::os::unix::fs::symlink(&secret, snapshot_dir.join("escape.txt")).unwrap();
        assert_eq!(get_live(&dist_dir, &snapshot_dir, "/live-snapshot/escape.txt").code, 404);
    }
}
