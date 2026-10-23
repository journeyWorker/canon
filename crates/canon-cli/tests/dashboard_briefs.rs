//! The dashboard's briefs page endpoints and the server around them (0.25
//! D2/D3, #102 #103), against the actually-built `canon` binary: a real
//! server on a real port, a throwaway repo with real briefs, raw HTTP.
//!
//! The binary of a test build embeds no app, so every repo here is
//! "canon's own checkout" by the development override: a
//! `packages/dashboard/package.json` naming `@canon-dev/dashboard` and a
//! `dist/` with a page in it. The embedded table itself is unit-tested in
//! `src/dashboard.rs`; `duckdb` is taken off `PATH` so the snapshot cannot
//! be generated, which is the case for a consumer without it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use sha2::Digest;
use tempfile::TempDir;

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn ok(repo: &Path, args: &[&str]) {
    let out = canon(repo, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A complete English PRD brief with flow steps `F1`, `F2` (no blockers).
fn clean_brief(title: &str) -> String {
    format!(
        "# {title}\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\n{title}\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n"
    )
}

/// The policy approval as it is on disk (the setup's own approval of `settled` wrote one).
fn policy_approval(repo: &Path) -> Option<Vec<u8>> {
    std::fs::read(repo.join(".canon/policy.approval.json")).ok()
}

fn brief_path(repo: &Path, change: &str) -> PathBuf {
    repo.join(".canon/changes").join(change).join("brief.md")
}

/// A repo that looks like canon's checkout to the dashboard, with subject
/// `demo` and these changes: `ready` (a clean brief), `blocked` (the
/// scaffold's brief, which still has placeholders) and `settled` (approved
/// already); `archived` sits in the changes archive, unapproved.
fn repo_with_briefs() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    for change in ["ready", "blocked", "settled"] {
        ok(repo, &["change", "new", change, "--subject-id", "demo", "--title", change]);
    }
    std::fs::write(brief_path(repo, "ready"), clean_brief("Ready change")).unwrap();
    std::fs::write(brief_path(repo, "settled"), clean_brief("Settled change")).unwrap();
    ok(repo, &["brief", "approve",
                "settled", "--approver", "Ana", "--note", "Go."]);
    let archived = repo.join(".canon/changes/archive/archived");
    std::fs::create_dir_all(&archived).unwrap();
    std::fs::write(archived.join("brief.md"), clean_brief("Archived change")).unwrap();

    let package = repo.join("packages/dashboard");
    std::fs::create_dir_all(package.join("dist")).unwrap();
    std::fs::write(package.join("package.json"), r#"{"name":"@canon-dev/dashboard"}"#).unwrap();
    for page in ["index.html", "briefs.html"] {
        std::fs::write(package.join("dist").join(page), format!("<html>canon dashboard {page}</html>")).unwrap();
    }
    std::fs::create_dir_all(package.join("dist/snapshot")).unwrap();
    std::fs::write(package.join("dist/snapshot/manifest.json"), r#"{"demo":true}"#).unwrap();
    dir
}

/// A live `canon dashboard` and the port it bound; killed on drop.
struct Live {
    child: Child,
    port: u16,
    startup: Value,
    /// Everything the server wrote to stderr.
    stderr: PathBuf,
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts `canon dashboard --port 0 --json` with no `duckdb` on `PATH` and
/// waits for its one JSON document on stdout.
fn serve(repo: &Path) -> Live {
    let no_tools = repo.join("no-tools");
    std::fs::create_dir_all(&no_tools).unwrap();
    let stderr = repo.join("dashboard.stderr");
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dashboard", "--port", "0", "--json"])
        .current_dir(repo)
        .env("PATH", &no_tools)
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(&stderr).unwrap())
        .spawn()
        .expect("spawn canon dashboard");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).expect("the startup document");
    let startup: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("startup document {line:?}: {e}"));
    let url = startup["data"]["url"].as_str().expect("data.url").to_string();
    let port = url.trim_start_matches("http://127.0.0.1:").split('/').next().unwrap().parse().unwrap();
    Live { child, port, startup, stderr }
}

struct Reply {
    code: u16,
    head: String,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("body {:?}: {e}", self.body))
    }
}

