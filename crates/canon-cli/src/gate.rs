//! `canon gate` (S5 wave-2-part2): the trust-spine gate's CLI surface over
//! `canon-gate`'s library — `check` (the DISPATCHER, task 1.9, assembling
//! `canon_gate::check_set` over a repo's `GateContext`), `task`
//! (evidence-gated checkbox flip, task 3.2's `gate_task` wiring),
//! `promote` (O13 staging→committed, task 2.2's `promote` wiring),
//! `install-hooks` (task 4.1's CLI wiring over `install_hooks` +
//! `PRE_COMMIT_SCRIPT`), and `selftest` (task 5.2's CLI wiring over
//! `canon_gate::selftest::run`). Every subcommand that takes `--repo`
//! resolves it through [`crate::context::resolve_repo_root`] — the SAME
//! nearest-ancestor `canon.yaml` walk `canon context`/`canon fmt` use
//! (design D7) — so a gate subcommand run from a subdirectory reads the
//! repo ROOT's `<repo>/canon.yaml`/`<repo>/.canon/policy.yaml`, never a
//! subdirectory's absence of one; `canon_gate::GateCtx::from_repo`'s own
//! doc is explicit that it takes `repo` AS GIVEN, no walk — the walk
//! lives here, exactly once, mirroring `run_context`'s identical split.
//!
//! # `canon gate task` is dialect-agnostic (s35 `gate-plan-dialect-seam`)
//! [`run_task`] no longer hardcodes one plan dialect's directory layout.
//! It resolves the task's plan document from `canon.yaml`'s `plans:`
//! sources ([`crate::plans::load_plan_sources_for_gate`]; an absent
//! `plans:` section falls back to the documented compat default
//! `[{ dialect: openspec, root: <repo> }]`, so every pre-s35 consumer
//! keeps working). Each source's dialect is looked up in
//! `canon_ingest::plan_registry`; the FIRST source whose
//! `PlanWriteBack::locate_task` finds the task's document wins, and the
//! flip (and the typed-atoms-file resolution) is delegated to THAT
//! dialect. No source locating it at all is a loud usage failure naming
//! the sources consulted. The pure evidence DECISION
//! (`canon_gate::gate_task`) and the document MUTATION
//! (`PlanWriteBack::flip_task`) are cleanly split: canon-gate never
//! reads/writes a plan document, and this module never encodes a
//! dialect's on-disk shape.
//!
//! # `canon gate task`'s typed-evidence path (S10 part2, design.md D4)
//! [`run_task`] additionally consults the winning dialect's typed-atoms
//! file (`PlanWriteBack::typed_atoms_path`, e.g. the openspec dialect's
//! `<root>/openspec/changes/<change_id>/tasks.vocab.yaml`) — carrying
//! `{id, tag: "task", attrs}` typed atoms (`canon_vocab::atom::
//! AtomRecord`, S10 design.md D2) for whichever task_ids the change has
//! opted into the typed vocabulary. When `task_id` names such an atom,
//! the gate compiles it against a FRESH `canon_vocab::resolve_snapshot`
//! (never the authoring-time snapshot — design.md Risks: "policy is the
//! live source of truth ... at gate time, not authoring time"), reads
//! its validated `evidence: {kind, ref}`, and narrows the evidence slice
//! `canon_gate::gate_task` is handed to exactly the ledger records
//! carrying a matching `evidence: {kind, ref}` companion (this module's
//! own convention, mirroring `canon_gate::markers::evidence_note_of`/
//! `trust_ladder`/`evidence_sha`'s established "re-read the raw ledger
//! JSON for a companion key `EvidenceRecord`'s own strict `Deserialize`
//! silently drops" pattern) — and its `EvidenceNote` companions are
//! narrowed to that SAME matched set too ([`typed_path_evidence`]/
//! [`notes_of`], S10 part2 fix: a stale/wrong-kind record sharing
//! `task_id` can supply neither evidence nor a note to the typed flip).
//! No matching atom (the dialect has no typed-atoms convention, no such
//! file, or `task_id` absent from it) falls straight through to the
//! untyped free path — every non-`Divergent` `EvidenceRecord` for
//! `task_id`, kind-agnostic — additive, never a migration.
//!
//! # Exit-code contract (design decision 9's own two-way half + usage)
//! `0` clean, `1` gate-red (any violation/refusal/mismatch found), `2`
//! usage-or-infra failure (bad `--repo`, unreadable `tasks.md`, a
//! `canon-store`/`canon-gate` load error) — the third state
//! `crate::fmt`/`report.rs`'s own module docs name but never themselves
//! need to return, since a CLI subcommand is the first layer that can
//! actually distinguish "the gate ran and found problems" from "the gate
//! could not run at all". A typed atom that fails vocabulary validation,
//! or resolves evidence outside the policy-derived kind domain, is a
//! gate-red `1` (the same class of "the gate ran and found the task
//! isn't ready" outcome the free path's `unevidenced-flip` already is) —
//! never a usage failure, since the repo/CLI invocation itself is fine.
//!
//! # s42 (`close-the-open-loops`): the authoring half exists now
//! Until s42 the ONLY production writer of an `EvidenceRecord` was
//! `crate::demo`, so [`run_task`]'s `unevidenced-flip` refusal was
//! unsatisfiable outside the demo: this repo's plan corpus carried
//! hundreds of checked task boxes against exactly ONE committed
//! `EvidenceRecord`. [`crate::evidence`] is the
//! missing seam — it stages a record into [`evidence_staging_dir`],
//! [`run_promote`] commits it, and [`run_task`] then flips on it with
//! no `--force` anywhere in the path. Three pieces of THIS module are
//! shared with it rather than copied, so the author and the gate cannot
//! drift: [`evidence_staging_dir`] (one path literal), [`locate_task`]
//! (one plan-source resolution), and
//! [`typed_evidence_contract_for_task`] (one reading of a task's typed
//! `{kind, ref}` contract). The authoring command grades a refusal with
//! the SAME exit code this module's contract above assigns the same
//! condition — see [`TypedContractError`].

