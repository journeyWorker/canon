//! Every refusal carries a hint (0.22 D5). A consumer that reads
//! `{"error":{"code","message","hint"}}` must find a next step, not `null`.
//!
//! The test reads the sources, finds every `CliError::refused(` and
//! `CliError::failed(` construction in `crates/canon-cli/src` (outside
//! `#[cfg(test)]` code and comments), follows its builder chain
//! (`.field(…)`, `.expected(…)`, `?`, …) and fails when the chain has no
//! `.hint(`. A construction that is allowed to go without one is named in
//! [`ALLOWED`] with the reason its message already is the next step; an entry
//! that matches no construction fails too, so the list cannot rot.

use std::fs;
use std::path::{Path, PathBuf};

/// `(file, code, reason)`: constructions whose chain has no `.hint(` and may keep none.
const ALLOWED: &[(&str, &str, &str)] = &[
];

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())).map(|e| e.expect("dir entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The source text with comments and the `#[cfg(test)]` tail blanked, string
/// literals kept (the code argument is one) but their delimiters made opaque
/// to the scanner through [`skip_literal`].
fn live_source(file: &Path) -> String {
    let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    let live = text.split("#[cfg(test)]").next().expect("split yields one piece");
    live.lines().map(|l| if l.trim_start().starts_with("//") { "" } else { l }).collect::<Vec<_>>().join("\n")
}

/// The index just past the string or char literal starting at `i`, or `i`
/// itself when `bytes[i]` starts none.
fn skip_literal(bytes: &[u8], i: usize) -> usize {
    match bytes[i] {
        b'"' => {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            j + 1
        }
        b'r' if matches!(bytes.get(i + 1), Some(b'"' | b'#')) && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')) => {
            let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
            if bytes.get(i + 1 + hashes) != Some(&b'"') {
                return i;
            }
            let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat_n(b'#', hashes)).collect();
            let body = i + 2 + hashes;
            let end = bytes[body..].windows(close.len()).position(|w| w == close.as_slice()).expect("raw string closes");
            body + end + close.len()
        }
        // A char literal (`'('`, `'\''`); a lifetime has no closing quote two or three bytes on.
        b'\'' => match (bytes.get(i + 1), bytes.get(i + 2), bytes.get(i + 3)) {
            (Some(b'\\'), _, Some(b'\'')) => i + 4,
            (Some(_), Some(b'\''), _) => i + 3,
            _ => i,
        },
        _ => i,
    }
}

/// The index just past the balanced `open … close` group starting at `i`.
fn skip_group(bytes: &[u8], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < bytes.len() {
        let next = skip_literal(bytes, i);
        if next != i {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unbalanced group");
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

struct Site {
    file: String,
    line: usize,
    code: String,
    hinted: bool,
}

/// Every `CliError::refused(` / `CliError::failed(` in `text`, with whether
/// its builder chain contains `.hint(`.
fn sites(file: &str, text: &str) -> Vec<Site> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let skipped = skip_literal(bytes, i);
        if skipped != i {
            i = skipped;
            continue;
        }
        let Some(ctor) = ["CliError::refused(", "CliError::failed("].iter().find(|c| bytes[i..].starts_with(c.as_bytes())) else {
            i += 1;
            continue;
        };
        let open = i + ctor.len() - 1;
        let args_end = skip_group(bytes, open);
        let code = text[open + 1..args_end - 1].split(',').next().unwrap_or("").trim().trim_matches('"').to_string();
        // The chain: `.name(…)`, `.name`, `?` after the call, across lines.
        let mut hinted = false;
        let mut j = args_end;
        loop {
            let k = skip_ws(bytes, j);
            if bytes.get(k) == Some(&b'?') {
                j = k + 1;
            } else if bytes.get(k) == Some(&b'.') && bytes.get(k + 1).is_some_and(|b| b.is_ascii_alphabetic()) {
                let name_start = k + 1;
                let mut name_end = name_start;
                while bytes.get(name_end).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') {
                    name_end += 1;
                }
                hinted |= &text[name_start..name_end] == "hint";
                j = if bytes.get(name_end) == Some(&b'(') { skip_group(bytes, name_end) } else { name_end };
            } else {
                break;
            }
        }
        found.push(Site { file: file.to_string(), line: text[..i].matches('\n').count() + 1, code, hinted });
        i = args_end;
    }
    found
}

fn all_sites() -> Vec<Site> {
    let mut files = Vec::new();
    rust_files(&src_dir(), &mut files);
    files.iter().flat_map(|f| sites(f.strip_prefix(src_dir()).unwrap().to_str().unwrap(), &live_source(f))).collect()
}

#[test]
fn every_refusal_carries_a_hint_or_is_allowed_with_a_reason() {
    let found = all_sites();
    assert!(found.len() > 100, "the scan found only {} constructions; it is not reading the sources", found.len());

    let unhinted: Vec<&Site> = found.iter().filter(|s| !s.hinted).collect();
    let missing: Vec<String> = unhinted
        .iter()
        .filter(|s| !ALLOWED.iter().any(|(file, code, _)| *file == s.file && *code == s.code))
        .map(|s| format!("{}:{}: `{}` has no .hint(…)", s.file, s.line, s.code))
        .collect();
    assert!(missing.is_empty(), "refusals without a hint (add `.hint(\"<the command or action that fixes it>\")`, or allow with a reason):\n{}", missing.join("\n"));

    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|(file, code, _)| !unhinted.iter().any(|s| s.file == *file && s.code == *code))
        .map(|(file, code, _)| format!("{file} `{code}`: allowed without a hint, but every such construction has one now"))
        .collect();
    assert!(stale.is_empty(), "{}", stale.join("\n"));
    assert!(ALLOWED.iter().all(|(_, _, reason)| reason.len() >= 10), "every allowlist entry states its reason");
}

#[test]
fn the_scanner_follows_multi_line_chains_and_ignores_literals() {
    let text = r#"
        let a = CliError::refused("A", format!("a (b)")).field("x")
            .expected(["p", "q"])
            .hint("run `canon status`");
        let b = CliError::failed("B", "msg").hint("h")?;
        let c = CliError::refused("C", "an unmatched ) and \" quote");
        let d = CliError::refused('(' .to_string(), "m")
            .field("f");
    "#;
    let found = sites("t.rs", text);
    let by = |code: &str| found.iter().find(|s| s.code == code).unwrap_or_else(|| panic!("{code} not found")).hinted;
    assert_eq!(found.len(), 4);
    assert!(by("A") && by("B"));
    assert!(!by("C"));
}
