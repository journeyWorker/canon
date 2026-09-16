//! A minimal, line-based `.feature` file scan — never a real Gherkin
//! parser (the donor's own parsing engine, `gherkin-official`, stays
//! authoritative for `parity.py`; canon only needs enough structure to
//! find `Feature:`/`Scenario:` headers, their immediately-following
//! `# canon: {...}` provenance comment if present, and each scenario's
//! leading `@<area>.<surface>.<nn>`-shaped tag for partition-value
//! cross-checking). s15 P3a (task 3.2) extends the scan ADDITIVELY —
//! `headers`/`scenario_ids` are unchanged, so `check.rs`'s existing
//! `resolve::resolve_feature(&scan.scenario_ids)` caller keeps working
//! untouched — to also SURFACE what the line-scan already reads: each
//! scenario tag paired with the `Scenario:`/`Scenario Outline:` header
//! immediately following it, exposing that header's label as the
//! scenario's `title` (design D4). This is retention of information
//! the scan already walks past, never a new parser.

use canon_model::family::feature::FeatureProvenance;
use canon_model::ids::SpecDigest;

/// One `Feature:` or `Scenario:` header's provenance-comment state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderScan {
    pub line_no: usize,
    pub label: String,
    pub has_provenance: bool,
}

/// One `@<area>.<surface>.<nn>`-shaped tag paired with the header
/// label of the `Scenario:`/`Scenario Outline:` it immediately
/// precedes (design D4/task 3.2) — `canon inventory sync`'s `title`
/// source. A tag with no following scenario header (e.g. the last line
/// of a malformed file) never appears here — [`FeatureScan::
/// scenario_ids`] still collects it for `resolve_feature`'s area check,
/// but a titled index record can only be minted for a tag the scan
/// could actually pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioScan {
    pub scenario_id: String,
    pub title: String,
    /// Raw `@subject:<value>` tag values (the substring after
    /// `subject:`), retained exactly as authored for inventory sync's
    /// fail-soft grammar/duplicate diagnostic.
    pub subject_tags: Vec<String>,
    /// Raw `@lane:<value>` tag values, retained exactly as authored for
    /// inventory sync's fail-soft grammar/duplicate diagnostic.
    pub lane_tags: Vec<String>,
    /// Namespaces from `@name:value`-shaped tags other than the
    /// recognized `subject` and `lane` axes. Plain tags are ignored.
    pub unknown_namespace_tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FeatureScan {
    pub headers: Vec<HeaderScan>,
    pub scenario_ids: Vec<String>,
    /// Only scenario-id tags that pair with a following Scenario header
    /// carry a titled scan entry.
    pub scenarios: Vec<ScenarioScan>,
}

impl FeatureScan {
    pub fn missing_provenance_count(&self) -> usize {
        self.headers.iter().filter(|h| !h.has_provenance).count()
    }
}

