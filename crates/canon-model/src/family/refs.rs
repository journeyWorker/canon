//! [`Ref`]: the artifact family's structured code/behavior reference —
//! `{file, symbol, lines?}` — replacing the donor's `;`-joined strings
//! and free text (S11 design D4). Shared by ledger review-family
//! records (`upstream_ref`/`port_ref`) and divergence review events
//! (`port_ref`); `canon-fmt`'s `refparse` module is what PARSES a
//! legacy ref string into this shape — this module only defines the
//! target shape itself, so `canon-model` never depends on the parser.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An inclusive `<a>-<b>` line range, as seen in donor ref strings like
/// `#_storySpecFor:397-447` or an inventory entry's `lines: 262-270`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// A single structured code/behavior reference (S11 design D4): a file
/// path, the symbol within it, and an optional line range. Never
/// constructed by guessing from prose — [`crate::family::refs::Ref`]
/// values only ever come from successfully parsing a donor
/// `<file>#<symbol>[:<a>-<b>]`-shaped segment; unparseable text is left
/// as the original string and reported as a `canon fmt --check`
/// `free-text-ref` violation instead (S11 design D4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ref {
    pub file: String,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
}

impl Ref {
    pub fn new(file: impl Into<String>, symbol: impl Into<String>, lines: Option<LineRange>) -> Self {
        Self { file: file.into(), symbol: symbol.into(), lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_without_lines_omits_the_field() {
        let r = Ref::new("a.dart", "Foo", None);
        let json = serde_json::to_value(&r).unwrap();
        assert!(json.get("lines").is_none());
    }

    #[test]
    fn ref_round_trips_with_lines() {
        let r = Ref::new("a.dart", "Foo", Some(LineRange::new(1, 2)));
        let json = serde_json::to_value(&r).unwrap();
        let back: Ref = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }
}
