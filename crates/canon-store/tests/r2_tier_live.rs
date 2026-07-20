//! `R2Tier` against a REAL S3-compatible bucket — compiled only under
//! `--features live-r2` (so `cargo test --workspace` with no extra
//! flags never attempts network I/O) AND additionally skip-if-down at
//! runtime: a cheap TCP probe against `CANON_S3_ENDPOINT` (docker-
//! compose default `http://127.0.0.1:59000`, the repo-root
//! `docker-compose.yml` `minio` service — operator directive
//! 2026-07-10's local-first retrofit) runs BEFORE any real `R2Tier`
//! I/O.
//!
//! Two probe outcomes:
//! - reachable: the full write/read/dedup round trip runs, and any
//!   real failure from here on is a genuine test failure (never
//!   swallowed) — a passing probe means "docker compose is up", so a
//!   subsequent I/O error is a real bug, not an environment gap.
//! - unreachable: `CANON_REQUIRE_LIVE=1` (the CI live-tier job's own
//!   env, after `docker compose up -d --wait`) turns this into a hard
//!   failure — "the live-tier CI job MUST actually exercise MinIO, not
//!   silently go green" — otherwise (a bare local `cargo test`) this
//!   is a clean, eprintln'd skip.
#![cfg(feature = "live-r2")]

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RoleId, RunId};
use canon_model::records::Trajectory;
use canon_store::r2_tier::R2Tier;
use canon_store::tier::{Tier, TierQuery};
use chrono::Utc;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:59000";
const DEFAULT_BUCKET: &str = "canon";

fn require_live() -> bool {
    std::env::var("CANON_REQUIRE_LIVE").as_deref() == Ok("1")
}

/// `scheme://host[:port][/path]` -> `(host, port)`, defaulting the port
/// from the scheme when absent. Deliberately tiny/hand-rolled — no
/// `url`-crate dependency needed for a probe this narrow (S2
/// assignment constraint: no new dependency for this retrofit).
fn endpoint_host_port(endpoint: &str) -> (String, u16) {
    let is_https = endpoint.starts_with("https://");
    let rest = endpoint.split("://").nth(1).unwrap_or(endpoint);
    let host_port = rest.split('/').next().unwrap_or(rest);
    match host_port.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(if is_https { 443 } else { 80 })),
        None => (host_port.to_string(), if is_https { 443 } else { 80 }),
    }
}

/// Best-effort "is anything listening" check — deliberately cheap and
/// side-effect-free, run before any real `R2Tier` I/O so an
/// unreachable docker-compose stack is told apart from a genuine
/// round-trip bug (this file's module doc).
fn tcp_reachable(host: &str, port: u16) -> bool {
    (host, port)
        .to_socket_addrs()
        .map(|addrs| addrs.into_iter().any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(800)).is_ok()))
        .unwrap_or(false)
}

#[test]
fn write_read_round_trip_against_a_live_minio_bucket() {
    let endpoint = std::env::var("CANON_S3_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
    let (host, port) = endpoint_host_port(&endpoint);

    if !tcp_reachable(&host, port) {
        if require_live() {
            panic!(
                "CANON_REQUIRE_LIVE=1 but MinIO at {endpoint} is unreachable — \
                 run `docker compose up -d --wait postgres minio` (repo root) first"
            );
        }
        eprintln!("skipping: MinIO at {endpoint} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
        return;
    }

    // `connect_live`'s bucket resolution is deliberately never
    // defaulted (r2_tier.rs's own doc comment) — this test supplies
    // the docker-compose default itself via the well-known
    // `CANON_S3_BUCKET` name, exactly like a local dev/CI shell would.
    if std::env::var("CANON_S3_BUCKET").is_err() {
        std::env::set_var("CANON_S3_BUCKET", DEFAULT_BUCKET);
    }

    let tier = R2Tier::connect_live("CANON_S3_BUCKET", "canon-test/").expect("connect to live MinIO");
    let actor = Actor::new("live-test", RoleId::parse("implementer").unwrap());
    let trajectory =
        Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor), RunId::default(), None, None, None, None, Some(0.5));

    let receipt = tier.write(&trajectory).expect("write");
    assert!(!receipt.deduped, "a fresh natural key must not be reported as deduped");
    assert!(receipt.location.starts_with("canon-test/kind=trajectory/"), "unexpected Hive object key: {}", receipt.location);
    assert!(receipt.location.ends_with(".parquet"), "unexpected Hive object key: {}", receipt.location);

    let redundant = tier.write(&trajectory).expect("re-write identical content");
    assert!(redundant.deduped, "identical content re-write must be a digest-dedup no-op against a live bucket too");
    assert_eq!(receipt.digest, redundant.digest);
    assert_eq!(receipt.location, redundant.location, "a dedup no-op resolves to the SAME object key, never a second object");

    let result = tier.read(&TierQuery::kind(RecordKind::Trajectory)).expect("read");
    assert!(!result.records.is_empty());
    assert!(
        result.records.iter().any(|r| r.0["run_id"] == trajectory.run_id.to_string()),
        "expected the written trajectory's own run_id back out of a live read"
    );
}
