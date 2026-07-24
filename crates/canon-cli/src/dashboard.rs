//! `canon dashboard [--repo <dir>] [--snapshot <dir>] [--port <n>]` (S9
//! part3, `s9-unified-surface` tasks.md 6.1): serves the already-built
//! `packages/dashboard` static app locally, pointed at a real snapshot
//! via the app's own `?snapshot=` override
//! (`packages/dashboard/src/main.ts`'s own module doc names this exact
//! contract for `canon dashboard` to drive: "`?snapshot=<url>` overrides
//! it, e.g. when `canon dashboard --snapshot <dir>` (task 6.1) serves a
//! real snapshot at a different path alongside the built app"). This
//! module never re-implements snapshot/report generation (design D1) —
//! it calls straight into [`canon_report::snapshot`], the exact library
//! entry point [`crate::report`]'s `canon report --snapshot` arm already
//! wires.
//!
//! # Default vs. explicit `--snapshot`
//! `--snapshot` omitted: the snapshot is regenerated fresh, every run,
//! at the conventional [`DEFAULT_SNAPSHOT_DIR`] scratch directory — a
//! sibling of `.canon/REPORT.md`
//! ([`canon_report::render::DEFAULT_REPORT_PATH`]) and the other
//! `.canon/*` tier roots (`.canon/ledger`, `.canon/r2`, `.canon/learn`).
//! There is no separately-persisted "last used directory" state to
//! track (no other `canon-cli` subcommand keeps one either) — task
//! 6.1's "defaulting to the repo's last `canon report --snapshot`
//! output" is satisfied because regenerating into the SAME conventional
//! path on every invocation makes that path always hold the most recent
//! snapshot. `--snapshot <dir>` given: served AS-IS when `<dir>` already
//! has a `manifest.json` (the caller's own pre-generated or hand-placed
//! snapshot, e.g. a downloaded CI artifact or the committed dashboard
//! fixture — never silently overwritten), else generated there once.
//!
//! # Server shape
//! A minimal, dependency-free [`std::net::TcpListener`] static file
//! server — the same "hand-roll a tiny stdlib-only server instead of
//! adding an HTTP dependency" discipline already established elsewhere
//! in this workspace for a stub server. One spawned thread per
//! connection, `GET`-only, `Connection: close`, two routes:
//!
//! - `/` → [`DASHBOARD_DIST_DIR`] (the built static app; any path
//!   ending in `/`, including bare `/`, resolves to that directory's
//!   `index.html`).
//! - [`LIVE_SNAPSHOT_ROUTE`] → the resolved snapshot directory's own
//!   files (`manifest.json` + the panel-mart Parquet files) — served
//!   ALONGSIDE the app's own bundled `/snapshot/` fixture route
//!   (`packages/dashboard/vite.config.ts`'s `publicDir: "fixtures"`),
//!   never overwriting or touching it.
//!
//! [`BoundDashboard::url`] prints `/?snapshot=<LIVE_SNAPSHOT_ROUTE>`, so
//! opening it overrides the app's bundled-fixture default with the
//! real, just-generated (or given) snapshot.
//!
//! # Path resolution is bounded to the served root
//! Every resolved file is canonicalized and REQUIRED to stay under its
//! route's own canonical root ([`resolve_bounded_file`]) before it is
//! ever read — closing not just `..` segment traversal but also an
//! absolute remainder (which would otherwise make `Path::join` discard
//! the root entirely instead of nesting under it), a leading `//`, and
//! a symlink inside the root that points outside it. A request that
//! resolves outside its route's root is a 404, the same as any other
//! miss.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use canon_model::paths;

use crate::report::resolve_inputs;

/// Conventional default snapshot scratch dir (module doc).
pub const DEFAULT_SNAPSHOT_DIR: &str = paths::DASHBOARD_SNAPSHOT_DIR;

/// The built `packages/dashboard` static app, relative to a resolved
/// repo root — `packages/dashboard`'s own `vite build` output
/// (`package.json`'s `build` script, `outDir: "dist"` in
/// `vite.config.ts`).
pub const DASHBOARD_DIST_DIR: &str = "packages/dashboard/dist";

