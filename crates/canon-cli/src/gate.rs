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
//! repo ROOT's `<repo>/canon.yaml`/`<repo>/canon/policy.yaml`, never a
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

use std::path::{Path, PathBuf};

use canon_gate::{
    evidence_note_of, gate_task, install_hooks, promote as gate_promote, selftest, EvidenceNote, FailureClass, GateContext, GateCtx, GateReport, HookEntry, InstallOutcome,
    PromoteReport, TaskFlipDecision, FAILURE_CLASSES, PRE_COMMIT_SCRIPT,
};
use canon_ingest::{find_plan_adapter, PlanWriteBack, WriteBackError};
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
    let ctx = GateCtx::from_repo(&repo);
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
    report.exit_code()
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
    // sources, first-hit-wins (module doc). `located` carries the
    // winning dialect's write-back, its document path, and that source's
    // root (the typed-atoms file is resolved from the SAME source).
    let sources = match crate::plans::load_plan_sources_for_gate(&repo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("canon gate task: {e}");
            return 2;
        }
    };
    let mut consulted: Vec<String> = Vec::new();
    let mut located: Option<(&'static dyn PlanWriteBack, PathBuf, PathBuf)> = None;
    for src in &sources {
        consulted.push(format!("{} @ {}", src.dialect(), src.root().display()));
        let Some(entry) = find_plan_adapter(src.dialect()) else {
            eprintln!("canon gate task: `{}` is not a registered plan dialect", src.dialect());
            return 2;
        };
        // A dialect that registered no write-back capability at all
        // cannot own a flip — skip it for location (a later source may
        // still hold the task); it stays in `consulted` for the loud
        // not-found message.
        let Some(write_back) = entry.write_back else {
            continue;
        };
        if let Some(location) = write_back.locate_task(src.root(), &task_id) {
            located = Some((write_back, location.document_path, src.root().to_path_buf()));
            break;
        }
    }
    let Some((write_back, document_path, source_root)) = located else {
        eprintln!("canon gate task: no plan source locates {task_id} (consulted: {})", consulted.join("; "));
        return 2;
    };

    let document = match std::fs::read_to_string(&document_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("canon gate task: cannot read {}: {e}", document_path.display());
            return 2;
        }
    };

    let ctx = GateCtx::from_repo(&repo);
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
            println!("canon gate task: {task_id} flipped");
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

/// Build the [`EvidenceNote`] companions carried by `records` (S10 part2
/// fix, `ReviewS10Part2` finding): a note and the evidence it may pair
/// with inside `canon_gate::gate_task` MUST be derived from the exact
/// SAME raw-record set a caller already narrowed to — the free path's
/// every-record-sharing-`task_id` set, or [`typed_path_evidence`]'s own
/// kind/ref-narrowed set — so a record a caller's filter already
/// excluded (stale, wrong kind, whatever the filter was) can never
/// still slip its `evidence_note` companion into the notes `gate_task`
/// pairs against the narrowed evidence slice.
fn notes_of<'a>(records: impl IntoIterator<Item = &'a RawRecord>, task_id: &TaskId) -> Result<Vec<EvidenceNote>, String> {
    let mut notes = Vec::new();
    for raw in records {
        match evidence_note_of(&raw.0, task_id) {
            Some(Ok(note)) => notes.push(note),
            Some(Err(e)) => {
                return Err(format!("{task_id}'s `evidence_note` companion is present but unparseable ({e}) — never silently treated as absent"));
            }
            None => {}
        }
    }
    Ok(notes)
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

/// D4's typed-evidence path proper: resolve THIS repo's vocabulary
/// snapshot fresh (module doc — never the authoring-time snapshot),
/// compile `atom` against it (validates + produces the S1 `Task` the
/// design's own "given a task compiled from a typed atom" language
/// names — an atom that fails vocabulary validation, e.g. an
/// `evidence.kind` outside the policy-derived domain, yields `Err` here,
/// never a `Task`), then narrow `raw_records` to exactly the ones whose
/// own `evidence: {kind, ref}` companion ([`raw_evidence_kind_ref`])
/// matches the compiled task's declared kind/ref — and build BOTH the
/// returned `EvidenceRecord`s AND their `EvidenceNote` companions from
/// that SAME matched set ([`notes_of`]), never from every raw record
/// sharing `task_id` (S10 part2 fix, `ReviewS10Part2` finding: the old
/// shape let a stale/wrong-kind record's note pair with a narrowed
/// evidence slice it never matched into, or block a valid typed flip).
fn typed_path_evidence(
    repo: &Path,
    atom: &canon_vocab::AtomRecord,
    raw_records: &[RawRecord],
    task_id: &TaskId,
) -> Result<(Vec<canon_model::EvidenceRecord>, Vec<EvidenceNote>), String> {
    let (snapshot, _resolve_diags) = canon_vocab::resolve_snapshot(repo, None);
    // A record canon itself originates on the fly, purely to extract the
    // atom's own validated `evidence.kind`/`ref` — never an agent-authored
    // record, so `Actor::new_unattributed` (never a `RoleId` needing an
    // infallible-by-construction parse). This `Utc::now()` stamps only the
    // throwaway envelope's `at`; it is NOT the gate clock (that is the single
    // dispatch-boundary `now` threaded on `GateContext`) and never enters a
    // verdict — `compile_task` reads the atom's evidence, not this `at`.
    let envelope = Envelope::new(1, RecordKind::Task, Utc::now(), Actor::new_unattributed("canon-gate"));
    let task = canon_vocab::compile_task(atom, &snapshot, envelope).map_err(|diags| {
        let rendered = diags.iter().map(|d| format!("{}: {} ({})", d.code, d.message, d.subject)).collect::<Vec<_>>().join("; ");
        format!("typed task atom `{}` failed vocabulary validation: {rendered}", atom.id)
    })?;

    let Some((kind, evidence_ref)) = task_evidence_kind_ref(&task) else {
        return Err(format!("typed task atom `{}` compiled with no `evidence.kind`/`ref` to gate against", atom.id));
    };

    let matching: Vec<RawRecord> = raw_records
        .iter()
        .filter(|raw| raw.0.get("task_id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
        .filter(|raw| raw_evidence_kind_ref(&raw.0).as_ref() == Some(&(kind.clone(), evidence_ref.clone())))
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

/// `canon gate promote [--repo] [--dry-run]` (task 2.2/2.3's CLI wiring):
/// `_staging/` → committed, monotonic per-(role, surface) `run_seq`.
pub fn run_promote(repo: &Path, dry_run: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let ctx = GateCtx::from_repo(&repo);
    let staging = GitTier::new(ctx.ledger_root.join("_staging"));
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

fn format_promote_report(report: &PromoteReport, dry_run: bool) -> String {
    let verb = if dry_run { "would promote" } else { "promoted" };
    let mut out = String::new();
    for p in &report.promoted {
        out.push_str(&format!("{verb} {}/{} run_seq={} -> {}\n", p.role.as_str(), p.surface, p.run_seq, p.target.display()));
    }
    for r in &report.refused {
        out.push_str(&format!("refused: {}\n", r.violation.line()));
    }
    if report.promoted.is_empty() && report.refused.is_empty() {
        out.push_str("canon gate promote: nothing staged\n");
    }
    out
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
/// `<repo>/scripts/`, matching spec.md's "a non-donor repo gets a generic
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
        let script_path = repo.join("scripts").join("canon-gate-pre-commit.sh");
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
