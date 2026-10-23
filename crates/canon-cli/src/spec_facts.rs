//! What the generated command spec (`crate::spec`) says about a leaf that
//! neither clap nor [`crate::schema::META`] can: how a repeat behaves
//! (`idempotency`), what the command prints (`outputs`), and operand
//! descriptions clap has no room for.
//!
//! One row per leaf, in [`FACTS`], keyed by canonical path like `META`;
//! [`check_facts`] reports every leaf without a row and every row without a
//! leaf, and `tests/cli_tree.rs` fails on either. Report names are the
//! per-leaf table's (`docs/design/cli-commands-0.28.md`, Output column), so
//! a leaf whose 0.28 spelling differs from its path today carries the report
//! of its 0.28 row.

use clap::Command;
use serde::Serialize;

/// What running a writing leaf a second time with the same arguments does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Idempotency {
    /// A repeat is a no-op, reported `unchanged`.
    ReplaySafe,
    /// Identical content is a no-op; different content under the same key is refused.
    ContentKeyed,
    /// Every call adds a record.
    Appends,
    /// A repeat is refused with exit 2 and changes nothing.
    Once,
}

/// The form a leaf's output takes on stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    /// An enveloped report.
    Report,
    /// Raw bytes on stdout without `--json`; `--json` envelopes them.
    Artifact,
    /// A long-running stdout.
    Stream,
}

/// One output of a leaf.
#[derive(Debug, Clone, Copy)]
pub struct OutputMeta {
    pub kind: OutputKind,
    /// The report schema's name: `<PascalCaseCommandId>Report`. For an
    /// artifact, the report that envelopes it under `--json`.
    pub report: &'static str,
    /// The media type of an artifact's bytes.
    pub media_type: Option<&'static str>,
}

/// A description of one operand, where the doc comment on its clap argument
/// cannot say it (a format, a default).
#[derive(Debug, Clone, Copy)]
pub struct OperandMeta {
    /// The operand's clap id.
    pub name: &'static str,
    pub about: &'static str,
}

/// The spec facts of one leaf.
pub struct LeafFacts {
    pub path: &'static str,
    /// Required for a writing leaf; a read has one only where the per-leaf table declares it.
    pub idempotency: Option<Idempotency>,
    pub outputs: &'static [OutputMeta],
    pub operands: &'static [OperandMeta],
}

const fn leaf(path: &'static str) -> LeafFacts {
    LeafFacts { path, idempotency: None, outputs: &[], operands: &[] }
}

impl LeafFacts {
    const fn idem(mut self, idempotency: Idempotency) -> Self {
        self.idempotency = Some(idempotency);
        self
    }

    const fn out(mut self, outputs: &'static [OutputMeta]) -> Self {
        self.outputs = outputs;
        self
    }

    const fn operands(mut self, operands: &'static [OperandMeta]) -> Self {
        self.operands = operands;
        self
    }
}

const fn report(report: &'static str) -> OutputMeta {
    OutputMeta { kind: OutputKind::Report, report, media_type: None }
}

const fn artifact(media_type: &'static str, report: &'static str) -> OutputMeta {
    OutputMeta { kind: OutputKind::Artifact, report, media_type: Some(media_type) }
}

const fn stream(report: &'static str) -> OutputMeta {
    OutputMeta { kind: OutputKind::Stream, report, media_type: None }
}

const fn operand(name: &'static str, about: &'static str) -> OperandMeta {
    OperandMeta { name, about }
}

use Idempotency::{Appends, ContentKeyed, Once, ReplaySafe};

