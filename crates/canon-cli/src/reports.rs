//! Typed reports and the schemas the spec publishes (0.28 D2, D3).
//!
//! Every `--json` payload is a struct that implements [`Report`]. The trait
//! ties `Serialize` to `JsonSchema`, and `json_out::data` is generic over it,
//! so a payload without a schema does not compile. [`report_schemas`] lists one schema
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

/// A refusal that `--override-reason` waived (0.28 D12): the error code the
/// command would have refused with, and the reason given. Listed in
/// `data.overrides[]` of every command that accepts the option, and recorded
/// on the record the waiver let through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Override {
    /// The code of the waived refusal (`BRIEF_UNAPPROVED`, `SUBJECT_GATE_VIOLATION`, …).
    pub code: String,
    /// The reason the caller gave, verbatim.
    pub reason: String,
}

/// One typed report in the generated spec.
#[derive(Debug, Clone)]
pub struct ReportSchema {
    /// The command path (as `schema` lists it today) whose `data` this is.
    pub command: &'static str,
    /// `<PascalCaseCommandId>Report`, the name the per-leaf table gives it.
    pub name: String,
    pub schema_version: &'static str,
    /// The Rust type of the report (`std::any::type_name`): the witness (`CANON_WITNESS_DIR`) names the type it saw.
    pub type_name: &'static str,
    /// A JSON Schema 2020-12 for `data`.
    pub schema: Value,
}

fn entry<R: Report>(command: &'static str, name: &str) -> ReportSchema {
    ReportSchema { command, name: name.to_string(), schema_version: R::SCHEMA_VERSION, type_name: std::any::type_name::<R>(), schema: schema_of::<R>() }
}

fn batch_entry<R: Report + 'static>(command: &'static str, name: &str) -> ReportSchema {
    ReportSchema { command, name: name.to_string(), schema_version: crate::input::BatchReport::<R>::SCHEMA_VERSION, type_name: std::any::type_name::<crate::input::BatchReport<R>>(), schema: schema_of::<crate::input::BatchReport<R>>() }
}

/// The JSON Schema of `R` as a value.
pub fn schema_of<R: JsonSchema>() -> Value {
    // A report is only ever serialized, so its schema follows what `Serialize` writes (renames, skipped fields).
    let generator = schemars::generate::SchemaSettings::draft2020_12().for_serialize().into_generator();
    serde_json::to_value(generator.into_root_schema_for::<R>()).expect("a JSON Schema serializes")
}