use std::path::{Path, PathBuf};

use canon_gate::{
    evidence_note_of, gate_task, install_hooks, promote as gate_promote, selftest, EvidenceNote, FailureClass, GateContext, GateCtx, GateReport, HookEntry, InstallOutcome,
    Promoted, PromoteReport, TaskFlipDecision, FAILURE_CLASSES, PRE_COMMIT_SCRIPT, STAGED_KINDS,
};
use canon_ingest::{find_plan_adapter, PlanWriteBack, WriteBackError};
use canon_model::paths;
use canon_model::{validate_evidence_batch, Actor, Envelope, RawRecord, RecordKind, TaskId};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};
use chrono::Utc;

use crate::context::resolve_repo_root;

/// `canon gate check [--repo] [--release]` (task 1.9, the dispatcher):
/// assembles `canon_gate::check_set(release)` (coverage/ledger/staleness/
/// trust-ladder, plus the release-scoped `ReleaseTrustCheck` when
/// `--release` is given — `canon_gate::dispatch`'s own module doc: the
/// dispatcher never drops `TrustLadderCheck` when a release profile is
/// engaged) and runs it over the resolved repo's `GateContext`.
pub fn run_check(repo: &Path, release: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("canon gate check: {e}");
            return 2;
        }
    };
    let registry = SchemaRegistry::load();
    // The ONE `Utc::now()` call for this invocation (s21
    // `deterministic-gate-clock` D6, mirroring `scaffold.rs`'s
    // `run_scenario_new`/`run_feature_new` dispatch-boundary idiom) —
    // every check this run engages reads `gate_context.now`, never its
    // own wall-clock read.
    let now = Utc::now();
    let gate_context = match GateContext::load(ctx, &registry, now) {
        Ok(gc) => gc,
        Err(e) => {
            eprintln!("canon gate check: {e}");
            return 2;
        }
    };

    let checks = canon_gate::check_set(release);
    let report = GateReport::from_violations(checks.iter().flat_map(|check| check.run(&gate_context)).collect());
    print!("{}", format_gate_report(&report));
    if let Some(count) = canon_gate::spec_coverage::coverage_off_scenarios(&gate_context) {
        print!("{}", format_coverage_off_advisory(count));
    }
    if let Some(summary) = canon_gate::binding_summary(&gate_context) {
        print!("{}", format_binding_summary(&summary));
    }
    if let Some(advisories) = canon_gate::review_advisories(&gate_context) {
        print!("{}", format_review_advisories(&advisories));
    }
    print!("{}", format_unstored_attachments(&canon_gate::unstored_attachments(&gate_context)));
    report.exit_code()
}

/// The one-line advisory (0.14 D3) printed when the spec corpus has
/// scenarios but `policy.yaml` has no `spec_coverage` section, so none
/// of them is checked for evidence. Stdout, after the gate result, like
/// every other gate advisory; it never changes the exit code.
fn format_coverage_off_advisory(count: usize) -> String {
    format!("\nadvisory: spec_coverage is off — {count} scenario(s) are not checked for evidence; see .canon/policy.yaml\n")
}

/// Bound files with no stored blob that the gate verified against the
/// working tree instead (0.14 D4's back-compat path for 0.11–0.13
/// records). Advisories, never violations; prints nothing when every
/// bound file is stored.
fn format_unstored_attachments(unstored: &[canon_gate::UnstoredAttachment]) -> String {
    if unstored.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "\nevidence artifacts: {} bound file(s) not in the artifact store — not failing the gate; run `canon evidence vault` to store them before the working tree changes:\n",
        unstored.len()
    );
    for attachment in unstored {
        out.push_str(&format!("  unstored {}\n", attachment.line()));
    }
    out
}
/// `spec_coverage.require_review`'s waived gaps (issue #2): a subject
/// moved under `canon subject status --override-reason` keeps its gaps
/// visible here, named with the waiver, without failing the gate.
/// Prints nothing when there are none, so a repo without waivers sees
/// no new output.
fn format_review_advisories(advisories: &[canon_gate::ReviewAdvisory]) -> String {
    if advisories.is_empty() {
        return String::new();
    }
    let mut out = format!("\nreview waivers: {} advisory(ies) — not failing the gate:\n", advisories.len());
    for advisory in advisories {
        out.push_str(&format!("  waived {}\n", advisory.line()));
    }
    out
}