/// The URL path prefix the live snapshot is served under (module doc) —
/// distinct from the app's own bundled `/snapshot/` fixture route so
/// neither can ever shadow the other.
pub const LIVE_SNAPSHOT_ROUTE: &str = "/live-snapshot/";

#[derive(Debug, thiserror::Error)]
pub enum DashboardError {
    #[error(transparent)]
    Report(#[from] canon_report::ReportError),
    #[error("`packages/dashboard` is not built — run `bun install && bun run build` in {0} first (or from the repo root: `bun install && bun run --cwd packages/dashboard build`)")]
    DashboardNotBuilt(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolves the snapshot directory `canon dashboard` will serve:
/// `--snapshot` verbatim when given, else the conventional
/// [`DEFAULT_SNAPSHOT_DIR`] under the resolved repo root (module doc).
pub fn resolve_snapshot_dir(repo: &Path, snapshot: Option<&Path>) -> PathBuf {
    snapshot.map(Path::to_path_buf).unwrap_or_else(|| repo.join(DEFAULT_SNAPSHOT_DIR))
}

/// Regenerates `dir`'s snapshot unconditionally when `explicit` is
/// false (the default scratch dir — module doc), else only when `dir`
/// has no `manifest.json` yet (an explicitly-given dir is the caller's
/// own; served as-is when it already has content, never stomped).
fn ensure_snapshot(inputs: &canon_report::ReportInputs, dir: &Path, explicit: bool) -> Result<(), DashboardError> {
    if !explicit || !dir.join("manifest.json").is_file() {
        canon_report::snapshot(inputs, dir)?;
    }
    Ok(())
}

/// One bound, ready-to-serve dashboard: the resolved app/snapshot dirs
/// (for logging) plus the live [`TcpListener`], already bound — so a
/// caller (or a test) can read the actually-assigned port
/// ([`BoundDashboard::port`]) before handing off to
/// [`BoundDashboard::serve_forever`]'s infinite accept loop.
pub struct BoundDashboard {
    pub listener: TcpListener,
    pub dist_dir: PathBuf,
    pub snapshot_dir: PathBuf,
}

/// Resolves `--repo`/`--snapshot`, (re)generates the snapshot per
/// [`ensure_snapshot`], verifies the app is built, and binds the
/// listener — everything [`BoundDashboard::serve_forever`] needs, split
/// out so callers (and tests) can bind + inspect/query before the
/// actually-infinite serve loop starts.
pub fn prepare(repo: &Path, snapshot: Option<&Path>, port: u16) -> Result<BoundDashboard, DashboardError> {
    let (repo, inputs) = resolve_inputs(repo);
    let explicit = snapshot.is_some();
    let snapshot_dir = resolve_snapshot_dir(&repo, snapshot);
    ensure_snapshot(&inputs, &snapshot_dir, explicit)?;

    let dist_dir = repo.join(DASHBOARD_DIST_DIR);
    if !dist_dir.join("index.html").is_file() {
        return Err(DashboardError::DashboardNotBuilt(dist_dir));
    }

    let listener = TcpListener::bind(("127.0.0.1", port))?;
    Ok(BoundDashboard { listener, dist_dir, snapshot_dir })
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

    /// Accepts connections forever, one spawned thread per connection
    /// (module doc) — `canon dashboard`'s actual run loop. Never
    /// returns; a caller wanting a bounded lifetime (tests) drives the
    /// listener directly instead of calling this.
    pub fn serve_forever(&self) {
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            let dist_dir = self.dist_dir.clone();
            let snapshot_dir = self.snapshot_dir.clone();
            std::thread::spawn(move || handle_connection(stream, &dist_dir, &snapshot_dir));
        }
    }
}

/// Handles one connection: reads the request line + drains headers,
/// resolves the requested path to an on-disk file under `dist_dir` or
/// `snapshot_dir`, canonicalized and bound-checked into its route's own
/// root ([`resolve_bounded_file`]), and writes it back (or a 404/400)
/// — module doc's "GET-only, `Connection: close`" server shape. Never
/// panics the accept loop: every fallible step degrades to an error
/// response instead of propagating.
fn handle_connection(stream: TcpStream, dist_dir: &Path, snapshot_dir: &Path) {
    let mut stream = stream;
    let peer = stream.try_clone();
    let Ok(peer) = peer else { return };
    let mut reader = BufReader::new(peer);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    // Drain headers — this server reads no request body (GET-only).
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    }

    let Some(path) = parse_request_path(&request_line) else {
        write_status(&mut stream, 400, "Bad Request", b"bad request: GET only");
        return;
    };

    match resolve_bounded_file(&path, dist_dir, snapshot_dir) {
        Some(file) => match std::fs::read(&file) {
            Ok(body) => write_response(&mut stream, 200, "OK", content_type(&file), &body),
            Err(_) => write_status(&mut stream, 404, "Not Found", b"not found"),
        },
        None => write_status(&mut stream, 404, "Not Found", b"not found"),
    }
}

/// Parses `GET /path?query HTTP/1.1\r\n` down to the query-stripped
/// path. `None` for anything not a `GET` — this server has no write
/// routes at all.
fn parse_request_path(request_line: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" {
        return None;
    }
    let raw_path = parts.next()?;
    let path = raw_path.split('?').next().unwrap_or(raw_path);
    Some(path.to_string())
}

