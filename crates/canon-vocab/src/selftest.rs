//! [`selftest`]: the fail-soft `Result<usize, Vec<String>>` entry point a
//! `canon selftest` aggregator (S10 wave contract, deferred to a Wave-2
//! `canon-cli` change — NOT built here) registers this crate's own fixture
//! suite through, so it can run this crate's corpus without a `cargo test`
//! harness. Wraps the SAME resolve/validate/compile round-trip
//! `tests/canon_core_selftest.rs` proves as five separate `#[test]`s — the
//! REAL, checked-in `.canon/vocab/canon.core/` manifest, this crate's own
//! checked-in `fixtures/atoms/*.yaml` corpus, resolved and validated exactly
//! as any real consumer repo would (module doc there: this repo has no real
//! `.canon/policy.yaml` yet, so this module supplies its own, in a COPY of
//! the tree, never touching the real one).
//!
//! # Rebindable root, no `tempfile` dependency
//! [`GateCtx::from_fixture`]-style "rebindable root" (`canon-gate::context`'s
//! own doc; `canon-gate::selftest`'s "fixture corpora with rebindable
//! roots" testing-strategy discipline, design.md §8) — every call gets a
//! FRESH scratch directory, never touches the real repo tree. `tempfile` is
//! this crate's `[dev-dependencies]` only (not linked into the compiled
//! library), and this change's own constraint forbids moving it into
//! `[dependencies]` (that edits `Cargo.lock`'s `canon-vocab` package entry)
//! — [`ScratchDir`] is a minimal std-only equivalent (unique
//! `std::env::temp_dir()` subdirectory, `Drop`-cleaned).
//!
//! Side-effect-free against the real repo: every read is scoped to a copy
//! under [`ScratchDir`] plus this crate's own `fixtures/atoms/` (read-only);
//! nothing under the real repo's `.canon/` tree is ever written.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A `std`-only, `Drop`-cleaned scratch directory — the `tempfile::TempDir`
/// equivalent this module needs without adding `tempfile` to
/// `[dependencies]` (module doc).
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("canon-vocab-selftest-{}-{nanos}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| format!("create scratch dir {}: {e}", path.display()))?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn fixture(name: &str) -> Result<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/atoms").join(name);
    std::fs::read_to_string(&path).map_err(|e| format!("read fixture {name} ({}): {e}", path.display()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create dir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read dir {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read dir entry under {}: {e}", src.display()))?;
        let dst_path = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path).map_err(|e| format!("copy {} -> {}: {e}", entry.path().display(), dst_path.display()))?;
        }
    }
    Ok(())
}

/// A scratch dir carrying a COPY of the real `.canon/vocab/canon.core/` tree
/// plus a selftest-owned `.canon/policy.yaml` declaring the evidence kinds
/// `fixtures/atoms/good.yaml` uses (module doc).
fn project_with_real_canon_core() -> Result<ScratchDir, String> {
    let scratch = ScratchDir::new()?;
    let core_rel = format!("{}canon.core", canon_model::paths::VOCAB_DIR);
    copy_dir(&repo_root().join(&core_rel), &scratch.path().join(&core_rel))?;
    std::fs::write(scratch.path().join(canon_model::paths::POLICY_FILE), "trust_required:\n  test-run: agent\n  manual-review: human\n")
        .map_err(|e| format!("write selftest .canon/policy.yaml: {e}"))?;
    Ok(scratch)
}