/// The experimental evidence-binding block, printed only when the policy
/// turns it on. In `warn` mode the gaps are advisories (listed here and
/// nowhere else); in `require` mode they are already violations above,
/// so only the distribution is printed.
fn format_binding_summary(summary: &canon_gate::BindingSummary) -> String {
    use canon_gate::{BindingMode, BindingStrength};
    let mode = match summary.mode {
        BindingMode::Off => "off",
        BindingMode::Warn => "warn",
        BindingMode::Require => "require",
    };
    let count = |s| summary.counts.get(&s).copied().unwrap_or(0);
    let mut out = format!(
        "\nexperimental evidence binding ({mode}, requires {}): report {}, artifact {}, attested-only {}\n",
        summary.required.as_str(),
        count(BindingStrength::Report),
        count(BindingStrength::Artifact),
        count(BindingStrength::Attested)
    );
    if summary.mode == BindingMode::Warn && !summary.gaps.is_empty() {
        out.push_str(&format!("  {} advisory(ies) — not failing the gate:\n", summary.gaps.len()));
        for gap in &summary.gaps {
            out.push_str(&format!("  warn {} — {}\n", gap.scenario_id.as_str(), gap.detail()));
        }
    }
    out
}

fn format_gate_report(report: &GateReport) -> String {
    if report.is_clean() {
        return "canon gate check: clean (0 violations)\n".to_string();
    }
    let mut out = format!("canon gate check: {} violation(s)\n", report.violations.len());
    for class_str in FAILURE_CLASSES {
        let class = FailureClass::from_str_exact(class_str).expect("FAILURE_CLASSES round-trips to FailureClass");
        let lines: Vec<String> = report.by_class(class).map(|v| v.line()).collect();
        if lines.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{class_str} ({}):\n", lines.len()));
        for line in lines {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// The plan source that owns `task_id`'s document: the winning
/// dialect's write-back, the document itself, and THAT source's root —
/// the typed-atoms file is resolved against the same root, never a
/// neighbouring source's.
///
/// Named fields, not the positional `(&dyn PlanWriteBack, PathBuf,
/// PathBuf)` triple [`run_task`] used to destructure: the two
/// `PathBuf`s are same-typed, so swapping them at a call site would
/// compile while silently resolving the typed-atoms file against the
/// document path.
pub(crate) struct LocatedTask {
    pub dialect: String,
    pub write_back: &'static dyn PlanWriteBack,
    pub document_path: PathBuf,
    pub source_root: PathBuf,
}

/// Resolve `task_id`'s plan document across the configured sources,
/// first-hit-wins (module doc). Extracted out of [`run_task`]'s body by
/// s42 (`close-the-open-loops`) so `canon evidence add`'s typed-contract
/// pre-flight ([`typed_evidence_contract_for_task`]) predicts the flip
/// against the SAME resolution the flip itself performs — a second copy
/// could pick a different source's typed-atoms file and pre-approve a
/// record this function's caller then refuses.
///
/// `Err` carries the operator-facing message ALREADY formed, including
/// the sources consulted, minus the `canon gate task: ` prefix: every
/// location failure is a usage failure (exit `2`) for the flip, so the
/// caller needs no further discrimination between them.
fn locate_task(repo: &Path, task_id: &TaskId) -> Result<LocatedTask, String> {
    let sources = crate::plans::load_plan_sources_for_gate(repo).map_err(|e| e.to_string())?;
    if sources.is_empty() {
        // Distinct from "consulted N sources, none held it": this repo
        // declared `plans.sources: []` (what `canon init` scaffolds),
        // so there is no plan corpus for ANY task id to live in.
        // Naming a synthesized openspec source here would blame a
        // dialect the operator never configured — the same refusal
        // `crate::dispatch::DispatchError::NoPlanSources` already makes
        // for `--task`.
        return Err(format!(
            "this repo configures no plan sources (canon.yaml has no `plans.sources` entries), so no plan document exists for {task_id} to live in — configure `plans:` with the dialect holding your plan corpus"
        ));
    }
    let mut consulted: Vec<String> = Vec::new();
    for src in &sources {
        consulted.push(format!("{} @ {}", src.dialect(), src.root().display()));
        let Some(entry) = find_plan_adapter(src.dialect()) else {
            return Err(format!("`{}` is not a registered plan dialect", src.dialect()));
        };
        // A dialect that registered no write-back capability at all
        // cannot own a flip — skip it for location (a later source may
        // still hold the task); it stays in `consulted` for the loud
        // not-found message.
        let Some(write_back) = entry.write_back else {
            continue;
        };
        if let Some(location) = write_back.locate_task(src.root(), task_id) {
            return Ok(LocatedTask { dialect: src.dialect().to_string(), write_back, document_path: location.document_path, source_root: src.root().to_path_buf() });
        }
    }
    Err(format!("no plan source locates {task_id} (consulted: {})", consulted.join("; ")))
}

/// `canon gate task <task_id> [--repo]` (task 3.2's CLI wiring, extended
/// by S10 part2 task 4.4, made dialect-agnostic by s35 `gate-plan-
/// dialect-seam`): resolves the task's plan document via the configured
/// plan sources' [`PlanWriteBack::locate_task`] (first hit wins, compat
/// default openspec@repo when `plans:` is absent — module doc), loads
/// the repo's `GateContext` for its evidence, runs the pure dialect-free
/// `canon_gate::gate_task` decision, and delegates the file mutation to
/// the winning dialect's [`PlanWriteBack::flip_task`]. The row-state
/// facts (already-`[x]`, no-such-row) come from `flip_task`, never from
/// the evidence decision — so an already-done row is a success no-op and
/// a missing row is a gate-red "no matching row" regardless of what the
/// evidence says (the pre-s35 precedence, preserved).
pub fn run_task(repo: &Path, task_id_str: &str) -> i32 {
    let repo = resolve_repo_root(repo);
    let task_id = match TaskId::parse(task_id_str) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    // Resolve + locate the task's plan document across the configured
    // sources, first-hit-wins ([`locate_task`], module doc). `located`
    // carries the winning dialect's write-back, its document path, and
    // that source's root (the typed-atoms file is resolved from the
    // SAME source).
    let LocatedTask { dialect, write_back, document_path, source_root } = match locate_task(&repo, &task_id) {
        Ok(located) => located,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    let document = match std::fs::read_to_string(&document_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("canon gate task: cannot read {}: {e}", document_path.display());
            return 2;
        }
    };

    let ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };
    let registry = SchemaRegistry::load();
    // The ONE `Utc::now()` call for this invocation (s21
    // `deterministic-gate-clock` D6) — mirrors `run_check`'s identical
    // dispatch-boundary discipline.
    let now = Utc::now();
    let gate_context = match GateContext::load(ctx, &registry, now) {
        Ok(gc) => gc,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    let raw_records = match GitTier::new(gate_context.ctx.ledger_root.clone()).read(&TierQuery::kind(RecordKind::EvidenceRecord)) {
        Ok(read) => read.records,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    // D4 (S10 part2): the winning dialect's typed-atoms file for this
    // change, if the dialect has one AND it carries an atom for
    // `task_id`, narrows BOTH the evidence slice AND its `EvidenceNote`
    // companions to exactly the raw records whose own `evidence.kind`/
    // `ref` companion matches the compiled task's declared kind/ref
    // (`typed_path_evidence`'s own doc). No atom at all: unchanged free
    // path, every non-`Divergent` record for `task_id` regardless of
    // kind supplies BOTH the evidence and the notes.
    let typed_atoms_path = write_back.typed_atoms_path(&source_root, &task_id.change_id());
    let (evidence, notes) = match typed_atom_for_task(typed_atoms_path.as_deref(), &task_id) {
        Ok(Some(atom)) => match typed_path_evidence(&repo, &atom, &raw_records, &task_id) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("canon gate task: {e}");
                return 1;
            }
        },
        Ok(None) => {
            let same_task_id = raw_records.iter().filter(|raw| raw.0.get("task_id").and_then(|v| v.as_str()) == Some(task_id.as_str()));
            let notes = match notes_of(same_task_id, &task_id) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("canon gate task: {e}");
                    return 1;
                }
            };
            (gate_context.evidence.clone(), notes)
        }
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    // Pure, dialect-free evidence decision (canon-gate).
    let decision = gate_task(&task_id, &evidence, &notes);
    let approved_note = match &decision {
        TaskFlipDecision::Approved { evidence_note } => Some(evidence_note.clone()),
        TaskFlipDecision::Blocked { .. } => None,
    };

    // Delegate the document mutation to the winning dialect. A Blocked
    // decision passes an empty note; `flip_task` still establishes
    // row-presence/state so a missing/already-done row is reported
    // correctly regardless of the evidence verdict — its mutated
    // document is DISCARDED in the Blocked branch below, never written.
    let flip = match write_back.flip_task(&document, &task_id, approved_note.as_deref().unwrap_or("")) {
        Ok(o) => o,
        Err(e @ WriteBackError::RowNotFound(_)) => {
            eprintln!("canon gate task: {e}");
            return 1;
        }
        // Gate-red, not usage (the variant's own doc): the offending
        // text came from an already-COMMITTED ledger record's approved
        // note, so it is evidence that cannot support a flip — the same
        // grade `fabricated-evidence` and `unevidenced-flip` get. s42
        // (`close-the-open-loops`) review: `canon evidence add` refuses a
        // line separator at authoring time, and this arm is why a record
        // authored some other way still cannot forge a second checked
        // row.
        Err(e @ WriteBackError::MultiLineEvidenceNote { .. }) => {
            eprintln!("canon gate task: {e}");
            return 1;
        }
        Err(e @ WriteBackError::Unsupported { .. }) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };

    if !flip.flipped {
        // Row already `[x]` — idempotent no-op, regardless of the
        // evidence decision (fail-open only for an ALREADY-satisfied
        // row, never a fresh flip).
        println!("canon gate task: {task_id} already done (idempotent no-op)");
        return 0;
    }

    match decision {
        TaskFlipDecision::Approved { .. } => {
            if let Err(e) = std::fs::write(&document_path, &flip.document) {
                eprintln!("canon gate task: failed to write {}: {e}", document_path.display());
                return 2;
            }
            println!("canon gate task: {task_id} flipped — {}", crate::write_mode::DIRECT);
            refresh_task_status(&repo, &dialect, &source_root);
            0
        }
        TaskFlipDecision::Blocked { violations } => {
            for v in &violations {
                eprintln!("{}", v.line());
            }
            1
        }
    }
}

/// Re-ingest the plan source that owns a just-flipped row (0.14 D5,
/// dogfood F14), so the record store's `Task` status agrees with the
/// checkbox and `canon query --kind task` matches the plan document. The
/// SAME `canon ingest plans` pass, narrowed to the one source: the new
/// `Task` version carries the document's fresh mtime, so it supersedes
/// the open one in every folded read. A repo that routes no tier for
/// tasks has nothing to disagree with, so an unwritten task is silent.
/// The flip itself already succeeded; a failure here is reported, never
/// turned into a failed flip.
fn refresh_task_status(repo: &Path, dialect: &str, source_root: &Path) {
    match crate::plans::run(repo, Some(dialect), Some(source_root)) {
        Ok(outcome) if outcome.non_clean_sources.is_empty() => {}
        Ok(_) => eprintln!("canon gate task: WARN the plan source re-ingest found malformed constructs; run `canon ingest plans` to see them"),
        Err(e) => eprintln!("canon gate task: WARN the task status record was not refreshed ({e}); run `canon ingest plans` so `canon query --kind task` matches the plan"),
    }
}

/// Build the [`EvidenceNote`] companions carried by `records` (S10 part2
/// fix, `ReviewS10Part2` finding): a note and the evidence it may pair
/// with inside `canon_gate::gate_task` MUST be derived from the exact
/// SAME raw-record set a caller already narrowed to — the free path's
/// every-record-sharing-`task_id` set, or [`typed_path_evidence`]'s own
/// kind/ref-narrowed set — so a record a caller's filter already
/// excluded (stale, wrong kind, whatever the filter was) can never
/// still slip its `evidence_note` companion into the notes `gate_task`
/// pairs against the narrowed evidence slice.
///
/// Returned oldest first by each record's own `at` (0.14 D5): `gate_task`
/// reads the LAST note as the task's latest summary, and a tier read is
/// in path order, not time order.
fn notes_of<'a>(records: impl IntoIterator<Item = &'a RawRecord>, task_id: &TaskId) -> Result<Vec<EvidenceNote>, String> {
    let mut notes = Vec::new();
    for raw in records {
        match evidence_note_of(&raw.0, task_id) {
            Some(Ok(note)) => {
                let at = raw.0.get("at").and_then(|v| v.as_str()).and_then(|s| s.parse::<chrono::DateTime<Utc>>().ok());
                notes.push((at, note));
            }
            Some(Err(e)) => {
                return Err(format!("{task_id}'s `evidence_note` companion is present but unparseable ({e}) — never silently treated as absent"));
            }
            None => {}
        }
    }
    notes.sort_by_key(|(at, _)| *at);
    Ok(notes.into_iter().map(|(_, note)| note).collect())
}

