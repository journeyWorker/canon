//! The tests' EIO seam (`canon-0-36-code-reasons`): debug builds only, `CANON_TEST_EIO=<path glob>` makes the reads below fail with
//! `EIO`, the operating-system error a repeat can pass, for every path the glob matches. It lets a test drive the real binary through each
//! call site that answers a subject's `_UNAVAILABLE` code, which no real disk would fail on demand. A release build has no seam: every
//! function here is the `std::fs` call it wraps.
//!
//! The glob is matched against the path as the caller gave it and against its absolute form. `*` matches any run of characters (a `/`
//! too) and `?` matches one character, so `*/canon.yaml` names a repository's canon.yaml and `*/.canon/ledger/*` its ledger.

use std::fs;
use std::io;
use std::path::Path;

/// The error the seam gives: `EIO` where the operating system has one, a timed-out call elsewhere (both are errors a repeat can pass).
#[cfg(debug_assertions)]
fn injected_error() -> io::Error {
    #[cfg(unix)]
    {
        io::Error::from_raw_os_error(5)
    }
    #[cfg(not(unix))]
    {
        io::Error::from(io::ErrorKind::TimedOut)
    }
}

/// Whether `pattern` matches the whole of `text` (`*` any run, `?` one character).
#[cfg(debug_assertions)]
fn wildcard(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| wildcard(rest, &text[skip..])),
        Some(('?', rest)) => text.split_first().is_some_and(|(_, more)| wildcard(rest, more)),
        Some((literal, rest)) => text.split_first().is_some_and(|(first, more)| first == literal && wildcard(rest, more)),
    }
}

/// Fails with `EIO` when `CANON_TEST_EIO` names a glob that matches `path`; always `Ok` in a release build.
pub fn check(path: &Path) -> io::Result<()> {
    #[cfg(debug_assertions)]
    {
        if let Some(glob) = std::env::var("CANON_TEST_EIO").ok().filter(|glob| !glob.is_empty()) {
            let pattern: Vec<char> = glob.chars().collect();
            let given = path.to_string_lossy();
            let absolute = std::path::absolute(path).ok().map(|absolute| absolute.to_string_lossy().into_owned());
            if std::iter::once(given.into_owned()).chain(absolute).any(|text| wildcard(&pattern, &text.chars().collect::<Vec<_>>())) {
                return Err(injected_error());
            }
        }
    }
    let _ = path;
    Ok(())
}

/// [`fs::read`], through the seam.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    check(path.as_ref())?;
    fs::read(path)
}

/// [`fs::read_to_string`], through the seam.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    check(path.as_ref())?;
    fs::read_to_string(path)
}

/// [`fs::read_dir`], through the seam.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<fs::ReadDir> {
    check(path.as_ref())?;
    fs::read_dir(path)
}

/// [`fs::File::open`], through the seam.
pub fn open(path: impl AsRef<Path>) -> io::Result<fs::File> {
    check(path.as_ref())?;
    fs::File::open(path)
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::wildcard;

    fn matches(pattern: &str, text: &str) -> bool {
        wildcard(&pattern.chars().collect::<Vec<_>>(), &text.chars().collect::<Vec<_>>())
    }

    #[test]
    fn a_glob_matches_the_whole_path_with_star_and_question_mark() {
        assert!(matches("*/canon.yaml", "/tmp/repo/canon.yaml"));
        assert!(!matches("*/canon.yaml", "/tmp/repo/canon.yaml.bak"));
        assert!(matches("*/.canon/ledger/*", "/tmp/r/.canon/ledger/kind=finding/a.json"));
        assert!(matches("/t?/x", "/tm/x"));
        assert!(!matches("/t?/x", "/t/x"));
        assert!(!matches("", "x") && matches("", ""));
    }
}