/// One raw HTTP/1.1 request; `headers` are sent as given.
fn http(port: u16, method: &str, path: &str, headers: &[(&str, String)], body: &[u8]) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the response");
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("a header/body separator");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let code = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).expect("a status code");
    Reply { code, head, body: String::from_utf8_lossy(&raw[split + 4..]).into_owned() }
}

fn get(live: &Live, path: &str) -> Reply {
    http(live.port, "GET", path, &[("Host", format!("127.0.0.1:{}", live.port))], b"")
}

/// The headers of a well-formed approval from the page.
fn page_headers(port: u16, length: usize) -> Vec<(&'static str, String)> {
    vec![
        ("Host", format!("127.0.0.1:{port}")),
        ("Origin", format!("http://127.0.0.1:{port}")),
        ("Content-Type", "application/json".to_string()),
        ("Content-Length", length.to_string()),
    ]
}

fn post_approval(live: &Live, request: &Value) -> Reply {
    let body = request.to_string();
    http(live.port, "POST", "/api/briefs/approve", &page_headers(live.port, body.len()), body.as_bytes())
}

fn brief_of(live: &Live, change: &str) -> Value {
    let list = get(live, "/api/briefs").json();
    list["data"]["briefs"].as_array().unwrap().iter().find(|b| b["changeId"] == change).unwrap_or_else(|| panic!("no pending brief `{change}` in {list}")).clone()
}

fn approval_request(live: &Live, change: &str) -> Value {
    json!({ "changeId": change, "by": "Ana", "note": "Ship it.", "sha256": brief_of(live, change)["sha256"] })
}

/// Copies a directory tree (the repo twin for the CLI comparison).
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// `text` with the approval time (RFC 3339, to the second) blanked out.
fn without_timestamp(text: &str) -> String {
    text.split(' ').map(|w| if w.len() == 20 && w.ends_with('Z') && w.as_bytes()[10] == b'T' { "<when>" } else { w }).collect::<Vec<_>>().join(" ")
}

