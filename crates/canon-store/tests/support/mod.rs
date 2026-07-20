//! Spins up a genuinely LOCAL, ephemeral, unix-socket-only (no TCP
//! listener at all) Postgres 17 cluster for a real (not SQL-string-only)
//! `PgTier` integration test — zero network egress, zero cloud
//! credentials, "local Postgres" in the exact sense `psql`/`initdb`/
//! `pg_ctl` being present on this dev machine's PATH already implies
//! (S2 assignment: "NO cloud creds available" — this is a substitute,
//! not a workaround, for that constraint, distinct from the `live-pg`
//! Cargo feature's hosted-Postgres-specific gating). Gracefully unavailable
//! (`try_start` returns `None`) on any machine lacking `initdb`/`pg_ctl`
//! on PATH, so `cargo test --workspace` never depends on this.

use std::path::PathBuf;
use std::process::Command;

pub struct LocalPg {
    _work_dir: tempfile::TempDir,
    pgdata: PathBuf,
    socket_dir: PathBuf,
    port: u16,
    started: bool,
}

impl LocalPg {
    pub fn try_start() -> Option<Self> {
        if Command::new("initdb").arg("--version").output().is_err() || Command::new("pg_ctl").arg("--version").output().is_err() {
            return None;
        }

        let work_dir = tempfile::tempdir().ok()?;
        let pgdata = work_dir.path().join("data");
        let socket_dir = work_dir.path().join("sock");
        std::fs::create_dir_all(&socket_dir).ok()?;
        let port: u16 = 55432;

        let initdb = Command::new("initdb")
            .args(["-D", pgdata.to_str()?, "-U", "postgres", "--auth=trust", "--no-sync", "-E", "UTF8"])
            .output()
            .ok()?;
        if !initdb.status.success() {
            eprintln!("initdb failed: {}", String::from_utf8_lossy(&initdb.stderr));
            return None;
        }

        let start = Command::new("pg_ctl")
            .args([
                "-D",
                pgdata.to_str()?,
                "-o",
                &format!("-c listen_addresses='' -c unix_socket_directories={} -c port={port}", socket_dir.display()),
                "-w",
                "-l",
                work_dir.path().join("pg.log").to_str()?,
                "start",
            ])
            .output()
            .ok()?;
        if !start.status.success() {
            eprintln!("pg_ctl start failed: {}", String::from_utf8_lossy(&start.stderr));
            return None;
        }

        Some(Self { _work_dir: work_dir, pgdata, socket_dir, port, started: true })
    }

    pub fn dsn(&self) -> String {
        format!("postgres:///postgres?host={}&port={}&user=postgres", self.socket_dir.display(), self.port)
    }
}

impl Drop for LocalPg {
    fn drop(&mut self) {
        if self.started {
            let _ = Command::new("pg_ctl").args(["-D", self.pgdata.to_str().unwrap_or_default(), "-m", "fast", "stop"]).output();
        }
    }
}