/// Look up `task_id` in the typed-atoms file the winning dialect
/// resolved (`PlanWriteBack::typed_atoms_path`, s35), if any. `path`
/// `None` = the dialect has no typed-vocabulary convention at all;
/// `Ok(None)` additionally covers "no such file" (this change has not
/// opted into the typed vocabulary) and "file exists but no atom carries
/// this `id`" — all three fall through to the untyped free path
/// identically (module doc). `Err` is reserved for a PRESENT file that
/// fails to PARSE — a real authoring mistake, reported as a usage/infra
/// failure (exit `2`), never silently treated as "no typed atom".
fn typed_atom_for_task(path: Option<&Path>, task_id: &TaskId) -> Result<Option<canon_vocab::AtomRecord>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let atoms = canon_vocab::atom::parse_atoms_file(&text).map_err(|e| format!("{} is not a valid typed-atoms file: {e}", path.display()))?;
    Ok(atoms.into_iter().find(|a| a.id == task_id.as_str()))
}

/// The `{kind, ref}` pair a task's TYPED atom declares its evidence must
/// carry — the gate's narrowing key on the typed path
/// ([`typed_path_evidence`]) and, since s42 (`close-the-open-loops`),
/// the contract `canon evidence add` checks `--kind`/`--ref` against
/// before it stages anything. Named fields, not the `(String, String)`
/// tuple this used to be threaded around as: both halves are strings,
/// so a swap would compile and silently narrow the evidence slice to
/// nothing.
pub(crate) struct TypedEvidenceContract {
    pub kind: String,
    pub evidence_ref: String,
}

