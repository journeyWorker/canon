//! Parse a donor ref string (`upstream_ref`/`port_ref`) into
//! [`canon_model::family::refs::Ref`]s (S11 design D4). Grounded
//! directly in the real corpus, not just the design doc's illustrative
//! single-segment example — reading a donor project's
//! `spec/ledger/kind={code,design}-review/**`
//! (read-only) shows `port_ref` joined by BOTH `;` (750 files) AND `,`
//! (1561 files) in the live corpus, plus a same-file multi-symbol shape
//! (`#Foo,Bar` — a second segment with no `#` at all, continuing the
//! previous segment's file). This parser therefore splits on either
//! delimiter and lets a bare, whitespace-free continuation segment
//! inherit the last successfully-parsed file — a deterministic,
//! structurally-derived rule (Dart/TS symbols never contain `,`/`;`),
//! never a guess from prose. A segment that has neither a `#` nor a
//! prior file to inherit is left unparsed (design D4:
//! "never fabricate a `{file, symbol}` guess from prose").

use canon_model::family::refs::{LineRange, Ref};

/// One ref string's parse outcome: every segment that resolved to a
/// structured [`Ref`], plus every segment that didn't (in source
/// order) — the un-parsed segments are exactly what `canon fmt
/// --check` reports as `free-text-ref` (S11 design D4/D8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefParseOutcome {
    pub refs: Vec<Ref>,
    pub unparsed: Vec<String>,
}

impl RefParseOutcome {
    pub fn is_fully_parsed(&self) -> bool {
        self.unparsed.is_empty() && !self.refs.is_empty()
    }
}

/// A donor ref string is "joined" (the design's `;`/`,`-joined gap,
/// distinct from free-text) when it splits into more than one
/// non-empty segment.
pub fn is_joined(raw: &str) -> bool {
    split_segments(raw).len() > 1
}

pub fn parse_refs(raw: &str) -> RefParseOutcome {
    let mut outcome = RefParseOutcome::default();
    let mut last_file: Option<String> = None;
    for segment in split_segments(raw) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(hash_idx) = segment.find('#') {
            let file = segment[..hash_idx].trim().to_string();
            let rest = segment[hash_idx + 1..].trim();
            if file.is_empty() || rest.is_empty() {
                outcome.unparsed.push(segment.to_string());
                continue;
            }
            let (symbol, lines) = split_lines_suffix(rest);
            last_file = Some(file.clone());
            outcome.refs.push(Ref::new(file, symbol, lines));
        } else if looks_like_bare_symbol(segment) {
            if let Some(file) = &last_file {
                let (symbol, lines) = split_lines_suffix(segment);
                outcome.refs.push(Ref::new(file.clone(), symbol, lines));
            } else {
                outcome.unparsed.push(segment.to_string());
            }
        } else {
            outcome.unparsed.push(segment.to_string());
        }
    }
    outcome
}

/// `;` and `,` are both real donor delimiters (module doc). Splitting
/// on both, unconditionally, is safe here because a bare file path or
/// Dart/TS symbol never legitimately contains either character.
fn split_segments(raw: &str) -> Vec<&str> {
    raw.split([';', ',']).collect()
}

/// A "same-file continuation" segment (`#Foo,Bar`'s `Bar`) is
/// whitespace-free and starts with a letter or underscore — free prose
/// ("see spec/inventory/…") always contains a space and fails this,
/// routing to `unparsed` instead of a wrong guess.
fn looks_like_bare_symbol(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else { return false };
    (first.is_alphabetic() || first == '_') && !segment.chars().any(char::is_whitespace)
}

/// `<symbol>:<a>-<b>` — the trailing `:<a>-<b>` line-range suffix
/// observed on real `port_ref` segments (`_storySpecFor:397-447`).
fn split_lines_suffix(s: &str) -> (String, Option<LineRange>) {
    if let Some(idx) = s.rfind(':') {
        let (sym, tail) = (&s[..idx], &s[idx + 1..]);
        if let Some((start, end)) = parse_line_range(tail) {
            return (sym.to_string(), Some(LineRange::new(start, end)));
        }
    }
    (s.to_string(), None)
}

fn parse_line_range(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_ref_with_lines() {
        let outcome = parse_refs(
            "lib/features/idolive/widgets/photocard_viewer_container.dart#_storySpecFor:397-447; \
             packages/oshiz_ui/lib/src/components/oz_photocard_viewer.dart#_OzPhotocardViewerState._buildBack:764-860",
        );
        assert_eq!(outcome.refs.len(), 2);
        assert_eq!(outcome.refs[0].file, "lib/features/idolive/widgets/photocard_viewer_container.dart");
        assert_eq!(outcome.refs[0].symbol, "_storySpecFor");
        assert_eq!(outcome.refs[0].lines, Some(LineRange::new(397, 447)));
        assert_eq!(outcome.refs[1].symbol, "_OzPhotocardViewerState._buildBack");
        assert_eq!(outcome.refs[1].lines, Some(LineRange::new(764, 860)));
        assert!(outcome.unparsed.is_empty());
    }

    #[test]
    fn comma_joined_two_full_refs() {
        let outcome = parse_refs(
            "lib/features/idolive/widgets/photocard_viewer_container.dart#_storySpecFor, \
             packages/oshiz_ui/lib/src/components/oz_photocard_viewer.dart#_buildBack",
        );
        assert_eq!(outcome.refs.len(), 2);
        assert!(outcome.unparsed.is_empty());
    }

    #[test]
    fn comma_joined_same_file_continuation_inherits_file() {
        let outcome = parse_refs("lib/features/home/widgets/daily_mission_sheet.dart#_MissionRow,_targetConditionLabel");
        assert_eq!(outcome.refs.len(), 2);
        assert_eq!(outcome.refs[0].file, "lib/features/home/widgets/daily_mission_sheet.dart");
        assert_eq!(outcome.refs[1].file, "lib/features/home/widgets/daily_mission_sheet.dart");
        assert_eq!(outcome.refs[1].symbol, "_targetConditionLabel");
        assert!(outcome.unparsed.is_empty());
    }

    #[test]
    fn free_text_ref_is_wholly_unparsed_never_guessed() {
        let outcome = parse_refs("reconciled vs upstream @9c93d024b (see spec/inventory/idolive-hub.yaml)");
        assert!(outcome.refs.is_empty());
        assert_eq!(outcome.unparsed.len(), 1);
        assert!(!outcome.is_fully_parsed());
    }

    #[test]
    fn simple_single_ref_no_delimiters() {
        let outcome = parse_refs("routes/idolive/replays/index.tsx#RouteComponent");
        assert_eq!(outcome.refs, vec![Ref::new("routes/idolive/replays/index.tsx", "RouteComponent", None)]);
        assert!(outcome.is_fully_parsed());
    }

    #[test]
    fn is_joined_detects_multi_segment_strings() {
        assert!(is_joined("a.dart#Foo; b.dart#Bar"));
        assert!(is_joined("a.dart#Foo,Bar"));
        assert!(!is_joined("a.dart#Foo"));
    }
}