/// Maps a URL path to an on-disk *candidate* file: [`LIVE_SNAPSHOT_ROUTE`]
/// under `snapshot_dir`, everything else under `dist_dir` (a
/// directory-shaped path, including bare `/`, resolves to that
/// directory's `index.html`). Rejects any `..` path segment
/// (path-traversal guard) and any remainder that is itself
/// `Path::is_absolute()` — an absolute remainder would make the
/// subsequent `Path::join` *discard* `dist_dir`/`snapshot_dir`
/// entirely instead of nesting under it (e.g. `GET
/// /live-snapshot//etc/passwd` strips to the absolute rest
/// `/etc/passwd`), so this is rejected here, before the join ever
/// happens — `None` either way is the caller's cue to answer 404/400.
///
/// This function is deliberately fs-free (pure candidate-path
/// arithmetic, unit-tested against non-existent paths below); it does
/// NOT prove the result stays inside its root — a symlink inside
/// `dist_dir`/`snapshot_dir` can still point outside it. The actual
/// security boundary is [`resolve_bounded_file`], which canonicalizes
/// this candidate and REQUIRES it stay under the canonical root before
/// any caller reads it.
fn resolve_file(path: &str, dist_dir: &Path, snapshot_dir: &Path) -> Option<PathBuf> {
    if path.split('/').any(|segment| segment == "..") {
        return None;
    }

    if let Some(rest) = path.strip_prefix(LIVE_SNAPSHOT_ROUTE) {
        if rest.is_empty() || Path::new(rest).is_absolute() {
            return None;
        }
        return Some(snapshot_dir.join(rest));
    }

    let rest = path.trim_start_matches('/');
    if Path::new(rest).is_absolute() {
        return None;
    }
    if rest.is_empty() || rest.ends_with('/') {
        return Some(dist_dir.join(rest).join("index.html"));
    }
    Some(dist_dir.join(rest))
}

