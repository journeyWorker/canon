//! The two markdown line rules canon reads a document's own lines by:
//! a CommonMark fence ([`fence_of`]) and a `Key:` line that counts only
//! outside fences and HTML comments ([`field`]). `canon init` finds its
//! `AGENTS.md` markers with the first; a change's `brief.md` carries its
//! `Approved:` and `Source:` lines under the second (0.16 D1, 0.17 D3),
//! read by `canon status`, the ship guard, `canon evidence add` and the
//! native `canon` plan dialect alike, so a quoted or commented-out line
//! is never one.

/// A fence opener/closer line: up to three spaces, then three or more
/// backticks or tildes (CommonMark). Returns the fence character, its
/// run length, and whether only whitespace follows the run (a closing
/// fence may carry nothing else).
pub fn fence_of(line: &str) -> Option<(char, usize, bool)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = rest.len() - rest.trim_start_matches(ch).len();
    (run >= 3).then(|| (ch, run, rest[run..].trim().is_empty()))
}

/// The text after `key` (for example `Approved:`) on the first line of
/// `text` that starts with it (leading whitespace allowed), says
/// something, and sits outside fenced code blocks and HTML comments,
/// which may span lines.
pub fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    field_lines(text, key).next().map(|(_, value)| value)
}

/// The 0-based index of the line [`field`] reads for `key`.
pub fn field_index(text: &str, key: &str) -> Option<usize> {
    field_lines(text, key).next().map(|(index, _)| index)
}

/// `text` with every line [`field`] would read for `key` removed (a line
/// inside a fence or an HTML comment is not a field and stays). Every
/// other byte, line endings included, is kept.
pub fn without_field(text: &str, key: &str) -> String {
    let dropped: Vec<usize> = field_lines(text, key).map(|(index, _)| index).collect();
    text.split_inclusive('\n').enumerate().filter(|(index, _)| !dropped.contains(index)).map(|(_, line)| line).collect()
}

/// Every line [`field`] reads for `key`, with its 0-based line index.
fn field_lines<'t, 'k>(text: &'t str, key: &'k str) -> impl Iterator<Item = (usize, &'t str)> + use<'t, 'k> {
    let mut fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    text.lines().enumerate().filter_map(move |(index, line)| {
        if !in_comment {
            match (fence, fence_of(line)) {
                (None, Some((ch, run, _))) => {
                    fence = Some((ch, run));
                    return None;
                }
                (Some((open_ch, open_run)), Some((ch, run, true))) if ch == open_ch && run >= open_run => {
                    fence = None;
                    return None;
                }
                (Some(_), _) => return None,
                (None, None) => {}
            }
            if let Some(value) = line.trim_start().strip_prefix(key).map(str::trim).filter(|rest| !rest.is_empty()) {
                in_comment = comment_open_after(line, in_comment);
                return Some((index, value));
            }
        }
        in_comment = comment_open_after(line, in_comment);
        None
    })
}

/// Whether an HTML comment is open at the end of `line`, given whether
/// one was open at its start.
fn comment_open_after(line: &str, mut open: bool) -> bool {
    let mut rest = line;
    loop {
        let marker = if open { "-->" } else { "<!--" };
        let Some(at) = rest.find(marker) else {
            return open;
        };
        rest = &rest[at + marker.len()..];
        open = !open;
    }
}