/// Every typed report, one entry per `(command, name)`, sorted by command.
pub fn report_schemas() -> Vec<ReportSchema> {
    use crate::{adapter, artifact_ingest, artifacts, brief_approve, brief_check, change, completion, context, context_pack, demo, dispatch, divergence, evidence, finding, gate, learn, ledger, migrate, plans, policy_approval, retention, retrieve, review, scaffold, selftest, status, subject, tier};
    let mut schemas: Vec<ReportSchema> = vec![
        entry::<change::ChangeNewReport<'static>>("change new", "ChangeNewReport"),
        entry::<demo::DemoReviewReport>("demo review", "DemoReviewReport"),
        entry::<demo::DemoInitReport>("demo init", "DemoInitReport"),
        entry::<dispatch::DispatchBeginReport<'static>>("dispatch begin", "DispatchBeginReport"),
        entry::<dispatch::DispatchDiffReport<'static>>("dispatch diff", "DispatchDiffReport"),
        entry::<dispatch::DispatchEndReport<'static>>("dispatch end", "DispatchEndReport"),
        entry::<divergence::DivergenceAcceptReport<'static>>("divergence accept", "DivergenceAcceptReport"),
        entry::<divergence::DivergenceAddReport>("divergence add", "DivergenceAddReport"),
        entry::<divergence::DivergenceCommitReport>("divergence defer", "DivergenceDeferReport"),
        entry::<divergence::DivergenceCommitReport>("divergence resolve", "DivergenceResolveReport"),
        entry::<divergence::DivergenceListReport<'static>>("divergence list", "DivergenceListReport"),
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
        entry::<gate::TaskDoneReport<'static>>("task done", "TaskDoneReport"),
        entry::<learn::StrategyPromoteReport>("strategy promote", "StrategyPromoteReport"),
        entry::<learn::StrategyRollbackReport>("strategy rollback", "StrategyRollbackReport"),
        entry::<policy_approval::PolicyApproveReport<'static>>("policy approve", "PolicyApproveReport"),
        entry::<policy_approval::PolicyShowReport<'static>>("policy show", "PolicyShowReport"),
        entry::<review::ReviewReport>("review change", "ReviewChangeReport"),
        entry::<review::ReviewReport>("review scenario", "ReviewScenarioReport"),
        entry::<scaffold::ScenarioNewReport<'static>>("scenario new", "ScenarioNewReport"),
        entry::<evidence::ArtifactsStoreReport<'static>>("artifacts store", "ArtifactsStoreReport"),
        entry::<artifacts::ArtifactsVerifyReport<'static>>("artifacts verify", "ArtifactsVerifyReport"),
        entry::<adapter::AuthorizationResult>("adapter authorize", "AdapterAuthorizeReport"),
        entry::<adapter::ValidationSummary>("adapter validate", "AdapterValidateReport"),
        entry::<brief_check::BriefCheckReport>("brief check", "BriefCheckReport"),
        entry::<brief_check::BriefShowReport>("brief show", "BriefShowReport"),
        entry::<brief_approve::Approved>("brief approve", "BriefApproveReport"),
        entry::<brief_approve::ApprovedMany>("brief approve", "BriefApproveManyReport"),
        entry::<completion::CompletionReport>("completion", "CompletionReport"),
        entry::<context::ContextReport>("context", "ContextReport"),
        entry::<context_pack::ContextPack>("context-pack add", "ContextPackAddReport"),
        entry::<context_pack::ContextPack>("context-pack show", "ContextPackShowReport"),
        entry::<context_pack::VerifyReport>("context-pack verify", "ContextPackVerifyReport"),
        entry::<artifact_ingest::ArtifactIngestOutcome>("ingest records", "IngestRecordsReport"),
        entry::<plans::PlansOutcome>("ingest plans", "IngestPlansReport"),
        entry::<artifacts::ArtifactsMoveReport>("artifacts move", "ArtifactsMoveReport"),
        entry::<ledger::LedgerCompactReport>("ledger compact", "LedgerCompactReport"),
        entry::<migrate::MigrateReport>("migrate", "MigrateReport"),
        entry::<retention::PurgeReport>("purge", "PurgeReport"),
        entry::<tier::TierAgeReport>("tier age", "TierAgeReport"),
        entry::<context_pack::PromptBundleVersion>("prompt add", "PromptAddReport"),
        entry::<context_pack::PromptBundleVersion>("prompt show", "PromptShowReport"),
        entry::<learn::StrategyApprovalPayloadReport>("strategy approval-payload", "StrategyApprovalPayloadReport"),
        entry::<retrieve::RetrieveReport<'static>>("retrieve", "RetrieveReport"),
        entry::<selftest::SelftestSummary>("selftest", "SelftestReport"),
        entry::<status::StatusReport>("status", "StatusReport"),
        entry::<subject::SubjectReport<'static>>("subject adopt", "SubjectAdoptReport"),
        entry::<subject::SubjectReport<'static>>("subject new", "SubjectNewReport"),
        entry::<subject::SubjectMoveReport<'static>>("subject move", "SubjectMoveReport"),
        entry::<subject::SubjectReport<'static>>("subject show", "SubjectShowReport"),
        entry::<subject::SubjectReport<'static>>("subject unadopt", "SubjectUnadoptReport"),
        entry::<crate::dashboard::DashboardReport>("dashboard", "DashboardReport"),
        entry::<crate::dashboard::DashboardStatusReport<'static>>("dashboard", "DashboardStatusReport"),
        entry::<crate::dashboard_briefs::DashboardBriefsReport>("dashboard", "DashboardBriefsReport"),
        entry::<crate::retention::ExportReport>("export", "ExportReport"),
        entry::<crate::fmt::FormatReport<'static>>("format", "FormatReport"),
        entry::<crate::ingest::IngestSessionsReport<'static>>("ingest sessions", "IngestSessionsReport"),
        entry::<crate::init::InitReport>("init", "InitReport"),
        entry::<crate::init::InitCheckReport>("init", "InitCheckReport"),
        entry::<crate::inventory::InventorySyncReport<'static>>("inventory sync", "InventorySyncReport"),
        entry::<crate::plugin_sync::PluginSyncReport<'static>>("plugin sync", "PluginSyncReport"),
        entry::<crate::query::QueryReport>("query", "QueryReport"),
        entry::<crate::search::SearchReport>("search", "SearchReport"),
        entry::<crate::search::TraceReport>("trace", "TraceReport"),
        entry::<crate::retrieve::RegimeShowReport<'static>>("regime show", "RegimeShowReport"),
        entry::<crate::report::ReportBuildReport>("report build", "ReportBuildReport"),
        entry::<crate::report::ReportCheckReport>("report check", "ReportCheckReport"),
        entry::<crate::report::ReportSnapshotReport<'static>>("report snapshot", "ReportSnapshotReport"),
        entry::<crate::schema::SchemaReport<'static>>("schema", "SchemaReport"),
        entry::<crate::skills::SkillsCheckReport>("skills check", "SkillsCheckReport"),
        entry::<crate::skills::SkillsDoctorReport<'static>>("skills doctor", "SkillsDoctorReport"),
        entry::<crate::skills::SkillsInstallReport<'static>>("skills install", "SkillsInstallReport"),
        entry::<crate::skills::SkillsShowReport>("skills show", "SkillsShowReport"),
        batch_entry::<change::ChangeNewReport<'static>>("change new", "BatchChangeNewReport"),
        batch_entry::<demo::DemoReviewReport>("demo review", "BatchDemoReviewReport"),
        batch_entry::<demo::DemoInitReport>("demo init", "BatchDemoInitReport"),
        batch_entry::<dispatch::DispatchBeginReport<'static>>("dispatch begin", "BatchDispatchBeginReport"),
        batch_entry::<dispatch::DispatchEndReport<'static>>("dispatch end", "BatchDispatchEndReport"),
        batch_entry::<divergence::DivergenceAcceptReport<'static>>("divergence accept", "BatchDivergenceAcceptReport"),
        batch_entry::<divergence::DivergenceAddReport>("divergence add", "BatchDivergenceAddReport"),
        batch_entry::<divergence::DivergenceCommitReport>("divergence defer", "BatchDivergenceDeferReport"),
        batch_entry::<divergence::DivergenceCommitReport>("divergence resolve", "BatchDivergenceResolveReport"),
        batch_entry::<evidence::EvidenceAddReport>("evidence add", "BatchEvidenceAddReport"),
        batch_entry::<evidence::EvidenceAddReport>("evidence audit", "BatchEvidenceAuditReport"),
        batch_entry::<scaffold::FeatureNewReport<'static>>("feature new", "BatchFeatureNewReport"),
        batch_entry::<finding::FindingAddReport<'static>>("finding add", "BatchFindingAddReport"),
        batch_entry::<finding::FindingCloseReport<'static>>("finding close", "BatchFindingCloseReport"),
        batch_entry::<gate::GateInstallHooksReport>("gate install-hooks", "BatchGateInstallHooksReport"),
        batch_entry::<gate::GatePromoteReport>("gate promote", "BatchGatePromoteReport"),
        batch_entry::<gate::TaskDoneReport<'static>>("task done", "BatchTaskDoneReport"),
        batch_entry::<learn::StrategyPromoteReport>("strategy promote", "BatchStrategyPromoteReport"),
        batch_entry::<learn::StrategyRollbackReport>("strategy rollback", "BatchStrategyRollbackReport"),
        batch_entry::<policy_approval::PolicyApproveReport<'static>>("policy approve", "BatchPolicyApproveReport"),
        batch_entry::<review::ReviewReport>("review change", "BatchReviewChangeReport"),
        batch_entry::<review::ReviewReport>("review scenario", "BatchReviewScenarioReport"),
        batch_entry::<scaffold::ScenarioNewReport<'static>>("scenario new", "BatchScenarioNewReport"),
        batch_entry::<evidence::ArtifactsStoreReport<'static>>("artifacts store", "BatchArtifactsStoreReport"),
        batch_entry::<brief_approve::Approved>("brief approve", "BatchBriefApproveReport"),
        batch_entry::<context_pack::ContextPack>("context-pack add", "BatchContextPackAddReport"),
        batch_entry::<artifact_ingest::ArtifactIngestOutcome>("ingest records", "BatchIngestRecordsReport"),
        batch_entry::<plans::PlansOutcome>("ingest plans", "BatchIngestPlansReport"),
        batch_entry::<context_pack::PromptBundleVersion>("prompt add", "BatchPromptAddReport"),
        batch_entry::<subject::SubjectReport<'static>>("subject adopt", "BatchSubjectAdoptReport"),
        batch_entry::<subject::SubjectReport<'static>>("subject new", "BatchSubjectNewReport"),
        batch_entry::<subject::SubjectReport<'static>>("subject move", "BatchSubjectMoveReport"),
        batch_entry::<subject::SubjectReport<'static>>("subject unadopt", "BatchSubjectUnadoptReport"),
        batch_entry::<crate::retention::ExportReport>("export", "BatchExportReport"),
        batch_entry::<crate::ingest::IngestSessionsReport<'static>>("ingest sessions", "BatchIngestSessionsReport"),
        batch_entry::<crate::init::InitReport>("init", "BatchInitReport"),
        batch_entry::<crate::inventory::InventorySyncReport<'static>>("inventory sync", "BatchInventorySyncReport"),
        batch_entry::<crate::plugin_sync::PluginSyncReport<'static>>("plugin sync", "BatchPluginSyncReport"),
        batch_entry::<crate::report::ReportBuildReport>("report build", "BatchReportBuildReport"),
        batch_entry::<crate::report::ReportSnapshotReport<'static>>("report snapshot", "BatchReportSnapshotReport"),
        batch_entry::<crate::skills::SkillsInstallReport<'static>>("skills install", "BatchSkillsInstallReport"),
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