/// Scan `text` (a `.feature` file's full content).
pub fn scan(text: &str) -> FeatureScan {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = FeatureScan::default();
    // Scenario tags seen since the last header was consumed — paired
    // with the NEXT `Scenario:`/`Scenario Outline:` header (design D4).
    // A `Feature:` header (or any other non-scenario header) clears
    // this without pairing, so a stray pre-`Feature:` tag never links
    // to a scenario several blocks later.
    let mut pending_scenario_ids: Vec<String> = Vec::new();
    // Axis and unknown namespace tags travel with the same pending
    // scenario block as the scenario id.
    let mut pending_subject_tags: Vec<String> = Vec::new();
    let mut pending_lane_tags: Vec<String> = Vec::new();
    let mut pending_unknown_namespace_tags: Vec<String> = Vec::new();
    // The line index of the provenance comment the PREVIOUS header
    // claimed in the trailing position. One comment serves one header:
    // a `Feature:`'s own trailing comment must not double as the
    // leading comment of a scenario written directly beneath it with
    // no blank line between.
    let mut claimed_trailing: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(label) = header_label(trimmed) {
            // Trailing form: the first non-blank line after the header.
            // Leading form: any provenance comment in the contiguous
            // `@`/`#` block directly above the header, excluding a line
            // the previous header already claimed.
            let trailing_provenance = lines[i + 1..]
                .iter()
                .position(|l| !l.trim().is_empty())
                .map(|offset| i + 1 + offset)
                .filter(|&j| FeatureProvenance::parse_comment_line(lines[j]).is_some());
            let mut leading_provenance = false;
            for j in (0..i).rev() {
                if Some(j) == claimed_trailing {
                    break;
                }
                let above = lines[j].trim_start();
                if above.is_empty() || !(above.starts_with('@') || above.starts_with('#')) {
                    break;
                }
                if FeatureProvenance::parse_comment_line(above).is_some() {
                    leading_provenance = true;
                }
            }
            let has_provenance = trailing_provenance.is_some() || leading_provenance;
            claimed_trailing = trailing_provenance;
            result.headers.push(HeaderScan { line_no: i + 1, label: label.clone(), has_provenance });
            if is_scenario_header(trimmed) {
                for scenario_id in pending_scenario_ids.drain(..) {
                    result.scenarios.push(ScenarioScan {
                        scenario_id,
                        title: label.clone(),
                        subject_tags: pending_subject_tags.clone(),
                        lane_tags: pending_lane_tags.clone(),
                        unknown_namespace_tags: pending_unknown_namespace_tags.clone(),
                    });
                }
                pending_subject_tags.clear();
                pending_lane_tags.clear();
                pending_unknown_namespace_tags.clear();
            } else {
                pending_scenario_ids.clear();
                pending_subject_tags.clear();
                pending_lane_tags.clear();
                pending_unknown_namespace_tags.clear();
            }
        }
        for tag in trimmed.split_whitespace().filter(|t| t.starts_with('@')) {
            let candidate = &tag[1..];
            if let Some(subject) = candidate.strip_prefix("subject:") {
                pending_subject_tags.push(subject.to_string());
            } else if let Some(lane) = candidate.strip_prefix("lane:") {
                pending_lane_tags.push(lane.to_string());
            } else if let Some((namespace, _value)) = candidate.split_once(':') {
                if !namespace.is_empty()
                    && namespace.as_bytes()[0].is_ascii_lowercase()
                    && namespace.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    pending_unknown_namespace_tags.push(namespace.to_string());
                }
            } else if looks_like_scenario_id(candidate) {
                result.scenario_ids.push(candidate.to_string());
                pending_scenario_ids.push(candidate.to_string());
            }
        }
    }
    result
}

