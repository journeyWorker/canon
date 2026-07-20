//! `cargo xtask <check-generated|write>` (aliased by the repo's
//! `.cargo/config.toml`) — the developer/CI entry point for
//! canon-model's generated `JOIN_SPINE.md` + `schemas/*.schema.json`
//! (S1 design D3, tasks 2.3/3.2). The same regeneration logic also runs
//! as a `cargo test --workspace` assertion
//! (`canon_model::gen::tests::committed_generated_output_matches_current_source`),
//! so `check-generated` is a convenience CLI, not the only place drift
//! is caught.

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("check-generated") => check_generated(),
        Some("write") => write_generated(),
        other => {
            eprintln!("usage: cargo xtask <check-generated|write>");
            if let Some(cmd) = other {
                eprintln!("unknown subcommand: {cmd}");
            }
            std::process::exit(2);
        }
    }
}

fn check_generated() {
    let report = canon_model::gen::check();
    if report.is_clean() {
        println!("canon-model generated output is up to date (JOIN_SPINE.md + schemas/*.schema.json).");
        return;
    }
    for path in &report.missing_or_stale {
        eprintln!("DRIFT (missing or stale): {}", path.display());
    }
    for path in &report.unexpected {
        eprintln!("DRIFT (unexpected committed file, no longer generated): {}", path.display());
    }
    eprintln!("Run `cargo xtask write` to regenerate, then commit the diff.");
    std::process::exit(1);
}

fn write_generated() {
    canon_model::gen::write().expect("writing canon-model's generated output");
    println!("wrote JOIN_SPINE.md + schemas/*.schema.json");
}
