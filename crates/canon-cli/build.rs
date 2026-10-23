//! Embeds the built dashboard app (`packages/dashboard/dist`) into the
//! binary, so `canon dashboard` runs in any repo (0.25 D2).
//!
//! `CANON_DASHBOARD_DIST=<dir>` at build time names the built app: every
//! file under it except `snapshot/**` (the demo fixture the app is built
//! with, which no served page may show) is gzip-compressed into `OUT_DIR`
//! and listed in `dashboard_embedded.rs`, a `static EMBEDDED_APP` table
//! that `src/dashboard.rs` includes. The variable unset embeds nothing
//! (a debug build, a `cargo test`), and the table is empty. The variable
//! set, even to an empty string, with no `index.html` under it fails the
//! build: a release must not silently ship without its app.
//!
//! The guide (0.28 D10): every file of `canon/skills/` (`SKILL.src.md`,
//! `reference/**`, `scripts/**`) is embedded as text, in sorted path order,
//! into `guide_embedded.rs`, a `static GUIDE_FILES` table that `src/guide.rs`
//! includes. The directory is part of the repo, so a build without it fails.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;

const DIST_ENV: &str = "CANON_DASHBOARD_DIST";

/// The skill sources the guide is embedded from, relative to this crate.
const GUIDE_DIR: &str = "../../canon/skills";

/// The entries of the guide directory that are embedded.
const GUIDE_ENTRIES: [&str; 3] = ["SKILL.src.md", "reference", "scripts"];

/// The demo fixture directory at the root of the built app.
const DEMO_FIXTURE: &str = "snapshot";

fn main() {
    println!("cargo:rerun-if-env-changed={DIST_ENV}");
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let mut table = String::from("pub static EMBEDDED_APP: &[EmbeddedFile] = &[\n");

    if let Some(dist) = std::env::var_os(DIST_ENV) {
        let dist = PathBuf::from(dist);
        if !dist.join("index.html").is_file() {
            panic!("{DIST_ENV}={} has no index.html: build the app first (`bun install --frozen-lockfile && bun run --cwd packages/dashboard build`)", dist.display());
        }
        let dist = dist.canonicalize().expect("the app directory resolves");
        println!("cargo:rerun-if-changed={}", dist.display());
        let gz_dir = out.join("dashboard");
        fs::create_dir_all(&gz_dir).expect("create the embedded app directory");
        let mut files = Vec::new();
        collect(&dist, &dist, &mut files);
        files.sort();
        for (n, relative) in files.iter().enumerate() {
            let bytes = fs::read(dist.join(relative)).unwrap_or_else(|e| panic!("read {relative}: {e}"));
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&bytes).expect("gzip into memory");
            let gz_path = gz_dir.join(format!("{n}.gz"));
            fs::write(&gz_path, encoder.finish().expect("gzip into memory")).unwrap_or_else(|e| panic!("write {}: {e}", gz_path.display()));
            table.push_str(&format!("    EmbeddedFile {{ path: {relative:?}, gz: include_bytes!({:?}) }},\n", gz_path.display().to_string()));
        }
    }

    table.push_str("];\n");
    fs::write(out.join("dashboard_embedded.rs"), table).expect("write the embedded app table");

    embed_guide(&out);
}

/// Writes `guide_embedded.rs`: `(path, text)` of every guide file, sorted by path.
fn embed_guide(out: &Path) {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join(GUIDE_DIR).canonicalize().unwrap_or_else(|e| panic!("the guide sources {GUIDE_DIR} are missing: {e}"));
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    for entry in GUIDE_ENTRIES {
        let path = root.join(entry);
        if path.is_dir() {
            collect(&root, &path, &mut files);
        } else if path.is_file() {
            files.push(entry.to_string());
        } else {
            panic!("the guide source {} is missing", path.display());
        }
    }
    files.sort();
    let mut table = String::from("pub static GUIDE_FILES: &[(&str, &str)] = &[\n");
    for relative in &files {
        let path = root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        table.push_str(&format!("    ({relative:?}, include_str!({:?})),\n", path.display().to_string()));
    }
    table.push_str("];\n");
    fs::write(out.join("guide_embedded.rs"), table).expect("write the embedded guide table");
}

/// Every file under `dir` as a `/`-separated path relative to `root`,
/// skipping the demo fixture at the root.
fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        let relative = path.strip_prefix(root).expect("under the root");
        if relative == Path::new(DEMO_FIXTURE) {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            files.push(relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
        }
    }
}
