//! `spec/features/kind=feature/area=<area>/<surface>.feature` (S11
//! design D2): authoring provenance for a Gherkin `Feature:`/
//! `Scenario:` header, carried as a structured `# canon: {...}` comment
//! line — valid Gherkin (a `#`-prefixed line is a comment to every
//! Gherkin parser), so purely additive and never breaks a Gherkin
//! consumer that doesn't know about it. `.feature` files are not JSON,
//! so unlike every other family member this is NOT the whole file's
//! envelope — it is the payload of ONE comment line, one per
//! `Feature:`/`Scenario:` header (design D2's literal example has no
//! `kind` field: the file's own `kind=feature/` path segment already
//! names it).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::Actor;

pub const CANON_COMMENT_PREFIX: &str = "# canon: ";

/// The `# canon: {...}` comment payload (S11 design D2's literal
/// example shape: `{schema, at, actor}`, no `kind`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureProvenance {
    pub schema: u32,
    pub at: DateTime<Utc>,
    pub actor: Actor,
}

impl FeatureProvenance {
    pub fn new(schema: u32, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema, at, actor }
    }

    /// Render as the exact `# canon: {...}` comment line design D2
    /// specifies (no trailing newline — callers insert one per their
    /// own line-joining convention).
    pub fn render_comment_line(&self) -> String {
        format!("{CANON_COMMENT_PREFIX}{}", serde_json::to_string(self).expect("FeatureProvenance serializes"))
    }

    /// Parse a `# canon: {...}` comment line back into its payload —
    /// `None` for any line that isn't one (a normal narrative comment,
    /// or a line that happens to start with `# canon:` but carries
    /// malformed JSON, which `canon fmt --check` reports as a
    /// `missing-provenance` gap rather than a parse panic).
    pub fn parse_comment_line(line: &str) -> Option<Self> {
        let trimmed = line.trim_start();
        let payload = trimmed.strip_prefix(CANON_COMMENT_PREFIX)?;
        serde_json::from_str(payload.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RoleId;
    use chrono::TimeZone;

    #[test]
    fn comment_line_round_trips() {
        let provenance = FeatureProvenance::new(
            1,
            Utc.with_ymd_and_hms(2026, 7, 10, 0, 0, 0).unwrap(),
            Actor::new("canon-fmt", RoleId::parse("implementer").unwrap()),
        );
        let line = provenance.render_comment_line();
        assert!(line.starts_with("# canon: "));
        let parsed = FeatureProvenance::parse_comment_line(&line).unwrap();
        assert_eq!(parsed, provenance);
    }

    #[test]
    fn ordinary_comment_line_is_not_provenance() {
        assert!(FeatureProvenance::parse_comment_line("# just a narrative comment").is_none());
    }

    #[test]
    fn indented_canon_comment_still_parses() {
        let provenance =
            FeatureProvenance::new(1, Utc.with_ymd_and_hms(2026, 7, 10, 0, 0, 0).unwrap(), Actor::new_unattributed("canon-fmt"));
        let indented = format!("  {}", provenance.render_comment_line());
        assert_eq!(FeatureProvenance::parse_comment_line(&indented), Some(provenance));
    }
}