/// One row per leaf command, canonical path first.
pub const FACTS: &[LeafFacts] = &[
    leaf("adapter authorize").out(&[report("AdapterAuthorizeReport")]),
    leaf("adapter validate").out(&[report("AdapterValidateReport")]),
    leaf("artifacts migrate").idem(ReplaySafe).out(&[report("ArtifactsMoveReport")]),
    leaf("artifacts verify").out(&[report("ArtifactsVerifyReport")]),
    leaf("brief approve").idem(ReplaySafe).out(&[report("BriefApproveReport")]),
    leaf("brief check").out(&[report("BriefCheckReport")]),
    leaf("brief show").out(&[report("BriefShowReport")]),
    leaf("change new").idem(Once).out(&[report("ChangeNewReport")]),
    leaf("completion").out(&[artifact("text/x-shellscript", "CompletionReport")]),
    leaf("context").out(&[report("ContextReport")]),
    leaf("context-pack new").idem(ContentKeyed).out(&[report("ContextPackAddReport")]),
    leaf("context-pack show").out(&[report("ContextPackShowReport")]),
    leaf("context-pack verify").out(&[report("ContextPackVerifyReport")]),
    leaf("dashboard").idem(ReplaySafe).out(&[stream("DashboardReport")]),
    leaf("demo add").idem(Appends).out(&[report("DemoReviewReport")]),
    leaf("demo init").idem(Once).out(&[report("DemoInitReport")]),
    leaf("dispatch begin").idem(Appends).out(&[report("DispatchBeginReport")]),
    leaf("dispatch diff").out(&[report("DispatchDiffReport")]),
    leaf("dispatch end").idem(Once).out(&[report("DispatchEndReport")]),
    leaf("divergence accept").idem(Appends).out(&[report("DivergenceAcceptReport")]),
    leaf("divergence add").idem(ContentKeyed).out(&[report("DivergenceAddReport")]),
    leaf("divergence defer").idem(Appends).out(&[report("DivergenceDeferReport")]),
    leaf("divergence promote").idem(ReplaySafe).out(&[report("GatePromoteReport")]),
    leaf("divergence resolve").idem(Appends).out(&[report("DivergenceResolveReport")]),
    leaf("divergence status").out(&[report("DivergenceListReport")]),
    leaf("evidence add").idem(ContentKeyed).out(&[report("EvidenceAddReport")]),
    leaf("evidence audit").idem(ContentKeyed).out(&[report("EvidenceAuditReport")]),
    leaf("evidence approval-payload").idem(ReplaySafe).out(&[artifact("application/octet-stream", "EvidenceApprovalPayloadReport")]),
    leaf("evidence vault").idem(ReplaySafe).out(&[report("ArtifactsStoreReport")]),
    leaf("export").idem(ReplaySafe).out(&[artifact("application/json", "ExportReport")]),
    leaf("feature new").idem(Once).out(&[report("FeatureNewReport")]),
    leaf("finding add").idem(ContentKeyed).out(&[report("FindingAddReport")]),
    leaf("finding close").idem(ReplaySafe).out(&[report("FindingCloseReport")]),
    leaf("finding list").out(&[report("FindingListReport")]),
    leaf("format").out(&[report("FormatReport")]),
    leaf("gate check").out(&[report("GateCheckReport")]),
    leaf("gate install-hooks").idem(ReplaySafe).out(&[report("GateInstallHooksReport")]),
    leaf("gate promote").idem(ReplaySafe).out(&[report("GatePromoteReport")]),
    leaf("gate selftest").out(&[report("SelftestReport")]),
    leaf("gate task").idem(ReplaySafe).out(&[report("TaskDoneReport")]),
    leaf("ingest artifacts").idem(ReplaySafe).out(&[report("IngestRecordsReport")]),
    leaf("ingest plans").idem(ReplaySafe).out(&[report("IngestPlansReport")]),
    leaf("ingest sessions").idem(ReplaySafe).out(&[report("IngestSessionsReport")]),
    leaf("init").idem(ReplaySafe).out(&[report("InitReport")]),
    leaf("inventory sync").idem(ReplaySafe).out(&[report("InventorySyncReport")]),
    // 0.28 reads this leaf (`strategy approval-payload`); its table row declares replay-safe.
    leaf("learn approve").idem(ReplaySafe).out(&[artifact("application/octet-stream", "StrategyApprovalPayloadReport")]).operands(&[operand("strategy_id", "The StrategyId (ULID) whose approval payload is printed")]),
    leaf("learn promote").idem(ReplaySafe).out(&[report("StrategyPromoteReport")]),
    leaf("learn rollback").idem(ReplaySafe).out(&[report("StrategyRollbackReport")]).operands(&[operand("strategy_id", "The StrategyId (ULID) to roll back")]),
    leaf("ledger compact").idem(ReplaySafe).out(&[report("LedgerCompactReport")]),
    leaf("migrate").idem(ReplaySafe).out(&[report("MigrateReport")]),
    leaf("plugin sync").idem(ReplaySafe).out(&[report("PluginSyncReport")]),
    leaf("policy approve").idem(ReplaySafe).out(&[report("PolicyApproveReport")]),
    leaf("policy show").out(&[report("PolicyShowReport")]),
    leaf("prompt add").idem(ContentKeyed).out(&[report("PromptAddReport")]),
    leaf("prompt show").out(&[report("PromptShowReport")]),
    leaf("purge").idem(ReplaySafe).out(&[report("PurgeReport")]),
    leaf("query").out(&[report("QueryReport")]),
    leaf("regime-key").out(&[report("RegimeShowReport")]),
    leaf("report").idem(ReplaySafe).out(&[report("ReportBuildReport")]),
    leaf("retrieve").out(&[report("RetrieveReport")]),
    leaf("review change").idem(ContentKeyed).out(&[report("ReviewChangeReport")]),
    leaf("review scenario").idem(ContentKeyed).out(&[report("ReviewScenarioReport")]),
    leaf("scenario new").idem(Once).out(&[report("ScenarioNewReport")]),
    leaf("schema").out(&[report("SchemaReport")]),
    leaf("selftest").out(&[report("SelftestReport")]),
    leaf("skills check").out(&[report("SkillsCheckReport")]),
    leaf("skills doctor").out(&[report("SkillsDoctorReport")]),
    leaf("skills install").idem(ReplaySafe).out(&[report("SkillsInstallReport")]),
    leaf("status").out(&[report("StatusReport")]),
    leaf("subject adopt").idem(ReplaySafe).out(&[report("SubjectAdoptReport")]),
    leaf("subject new").idem(Once).out(&[report("SubjectNewReport")]),
    leaf("subject status").idem(Once).out(&[report("SubjectMoveReport")]),
    leaf("subject unadopt").idem(ReplaySafe).out(&[report("SubjectUnadoptReport")]),
    leaf("tier age").idem(ReplaySafe).out(&[report("TierAgeReport")]),
];

