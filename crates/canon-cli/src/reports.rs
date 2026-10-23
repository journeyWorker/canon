//! Typed reports and the schemas the spec publishes (0.28 D2, D3).
//!
//! Every `--json` payload is a struct that implements [`Report`]. The trait
//! ties `Serialize` to `JsonSchema`, and `json_out::data` is generic over it,
//! so a payload without a schema does not compile (the temporary
//! `json_out::untyped` entry point is the one exception, removed by the
//! slices that type the `json!` payloads). [`report_schemas`] lists one schema
//! per typed report for the generated spec; [`envelope_schemas`] holds the
//! success and failure envelopes every report travels in.

use schemars::JsonSchema;
use serde_json::{json, Value};

/// A typed `--json` payload: serializable, with a derived JSON Schema.
///
/// To type a payload: define a struct with `#[derive(Serialize, JsonSchema)]`
/// and `#[serde(rename_all = "camelCase")]`, `impl Report for It {}`, and pass
/// `&It { … }` where the code passed `json!({ … })`. Bump `SCHEMA_VERSION`'s
/// major when a key is removed or changes meaning.
pub trait Report: serde::Serialize + JsonSchema {
    /// `"<major>.<minor>"` of this report's shape; a consumer accepts unknown
    /// fields within a major and rejects an unknown major.
    const SCHEMA_VERSION: &'static str = "1.0";
}

/// One typed report in the generated spec.
#[derive(Debug, Clone)]
pub struct ReportSchema {
    /// The command path (as `schema` lists it today) whose `data` this is.
    pub command: &'static str,
    /// `<PascalCaseCommandId>Report`, the name the per-leaf table gives it.
    pub name: String,
    pub schema_version: &'static str,
    /// A JSON Schema 2020-12 for `data`.
    pub schema: Value,
}

fn entry<R: Report>(command: &'static str, name: &str) -> ReportSchema {
    ReportSchema { command, name: name.to_string(), schema_version: R::SCHEMA_VERSION, schema: schema_of::<R>() }
}

/// The JSON Schema of `R` as a value.
pub fn schema_of<R: JsonSchema>() -> Value {
    // A report is only ever serialized, so its schema follows what `Serialize` writes (renames, skipped fields).
    let generator = schemars::generate::SchemaSettings::draft2020_12().for_serialize().into_generator();
    serde_json::to_value(generator.into_root_schema_for::<R>()).expect("a JSON Schema serializes")
}