/// Why [`typed_evidence_contract_for_task`] could not decide, split by
/// the exit code [`run_task`] itself grades the SAME condition with
/// (module doc's exit-code contract) — so `canon evidence add` reports
/// the identical verdict two commands earlier instead of inventing its
/// own grading for a condition the gate already classifies.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TypedContractError {
    /// A PRESENT typed-atoms file that fails to PARSE: an authoring
    /// mistake in the plan corpus, not in the record being staged —
    /// usage/infra, exit `2`, exactly as [`typed_atom_for_task`]'s own
    /// doc specifies.
    #[error("{0}")]
    Corpus(String),
    /// The atom parsed but fails vocabulary validation (e.g. an
    /// `evidence.kind` outside the policy-derived domain), or compiled
    /// with no `evidence` to gate against at all: gate-red `1`, the
    /// same grade [`run_task`] gives a `typed_path_evidence` failure.
    #[error("{0}")]
    Invalid(String),
}

impl TypedContractError {
    /// `true` when this is a fixable INVOCATION/corpus problem (exit
    /// `2`) rather than a gate-red refusal (`1`), mirroring
    /// `crate::dispatch::DispatchError::is_usage`'s established shape.
    pub(crate) fn is_usage(&self) -> bool {
        matches!(self, Self::Corpus(_))
    }
}

