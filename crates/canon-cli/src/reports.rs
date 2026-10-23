//! Report schemas and the envelope schemas the generated command spec embeds
//! (`crate::spec`). Owned by the envelope slice; this file holds only the
//! signatures the spec generator consumes.

/// The JSON Schema of one typed report.
pub struct ReportSchema {
    /// The leaf command the report is emitted under.
    pub command: &'static str,
    /// `<PascalCaseCommandId>Report`.
    pub name: String,
    pub schema_version: &'static str,
    pub schema: serde_json::Value,
}

/// One entry per typed report.
pub fn report_schemas() -> Vec<ReportSchema> {
    Vec::new()
}

/// The success and failure envelope JSON Schemas (2020-12).
pub fn envelope_schemas() -> serde_json::Value {
    serde_json::json!({})
}
