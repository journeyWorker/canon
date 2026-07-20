//! Shared `canon fmt --check` output types: stable failure-class
//! strings (task 2.2) mapped 1:1 onto the 2026-07-10 artifact audit's
//! gap categories (design §5 S11 table / proposal.md's verbatim
//! reproduction) — chosen so the audit's own gap list is exactly
//! `FmtFailureClass::ALL`'s cardinality, no more, no less (spec
//! scenario "no unaudited gap reported and no audited gap missed").

use std::path::PathBuf;

/// One audited gap category. `as_str()` is the STABLE wire string
/// (never renamed without a coordinated fixture update, same
/// discipline as `canon_model::evidence::FailureClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FmtFailureClass {
    /// features/ third partition grammar; inventory/ partition-key
    /// smeared into filenames; any leaf/segment mismatch against a
    /// kind's [`canon_model::family::LayoutDescriptor`].
    LayoutGrammar,
    /// inventory/ (and, pre-migration, features/) missing the
    /// `{schema, kind, at, actor}` envelope entirely.
    MissingEnvelope,
    /// features/ scenario/feature header missing its `# canon: {...}`
    /// authoring-provenance comment (design D2).
    MissingProvenance,
    /// A record's actor is absent or still a bare `by` string (ledger
    /// run/drill) rather than the structured envelope actor;
    /// divergence events missing `actor` entirely.
    MissingActor,
    /// A ledger `run`/`drill` record's `evidence` array carries no
    /// typed content (donor's `evidence: []` — audit: "unspecified").
    UnspecifiedEvidence,
    /// A ref string (`upstream_ref`/`port_ref`/divergence `port_ref`) does
    /// not match the `<file>#<symbol>[:<a>-<b>]` grammar at all —
    /// free prose, never guessed into a `{file, symbol}` (design D4).
    FreeTextRef,
    /// A ref string is `;`/`,`-joined (multiple segments) but has not
    /// yet been split into the structured `refs` array.
    JoinedRef,
    /// `app_sha`/`harness_sha` is present but shorter than the full
    /// 40-hex sha grammar.
    AbbreviatedSha,
    /// A divergence review event's `ledger_ref` has no reciprocal
    /// `divergence_refs` entry on the ledger record it names.
    OneWayBackref,
    /// Corpus-wide: no record in this family carries `change_id`/
    /// `task_id` (or `session_id` on an actor) — reported ONCE per
    /// corpus, not per record (S11 design Non-Goal: backfilling
    /// historical joins is explicitly out of scope; only new records
    /// populate them going forward, so this is a standing gap note,
    /// not a per-record defect to fix).
    MissingJoinIdentity,
    /// A family record fails the REGISTERED `canon-model` JSON schema
    /// for its own resolved kind (`canon_model::schema_export::family_schemas`)
    /// — a required envelope/content field absent, or present with the
    /// wrong JSON type. Also covers a wire `kind` string with no
    /// registered schema at all ("no registered schema for kind …"),
    /// proving the registry lookup runs rather than being silently
    /// skipped. Distinct from `LayoutGrammar` (path shape) and the
    /// hand-rolled field checks above (business-rule gaps a schema
    /// alone cannot express, e.g. an abbreviated sha still being a
    /// syntactically valid string).
    SchemaViolation,
}

impl FmtFailureClass {
    pub const ALL: [FmtFailureClass; 11] = [
        FmtFailureClass::LayoutGrammar,
        FmtFailureClass::MissingEnvelope,
        FmtFailureClass::MissingProvenance,
        FmtFailureClass::MissingActor,
        FmtFailureClass::UnspecifiedEvidence,
        FmtFailureClass::FreeTextRef,
        FmtFailureClass::JoinedRef,
        FmtFailureClass::AbbreviatedSha,
        FmtFailureClass::OneWayBackref,
        FmtFailureClass::MissingJoinIdentity,
        FmtFailureClass::SchemaViolation,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FmtFailureClass::LayoutGrammar => "layout-grammar",
            FmtFailureClass::MissingEnvelope => "missing-envelope",
            FmtFailureClass::MissingProvenance => "missing-provenance",
            FmtFailureClass::MissingActor => "missing-actor",
            FmtFailureClass::UnspecifiedEvidence => "unspecified-evidence",
            FmtFailureClass::FreeTextRef => "free-text-ref",
            FmtFailureClass::JoinedRef => "joined-ref",
            FmtFailureClass::AbbreviatedSha => "abbreviated-sha",
            FmtFailureClass::OneWayBackref => "one-way-backref",
            FmtFailureClass::MissingJoinIdentity => "missing-join-identity",
            FmtFailureClass::SchemaViolation => "schema-violation",
        }
    }
}

impl std::fmt::Display for FmtFailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One reported gap, always corpus-root-relative-path-scoped (never an
/// absolute path — a fixture tmpdir and the real consumer-repo checkout
/// must produce byte-identical reports for the same corpus shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub class: FmtFailureClass,
    pub path: PathBuf,
    pub detail: String,
}

impl Violation {
    pub fn new(class: FmtFailureClass, path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Self { class, path: path.into(), detail: detail.into() }
    }
}

/// `canon fmt --check`'s full output: per-record violations plus
/// corpus-wide summary notes ([`FmtFailureClass::MissingJoinIdentity`]
/// only, today).
#[derive(Debug, Clone, Default)]
pub struct FmtReport {
    pub violations: Vec<Violation>,
    pub files_checked: usize,
}

impl FmtReport {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn count_by_class(&self, class: FmtFailureClass) -> usize {
        self.violations.iter().filter(|v| v.class == class).count()
    }

    /// Every distinct class actually observed, sorted — the audit-match
    /// scenario's "no unaudited gap reported and no audited gap missed"
    /// check compares this set against the expected audit categories.
    pub fn observed_classes(&self) -> Vec<FmtFailureClass> {
        let mut classes: Vec<FmtFailureClass> = self.violations.iter().map(|v| v.class).collect();
        classes.sort();
        classes.dedup();
        classes
    }

    /// Human-readable report, grouped by failure class, stable order —
    /// `canon fmt --check`'s stdout.
    pub fn format_human(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("canon fmt --check: {} file(s) checked, {} violation(s)\n", self.files_checked, self.violations.len()));
        for class in FmtFailureClass::ALL {
            let matching: Vec<&Violation> = self.violations.iter().filter(|v| v.class == class).collect();
            if matching.is_empty() {
                continue;
            }
            out.push_str(&format!("\n[{}] {} occurrence(s)\n", class.as_str(), matching.len()));
            for v in matching {
                out.push_str(&format!("  {} — {}\n", v.path.display(), v.detail));
            }
        }
        out
    }
}