/// Compile `atom` against a FRESH vocabulary snapshot (module doc —
/// never the authoring-time snapshot) and read the `{kind, ref}` it
/// declares. Split out of [`typed_path_evidence`] by s42
/// (`close-the-open-loops`) so `canon evidence add` reads the contract
/// through the same compile, never a second raw read of the atom's
/// `attrs`: the compile is what VALIDATES the declared kind against the
/// policy-derived domain, so a reader that skipped it would happily
/// pre-approve a record against an atom the gate itself rejects.
fn typed_evidence_contract(repo: &Path, atom: &canon_vocab::AtomRecord) -> Result<TypedEvidenceContract, String> {
    let (snapshot, _resolve_diags) = canon_vocab::resolve_snapshot(repo, None);
    // A record canon itself originates on the fly, purely to extract the
    // atom's own validated `evidence.kind`/`ref` — never an agent-authored
    // record, so `Actor::new_unattributed` (never a `RoleId` needing an
    // infallible-by-construction parse). This `Utc::now()` stamps only the
    // throwaway envelope's `at`; it is NOT the gate clock (that is the single
    // dispatch-boundary `now` threaded on `GateContext`) and never enters a
    // verdict — `compile_task` reads the atom's evidence, not this `at`.
    let envelope = Envelope::current(RecordKind::Task, Utc::now(), Actor::new_unattributed("canon-gate"));
    let task = canon_vocab::compile_task(atom, &snapshot, envelope).map_err(|diags| {
        let rendered = diags.iter().map(|d| format!("{}: {} ({})", d.code, d.message, d.subject)).collect::<Vec<_>>().join("; ");
        format!("typed task atom `{}` failed vocabulary validation: {rendered}", atom.id)
    })?;

    let Some((kind, evidence_ref)) = task_evidence_kind_ref(&task) else {
        return Err(format!("typed task atom `{}` compiled with no `evidence.kind`/`ref` to gate against", atom.id));
    };
    Ok(TypedEvidenceContract { kind, evidence_ref })
}

/// Whatever typed evidence contract binds `task_id`, resolved exactly as
/// [`run_task`] will resolve it at flip time — s42
/// (`close-the-open-loops`) task 4.2's half of the loop: a record that
/// stages cleanly but that the gate then refuses is a worse outcome than
/// no authoring command at all, and a `--kind`/`--ref` disagreeing with
/// the task's atom is the ONE way an authored record can be narrowed
/// out of [`typed_path_evidence`]'s matched set and reported back as a
/// bare `unevidenced-flip` with no hint of why.
///
/// `Ok(None)` is the free path — the gate accepts any non-`Divergent`
/// record for `task_id` regardless of kind, so there is no contract to
/// enforce. It covers all four fall-through cases [`run_task`] itself
/// treats identically (the winning dialect has no typed-vocabulary
/// convention, the change never opted in, the file carries no atom for
/// this id) plus one more: no source LOCATES the task under the gate's
/// resolution. That last collapses into `None` rather than an error
/// because `canon evidence add` reaches here only after
/// `crate::dispatch::validate_task_binding` has already proved the
/// configured corpus both parses and carries the task — what remains is
/// a dialect whose write-back cannot locate a document, and such a
/// dialect has no typed-atoms file to declare a contract in either.
pub(crate) fn typed_evidence_contract_for_task(repo: &Path, task_id: &TaskId) -> Result<Option<TypedEvidenceContract>, TypedContractError> {
    let Ok(located) = locate_task(repo, task_id) else {
        return Ok(None);
    };
    let atoms_path = located.write_back.typed_atoms_path(&located.source_root, &task_id.change_id());
    let Some(atom) = typed_atom_for_task(atoms_path.as_deref(), task_id).map_err(TypedContractError::Corpus)? else {
        return Ok(None);
    };
    typed_evidence_contract(repo, &atom).map(Some).map_err(TypedContractError::Invalid)
}