/// report.dashboard.05: the list names only live, unapproved briefs.
#[test]
fn report_dashboard_05_lists_only_live_unapproved_briefs_with_sections_and_findings() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let list = get(&live, "/api/briefs");
    assert_eq!(list.code, 200);
    let list = list.json();
    let ids: Vec<&str> = list["data"]["briefs"].as_array().unwrap().iter().map(|b| b["changeId"].as_str().unwrap()).collect();
    assert_eq!(ids, ["blocked", "ready"], "settled is approved and archived is not live: {list}");

    let ready = brief_of(&live, "ready");
    let on_disk = std::fs::read(brief_path(dir.path(), "ready")).unwrap();
    assert_eq!(ready["text"].as_str().unwrap().as_bytes(), on_disk.as_slice());
    let digest: String = sha2::Sha256::digest(&on_disk).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(ready["sha256"], digest.as_str(), "the digest is the sha256 of the brief file's bytes");
    assert_eq!(ready["subjects"], json!(["demo"]));
    assert_eq!(ready["brief"], ".canon/changes/ready/brief.md");
    assert_eq!(ready["approvable"], true);
    let sections = ready["sections"].as_array().unwrap();
    let keys: Vec<&str> = sections.iter().map(|s| s["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["intent", "outcome", "flow", "detail", "acceptance", "questions", "out-of-scope"]);
    let flow = sections.iter().find(|s| s["key"] == "flow").unwrap();
    let steps: Vec<&str> = flow["steps"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(steps, ["F1", "F2"]);
    assert_eq!(flow["heading"], "User flow");
    assert!(flow["steps"][1]["text"].as_str().unwrap().starts_with("F2 The visitor submits a wrong password"));
    let advisories: Vec<&Value> = ready["findings"].as_array().unwrap().iter().filter(|f| f["severity"] == "blocker").collect();
    assert!(advisories.is_empty(), "{ready}");

    let blocked = brief_of(&live, "blocked");
    assert_eq!(blocked["approvable"], false);
    let finding = blocked["findings"].as_array().unwrap().iter().find(|f| f["severity"] == "blocker" && f["blocksApproval"] == true).expect("a blocker that blocks approval");
    assert!(finding["rule"].as_str().unwrap().starts_with("brief."), "{finding}");
    assert!(finding["line"].as_u64().unwrap() >= 1 && finding["message"].is_string());
    assert!(blocked["counts"]["blockers"].as_u64().unwrap() >= 1);
}

/// report.dashboard.11: the list carries each brief's advisory findings (rule
/// id, line, matched text) and the language source, and a brief that has only
/// advisories stays approvable. The grouping by rule and the badge are the
/// page's own (`packages/dashboard`).
#[test]
fn report_dashboard_11_lists_the_advisory_findings_of_a_brief_without_blockers() {
    let dir = repo_with_briefs();
    let text = clean_brief("Ready change").replace(" ==**F2 covers the failure path.**==", "");
    let lines: Vec<&str> = text.lines().collect();
    // The key-sentence finding sits on the first body line of the acceptance section.
    let heading = lines.iter().position(|l| l.starts_with("## Acceptance")).unwrap();
    let acceptance_line = (heading + 1..lines.len()).find(|&i| !lines[i].trim().is_empty()).unwrap() + 1;
    std::fs::write(brief_path(dir.path(), "ready"), &text).unwrap();
    let live = serve(dir.path());

    let ready = brief_of(&live, "ready");
    assert_eq!(ready["approvable"], true, "{ready}");
    assert_eq!(ready["languageSource"], "detected", "{ready}");
    let findings = ready["findings"].as_array().unwrap();
    let key: Vec<&Value> = findings.iter().filter(|f| f["rule"] == "prose.key-sentence").collect();
    assert_eq!(key.len(), 1, "{ready}");
    assert_eq!((key[0]["severity"].as_str(), key[0]["section"].as_str()), (Some("advisory"), Some("acceptance")));
    assert_eq!(key[0]["line"].as_u64(), Some(acceptance_line as u64), "{}", key[0]);
    assert_eq!(key[0]["message"], "no key sentence", "the matched text is the finding's excerpt");
    assert!(key[0]["hint"].as_str().is_some_and(|h| h.contains("==**")), "{}", key[0]);
    let advisories = findings.iter().filter(|f| f["severity"] == "advisory").count() as u64;
    assert!(advisories >= 1);
    assert_eq!(ready["counts"], json!({ "blockers": 0, "advisories": advisories }), "{ready}");
}

/// report.dashboard.05: sections are found by marker, so a Korean brief reads the same.
#[test]
fn report_dashboard_05_reads_korean_headings_by_their_markers() {
    let dir = repo_with_briefs();
    let text = clean_brief("Ready change").replace("## User flow", "## 사용자 흐름").replace("## Acceptance", "## 인수 기준");
    std::fs::write(brief_path(dir.path(), "ready"), text).unwrap();
    let live = serve(dir.path());
    let ready = brief_of(&live, "ready");
    let flow = ready["sections"].as_array().unwrap().iter().find(|s| s["key"] == "flow").unwrap();
    assert_eq!(flow["heading"], "사용자 흐름");
    assert_eq!(flow["steps"].as_array().unwrap().len(), 2);
    let acceptance = ready["sections"].as_array().unwrap().iter().find(|s| s["key"] == "acceptance").unwrap();
    assert_eq!(acceptance["heading"], "인수 기준");
}

/// report.dashboard.06: approving from the page records what the CLI records.
#[test]
fn report_dashboard_06_approving_from_the_page_equals_canon_brief_approve() {
    let dir = repo_with_briefs();
    let twin = TempDir::new().unwrap();
    copy_tree(dir.path(), twin.path());
    let live = serve(dir.path());

    let reply = post_approval(&live, &approval_request(&live, "ready"));
    assert_eq!(reply.code, 200, "{}", reply.body);
    let page = reply.json();

    let cli = canon(twin.path(), &["brief", "approve",
                "ready", "--approver", "Ana", "--note", "Ship it.", "--json"]);
    assert!(cli.status.success(), "{}", String::from_utf8_lossy(&cli.stderr));
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();

    assert_eq!(page["data"], cli["data"], "the response is the `brief approve --json` data");
    assert_eq!(page["meta"]["command"], "brief approve");
    assert!(page["meta"]["next"].is_array(), "a write returns meta.next: {page}");
    let page_brief = std::fs::read_to_string(brief_path(dir.path(), "ready")).unwrap();
    let cli_brief = std::fs::read_to_string(brief_path(twin.path(), "ready")).unwrap();
    assert_eq!(without_timestamp(&page_brief), without_timestamp(&cli_brief));
    assert!(page_brief.contains("Approved: Ana "), "{page_brief}");
    let record = |repo: &Path| -> Value {
        let mut approval: Value = serde_json::from_str(&std::fs::read_to_string(repo.join(".canon/policy.approval.json")).expect("the policy approval is recorded")).unwrap();
        assert!(approval.as_object_mut().unwrap().remove("approved_at").is_some(), "the policy approval carries its time: {approval}");
        approval
    };
    assert_eq!(record(dir.path()), record(twin.path()));

    // Approved, it leaves the list.
    let ids: Vec<String> = get(&live, "/api/briefs").json()["data"]["briefs"].as_array().unwrap().iter().map(|b| b["changeId"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, ["blocked"]);
}

/// report.dashboard.07: bytes that changed since the page loaded are refused.
#[test]
fn report_dashboard_07_refuses_a_brief_that_changed_since_the_page_loaded() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let request = approval_request(&live, "ready");
    let policy_before = policy_approval(dir.path());
    let path = brief_path(dir.path(), "ready");
    let edited = format!("{}\nOne more line.\n", std::fs::read_to_string(&path).unwrap());
    std::fs::write(&path, &edited).unwrap();

    let reply = post_approval(&live, &request);
    assert_eq!(reply.code, 409, "{}", reply.body);
    let error = &reply.json()["error"];
    assert_eq!(error["code"], "BRIEF_CHANGED_SINCE_PAGE_LOAD");
    assert!(error["hint"].as_str().unwrap().contains("reload"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), edited, "nothing was written");
    assert_eq!(policy_approval(dir.path()), policy_before, "no policy approval was recorded");
}

/// report.dashboard.08: a brief with blockers is not approved from the page.
#[test]
fn report_dashboard_08_refuses_a_brief_with_blockers() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let before = std::fs::read(brief_path(dir.path(), "blocked")).unwrap();
    let policy_before = policy_approval(dir.path());
    let reply = post_approval(&live, &approval_request(&live, "blocked"));
    assert_eq!(reply.code, 409, "{}", reply.body);
    assert_eq!(reply.json()["error"]["code"], "BRIEF_HAS_BLOCKERS");
    assert!(reply.json()["error"]["hint"].as_str().unwrap().len() > 10);
    assert_eq!(std::fs::read(brief_path(dir.path(), "blocked")).unwrap(), before);
    assert_eq!(policy_approval(dir.path()), policy_before, "no policy approval was recorded");
}

/// report.dashboard.09: a request that is not the page's own is refused before the brief is read.
#[test]
fn report_dashboard_09_refuses_requests_that_are_not_the_pages_own() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let port = live.port;
    let request = approval_request(&live, "ready");
    let body = request.to_string();
    let before = std::fs::read(brief_path(dir.path(), "ready")).unwrap();
    let with = |name: &str, value: Option<String>| -> Vec<(&'static str, String)> {
        let mut headers = page_headers(port, body.len());
        headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        if let Some(value) = value {
            headers.push((Box::leak(name.to_string().into_boxed_str()), value));
        }
        headers
    };
    let cases: Vec<(&str, Vec<(&'static str, String)>, u16, &str)> = vec![
        ("foreign Origin", with("Origin", Some("http://evil.example".into())), 403, "DASHBOARD_ORIGIN_REFUSED"),
        ("another port's Origin", with("Origin", Some(format!("http://127.0.0.1:{}", port.wrapping_add(1)))), 403, "DASHBOARD_ORIGIN_REFUSED"),
        ("no Origin", with("Origin", None), 403, "DASHBOARD_ORIGIN_REFUSED"),
        ("foreign Host", with("Host", Some("evil.example".into())), 403, "DASHBOARD_ORIGIN_REFUSED"),
        ("rebound Host", with("Host", Some(format!("evil.example:{port}"))), 403, "DASHBOARD_ORIGIN_REFUSED"),
        ("form content type", with("Content-Type", Some("application/x-www-form-urlencoded".into())), 400, "DASHBOARD_BAD_REQUEST"),
        ("text content type", with("Content-Type", Some("text/plain".into())), 400, "DASHBOARD_BAD_REQUEST"),
        ("no content type", with("Content-Type", None), 400, "DASHBOARD_BAD_REQUEST"),
        ("no length", with("Content-Length", None), 400, "DASHBOARD_BAD_REQUEST"),
    ];
    for (name, headers, code, error) in cases {
        let reply = http(port, "POST", "/api/briefs/approve", &headers, body.as_bytes());
        assert_eq!(reply.code, code, "{name}: {}", reply.body);
        assert_eq!(reply.json()["error"]["code"], error, "{name}");
        assert!(reply.json()["error"]["hint"].as_str().is_some_and(|h| !h.is_empty()), "{name}: a concrete hint");
    }
    assert_eq!(std::fs::read(brief_path(dir.path(), "ready")).unwrap(), before, "no refused request wrote");
    let wrong_method = http(port, "GET", "/api/briefs/approve", &[("Host", format!("127.0.0.1:{port}"))], b"");
    assert_eq!(wrong_method.code, 405);
    assert!(wrong_method.head.contains("Allow: POST"));
    for path in ["/api/status", "/api/briefs", "/api/briefs/approve", "/", "/index.html", "/briefs.html", "/assets/app.js", "/live-snapshot/manifest.json", "/live-snapshot/mart_subjects.parquet", "/nonexistent"] {
        for host in ["evil.example", "evil.example:80", &format!("evil.example:{port}"), &format!("127.0.0.1:{}", port.wrapping_add(1)), ""] {
            let rebound = http(port, "GET", path, &[("Host", host.to_string())], b"");
            assert_eq!(rebound.code, 403, "GET {path} under Host `{host}`: {}", rebound.body);
            assert_eq!(rebound.json()["error"]["code"], "DASHBOARD_ORIGIN_REFUSED", "GET {path} under Host `{host}`");
        }
        let no_host = http(port, "GET", path, &[], b"");
        assert_eq!(no_host.code, 403, "GET {path} with no Host");
    }
    assert_eq!(get(&live, "/briefs.html").code, 200, "the page's own Host is served");
    let localhost = http(port, "POST", "/api/briefs/approve", &[("Host", format!("localhost:{port}")), ("Origin", format!("http://localhost:{port}")), ("Content-Type", "application/json; charset=utf-8".into()), ("Content-Length", body.len().to_string())], body.as_bytes());
    assert_eq!(localhost.code, 200, "localhost is the same dashboard: {}", localhost.body);
    assert_ne!(std::fs::read(brief_path(dir.path(), "ready")).unwrap(), before, "only the localhost request wrote");
}

/// report.dashboard.09: a body over 64 KiB is refused from its Content-Length alone, before any body byte is read.
#[test]
fn report_dashboard_09_refuses_an_oversize_body_without_waiting_for_it() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let before = std::fs::read(brief_path(dir.path(), "ready")).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", live.port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let headers = page_headers(live.port, 70_000);
    let mut request = String::from("POST /api/briefs/approve HTTP/1.1\r\n");
    for (name, value) in &headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    // Headers only: the connection stays open and not one body byte follows.
    stream.write_all(request.as_bytes()).unwrap();
    let started = std::time::Instant::now();
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    while !raw.windows(4).any(|w| w == b"\r\n\r\n") || !String::from_utf8_lossy(&raw).contains('}') {
        let n = stream.read(&mut chunk).expect("an answer before the client sent any body");
        assert!(n > 0, "the server closed without answering");
        raw.extend_from_slice(&chunk[..n]);
    }
    assert!(started.elapsed() < Duration::from_secs(2), "answered after {:?}: the guard waited for the body", started.elapsed());
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 400"), "{text}");
    assert!(text.contains("DASHBOARD_BAD_REQUEST") && text.contains("the body is 70000 bytes; the limit is 65536"), "{text}");
    assert_eq!(std::fs::read(brief_path(dir.path(), "ready")).unwrap(), before);
}