/// The actual security boundary: resolves `path` to a [`resolve_file`]
/// candidate, then REQUIRES its canonicalized form stay under the
/// canonicalized form of the route's own root (`snapshot_dir` for
/// [`LIVE_SNAPSHOT_ROUTE`], `dist_dir` otherwise) before any caller may
/// read it. `Path::canonicalize` resolves `.`/`..`, repeated
/// separators, and symlinks all in one pass — closing the
/// absolute-remainder join-reset bug, residual `..` traversal, a
/// leading `//`, and symlink-escape with a single bounded check, no
/// matter which one (or which combination) produced the candidate.
/// `None` for a candidate that doesn't exist yet (canonicalize
/// requires existence — fine for a static server: unresolvable is
/// already a 404) or that resolves outside its root.
fn resolve_bounded_file(path: &str, dist_dir: &Path, snapshot_dir: &Path) -> Option<PathBuf> {
    let candidate = resolve_file(path, dist_dir, snapshot_dir)?;
    let root = if path.starts_with(LIVE_SNAPSHOT_ROUTE) { snapshot_dir } else { dist_dir };
    let canonical_root = root.canonicalize().ok()?;
    let canonical_candidate = candidate.canonicalize().ok()?;
    canonical_candidate.starts_with(&canonical_root).then_some(canonical_candidate)
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

fn write_response(stream: &mut TcpStream, code: u16, reason: &str, content_type: &str, body: &[u8]) {
    let header = format!("HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn write_status(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) {
    write_response(stream, code, reason, "text/plain; charset=utf-8", body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

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
    fn parse_request_path_strips_query_string() {
        assert_eq!(parse_request_path("GET /assets/app.js?v=2 HTTP/1.1\r\n").as_deref(), Some("/assets/app.js"));
    }

    #[test]
    fn parse_request_path_rejects_non_get_methods() {
        assert_eq!(parse_request_path("POST /manifest.json HTTP/1.1\r\n"), None);
        assert_eq!(parse_request_path("HEAD / HTTP/1.1\r\n"), None);
    }

    #[test]
    fn parse_request_path_rejects_a_malformed_request_line() {
        assert_eq!(parse_request_path(""), None);
        assert_eq!(parse_request_path("GET\r\n"), None);
    }

    #[test]
    fn resolve_file_maps_bare_root_to_index_html_under_dist_dir() {
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file("/", &dist, &snap), Some(dist.join("index.html")));
    }

    #[test]
    fn resolve_file_maps_an_asset_path_under_dist_dir() {
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file("/assets/duckdb-mvp.wasm", &dist, &snap), Some(dist.join("assets/duckdb-mvp.wasm")));
    }

    #[test]
    fn resolve_file_maps_the_live_snapshot_route_under_snapshot_dir() {
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file("/live-snapshot/manifest.json", &dist, &snap), Some(snap.join("manifest.json")));
        assert_eq!(resolve_file("/live-snapshot/mart_trust_matrix.parquet", &dist, &snap), Some(snap.join("mart_trust_matrix.parquet")));
    }

    #[test]
    fn resolve_file_rejects_bare_live_snapshot_route_with_no_file() {
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file(LIVE_SNAPSHOT_ROUTE, &dist, &snap), None);
    }

    #[test]
    fn resolve_file_rejects_any_path_traversal_segment() {
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file("/../../etc/passwd", &dist, &snap), None);
        assert_eq!(resolve_file("/live-snapshot/../../../etc/passwd", &dist, &snap), None);
    }

    #[test]
    fn resolve_file_rejects_an_absolute_remainder_on_the_live_snapshot_route() {
        // Defense-in-depth: `strip_prefix(LIVE_SNAPSHOT_ROUTE)` on
        // `/live-snapshot//etc/passwd` leaves the absolute rest
        // `/etc/passwd` — join-ing that onto `snapshot_dir` would
        // DISCARD `snapshot_dir` per `Path::join`'s documented
        // absolute-path-replaces-current-path behavior. Caught here,
        // before the join ever runs.
        let dist = PathBuf::from("/repo/packages/dashboard/dist");
        let snap = PathBuf::from("/repo/.canon/dashboard-snapshot");
        assert_eq!(resolve_file("/live-snapshot//etc/passwd", &dist, &snap), None);
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
    fn bound_dashboard_url_points_at_the_live_snapshot_route() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let bound = BoundDashboard { listener, dist_dir: PathBuf::from("/dist"), snapshot_dir: PathBuf::from("/snap") };
        let url = bound.url();
        assert!(url.starts_with("http://127.0.0.1:"));
        assert!(url.ends_with("/?snapshot=/live-snapshot/"));
    }

    /// Builds a real `dist_dir` (with a real `index.html`) and a real
    /// `snapshot_dir` (with a real `manifest.json`) on disk — the
    /// fixture the end-to-end tests below drive through the actual
    /// `handle_connection` code path (real sockets, real files), not
    /// `resolve_file` alone, so a regression in the
    /// canonicalize+starts_with bound is caught the same way a real
    /// request would hit it.
    fn served_roots() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dist_dir = root.path().join("dist");
        let snapshot_dir = root.path().join("snapshot");
        std::fs::create_dir_all(&dist_dir).unwrap();
        std::fs::create_dir_all(&snapshot_dir).unwrap();
        std::fs::write(dist_dir.join("index.html"), b"<html>dashboard</html>").unwrap();
        std::fs::write(snapshot_dir.join("manifest.json"), b"{\"ok\":true}").unwrap();
        (root, dist_dir, snapshot_dir)
    }

    /// Sends a raw `GET <path> HTTP/1.1` request to a one-shot
    /// `handle_connection` listener bound to an ephemeral port and
    /// returns `(status_code, body)`.
    fn get(dist_dir: &Path, snapshot_dir: &Path, path: &str) -> (u16, Vec<u8>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let dist_dir = dist_dir.to_path_buf();
        let snapshot_dir = snapshot_dir.to_path_buf();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &dist_dir, &snapshot_dir);
        });

        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes()).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        server.join().unwrap();

        let header_end = response.windows(4).position(|w| w == b"\r\n\r\n").expect("response has a header/body separator");
        let header = String::from_utf8_lossy(&response[..header_end]);
        let code = header
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .expect("status line has a numeric code");
        (code, response[header_end + 4..].to_vec())
    }

    #[test]
    fn live_snapshot_route_rejects_an_absolute_remainder_join_reset_bypass() {
        // The CVE this whole change locks: pre-fix, `Path::join` on an
        // absolute remainder discards `snapshot_dir` entirely and this
        // request serves the real `/etc/passwd` with a 200.
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let (code, body) = get(&dist_dir, &snapshot_dir, "/live-snapshot//etc/passwd");
        assert_eq!(code, 404);
        assert_eq!(body, b"not found");
    }

    #[test]
    fn live_snapshot_route_rejects_dotdot_traversal_end_to_end() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let (code, body) = get(&dist_dir, &snapshot_dir, "/live-snapshot/../../../etc/passwd");
        assert_eq!(code, 404);
        assert_eq!(body, b"not found");
    }

    #[test]
    fn dist_route_rejects_dotdot_traversal_end_to_end() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let (code, body) = get(&dist_dir, &snapshot_dir, "/../../etc/passwd");
        assert_eq!(code, 404);
        assert_eq!(body, b"not found");
    }

    #[test]
    fn dist_route_serves_a_legitimate_in_root_file() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let (code, body) = get(&dist_dir, &snapshot_dir, "/index.html");
        assert_eq!(code, 200);
        assert_eq!(body, b"<html>dashboard</html>");
    }

    #[test]
    fn live_snapshot_route_serves_a_legitimate_in_root_file() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let (code, body) = get(&dist_dir, &snapshot_dir, "/live-snapshot/manifest.json");
        assert_eq!(code, 200);
        assert_eq!(body, b"{\"ok\":true}");
    }

    #[cfg(unix)]
    #[test]
    fn dist_route_rejects_a_symlink_escaping_the_root() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET OUTSIDE dist_dir").unwrap();
        std::os::unix::fs::symlink(&secret, dist_dir.join("escape.txt")).unwrap();

        let (code, body) = get(&dist_dir, &snapshot_dir, "/escape.txt");
        assert_eq!(code, 404);
        assert_eq!(body, b"not found");
    }

    #[cfg(unix)]
    #[test]
    fn live_snapshot_route_rejects_a_symlink_escaping_the_root() {
        let (_root, dist_dir, snapshot_dir) = served_roots();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET OUTSIDE snapshot_dir").unwrap();
        std::os::unix::fs::symlink(&secret, snapshot_dir.join("escape.txt")).unwrap();

        let (code, body) = get(&dist_dir, &snapshot_dir, "/live-snapshot/escape.txt");
        assert_eq!(code, 404);
        assert_eq!(body, b"not found");
    }
}