/// D4's typed-evidence path proper: read the atom's declared
/// `{kind, ref}` contract ([`typed_evidence_contract`] — an atom that
/// fails vocabulary validation, e.g. an `evidence.kind` outside the
/// policy-derived domain, yields `Err` there, never a contract), then
/// narrow `raw_records` to exactly the ones whose own
/// `evidence: {kind, ref}` companion ([`raw_evidence_kind_ref`]) matches
/// it — and build BOTH the returned `EvidenceRecord`s AND their
/// `EvidenceNote` companions from that SAME matched set ([`notes_of`]),
/// never from every raw record sharing `task_id` (S10 part2 fix,
/// `ReviewS10Part2` finding: the old shape let a stale/wrong-kind
/// record's note pair with a narrowed evidence slice it never matched
/// into, or block a valid typed flip).
fn typed_path_evidence(
    repo: &Path,
    atom: &canon_vocab::AtomRecord,
    raw_records: &[RawRecord],
    task_id: &TaskId,
) -> Result<(Vec<canon_model::EvidenceRecord>, Vec<EvidenceNote>), String> {
    let contract = typed_evidence_contract(repo, atom)?;
    // Built ONCE, outside the filter: the pair is the same for every
    // candidate, so cloning it per raw record would allocate twice per
    // ledger row to answer one equality.
    let expected = (contract.kind, contract.evidence_ref);

    let matching: Vec<RawRecord> = raw_records
        .iter()
        .filter(|raw| raw.0.get("task_id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
        .filter(|raw| raw_evidence_kind_ref(&raw.0).as_ref() == Some(&expected))
        .cloned()
        .collect();

    let (records, _violations) = validate_evidence_batch(&matching);
    let notes = notes_of(&matching, task_id)?;
    Ok((records, notes))
}

/// Extract `{kind, ref}` off a `compile_task`-produced [`canon_model::Task`]'s
/// `evidence_note` — `compile_task` canonically JSON-encodes the atom's
/// FULL, checker-validated `attrs` map there (`canon_vocab::compile`
/// module doc), so this is a plain JSON navigation, never a second
/// vocabulary parse.
fn task_evidence_kind_ref(task: &canon_model::Task) -> Option<(String, String)> {
    let attrs: serde_json::Value = serde_json::from_str(task.evidence_note.as_deref()?).ok()?;
    let evidence = attrs.get("evidence")?;
    Some((evidence.get("kind")?.as_str()?.to_string(), evidence.get("ref")?.as_str()?.to_string()))
}

/// D4's companion convention: an `EvidenceRecord` authored for the typed
/// path carries an extra top-level `evidence: {kind, ref}` key in its raw
/// ledger JSON, mirroring the atom's own `evidence` attr shape 1:1 —
/// silently dropped by [`canon_model::EvidenceRecord`]'s own strict
/// `Deserialize` (no `deny_unknown_fields`), exactly the established
/// "re-read the raw ledger JSON for a companion key" pattern `canon_gate::
/// markers::evidence_note_of`/`trust_ladder`/`evidence_sha` already use
/// (module doc).
fn raw_evidence_kind_ref(raw: &serde_json::Value) -> Option<(String, String)> {
    let evidence = raw.get("evidence")?;
    Some((evidence.get("kind")?.as_str()?.to_string(), evidence.get("ref")?.as_str()?.to_string()))
}

/// The staging [`GitTier`] root `canon evidence add` writes to and
/// [`run_promote`] drains: `<ledger_root>/_staging`, a SIBLING of the
/// committed `kind=<k>/` tree — a committed read walks
/// `<ledger_root>/kind=evidence_record/`, so a staged record is
/// invisible to `canon gate task`/`canon gate check` until promotion
/// (the same sibling-directory arrangement `canon_gate::
/// divergence_staging_dir`'s `_staging_divergence` already uses).
///
/// One function since s42 (`close-the-open-loops`) gave the directory
/// its first writer: the author and the promoter disagreeing by one
/// path literal would strand every staged record silently, with both
/// commands reporting success.
pub(crate) fn evidence_staging_dir(ledger_root: &Path) -> PathBuf {
    ledger_root.join("_staging")
}

/// `canon gate promote [--repo] [--dry-run]` (task 2.2/2.3's CLI wiring;
/// s43 extends it to every kind in `canon_gate::STAGED_KINDS`):
/// `_staging/` → committed, with a monotonic per-(role, surface)
/// `run_seq` for the one kind that needs one.
///
/// Safe to retry after an interrupted run: `canon_gate::promote` is
/// idempotent per staged candidate (its own recovery section), so a
/// re-run drains a candidate whose record already landed instead of
/// committing it twice, and still exits `0`.
pub fn run_promote(repo: &Path, dry_run: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("canon gate promote: {e}");
            return 2;
        }
    };
    let staging = GitTier::new(evidence_staging_dir(&ctx.ledger_root));
    let committed = GitTier::new(ctx.ledger_root.clone());
    match gate_promote(&staging, &committed, dry_run) {
        Ok(report) => {
            print!("{}", format_promote_report(&report, dry_run));
            if report.is_clean() {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("canon gate promote: {e}");
            2
        }
    }
}

/// One [`PromoteReport`] as an operator reads it: a line per candidate,
/// then a PER-KIND tally.
///
/// The tally is not decoration. `canon gate promote` drains several
/// kinds in one call, and a batch that silently drained none of the
/// kind you cared about is the failure mode this command has: a bare
/// "promoted 7" cannot be checked against a ledger, where "promoted
/// evidence_record=1, finding=6" can. Every registered kind appears
/// with its count INCLUDING zero, so the output also answers "which
/// kinds can be promoted at all" — the question an author whose
/// records went nowhere is actually asking.
fn format_promote_report(report: &PromoteReport, dry_run: bool) -> String {
    let verb = if dry_run { "would promote" } else { "promoted" };
    let drain_verb = if dry_run { "would drain" } else { "drained" };
    let mut out = String::new();
    for p in &report.promoted {
        out.push_str(&format!("{verb} {} -> {}\n", p.label(), p.target.display()));
    }
    // Reported distinctly from `promoted`, never folded into it: the
    // record already existed, this call only finished the interrupted
    // run's staging cleanup, and an operator comparing promote output
    // against the ledger has to be able to tell which happened.
    for p in &report.recovered {
        out.push_str(&format!("{drain_verb} {} -> {} (already committed by an interrupted promote; not re-committed)\n", p.label(), p.target.display()));
    }
    for r in &report.refused {
        out.push_str(&format!("refused: {}\n", r.violation.line()));
    }

    if report.promoted.is_empty() && report.recovered.is_empty() && report.refused.is_empty() {
        // Named explicitly rather than left as silence: an author whose
        // `add` command printed "run `canon gate promote`" and then saw
        // a clean exit with no output has no way to tell success from a
        // record that went nowhere.
        out.push_str(&format!("canon gate promote: nothing staged — no records under `_staging/` for any promotable kind ({})\n", promotable_kinds()));
        return out;
    }

    out.push_str(&format!("canon gate promote: {verb} {}", kind_tally(&report.promoted)));
    if !report.recovered.is_empty() {
        out.push_str(&format!("; {drain_verb} {}", kind_tally(&report.recovered)));
    }
    if !report.refused.is_empty() {
        out.push_str(&format!("; refused {}", report.refused.len()));
    }
    out.push('\n');
    out
}

/// `kind=N` for every kind in `canon_gate::STAGED_KINDS`, in that
/// order — zeros included (see [`format_promote_report`]), and driven
/// off the registry so a newly promotable kind appears here without a
/// second list to remember to update.
fn kind_tally(landed: &[Promoted]) -> String {
    STAGED_KINDS
        .iter()
        .map(|staged| format!("{}={}", staged.kind.as_str(), landed.iter().filter(|p| p.kind == staged.kind).count()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn promotable_kinds() -> String {
    STAGED_KINDS.iter().map(|staged| staged.kind.as_str()).collect::<Vec<_>>().join(", ")
}

/// `canon gate install-hooks [--repo] [--event] [--matcher] [--command]
/// [--timeout]` (task 4.1's CLI wiring, design decision 8): idempotent,
/// diff-only merge of one hook-seam entry into BOTH
/// `<repo>/.claude/settings.json` and `<repo>/.codex/hooks.json` via
/// `canon_gate::install_hooks` — pure `serde_json::Value` merge logic,
/// this function only owns the file I/O around it. When neither file
/// carries ANY existing `canon gate`-invoking command (checked BEFORE
/// this call's own edit), also emits the generic
/// `canon-gate-pre-commit.sh` (`PRE_COMMIT_SCRIPT`, task 4.2) into
/// `<repo>/.canon/scripts/`, matching spec.md's "a non-donor repo gets a generic
/// pre-commit script" scenario.
#[allow(clippy::too_many_arguments)]
pub fn run_install_hooks(repo: &Path, event: &str, matcher: Option<&str>, command: &str, timeout: u32) -> i32 {
    let repo = resolve_repo_root(repo);
    let entry = HookEntry::new(event, matcher.map(str::to_string), command, timeout);

    let claude_path = repo.join(".claude").join("settings.json");
    let codex_path = repo.join(".codex").join("hooks.json");

    let already_has_canon_gate_command = any_canon_gate_command(&read_json_or_default(&claude_path)) || any_canon_gate_command(&read_json_or_default(&codex_path));

    let claude_outcome = match install_into(&claude_path, &entry) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("canon gate install-hooks: {e}");
            return 2;
        }
    };
    let codex_outcome = match install_into(&codex_path, &entry) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("canon gate install-hooks: {e}");
            return 2;
        }
    };

    if !already_has_canon_gate_command {
        let script_path = repo.join(paths::PRE_COMMIT_SCRIPT);
        if !script_path.exists() {
            if let Err(e) = write_pre_commit_script(&script_path) {
                eprintln!("canon gate install-hooks: failed to write {}: {e}", script_path.display());
                return 2;
            }
            println!("canon gate install-hooks: wrote {}", script_path.display());
        }
    }

    match (claude_outcome, codex_outcome) {
        (InstallOutcome::Unchanged, InstallOutcome::Unchanged) => println!("canon gate install-hooks: no diff, nothing written"),
        _ => println!("canon gate install-hooks: installed"),
    }
    0
}