/// report.dashboard.10: an empty or multi-line answer, an unknown change and a malformed body are refused and write nothing.
#[test]
fn report_dashboard_10_leaves_the_brief_untouched_on_every_refusal() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let port = live.port;
    let before = std::fs::read(brief_path(dir.path(), "ready")).unwrap();
    let policy_before = policy_approval(dir.path());
    let sha = brief_of(&live, "ready")["sha256"].clone();
    let post = |headers: Vec<(&'static str, String)>, body: &str| http(port, "POST", "/api/briefs/approve", &headers, body.as_bytes());

    for (by, note, field) in [("", "Ship it.", "--approver"), ("Ana", "", "--note"), ("Ana\nBob", "Ship it.", "--approver"), ("Ana", "one\ntwo", "--note")] {
        let reply = post_approval(&live, &json!({ "changeId": "ready", "by": by, "note": note, "sha256": sha }));
        assert_eq!(reply.code, 400, "{by:?} {note:?}: {}", reply.body);
        assert_eq!(reply.json()["error"]["code"], "APPROVAL_INCOMPLETE");
        assert!(reply.json()["error"]["message"].as_str().unwrap().contains(field));
    }
    let unknown = post_approval(&live, &json!({ "changeId": "no-such-change", "by": "Ana", "note": "Go.", "sha256": sha }));
    assert_eq!((unknown.code, unknown.json()["error"]["code"].clone()), (404, json!("BRIEF_NOT_FOUND")));
    let archived = post_approval(&live, &json!({ "changeId": "archived", "by": "Ana", "note": "Go.", "sha256": sha }));
    assert_eq!(archived.code, 404, "an archived change is not live");
    let malformed = post(page_headers(port, 2), "{}");
    assert_eq!((malformed.code, malformed.json()["error"]["code"].clone()), (400, json!("DASHBOARD_BAD_REQUEST")));
    let bad_id = post_approval(&live, &json!({ "changeId": "../../etc", "by": "Ana", "note": "Go.", "sha256": sha }));
    assert_eq!(bad_id.code, 400);

    assert_eq!(std::fs::read(brief_path(dir.path(), "ready")).unwrap(), before);
    assert_eq!(policy_approval(dir.path()), policy_before, "no policy approval was recorded");
}