/// Resolve + validate + compile the real `canon.core` manifest against this
/// crate's own fixture corpus (module doc). `Ok(n)` — `n` fixtures passed
/// (the manifest resolution itself, `good.yaml`, each `bad-*.yaml`, plus
/// task-atom and handoff-atom compile/round-trip); `Err(failures)` — one
/// human-readable description per failed fixture, EVERY failure collected
/// (never short-circuits on the first one), so a caller sees the whole
/// picture in one call. Never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let mut failures = Vec::new();
    let mut passed = 0usize;

    let scratch = match project_with_real_canon_core() {
        Ok(s) => s,
        Err(e) => return Err(vec![format!("could not build the canon.core fixture project: {e}")]),
    };

    let (snap, resolve_diags) = crate::resolve_snapshot(scratch.path(), None);
    if resolve_diags.is_empty() {
        let has_directives = ["task", "handoff-dev", "handoff-design", "handoff-content", "handoff-test"].iter().all(|d| snap.directive(d).is_some());
        let mut kinds = snap.evidence_kinds.clone();
        kinds.sort();
        if has_directives && kinds == vec!["manual-review".to_string(), "test-run".to_string()] {
            passed += 1;
        } else {
            failures.push(format!("canon.core: resolved but shape mismatch (directives ok={has_directives}, evidence_kinds={kinds:?})"));
        }
    } else {
        failures.push(format!("canon.core: resolution produced diagnostics: {resolve_diags:?}"));
    }

    let good_atoms = match fixture("good.yaml").and_then(|s| crate::atom::parse_atoms_file(&s).map_err(|e| format!("{e:?}"))) {
        Ok(atoms) => Some(atoms),
        Err(e) => {
            failures.push(format!("good.yaml: {e}"));
            None
        }
    };

    if let Some(atoms) = &good_atoms {
        let diags = crate::atom::validate_atoms(atoms, &snap);
        if diags.is_empty() {
            passed += 1;
        } else {
            failures.push(format!("good.yaml: expected zero diagnostics, got {diags:?}"));
        }
    }

    let bad_cases = [
        ("bad-unknown-directive.yaml", "E-UNKNOWN-DIRECTIVE"),
        ("bad-unknown-attr.yaml", "E-UNKNOWN-ATTR"),
        ("bad-missing-attr.yaml", "E-MISSING-ATTR"),
        ("bad-bad-enum.yaml", "E-BAD-ENUM"),
        ("bad-bad-evidence-kind.yaml", "E-BAD-EVIDENCE-KIND"),
        ("bad-unknown-evidence-field.yaml", "E-UNKNOWN-ATTR"),
    ];
    for (file, expected_code) in bad_cases {
        match fixture(file).and_then(|s| crate::atom::parse_atoms_file(&s).map_err(|e| format!("{e:?}"))) {
            Ok(atoms) => {
                let diags = crate::atom::validate_atoms(&atoms, &snap);
                if diags.iter().any(|d| d.code == expected_code) {
                    passed += 1;
                } else {
                    failures.push(format!("{file}: expected {expected_code}, got {diags:?}"));
                }
            }
            Err(e) => failures.push(format!("{file}: {e}")),
        }
    }

    if let Some(atoms) = &good_atoms {
        use canon_model::{Actor, Envelope, RecordKind, RoleId};

        let task_atoms: Vec<_> = atoms.iter().filter(|a| a.tag == "task").collect();
        let mut task_ok = task_atoms.len() == 2;
        for atom in &task_atoms {
            let Ok(role) = RoleId::parse("implementer") else {
                task_ok = false;
                failures.push("task round-trip: RoleId::parse(\"implementer\") failed".to_string());
                break;
            };
            let envelope = Envelope::new(1, RecordKind::Task, chrono::Utc::now(), Actor::new("selftest-agent", role));
            let task1 = match crate::compile_task(atom, &snap, envelope.clone()) {
                Ok(t) => t,
                Err(d) => {
                    task_ok = false;
                    failures.push(format!("{}: compile_task failed: {d:?}", atom.id));
                    continue;
                }
            };
            let decompiled = match crate::decompile_task(&task1) {
                Ok(d) => d,
                Err(e) => {
                    task_ok = false;
                    failures.push(format!("{}: decompile_task failed: {e:?}", atom.id));
                    continue;
                }
            };
            if decompiled.id != atom.id || decompiled.tag != atom.tag || decompiled.attrs != atom.attrs {
                task_ok = false;
                failures.push(format!("{}: decompiled atom does not round-trip", atom.id));
                continue;
            }
            let task2 = match crate::compile_task(&decompiled, &snap, envelope) {
                Ok(t) => t,
                Err(d) => {
                    task_ok = false;
                    failures.push(format!("{}: recompile after decompile failed: {d:?}", atom.id));
                    continue;
                }
            };
            if task1.task_id != task2.task_id || task1.title != task2.title || task1.status != task2.status || task1.evidence_note != task2.evidence_note {
                task_ok = false;
                failures.push(format!("{}: recompiled task diverges from the original compile", atom.id));
            }
        }
        if task_ok {
            passed += 1;
        } else if task_atoms.len() != 2 {
            failures.push(format!("good.yaml: expected exactly two task atoms, found {}", task_atoms.len()));
        }
    }

    if let Some(atoms) = &good_atoms {
        let handoff_atoms: Vec<_> = atoms.iter().filter(|a| a.tag.starts_with("handoff-")).collect();
        let mut handoff_ok = handoff_atoms.len() == 2;
        if handoff_atoms.len() != 2 {
            failures.push(format!("good.yaml: expected exactly two handoff atoms, found {}", handoff_atoms.len()));
        }
        for atom in &handoff_atoms {
            match crate::compile_handoff_body(atom, &snap) {
                Ok(body) => {
                    let rendered = crate::render_handoff_body(&body);
                    if rendered.is_empty() || !rendered.contains(body.domain.as_str()) {
                        handoff_ok = false;
                        failures.push(format!("{}: rendered handoff body missing its own domain", atom.id));
                    }
                }
                Err(d) => {
                    handoff_ok = false;
                    failures.push(format!("{}: compile_handoff_body failed: {d:?}", atom.id));
                }
            }
        }
        if handoff_ok {
            passed += 1;
        }
    }

    if failures.is_empty() {
        Ok(passed)
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_passes_against_the_real_repo_fixture_corpus() {
        match selftest() {
            Ok(passed) => assert_eq!(passed, 10, "expected all 10 fixture checks (1 core + 1 good + 6 bad + 1 task + 1 handoff) to pass"),
            Err(failures) => panic!("selftest failures: {failures:?}"),
        }
    }
}