fn header_label(trimmed: &str) -> Option<String> {
    for prefix in ["Feature:", "Scenario:", "Scenario Outline:"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Whether `trimmed` is a `Scenario:`/`Scenario Outline:` header
/// specifically — as opposed to `Feature:` — the ONLY kind a pending
/// scenario tag pairs with (design D4).
fn is_scenario_header(trimmed: &str) -> bool {
    trimmed.starts_with("Scenario:") || trimmed.starts_with("Scenario Outline:")
}

/// `<area>.<surface>.<nn>` — mirrors `canon_model::ids`'s
/// `is_scenario_id` grammar loosely (this module intentionally doesn't
/// depend on `ScenarioId::parse` failing softly; a tag that doesn't
/// look like an id is just not collected, never an error).
fn looks_like_scenario_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts.iter().take(2).all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
        && parts[2].len() >= 2
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// sha256-hex over a `.feature` file's raw bytes (design D4) — a thin,
/// gherkin-scoped wrapper over `SpecDigest::of` so `canon inventory
/// sync` has one import for "the digest a scanned `.feature` file's
/// `Scenario.source_digest` freshness signal is derived from". NOT a
/// digest over anything the line-scan interprets — the file's exact
/// byte content, so any edit (including one the scan can't see through,
/// e.g. a body step) changes it (design R8: file-granularity churn is
/// accepted).
pub fn source_digest(bytes: &[u8]) -> SpecDigest {
    SpecDigest::of(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@idolive-replay-detail\nFeature: Idolive replay detail\n  # rationale comment\n\n  @idolive.replay-detail.01 @p2 @live-api\n  Scenario: Opening a replay loads its detail\n    Given the guest opens the detail\n";

    #[test]
    fn finds_headers_and_missing_provenance() {
        let scan = scan(SAMPLE);
        assert_eq!(scan.headers.len(), 2);
        assert!(!scan.headers[0].has_provenance);
        assert!(!scan.headers[1].has_provenance);
        assert_eq!(scan.missing_provenance_count(), 2);
    }

    #[test]
    fn collects_scenario_id_shaped_tags_only() {
        let scan = scan(SAMPLE);
        assert_eq!(scan.scenario_ids, vec!["idolive.replay-detail.01".to_string()]);
    }

    #[test]
    fn provenance_comment_immediately_after_header_is_detected() {
        let text = "Feature: X\n  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}\n";
        let scan = scan(text);
        assert!(scan.headers[0].has_provenance);
    }

    const PROV: &str = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";

    #[test]
    fn provenance_comment_in_contiguous_tag_comment_block_above_header_is_detected() {
        let text = format!("Feature: X\n{PROV}\n\n{PROV}\n  @subject:s @lane:behavior\n  @a.b.01\n  Scenario: One\n    Given a\n");
        let scan = scan(&text);
        assert_eq!(scan.missing_provenance_count(), 0);
    }

    #[test]
    fn blank_line_between_leading_comment_and_header_breaks_provenance() {
        let text = format!("Feature: X\n{PROV}\n\n{PROV}\n\n  @a.b.01\n  Scenario: One\n    Given a\n");
        let scan = scan(&text);
        assert!(!scan.headers[1].has_provenance);
    }

    #[test]
    fn a_features_trailing_comment_never_doubles_as_the_next_scenarios_leading_one() {
        // No blank line between the Feature's own comment and the
        // scenario's tag: the comment is the Feature's, the scenario
        // has none.
        let text = format!("Feature: X\n{PROV}\n  @a.b.01\n  Scenario: One\n    Given a\n");
        let scan = scan(&text);
        assert!(scan.headers[0].has_provenance);
        assert!(!scan.headers[1].has_provenance);
    }

    #[test]
    fn provenance_in_both_positions_counts_as_one_present_state() {
        let text = format!("Feature: X\n{PROV}\n\n{PROV}\n  @a.b.01\n  Scenario: One\n{PROV}\n    Given a\n");
        let scan = scan(&text);
        assert_eq!(scan.headers.len(), 2);
        assert_eq!(scan.missing_provenance_count(), 0);
    }

    #[test]
    fn lane_and_unknown_namespace_tags_are_collected_but_plain_tags_are_ignored() {
        let text = "Feature: Tagged\n\n  @a.b.01 @lane:design @foo:bar @p2\n  Scenario: One\n    Given a\n";
        let scan = scan(text);
        assert_eq!(scan.scenarios[0].lane_tags, vec!["design"]);
        assert_eq!(scan.scenarios[0].unknown_namespace_tags, vec!["foo"]);
    }

    #[test]
    fn scenario_tag_pairs_with_its_following_header_as_title() {
        let scan = scan(SAMPLE);
        assert_eq!(scan.scenarios, vec![ScenarioScan { scenario_id: "idolive.replay-detail.01".to_string(), title: "Opening a replay loads its detail".to_string(), subject_tags: vec![], lane_tags: vec![], unknown_namespace_tags: vec![] }]);
    }

    #[test]
    fn a_feature_level_tag_never_pairs_with_a_later_scenario() {
        // The bare `@idolive-replay-detail` tag before `Feature:` isn't
        // scenario-id-shaped so it never enters `pending_scenario_ids`
        // to begin with -- confirms the `Feature:` header doesn't
        // wrongly inherit or leak a pairing either.
        let scan = scan(SAMPLE);
        assert_eq!(scan.scenarios.len(), 1, "only the real scenario tag pairs, not the feature-level one");
    }

    #[test]
    fn a_scenario_id_shaped_tag_before_feature_is_collected_but_never_paired() {
        // A scenario-id-SHAPED tag (`@world.hotdeal.01`, which DOES
        // enter `pending_scenario_ids`) placed before `Feature:` is a
        // feature-level tag, NOT within any scenario block. It is still
        // collected into `scenario_ids` for the area-resolution check,
        // but must NEVER pair into `scenarios`: the spec pairs a tag
        // only with a `Scenario:` header it "immediately precedes,
        // within the same scenario block", so `Feature:` drains pending
        // tags. Pairing it would wrongly attach the feature tag's id to
        // a later scenario's title.
        let text = "@world.hotdeal.01\nFeature: Firstbuy hotdeal\n  # rationale\n\n  @world.hotdeal.02\n  Scenario: Buying triggers the coupon\n    Given a precondition\n";
        let scan = scan(text);
        assert!(
            scan.scenario_ids.contains(&"world.hotdeal.01".to_string()),
            "the pre-`Feature:` scenario-shaped tag is still collected for area resolution"
        );
        assert_eq!(
            scan.scenarios,
            vec![ScenarioScan { scenario_id: "world.hotdeal.02".to_string(), title: "Buying triggers the coupon".to_string(), subject_tags: vec![], lane_tags: vec![], unknown_namespace_tags: vec![] }],
            "only the tag within the scenario block pairs; the pre-`Feature:` tag is drained, never paired to a later scenario"
        );
    }

    #[test]
    fn multiple_scenarios_in_one_file_each_pair_with_their_own_header() {
        let text = "Feature: Two scenarios\n\n  @a.b.01\n  Scenario: First one\n    Given a\n\n  @a.b.02\n  Scenario: Second one\n    Given b\n";
        let scan = scan(text);
        assert_eq!(
            scan.scenarios,
            vec![
                ScenarioScan { scenario_id: "a.b.01".to_string(), title: "First one".to_string(), subject_tags: vec![], lane_tags: vec![], unknown_namespace_tags: vec![] },
                ScenarioScan { scenario_id: "a.b.02".to_string(), title: "Second one".to_string(), subject_tags: vec![], lane_tags: vec![], unknown_namespace_tags: vec![] },
            ]
        );
    }

    #[test]
    fn a_subject_tag_pairs_with_its_scenario_as_a_raw_value() {
        let text = "Feature: Tagged\n\n  @a.b.01 @subject:payments-core\n  Scenario: One\n    Given a\n";
        let scan = scan(text);
        assert_eq!(
            scan.scenarios,
        vec![ScenarioScan { scenario_id: "a.b.01".to_string(), title: "One".to_string(), subject_tags: vec!["payments-core".to_string()], lane_tags: vec![], unknown_namespace_tags: vec![] }],
            "the raw @subject: value is paired with its scenario, unvalidated"
        );
    }

    #[test]
    fn multiple_subject_tags_on_one_scenario_are_all_collected_in_source_order() {
        // The scan is a pure lexer: it collects every raw value; the
        // "first wins + named violation" rule is `canon inventory
        // sync`'s, not this module's.
        let text = "Feature: Tagged\n\n  @a.b.01 @subject:first @subject:second\n  Scenario: One\n    Given a\n";
        let scan = scan(text);
        assert_eq!(scan.scenarios[0].subject_tags, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn a_subject_tag_before_feature_never_leaks_to_a_later_scenario() {
        let text = "@subject:leaked\nFeature: Tagged\n\n  @a.b.01\n  Scenario: One\n    Given a\n";
        let scan = scan(text);
        assert!(scan.scenarios[0].subject_tags.is_empty(), "a pre-Feature subject tag is drained, never paired");
    }

    #[test]
    fn source_digest_is_stable_across_repeated_scans_of_unmodified_bytes() {
        let bytes = SAMPLE.as_bytes();
        let first = source_digest(bytes);
        let second = source_digest(bytes);
        assert_eq!(first, second, "the same unmodified bytes must digest identically every time");
        assert_eq!(first, SpecDigest::of(bytes), "source_digest is exactly SpecDigest::of over the raw bytes");
    }

    #[test]
    fn source_digest_changes_when_file_bytes_change() {
        let unchanged = source_digest(SAMPLE.as_bytes());
        let changed = source_digest(format!("{SAMPLE}\n  # an added line\n").as_bytes());
        assert_ne!(unchanged, changed, "any byte-level edit must change the digest");
    }
}