/// report.dashboard.04: no snapshot is a message, never demo data.
#[test]
fn report_dashboard_04_a_repo_whose_snapshot_cannot_be_generated_is_still_served_and_says_why() {
    let dir = repo_with_briefs();
    let live = serve(dir.path());
    let warnings = live.startup["meta"]["warnings"].as_array().expect("meta.warnings");
    assert!(warnings.len() == 1 && warnings[0].as_str().unwrap().contains("duckdb"), "{}", live.startup);
    assert_eq!(live.startup["data"]["snapshotAvailable"], false);
    let stderr = std::fs::read_to_string(&live.stderr).unwrap();
    assert!(stderr.is_empty(), "JSON mode writes nothing to stderr: {stderr:?}");
    let warning = warnings[0].as_str().unwrap();
    assert!(warning.contains("duckdb") && warning.contains("the panels page says so instead of showing panels") && warning.contains("install DuckDB"), "{warning}");

    let status = get(&live, "/api/status");
    assert_eq!(status.code, 200);
    let snapshot = &status.json()["data"]["snapshot"];
    assert_eq!(snapshot["available"], false);
    assert!(snapshot["reason"].as_str().unwrap().contains("duckdb"), "{snapshot}");
    assert!(snapshot["fix"].as_str().unwrap().contains("install DuckDB"), "{snapshot}");

    assert_eq!(get(&live, "/briefs.html").code, 200, "the briefs page works without a snapshot");
    assert_eq!(get(&live, "/api/briefs").code, 200);
    assert_eq!(get(&live, "/live-snapshot/manifest.json").code, 404);
    let demo = get(&live, "/snapshot/manifest.json");
    assert_eq!(demo.code, 404, "the demo fixture is never served: {}", demo.body);
}