/// The facts row of the leaf `path`.
pub fn facts_for(path: &str) -> Option<&'static LeafFacts> {
    FACTS.iter().find(|f| f.path == path)
}

/// Drift between clap and [`FACTS`]: empty when every leaf has one row, every
/// row names a leaf, and every leaf declares an output.
pub fn check_facts(root: &Command) -> Vec<String> {
    let leaves = crate::schema::leaf_paths(root);
    let mut problems = Vec::new();
    for leaf in &leaves {
        if facts_for(leaf).is_none() {
            problems.push(format!("clap leaf `{leaf}` has no spec facts row"));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for facts in FACTS {
        if !leaves.iter().any(|l| l == facts.path) {
            problems.push(format!("spec facts row `{}` names no clap leaf command", facts.path));
        }
        if seen.contains(&facts.path) {
            problems.push(format!("spec facts row `{}` appears twice", facts.path));
        }
        seen.push(facts.path);
        if facts.outputs.is_empty() {
            problems.push(format!("spec facts row `{}` declares no output", facts.path));
        }
        let is_read = crate::schema::meta_for(facts.path).is_some_and(|m| m.effect == crate::schema::Effect::Read);
        if !is_read && facts.idempotency.is_none() {
            problems.push(format!("writing leaf `{}` declares no idempotency", facts.path));
        }
    }
    problems
}
