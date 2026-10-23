//! The guide the binary carries (0.28 D10): the skill sources of `canon/skills/`
//! embedded at build time (`build.rs`), so `canon skills show <NAME>` prints the
//! text that matches this very binary, and `canon skills install` writes a stub
//! that points here instead of a copy that can drift.
//!
//! A guide name (O8) is `canon` (the core guide, `SKILL.src.md`) or the stem of
//! a `reference/<stem>.md` file (`role-implementer`, `canon-gate`, ...). The
//! `scripts/**` files are not guides: they are what `skills install` projects
//! next to the stub.

use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/guide_embedded.rs"));

/// The name of the core guide.
pub const CORE: &str = "canon";

/// The core guide's path in the embedded table.
const CORE_FILE: &str = "SKILL.src.md";

const REFERENCE_DIR: &str = "reference/";
const SCRIPTS_DIR: &str = "scripts/";

/// Every embedded file as `(path relative to canon/skills, text)`, sorted by path.
pub fn files() -> &'static [(&'static str, &'static str)] {
    GUIDE_FILES
}

/// The embedded text of `path` (`SKILL.src.md`, `reference/x.md`, `scripts/x.sh`).
fn file(path: &str) -> Option<&'static str> {
    GUIDE_FILES.iter().find(|(name, _)| *name == path).map(|(_, text)| *text)
}

/// The core guide: `SKILL.src.md`, frontmatter included.
pub fn core() -> &'static str {
    file(CORE_FILE).expect("the build embeds SKILL.src.md")
}

/// The references as `(stem, text)`, sorted by stem.
fn references() -> impl Iterator<Item = (&'static str, &'static str)> {
    GUIDE_FILES.iter().filter_map(|(path, text)| path.strip_prefix(REFERENCE_DIR)?.strip_suffix(".md").map(|stem| (stem, *text)))
}

/// The scripts to project next to the stub, as `(path relative to the skill directory, text)`.
pub fn scripts() -> impl Iterator<Item = (&'static str, &'static str)> {
    GUIDE_FILES.iter().filter(|(path, _)| path.starts_with(SCRIPTS_DIR)).map(|(path, text)| (*path, *text))
}

/// The guide names: `canon`, then each reference stem in sorted order.
pub fn names() -> &'static [&'static str] {
    static NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| std::iter::once(CORE).chain(references().map(|(stem, _)| stem)).collect());
    &NAMES
}

/// The guide `name` names, or `None` for a name that is not embedded.
pub fn guide(name: &str) -> Option<&'static str> {
    if name == CORE {
        return Some(core());
    }
    references().find(|(stem, _)| *stem == name).map(|(_, text)| text)
}

/// The core guide followed by every reference, each after a rule and a comment naming its file.
pub fn full() -> String {
    let mut out = String::from(core());
    for (stem, text) in references() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("\n---\n\n<!-- reference/{stem}.md -->\n\n"));
        out.push_str(text);
    }
    out
}

/// The `---` frontmatter block of `source` (both fences, a trailing newline), or `None` without one.
fn frontmatter(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some(&source[..4 + end + 5])
}

/// What canon is, the loop in five lines and where the rest lives: the body of every stub.
fn stub_body(version: &str) -> String {
    format!(
        "
# Canon

This repo is managed with canon (canon {version}). Canon records the work as evidence and a gate grades it. This file is a stub: the full guide is built into the `canon` binary, so it always matches the version you run.

## The loop

1. Brief: write it, run `canon brief check`, then stop and report it. Only the human approves it (`canon brief approve`).
2. Spec: add the subject and its scenarios, failure paths included (`canon subject new`, `canon scenario new`).
3. Build the change in small units, one scenario at a time.
4. Evidence: record each result with `canon evidence add`; a different agent reviews it (`canon review scenario`).
5. Finish: `canon gate check` is clean and `canon status` names no next step.

## The rest comes from the binary

- Run `canon skills show canon` for the full guide (`--full` appends every reference).
- Run `canon skills show role-implementer` for your role (also `role-planner`, `role-reviewer`, `role-orchestrator`).
- Never guess a flag: `canon schema` lists every command and `canon schema task done` shows one.
"
    )
}

/// The `SKILL.md` stub `canon skills install` writes: `source`'s frontmatter (so harnesses still route to the
/// skill by its name and description) and the stub body carrying `version`.
pub fn stub(source: &str, version: &str) -> String {
    let mut out = frontmatter(source).unwrap_or("---\nname: canon\ndescription: Use for build, implement, change and fix requests in a repo that has canon.yaml.\n---\n").to_string();
    out.push_str(&stub_body(version));
    out
}
