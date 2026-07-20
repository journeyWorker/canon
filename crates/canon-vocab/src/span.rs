//! Source-imported from the donor span primitives (159 LOC, `serde` its only
//! dependency — design.md open-Q1's "leaf" of the leaf-plus-one-hop lift:
//! no other donor crate depends on this
//! module). Lifted verbatim: `TextIndex`/`Position`/`Span` (byte <-> line/col/
//! utf16 mapping) and `Severity`.
//!
//! Deliberately NOT lifted: `Diagnostic`/`Fixit`/`TextEdit`/`Layer`/
//! `StableId`. Those exist in the donor vocabulary system to carry a `Span`
//! produced by the donor's byte-accurate scene-text parser — D2 explicitly
//! does not
//! lift that parser (canon's task-atom/handoff-body records are validated
//! YAML *values*, already deserialized by `serde_yaml`, which does not carry
//! per-value byte spans in its `Value` type). `crate::checker::Diagnostic`
//! is therefore canon-vocab's own type: it identifies its subject by atom id
//! and attribute key (the natural anchor for a validated-record checker),
//! not a byte span. `Span`/`TextIndex`/`Position` are kept here, tested, as
//! the documented (D3 "a `pub` boundary, not a trait with no implementers")
//! extension point a future line-accurate atom-file scanner or LSP can
//! build on without changing this module's shape.

use serde::{Deserialize, Serialize};

/// Precomputed line-start table for byte <-> (line, col, utf16) mapping.
pub struct TextIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>, // byte offset of each line start
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: u32,      // 1-based
    pub column: u32,    // 1-based byte column within line
    pub utf16_col: u32, // 0-based UTF-16 column within line
}

impl<'a> TextIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { text, line_starts }
    }

    /// The source text this index was built over. The byte offsets every `Span`
    /// carries index into exactly this string.
    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn position(&self, byte: usize) -> Position {
        let line_ix = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[line_ix];
        let slice = &self.text[line_start..byte];
        let byte_col = (byte - line_start) as u32;
        let utf16_col = slice.chars().map(|c| c.len_utf16() as u32).sum();
        Position { line: line_ix as u32 + 1, column: byte_col + 1, utf16_col }
    }

    fn utf16_offset(&self, byte: usize) -> u32 {
        self.text[..byte].chars().map(|c| c.len_utf16() as u32).sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: u32,               // 1-based, of byte_start
    pub column: u32,             // 1-based byte column of byte_start
    pub utf16_range: (u32, u32), // file-relative UTF-16 offsets
}

impl Span {
    pub fn from_bytes(idx: &TextIndex, start: usize, end: usize) -> Self {
        let p = idx.position(start);
        Span { byte_start: start, byte_end: end, line: p.line, column: p.column, utf16_range: (idx.utf16_offset(start), idx.utf16_offset(end)) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_index_maps_byte_to_line_col_and_utf16() {
        // "a\nsé" : 'é' is 2 bytes (U+00E9), 1 UTF-16 unit
        let idx = TextIndex::new("a\nsé");
        let p0 = idx.position(0);
        assert_eq!((p0.line, p0.column), (1, 1));
        let p2 = idx.position(2);
        assert_eq!((p2.line, p2.column), (2, 1));
        let p3 = idx.position(3);
        assert_eq!(p3.line, 2);
        assert_eq!(p3.utf16_col, 1);
    }

    #[test]
    fn span_from_bytes_fills_both_encodings() {
        let idx = TextIndex::new("hello");
        let s = Span::from_bytes(&idx, 1, 4);
        assert_eq!((s.byte_start, s.byte_end), (1, 4));
        assert_eq!(s.line, 1);
        assert_eq!(s.column, 2);
        assert_eq!(s.utf16_range, (1, 4));
    }
}