/// Every typed report, one entry per `(command, name)`, sorted by command.
/// A command not listed still prints its `data` through `json_out::untyped`
/// until a later slice types it.
pub fn report_schemas() -> Vec<ReportSchema> {
    use crate::{adapter, artifact_ingest, brief_approve, brief_check, change, completion, context, context_pack, demo, dispatch, divergence, evidence, finding, gate, learn, plans, policy_approval, retrieve, review, scaffold, selftest, status, subject};
    let mut schemas: Vec<ReportSchema> = vec![
        entry::<change::ChangeNewReport<'static>>("change new", "ChangeNewReport"),
        entry::<demo::DemoReviewReport>("demo add", "DemoReviewReport"),
        entry::<demo::DemoInitReport>("demo init", "DemoInitReport"),
        entry::<dispatch::DispatchBeginReport<'static>>("dispatch begin", "DispatchBeginReport"),
        entry::<dispatch::DispatchDiffReport<'static>>("dispatch diff", "DispatchDiffReport"),
        entry::<dispatch::DispatchEndReport<'static>>("dispatch end", "DispatchEndReport"),
        entry::<divergence::DivergenceAcceptReport<'static>>("divergence accept", "DivergenceAcceptReport"),
        entry::<divergence::DivergenceAddReport>("divergence add", "DivergenceAddReport"),
        entry::<divergence::DivergenceCommitReport>("divergence defer", "DivergenceDeferReport"),
        entry::<gate::GatePromoteReport>("divergence promote", "DivergencePromoteReport"),
        entry::<divergence::DivergenceCommitReport>("divergence resolve", "DivergenceResolveReport"),
        entry::<divergence::DivergenceListReport<'static>>("divergence status", "DivergenceListReport"),
        entry::<evidence::EvidenceApprovalPayloadReport>("evidence approval-payload", "EvidenceApprovalPayloadReport"),
        entry::<evidence::EvidenceAddReport>("evidence add", "EvidenceAddReport"),
        entry::<evidence::EvidenceAddReport>("evidence audit", "EvidenceAuditReport"),
        entry::<scaffold::FeatureNewReport<'static>>("feature new", "FeatureNewReport"),
        entry::<finding::FindingAddReport<'static>>("finding add", "FindingAddReport"),
        entry::<finding::FindingCloseReport<'static>>("finding close", "FindingCloseReport"),
        entry::<finding::FindingListReport<'static>>("finding list", "FindingListReport"),
        entry::<gate::GateCheckReport<'static>>("gate check", "GateCheckReport"),
        entry::<gate::GateInstallHooksReport>("gate install-hooks", "GateInstallHooksReport"),
        entry::<gate::GatePromoteReport>("gate promote", "GatePromoteReport"),
        entry::<gate::GateSelftestReport>("gate selftest", "GateSelftestReport"),
        entry::<gate::TaskDoneReport<'static>>("gate task", "TaskDoneReport"),
        entry::<learn::StrategyApprovalPayloadReport>("learn approve", "StrategyApprovalPayloadReport"),
        entry::<learn::StrategyPromoteReport>("learn promote", "StrategyPromoteReport"),
        entry::<learn::StrategyRollbackReport>("learn rollback", "StrategyRollbackReport"),
        entry::<policy_approval::PolicyApproveReport<'static>>("policy approve", "PolicyApproveReport"),
        entry::<policy_approval::PolicyShowReport<'static>>("policy show", "PolicyShowReport"),
        entry::<review::ReviewReport>("review change", "ReviewChangeReport"),
        entry::<review::ReviewReport>("review scenario", "ReviewScenarioReport"),
        entry::<scaffold::ScenarioNewBatchReport>("scenario new", "ScenarioNewBatchReport"),
        entry::<scaffold::ScenarioNewReport<'static>>("scenario new", "ScenarioNewReport"),
        entry::<scaffold::ScenarioNewRefusal<'static>>("scenario new", "ScenarioNewRefusalReport"),
        entry::<adapter::AuthorizationResult>("adapter authorize", "AdapterAuthorizeReport"),
        entry::<adapter::ValidationSummary>("adapter validate", "AdapterValidateReport"),
        entry::<brief_check::BriefCheckReport>("brief check", "BriefCheckReport"),
        entry::<brief_check::BriefShowReport>("brief show", "BriefShowReport"),
        entry::<brief_approve::Approved>("brief approve", "BriefApproveReport"),
        entry::<completion::CompletionReport>("completion", "CompletionReport"),
        entry::<context::AuthoringSurface>("context", "ContextReport"),
        entry::<context_pack::ContextPack>("context-pack new", "ContextPackAddReport"),
        entry::<context_pack::ContextPack>("context-pack show", "ContextPackShowReport"),
        entry::<context_pack::VerifyReport>("context-pack verify", "ContextPackVerifyReport"),
        entry::<artifact_ingest::ArtifactIngestOutcome>("ingest artifacts", "IngestRecordsReport"),
        entry::<plans::PlansOutcome>("ingest plans", "IngestPlansReport"),
        entry::<context_pack::PromptBundleVersion>("prompt add", "PromptAddReport"),
        entry::<context_pack::PromptBundleVersion>("prompt show", "PromptShowReport"),
        entry::<retrieve::RetrieveReport<'static>>("retrieve", "RetrieveReport"),
        entry::<selftest::SelftestSummary>("selftest", "SelftestReport"),
        entry::<status::StatusReport>("status", "StatusReport"),
        entry::<subject::SubjectReport<'static>>("subject adopt", "SubjectAdoptReport"),
        entry::<subject::SubjectReport<'static>>("subject new", "SubjectNewReport"),
        entry::<subject::SubjectReport<'static>>("subject status", "SubjectMoveReport"),
        entry::<subject::SubjectReport<'static>>("subject status", "SubjectShowReport"),
        entry::<subject::SubjectReport<'static>>("subject unadopt", "SubjectUnadoptReport"),
    ];
    schemas.sort_by(|a, b| a.command.cmp(b.command).then_with(|| a.name.cmp(&b.name)));
    schemas
}

/// The success `{ok: true, data, meta}` and failure `{ok: false, error,
/// meta[, data]}` envelopes as JSON Schemas (2020-12): `{"success": …,
/// "failure": …}`. `data` is typed per command by [`report_schemas`].
pub fn envelope_schemas() -> Value {
    json!({ "success": envelope_schema::<crate::json_out::SuccessEnvelope>(true), "failure": envelope_schema::<crate::json_out::FailureEnvelope>(false) })
}

fn envelope_schema<E: JsonSchema>(ok: bool) -> Value {
    let mut schema = schema_of::<E>();
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert("ok".to_string(), json!({ "const": ok, "description": if ok { "true: the command succeeded" } else { "false: the command failed; see `error`" } }));
    }
    schema
}