fn read_json_or_default(path: &Path) -> serde_json::Value {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| serde_json::json!({}))
}

fn any_canon_gate_command(settings: &serde_json::Value) -> bool {
    let Some(events) = settings.get("hooks").and_then(serde_json::Value::as_object) else {
        return false;
    };
    events.values().filter_map(serde_json::Value::as_array).flatten().filter_map(|group| group.get("hooks")).filter_map(serde_json::Value::as_array).flatten().any(|hook| {
        hook.get("command").and_then(serde_json::Value::as_str).is_some_and(|c| c.starts_with("canon gate"))
    })
}

fn install_into(path: &Path, entry: &HookEntry) -> std::io::Result<InstallOutcome> {
    let mut settings = read_json_or_default(path);
    let outcome = install_hooks(&mut settings, entry);
    if matches!(outcome, InstallOutcome::Installed) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&settings).expect("hook settings always serialize")))?;
    }
    Ok(outcome)
}

fn write_pre_commit_script(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, PRE_COMMIT_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// `canon gate selftest` (task 5.2's CLI wiring): runs the shipped
/// fixture corpus (`canon_gate::selftest::run`), never touches a real
/// repo — no `--repo` flag.
pub fn run_selftest() -> i32 {
    let report = selftest::run();
    print!("{}", report.format_human());
    report.exit_code()
}