/// report.dashboard.04: an explicit snapshot with a manifest is served and reported available.
#[test]
fn report_dashboard_04_serves_an_explicit_snapshot_without_duckdb() {
    let dir = repo_with_briefs();
    let snapshot = dir.path().join("my-snapshot");
    std::fs::create_dir_all(&snapshot).unwrap();
    std::fs::write(snapshot.join("manifest.json"), r#"{"ok":true}"#).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dashboard", "--port", "0", "--json", "--snapshot", snapshot.to_str().unwrap()])
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let startup: Value = serde_json::from_str(&line).unwrap();
    let live = Live { port: startup["data"]["url"].as_str().unwrap().trim_start_matches("http://127.0.0.1:").split('/').next().unwrap().parse().unwrap(), child, startup, stderr: dir.path().join("unused") };
    assert_eq!(get(&live, "/api/status").json()["data"]["snapshot"]["available"], true);
    assert_eq!(get(&live, "/live-snapshot/manifest.json").body, r#"{"ok":true}"#);
}

/// report.dashboard.03: a taken port is its own refusal, naming the way out.
#[test]
fn report_dashboard_03_a_port_in_use_is_refused_with_the_flag_that_fixes_it() {
    let dir = repo_with_briefs();
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let out = canon(dir.path(), &["dashboard", "--port", &port, "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"]["code"], "DASHBOARD_PORT_IN_USE");
    let hint = error["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("--port <n>") && hint.contains("--port 0"), "{hint}");
    assert!(error["error"]["message"].as_str().unwrap().contains(&port));
}

/// report.dashboard.02: with no app to serve, the refusal names how to get one.
#[test]
fn report_dashboard_02_no_app_is_refused_and_another_projects_dashboard_is_never_served() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    let package = repo.join("packages/dashboard");
    std::fs::create_dir_all(package.join("dist")).unwrap();
    std::fs::write(package.join("package.json"), r#"{"name":"@acme/dashboard"}"#).unwrap();
    std::fs::write(package.join("dist/index.html"), "<html>someone else's app</html>").unwrap();
    let out = canon(repo, &["dashboard", "--port", "0", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"]["code"], "DASHBOARD_NOT_BUILT");
    let hint = error["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("bun add -g @journeykit/canon") && hint.contains("bun run --cwd packages/dashboard build"), "{hint}");
}

/// report.dashboard.02: a `--repo` that does not exist is refused and nothing is created there.
#[test]
fn report_dashboard_02_a_missing_repo_is_refused_and_not_created() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("no-such-repo");
    let out = canon(dir.path(), &["dashboard", "--repo", missing.to_str().unwrap(), "--port", "0", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stdout));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"]["code"], "REPO_UNREADABLE");
    assert!(error["error"]["message"].as_str().unwrap().contains("does not exist"));
    assert!(error["error"]["hint"].as_str().unwrap().contains("--repo"));
    assert!(!missing.exists(), "the refused repo was created");
}
