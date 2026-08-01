//! `canon finding add` (s43 `findings-are-records`, task group 2): the
//! AUTHORING surface for `RecordKind::Finding`.
//!
//! # Why this command exists
//! v0.4.0 shipped after a long run of code-review rounds. Canon
//! recorded none of their findings — `canon_ingest::artifact_adapter`'s
//! `ArtifactEventKind::CodeReviewFinding` recognised them on the way to
//! a verdict and then had nowhere to put them. So the release summary's
//! issue counts were typed from memory into a git tag, and they were
//! wrong. Nothing caught it, because canon gates `EvidenceRecord`s and
//! checkboxes, and a narrative claim in a tag is ungated prose.
//!
//! No number from that tag is repeated here, and none is asserted in
//! its place. The counts live in `mart_review_rounds`, read off
//! `.canon/REPORT.md`'s "Review rounds" panel or the dashboard's — a
//! command built to stop hand-typed release metrics has no business
//! shipping one in its own help, and s43 round 2 (finding 4) found this
//! doc and the `--help` text below both doing exactly that, with a
//! number that was itself wrong.
//!
//! This command is one half of the fix. It does not lint prose. It
//! makes the number DERIVABLE, so that the next release note is written
//! by reading a generated table instead of by remembering.
//!
//! # What a record from here IS: a RECORDED OBSERVATION
//! Not proof of anything. Canon does not verify that the defect
//! existed, that it was real, that it was fixed, or that
//! `--introduced-by` names the commit that actually caused it. State it
//! the blunt way, in the register `canon evidence add` set for
//! attestations (`crate::evidence`'s module doc), because this command
//! exists precisely because an unsupported claim shipped:
//!
//! - **`--summary` is whatever the author typed.** Canon does not read
//!   the diff, the review transcript, or the code. A finding saying
//!   "teardown races HMR" is recorded identically whether that race is
//!   real, misdiagnosed, or invented.
//! - **`--disposition fixed --resolution-sha <sha>` is not evidence the
//!   defect was fixed.** Canon never resolves the sha, never reads that
//!   commit, and never checks that it touches anything the finding
//!   mentions. It records that someone SAID this commit closed it.
//! - **`--introduced-by` is not causation.** It is the author's sourced
//!   claim about which commit introduced the defect. Canon cannot check
//!   it, and deliberately never guesses it (below).
//! - **The author and the beneficiary are the same party**, exactly as
//!   in `canon evidence add`: an agent that can run this command can
//!   record its own review round, with its own severities.
//!
//! What the record IS good for, and it is worth having:
//! **attribution** — a permanent, append-only row naming who raised
//! what, in which round, against which change, with which disposition —
//! and a **derived count** that stops being typed from memory. Both are
//! a strict improvement on a review round's findings evaporating with
//! its transcript. Neither is verification.
//!
//! # `--introduced-by` must be SOURCED, and unset is the CORRECT answer
//! This is the decision the CLI is where a user makes, so it is where
//! the guidance belongs. Set `--introduced-by` only when the
//! introducing commit was actually established — someone read the diff,
//! or the fixing commit's own message names it. Do NOT set it from
//! timing, commit adjacency, `git blame` output, or "it was probably
//! the last thing that touched this file". Leaving it unset is not an
//! incomplete record; it is the accurate one, and canon-model
//! deliberately offers no "guess it" constructor.
//!
//! The consequence, which every consumer must repeat: `None` means
//! UNSOURCED, never "no cause". A guessed value would inflate exactly
//! the number this change exists to make trustworthy — which would be
//! the same failure as the one in the tag, relocated from prose into a
//! record where it would look authoritative.
//!
//! What a derived fix-of-fix count MEANS is one sentence, stated here
//! exactly as `canon_report::render::FIX_OF_FIX_MEANING` states it and
//! as every other surface repeats it — verbatim, never paraphrased,
//! because paraphrase is how the previous wording went wrong on seven
//! surfaces at once: this doc asserted a one-directional bound, and it
//! was wrong in the direction it did not name. The retired word is not
//! repeated here; `FIX_OF_FIX_MEANING`'s own doc records the history.
//!
//! `fix_of_fix` bounds NOTHING — not from below, not from above: it
//! UNDER-counts, because an unsourced finding is never counted and a
//! fix in one change that breaks something first found while reviewing
//! a DIFFERENT change is not counted at all; it OVER-counts, because a
//! `resolution_sha` commit may carry work BEYOND the fix and every
//! finding recording that commit is counted regardless; and for any
//! individual match the data cannot say whether the fix or the other
//! work in that commit introduced the defect.
//!
//! # Fix-of-fix is DERIVED here too — this command cannot record it
//! There is no `--fix-of-fix` flag and there will not be one. A finding
//! is a fix-of-fix when its `introduced_by` equals some EARLIER
//! finding's `resolution_sha` (earlier = lower `(round, seq)` within the
//! same `change_id`); the join computes it, nobody labels it. A flag
//! here would merely relocate the hand-typing from the release notes
//! into the record.
//!
//! # Staged, then promoted — but NOT for the reason evidence is
//! [`run_add`] writes to `crate::gate::evidence_staging_dir`, which
//! `canon gate promote` drains (`canon_gate::STAGED_KINDS` registers
//! `Finding` under `canon_gate::StagedAssignment::Nothing`). The loop:
//!
//! ```text
//! canon finding add --change-id <c> --round <n> --seq <n> ...
//! canon gate promote
//! ```
//!
//! `canon evidence add` stages because `EvidenceRecord.run_seq` does not
//! exist until promotion assigns it. That reason does NOT apply here: a
//! `Finding`'s natural key is `{change_id}__{round}__{seq}`, complete
//! the moment it is authored, and promotion commits the staged body
//! unchanged. What staging buys a finding instead is RE-VALIDATION
//! (`staging.read()` runs `canon_store::partition::validate_body`, whose
//! `Finding` arm is `Finding::from_body`) and batch atomicity for
//! an author staging a whole round before committing any of it. Stated
//! because the next reader will otherwise assume it mirrors evidence's
//! reason, and it does not.
//!
//! # `--seq` is REQUIRED, and a collision is refused HERE and enforced at PROMOTE
//! Auto-assigning `seq` from the highest existing one in
//! `(change_id, round)` would be friendlier and is the wrong trade.
//! Two concurrent adds (the normal case in this repo, which reviews in
//! parallel) both read the same maximum and both pick `N+1`. Their
//! bodies differ, so they resolve to two DIFFERENT files under the SAME
//! natural key, and the corpus then carries two distinct findings each
//! claiming to be round `r` finding `N+1` — one of which every reader
//! that folds by natural key silently drops. A race that quietly loses
//! a reviewer's finding is not a friendliness win.
//!
//! Required is also the more USEFUL option: a review artifact already
//! numbers its findings, so `--seq` is transcription rather than
//! invention, and an explicit seq means a ledger row can be matched
//! back against the artifact it came from. Under auto-assignment `seq`
//! would only record insertion order and carry no meaning at all.
//!
//! [`run_add`] also refuses a `(change_id, round, seq)` that any
//! readable staged or committed record already occupies, naming the
//! occupant — and that refusal is a DIAGNOSTIC, not the defence. s43
//! round 1 caught the earlier claim that it was: the occupancy scan
//! ([`occupied_by`]) and the `staging.write` below are two separate
//! steps with no lock between them, so two concurrent adds both observe
//! a free key, write two different bodies to two different paths (the
//! content digest is in the filename, so `GitTier::write`'s
//! duplicate-PATH check does not fire), and both succeed. No amount of
//! rescanning here closes that; a check that runs before a write it
//! does not hold a lock over can only ever narrow the window.
//!
//! Uniqueness is therefore ENFORCED where records become durable:
//! `canon_gate::promote`'s `NaturalKeyRule::Unique` reads both tiers
//! and refuses a second, DIFFERENT body at an occupied key, leaving it
//! staged and named for a human rather than committing it. What THIS
//! check buys is the fast, well-worded refusal for the ordinary
//! sequential case — the one an author hits by mis-transcribing a seq,
//! which is far more common than the race — at the moment the author
//! can still fix it, instead of two commands later.
//!
//! # Line breaks are refused, not escaped
//! `--summary`, `--reviewer`, `--file-ref` and `--actor-id` are the
//! free-text fields of this record — the ones with no grammar
//! constraining them — and they are what a rendered finding row is
//! built from. A row is ONE line, so a line separator in any of them
//! does not produce a longer row; it produces a SECOND row, which no
//! record backs. s42's review found this exact vector against
//! `canon evidence add --summary`, where `$'ok\n- [x] 9.9 Forged task'`
//! appended a fully checked task row.
//!
//! Refused at the authoring boundary rather than escaped at each
//! renderer: there is no escaping that makes a two-line finding summary
//! meaningful in a one-line row, silently rewriting a reviewer's words
//! is worse than refusing them, and a refusal here holds for every
//! consumer instead of being re-derived correctly by each one.
//!
//! The character set is IMPORTED —
//! `canon_ingest::task_rows::first_row_line_break` over
//! `ROW_LINE_BREAKS`, the identical predicate `canon evidence add` and
//! `canon_ingest::reject_multi_line_note` use. Never restated here: two
//! copies of a "which characters break a row" set are two copies that
//! can disagree, and the one that disagrees is the one that lets a
//! forged row through.
//!
//! # A named sha must EXIST — checked, best-effort
//! `--reviewed-sha`, `--resolution-sha` and `--introduced-by` are
//! refused when this repository does not hold the commit they name.
//! [`parse_sha`] proves 40 hex characters and nothing else, and s43's
//! own round 2 proved that is not enough: nine findings were authored
//! with a short sha zero-padded to 40, all nine were accepted, and all
//! nine cited a commit that has never existed — a fabricated provenance
//! recorded, permanently, by the command built to stop fabricated
//! release facts.
//!
//! EXISTENCE only, and deliberately nothing else. `git cat-file -e
//! <sha>^{commit}` is the entire check. Canon still never reads the
//! commit's message, its diff, or its files (the observation section
//! above), because existence is a different question from content: a
//! resolvable `--introduced-by` is still only the author's sourced
//! claim, and this check does not make it more than that. It only
//! removes the case where the claim points at nothing at all.
//!
//! BEST-EFFORT, because a false refusal is worse than a missed check:
//! - `git` not on PATH, or `repo` not a work tree → SKIPPED, not
//!   failed. Authoring into a fixture directory or a corpus kept
//!   outside a checkout is legitimate, and refusing it would break the
//!   command where nothing is verifiable either way.
//! - a SHALLOW clone → skipped wholesale. Its history is truncated by
//!   construction, so "this clone does not hold that commit" says
//!   nothing about whether the commit exists.
//! - present as an object but UNREACHABLE from any ref (a not-yet-
//!   merged branch, a partial clone, a dangling commit) → ACCEPTED.
//!   This is the case worth stating explicitly, because the two answers
//!   are genuinely different and the choice is deliberate: the question
//!   asked is "does this object exist", which is exactly what
//!   `cat-file -e` answers. Requiring REACHABILITY would refuse a
//!   finding authored against the review branch it was raised on, which
//!   is the normal mid-review case.
//!
//! `GIT_NO_LAZY_FETCH=1` is set on every invocation, so a partial clone
//! answers from what it already holds instead of an authoring command
//! silently reaching the network.
//!
//! # Coherence is enforced by the TYPE; this command adds the diagnostic
//! `FindingDisposition::Fixed` is unreachable without a closing sha on
//! every construction path canon-model has — `Finding::fixed_by` is the
//! only constructor that sets it, `rejected`/`deferred` clear
//! `resolution_sha`, the pair is PRIVATE so no clone-and-mutate reaches
//! it, and `Finding`'s hand-written `Deserialize` re-checks it. So an
//! incoherent record is unconstructible through the API [`run_add`]
//! builds with, and there is deliberately NO second coherence check
//! here that could drift from `Finding::check_coherence`. What this
//! command adds is the LOUD, early diagnostic: `--disposition fixed`
//! with no `--resolution-sha`, or a `--resolution-sha` alongside any
//! other disposition, is refused by name before anything is staged,
//! rather than surfacing two commands later as a `malformed` record.
//!
//! # Exit-code contract
//! `0` staged; `2` usage-or-infra (a malformed flag combination, a line
//! separator in a free-text field, an empty required field, a sha this
//! repository does not hold, an occupied natural key, a tier write
//! failure). A malformed SHA never reaches [`run_add`] at all —
//! [`parse_sha`] is a clap `value_parser`, so clap refuses it by flag
//! name and grammar and exits `2` before this module runs.
//!
//! There is deliberately no `1`. `crate::evidence` grades `1` for "the
//! authored record would be gate-red", and no gate consumes findings:
//! grading a code nothing can produce would be a claim about a check
//! that does not exist.

use std::path::Path;
use std::process::Command;

use canon_gate::GateCtx;
use canon_ingest::task_rows::first_row_line_break;
use canon_model::{Actor, ChangeId, Envelope, Finding, FindingDisposition, FindingSeverity, RawRecord, RecordKind, RoleId, Sha};
use canon_store::git_tier::GitTier;
use canon_store::partition::resolve_partition;
use canon_store::tier::{RawWrite, Tier, TierQuery};
use chrono::Utc;

use crate::context::resolve_repo_root;
use crate::gate::evidence_staging_dir;

/// `--severity`'s clap `value_parser`. Kebab-cased on the CLI, matching
/// `crate::divergence::parse_status`'s established spelling for a
/// snake_case-serialized model enum, and exhaustive over
/// [`FindingSeverity`].
pub fn parse_severity(s: &str) -> Result<FindingSeverity, String> {
    match s {
        "blocker" => Ok(FindingSeverity::Blocker),
        "should-fix" => Ok(FindingSeverity::ShouldFix),
        "note" => Ok(FindingSeverity::Note),
        other => Err(format!("`{other}` is not a finding severity — expected one of: blocker, should-fix, note")),
    }
}

/// `--disposition`'s clap `value_parser`, exhaustive over
/// [`FindingDisposition`].
pub fn parse_disposition(s: &str) -> Result<FindingDisposition, String> {
    match s {
        "open" => Ok(FindingDisposition::Open),
        "fixed" => Ok(FindingDisposition::Fixed),
        "rejected" => Ok(FindingDisposition::Rejected),
        "deferred" => Ok(FindingDisposition::Deferred),
        other => Err(format!("`{other}` is not a finding disposition — expected one of: open, fixed, rejected, deferred")),
    }
}

/// `--change-id`'s clap `value_parser` — grammar-level
/// [`ChangeId::parse`], the same shape `crate::query::parse_change_id`
/// and `crate::subject::parse_change_id` already expose.
pub fn parse_change_id(s: &str) -> Result<ChangeId, String> {
    ChangeId::parse(s).map_err(|e| e.to_string())
}

/// The clap `value_parser` behind every sha-shaped flag on this command
/// (module doc's exit-code section): a malformed value is refused by
/// clap, by flag name, before [`run_add`] is entered — so there is no
/// runtime sha check in this module to drift from the grammar.
pub fn parse_sha(s: &str) -> Result<Sha, String> {
    Sha::parse(s).map_err(|e| e.to_string())
}

/// One `canon finding add` invocation's already-parsed flags.
///
/// A named struct rather than a positional parameter list, for
/// `crate::evidence::EvidenceArgs`'s reason and more acutely: thirteen
/// arguments, of which three are `Option<Sha>` and four are plain
/// strings — `reviewed_sha`/`resolution_sha`/`introduced_by` mean three
/// completely different things and would all compile if transposed,
/// and transposing them would silently produce a WRONG fix-of-fix
/// derivation rather than an error.
pub struct FindingArgs {
    /// The change under review — the finding's scope and the first
    /// component of its natural key.
    pub change_id: ChangeId,
    /// The commit whose state this round reviewed, when that state was
    /// committed at all. Absent is the common case: most rounds review
    /// an uncommitted working tree, and canon-model makes the field
    /// optional precisely so those rounds are recordable. Never
    /// borrow an adjacent sha to fill it.
    pub reviewed_sha: Option<Sha>,
    /// Which review round on this change raised it (1-based).
    pub round: u32,
    /// This finding's index within `round` (1-based). REQUIRED and
    /// collision-checked — see the module doc for why it is not
    /// auto-assigned.
    pub seq: u32,
    pub severity: FindingSeverity,
    /// The disposition to record. Only `fixed` may carry
    /// `resolution_sha`, and it MUST (module doc's coherence section).
    pub disposition: FindingDisposition,
    /// Who raised it — free text, since a reviewer may be a human name
    /// no canon [`RoleId`] models. Refused when it carries a line
    /// separator (module doc).
    pub reviewer: String,
    /// One line of what the finding IS, in the reviewer's own words.
    /// Never checked against anything (module doc's observation
    /// section). Refused when it carries a line separator.
    pub summary: String,
    /// The commit that closed it. Required by, and permitted only
    /// with, `disposition == Fixed`.
    pub resolution_sha: Option<Sha>,
    /// The SOURCED introducing commit, or `None`. `None` means
    /// UNSOURCED — never "no cause" — and is one of the two reasons a
    /// derived fix-of-fix count UNDER-counts. See the module doc's
    /// canonical sentence before setting this.
    pub introduced_by: Option<Sha>,
    /// Where in the tree, as `path/to/file.rs:120-134`. Refused when it
    /// carries a line separator.
    pub file_ref: Option<String>,
    /// The authoring actor's id — the ATTRIBUTION half of what this
    /// command buys. Refused when it carries a line separator.
    pub actor_id: String,
    /// The role the record is attributed to.
    ///
    /// Defaulted, unlike `canon evidence add`'s REQUIRED `--role`, and
    /// the difference is not stylistic: promotion derives an
    /// `EvidenceRecord`'s `run_seq` partition from `actor.role`, so a
    /// defaulted role there would silently mis-partition a sequence. A
    /// `Finding` has no `run_seq` and no partition derived from role,
    /// so this is attribution only and a wrong default costs nothing
    /// but an inaccurate author label.
    pub role: RoleId,
}

/// `canon finding add` (module doc). Returns the process exit code.
///
/// Order matters and is not incidental: every refusal below happens
/// BEFORE the single `staging.write` at the end, so a refused
/// invocation leaves the staging directory byte-identical — the same
/// "a refused add writes nothing" property `crate::evidence::run_add`
/// and `crate::review::run_add` hold.
///
/// Idempotent in the way that matters, and unlike `canon evidence add`:
/// a second add at an already-occupied `(change_id, round, seq)` is
/// REFUSED rather than staged as a second record. Two attestations at
/// two times are two pieces of evidence, but two records claiming to be
/// the same numbered finding are a corrupted count.
pub fn run_add(repo: &Path, args: &FindingArgs) -> i32 {
    let repo = resolve_repo_root(repo);

    if args.round == 0 || args.seq == 0 {
        eprintln!(
            "canon finding add: refused — --round and --seq are 1-based (got --round {} --seq {}); a 0 would sort ahead of the first real finding in a key whose whole job is ordering rounds",
            args.round, args.seq
        );
        return 2;
    }

    for (flag, value) in [("--reviewer", args.reviewer.as_str()), ("--summary", args.summary.as_str())] {
        if value.trim().is_empty() {
            eprintln!("canon finding add: refused — {flag} must be non-empty; a finding with no {} is a row that records nothing", &flag[2..]);
            return 2;
        }
    }

    // The free-text fields, refused before anything is staged (module
    // doc's line-break section). Checked through the row grammar's OWN
    // separator set rather than a local `contains('\n')`, so this
    // refusal cannot disagree with `canon evidence add`'s or with
    // `canon_ingest::reject_multi_line_note`'s about which inputs are
    // safe to put in a row.
    for (flag, value) in [
        ("--summary", Some(args.summary.as_str())),
        ("--reviewer", Some(args.reviewer.as_str())),
        ("--file-ref", args.file_ref.as_deref()),
        ("--actor-id", Some(args.actor_id.as_str())),
    ] {
        let Some((offset, separator)) = value.and_then(first_row_line_break) else { continue };
        eprintln!(
            "canon finding add: refused — {flag} carries the line separator {separator:?} at byte offset {offset}; a finding is rendered as ONE row, so a separator there appends a second row that no finding record backs"
        );
        return 2;
    }

    // The coherence pair, as a NAMED diagnostic rather than a second
    // enforcement layer (module doc): the construction below physically
    // cannot produce either of these states, so this exists to say
    // which flag was wrong, now, instead of leaving the caller a
    // `malformed` record two commands later.
    match (args.disposition, &args.resolution_sha) {
        (FindingDisposition::Fixed, None) => {
            eprintln!(
                "canon finding add: refused — --disposition fixed requires --resolution-sha; `fixed` means CLOSED BY A COMMIT, so a fixed finding that cannot name one is a fix nobody can go look at"
            );
            return 2;
        }
        (disposition, Some(sha)) if disposition != FindingDisposition::Fixed => {
            eprintln!(
                "canon finding add: refused — --resolution-sha {sha} was given with --disposition {}; only a `fixed` finding was closed by a commit, so drop the sha or set --disposition fixed",
                disposition.as_str()
            );
            return 2;
        }
        _ => {}
    }

    // Every sha-shaped flag must name a commit this repository actually
    // holds (module doc's sha-existence section). Placed here, after the
    // pure-string refusals and before anything is constructed, so a
    // refused add still writes nothing — and so the two cheap local
    // checks run before this one ever spawns `git`.
    if let Some(refusal) = unresolvable_sha(&repo, args) {
        eprintln!("{refusal}");
        return 2;
    }

    // Constructed through the builder that makes the pair above
    // unconstructible: `fixed_by` is the ONLY path to `Fixed` and takes
    // the sha by value, `rejected`/`deferred` clear it.
    let mut finding = Finding::new(
        Envelope::current(RecordKind::Finding, Utc::now(), Actor::new(args.actor_id.as_str(), args.role.clone())),
        args.change_id.clone(),
        args.round,
        args.seq,
        args.severity,
        args.reviewer.as_str(),
        args.summary.as_str(),
    );
    finding = match (args.disposition, args.resolution_sha.clone()) {
        (FindingDisposition::Fixed, Some(sha)) => finding.fixed_by(sha),
        (FindingDisposition::Rejected, _) => finding.rejected(),
        (FindingDisposition::Deferred, _) => finding.deferred(),
        // `Open` is what `Finding::new` already produced, and the match
        // above proved `resolution_sha` is `None` here.
        (FindingDisposition::Open, _) => finding,
        // Unreachable: `(Fixed, None)` was refused above. Handled
        // rather than assumed away, and never by fabricating a sha.
        (FindingDisposition::Fixed, None) => {
            eprintln!("canon finding add: refused — --disposition fixed requires --resolution-sha");
            return 2;
        }
    };
    if let Some(sha) = args.reviewed_sha.clone() {
        finding = finding.reviewing_sha(sha);
    }
    if let Some(sha) = args.introduced_by.clone() {
        finding = finding.with_introduced_by(sha);
    }
    if let Some(file_ref) = &args.file_ref {
        finding = finding.with_file_ref(file_ref.as_str());
    }

    let body = serde_json::to_value(&finding).expect("a Finding always serializes");
    let natural_key = match resolve_partition(RecordKind::Finding, &body) {
        Ok(key) => key.natural_key,
        Err(violation) => {
            eprintln!("canon finding add: {}", violation.detail);
            return 2;
        }
    };

    let ledger_root = GateCtx::from_repo(&repo).ledger_root;
    let staging = GitTier::new(evidence_staging_dir(&ledger_root));
    let committed = GitTier::new(&ledger_root);
    match occupied_by(&natural_key, &staging, &committed) {
        Ok(Some(occupant)) => {
            eprintln!(
                "canon finding add: refused — {} is already occupied by a {occupant} finding (`{}`, round {}, seq {}); pick the next free --seq rather than authoring a second record under one finding's identity",
                natural_key, args.change_id, args.round, args.seq
            );
            return 2;
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("canon finding add: {e}");
            return 2;
        }
    }

    match staging.write(&RawWrite(RawRecord(body))) {
        Ok(receipt) => {
            println!(
                "canon finding add: staged {} — {} round {} seq {} ({}, {}) — run `canon gate promote` to commit it",
                receipt.location,
                args.change_id,
                args.round,
                args.seq,
                severity_slug(args.severity),
                args.disposition.as_str()
            );
            if args.introduced_by.is_none() {
                // Said on the success path, not only in `--help`: this
                // is the moment the author still remembers whether the
                // introducing commit was sourced, and an unsourced
                // record is one of the two reasons the derived count
                // UNDER-counts (module doc's canonical sentence).
                println!(
                    "canon finding add: note — no --introduced-by, so this finding is UNSOURCED for fix-of-fix derivation. That is the correct record when the introducing commit was not established; never guess one."
                );
            }
            0
        }
        Err(e) => {
            eprintln!("canon finding add: {e}");
            2
        }
    }
}

/// Which tier, if either, already carries a readable finding at
/// `natural_key` — the DIAGNOSTIC half of the `--seq` collision story
/// (module doc), never the enforcement half.
///
/// It cannot be the enforcement half and the module doc says why: this
/// scan and [`run_add`]'s later `staging.write` are separate steps, so a
/// concurrent add can slip between them. `canon_gate::promote`'s
/// `NaturalKeyRule::Unique` is where "one key, one record" is actually
/// made to hold.
///
/// Scoped honestly: it scans the records each tier can READ. A record
/// so malformed that the tier reports it as a violation instead is not
/// matched here — it carries no body to derive a key from, it is
/// invisible to every consumer of the corpus, and `canon gate promote`
/// refuses it on its own. The key is derived with
/// `canon_store::partition::resolve_partition`, the same function the
/// layout itself uses, never a second `format!` of the composite.
fn occupied_by(natural_key: &str, staging: &GitTier, committed: &GitTier) -> Result<Option<&'static str>, canon_store::tier::StoreError> {
    for (label, tier) in [("staged", staging), ("committed", committed)] {
        for raw in tier.read(&TierQuery::kind(RecordKind::Finding))?.records {
            if resolve_partition(RecordKind::Finding, &raw.0).is_ok_and(|key| key.natural_key == natural_key) {
                return Ok(Some(label));
            }
        }
    }
    Ok(None)
}

/// The refusal for the first sha-shaped flag naming a commit `repo` does
/// not hold, or `None` when every present sha resolves — or when the
/// check does not apply at all (module doc's sha-existence section).
///
/// Returns the whole message rather than a bool so the flag NAME and the
/// value travel with the failure: "a sha did not resolve" is unusable
/// when three flags could have carried it.
fn unresolvable_sha(repo: &Path, args: &FindingArgs) -> Option<String> {
    let flags = [
        ("--reviewed-sha", args.reviewed_sha.as_ref()),
        ("--resolution-sha", args.resolution_sha.as_ref()),
        ("--introduced-by", args.introduced_by.as_ref()),
    ];
    // No sha, no git: an authoring command must not spawn a subprocess
    // to check nothing. (The common case — most findings carry none.)
    if flags.iter().all(|(_, sha)| sha.is_none()) {
        return None;
    }
    // Both skip conditions, in the order that makes the cheap negative
    // first: a non-checkout answers `false`/non-zero to both, so the
    // second call only ever runs inside a real work tree.
    if !git_answers(repo, &["rev-parse", "--is-inside-work-tree"], "true") {
        return None;
    }
    if git_answers(repo, &["rev-parse", "--is-shallow-repository"], "true") {
        return None;
    }

    for (flag, sha) in flags {
        let Some(sha) = sha else { continue };
        if commit_exists(repo, sha) {
            continue;
        }
        return Some(format!(
            "canon finding add: refused — {flag} {sha} is well-formed but names no commit in this repository. Canon does not read the commit's message or diff and is not doing so now; it only refuses a record whose provenance points at nothing. Check the sha (a short sha padded to 40 characters is the way this has actually happened), or omit the flag."
        ));
    }
    None
}

/// `true` iff `git -C repo <args>` succeeds and prints exactly
/// `expected`. Any failure — `git` missing, a non-zero exit, non-UTF-8
/// output — is `false`, which is what makes the two callers above SKIP
/// rather than refuse.
fn git_answers(repo: &Path, args: &[&str], expected: &str) -> bool {
    git_command(repo).args(args).output().is_ok_and(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == expected)
}

/// `true` iff `repo` holds `sha` as a COMMIT object.
///
/// `^{commit}` peels, so a sha naming a tag, tree or blob is not a
/// commit and does not pass — `--reviewed-sha <a tree>` is as wrong as
/// `--reviewed-sha <nothing>`. Existence, never reachability (module
/// doc): `cat-file -e` asks the object database directly, so a commit on
/// an unmerged review branch resolves, which is the normal mid-review
/// case.
fn commit_exists(repo: &Path, sha: &Sha) -> bool {
    let revision = format!("{sha}^{{commit}}");
    git_command(repo).args(["cat-file", "-e", &revision]).output().is_ok_and(|out| out.status.success())
}

/// The one place `git` is spawned for this module — `-C repo` so the
/// process CWD never decides which repository is asked, and
/// `GIT_NO_LAZY_FETCH=1` so a partial clone answers from local objects
/// instead of an authoring command silently hitting the network. (Older
/// gits ignore the variable, which costs nothing.)
fn git_command(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).env("GIT_NO_LAZY_FETCH", "1");
    command
}

/// A [`FindingSeverity`]'s operator-facing spelling, matching
/// [`parse_severity`]'s own accepted domain. An exhaustive match rather
/// than a `Debug` render, so a variant added later has to choose its
/// wording here instead of silently leaking a Rust identifier —
/// `crate::evidence::verdict_slug`'s established shape. (There is no
/// counterpart for [`FindingDisposition`]: canon-model already exposes
/// `FindingDisposition::as_str`, and a second spelling of it here is
/// exactly the drift this pattern exists to avoid.)
fn severity_slug(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Blocker => "blocker",
        FindingSeverity::ShouldFix => "should-fix",
        FindingSeverity::Note => "note",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(change_id: &str, round: u32, seq: u32) -> FindingArgs {
        FindingArgs {
            change_id: ChangeId::parse(change_id).unwrap(),
            reviewed_sha: None,
            round,
            seq,
            severity: FindingSeverity::Blocker,
            disposition: FindingDisposition::Open,
            reviewer: "reviewer-1".to_string(),
            summary: "the release count was typed from memory".to_string(),
            resolution_sha: None,
            introduced_by: None,
            file_ref: None,
            actor_id: "canon".to_string(),
            role: RoleId::parse("reviewer").unwrap(),
        }
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("canon.yaml"), "routing:\n  finding: local\n").expect("writing canon.yaml");
        dir
    }

    fn staged_bodies(repo: &Path) -> Vec<serde_json::Value> {
        let staging = GitTier::new(evidence_staging_dir(&GateCtx::from_repo(repo).ledger_root));
        staging.read(&TierQuery::kind(RecordKind::Finding)).expect("reading staging").records.into_iter().map(|raw| raw.0).collect()
    }

    /// A repo() that is ALSO a real git checkout with one commit, so the
    /// sha-existence check actually engages. `repo()` alone is not a work
    /// tree, which is exactly why every other test in this module is
    /// unaffected by that check.
    fn git_repo() -> (tempfile::TempDir, Sha) {
        let dir = repo();
        let git = |args: &[&str]| {
            let status = Command::new("git").arg("-C").arg(dir.path()).args(args).status().expect("git must be on PATH for this test");
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "test@canon.invalid"]);
        git(&["config", "user.name", "canon test"]);
        git(&["add", "canon.yaml"]);
        git(&["commit", "-q", "-m", "initial"]);
        let out = Command::new("git").arg("-C").arg(dir.path()).args(["rev-parse", "HEAD"]).output().expect("git rev-parse");
        let head = Sha::parse(String::from_utf8(out.stdout).unwrap().trim()).expect("HEAD is a 40-hex sha");
        (dir, head)
    }

    /// s43 round 2, seq 10 — the input that got past authoring: a
    /// well-formed 40-hex sha naming no commit. Nine records citing a
    /// commit that never existed were accepted by the command built to
    /// stop fabricated release facts.
    #[test]
    fn a_well_formed_sha_that_names_no_commit_is_refused_by_flag_name() {
        // The literal shape that caused it: a short sha zero-padded to 40.
        let fabricated = Sha::parse(format!("aa9a4552{}", "0".repeat(32))).unwrap();

        for (label, apply) in [
            ("--reviewed-sha", (|a: &mut FindingArgs, s: Sha| a.reviewed_sha = Some(s)) as fn(&mut FindingArgs, Sha)),
            ("--introduced-by", |a: &mut FindingArgs, s: Sha| a.introduced_by = Some(s)),
            ("--resolution-sha", |a: &mut FindingArgs, s: Sha| {
                a.disposition = FindingDisposition::Fixed;
                a.resolution_sha = Some(s);
            }),
        ] {
            let (dir, _head) = git_repo();
            let mut fabricating = args("s43-findings-are-records", 2, 10);
            apply(&mut fabricating, fabricated.clone());
            assert_eq!(run_add(dir.path(), &fabricating), 2, "{label} accepted a commit that does not exist");
            assert!(staged_bodies(dir.path()).is_empty(), "{label}: a refused add must stage nothing");
        }
    }

    /// The other half: a sha this repository DOES hold goes through on
    /// every flag, so the check is existence and not a blanket refusal.
    #[test]
    fn a_sha_this_repository_holds_is_accepted_on_every_flag() {
        let (dir, head) = git_repo();
        let mut resolvable = args("s43-findings-are-records", 2, 11);
        resolvable.reviewed_sha = Some(head.clone());
        resolvable.introduced_by = Some(head.clone());
        resolvable.disposition = FindingDisposition::Fixed;
        resolvable.resolution_sha = Some(head.clone());
        assert_eq!(run_add(dir.path(), &resolvable), 0);
        assert_eq!(staged_bodies(dir.path()).len(), 1);
    }

    /// Best-effort, stated in the module doc and pinned here: outside a
    /// git work tree the check is SKIPPED, never failed. A fixture
    /// directory has no history to check against, and refusing there
    /// would break authoring where nothing is verifiable either way.
    #[test]
    fn outside_a_git_checkout_the_sha_existence_check_is_skipped() {
        let dir = repo();
        assert!(!dir.path().join(".git").exists(), "the premise: this fixture is not a checkout");

        let mut unverifiable = args("s43-findings-are-records", 2, 12);
        unverifiable.reviewed_sha = Some(Sha::parse("f".repeat(40)).unwrap());
        assert_eq!(run_add(dir.path(), &unverifiable), 0, "a non-checkout must skip the check, not refuse");
        assert_eq!(staged_bodies(dir.path()).len(), 1);
    }

    /// The round-trip contract at the authoring surface: a record this
    /// command STAGED is a record the tier's own read returns, with no
    /// violations. `staging.read()` runs the identical
    /// `canon_store::partition::validate_body` the committed read path
    /// does, so a body canon wrote and canon cannot read would surface
    /// here as a violation rather than a record.
    #[test]
    fn every_staged_record_reads_back_without_a_violation() {
        let dir = repo();

        for (seq, apply) in [
            (1, (|_: &mut FindingArgs| {}) as fn(&mut FindingArgs)),
            (2, |a: &mut FindingArgs| {
                a.disposition = FindingDisposition::Fixed;
                a.resolution_sha = Some(Sha::parse("b".repeat(40)).unwrap());
            }),
            (3, |a: &mut FindingArgs| a.disposition = FindingDisposition::Rejected),
            (4, |a: &mut FindingArgs| a.disposition = FindingDisposition::Deferred),
        ] {
            let mut authored = args("s43-findings-are-records", 9, seq);
            apply(&mut authored);
            assert_eq!(run_add(dir.path(), &authored), 0, "seq {seq}");
        }

        let staging = GitTier::new(evidence_staging_dir(&GateCtx::from_repo(dir.path()).ledger_root));
        let read_back = staging.read(&TierQuery::kind(RecordKind::Finding)).expect("reading staging");
        assert!(read_back.violations.is_empty(), "canon wrote records canon cannot read: {:?}", read_back.violations);
        assert_eq!(read_back.records.len(), 4);
    }

    /// The whole point of `--seq` being required: the second add at an
    /// occupied key is REFUSED, not staged. Without this, two records
    /// would sit under one finding's identity and inflate the count s43
    /// exists to make trustworthy.
    #[test]
    fn a_second_add_at_an_occupied_natural_key_is_refused_naming_the_key() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 1)), 0);

        let mut clashing = args("s43-findings-are-records", 1, 1);
        // A DIFFERENT body at the SAME key — the case `GitTier::write`'s
        // own duplicate-path check cannot catch, because the content
        // digest in the filename differs.
        clashing.summary = "a completely different finding".to_string();
        assert_eq!(run_add(dir.path(), &clashing), 2);

        assert_eq!(staged_bodies(dir.path()).len(), 1, "the clashing add must stage nothing");
    }

    /// A different `seq` in the same round is a different finding and
    /// must go through — the collision check must not be a blanket
    /// per-round refusal.
    #[test]
    fn a_different_seq_in_the_same_round_stages_normally() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 1)), 0);
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 2)), 0);
        assert_eq!(staged_bodies(dir.path()).len(), 2);
    }

    /// Every optional field round-trips as asked, and `reviewed_sha`
    /// absent stays ABSENT rather than serializing as `null` — an
    /// explicit `null` fails `Finding`'s own deserialize, so a record
    /// staged that way would be unreadable.
    #[test]
    fn a_worktree_round_stages_with_no_reviewed_sha_key_at_all() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 8, 1)), 0);

        let body = &staged_bodies(dir.path())[0];
        assert!(body.get("reviewed_sha").is_none(), "an uncommitted round must omit the key, never write null: {body}");
        assert!(body.get("introduced_by").is_none(), "{body}");
        assert!(body.get("resolution_sha").is_none(), "{body}");
        assert_eq!(body["change_id"], "s43-findings-are-records");
        assert_eq!(body["round"], 8);
        assert_eq!(body["disposition"], "open");
    }

    /// The `Fixed` path builds through `fixed_by`, so the staged body
    /// carries both halves of the biconditional.
    #[test]
    fn a_fixed_finding_stages_with_its_resolution_sha() {
        let dir = repo();
        let mut fixed = args("s43-findings-are-records", 2, 1);
        fixed.disposition = FindingDisposition::Fixed;
        fixed.resolution_sha = Some(Sha::parse("b".repeat(40)).unwrap());
        fixed.introduced_by = Some(Sha::parse("c".repeat(40)).unwrap());
        assert_eq!(run_add(dir.path(), &fixed), 0);

        let body = &staged_bodies(dir.path())[0];
        assert_eq!(body["disposition"], "fixed");
        assert_eq!(body["resolution_sha"], "b".repeat(40));
        assert_eq!(body["introduced_by"], "c".repeat(40));
    }

    #[test]
    fn fixed_without_a_resolution_sha_is_refused_and_stages_nothing() {
        let dir = repo();
        let mut incoherent = args("s43-findings-are-records", 3, 1);
        incoherent.disposition = FindingDisposition::Fixed;
        assert_eq!(run_add(dir.path(), &incoherent), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    /// Every non-`Fixed` disposition, not just `Open` — `rejected` and
    /// `deferred` clear `resolution_sha` in the model, so a CLI that
    /// only refused the `Open` case would silently DROP a sha the
    /// caller supplied.
    #[test]
    fn a_resolution_sha_with_any_non_fixed_disposition_is_refused() {
        for disposition in [FindingDisposition::Open, FindingDisposition::Rejected, FindingDisposition::Deferred] {
            let dir = repo();
            let mut incoherent = args("s43-findings-are-records", 4, 1);
            incoherent.disposition = disposition;
            incoherent.resolution_sha = Some(Sha::parse("d".repeat(40)).unwrap());
            assert_eq!(run_add(dir.path(), &incoherent), 2, "{disposition:?} must not accept a resolution sha");
            assert!(staged_bodies(dir.path()).is_empty(), "{disposition:?} staged something");
        }
    }

    /// Every member of the IMPORTED set, over every free-text field —
    /// so a later edit that narrows the check to `'\n'`, or that misses
    /// a field, fails here.
    #[test]
    fn every_row_line_break_in_every_free_text_field_is_refused() {
        for separator in canon_ingest::task_rows::ROW_LINE_BREAKS {
            for field in ["summary", "reviewer", "file_ref", "actor_id"] {
                let dir = repo();
                let mut forged = args("s43-findings-are-records", 5, 1);
                let injected = format!("ok{separator}| forged | row |");
                match field {
                    "summary" => forged.summary = injected,
                    "reviewer" => forged.reviewer = injected,
                    "file_ref" => forged.file_ref = Some(injected),
                    _ => forged.actor_id = injected,
                }
                assert_eq!(run_add(dir.path(), &forged), 2, "{field} accepted separator {separator:?}");
                assert!(staged_bodies(dir.path()).is_empty(), "{field} staged a forged row for {separator:?}");
            }
        }
    }

    #[test]
    fn an_empty_reviewer_or_summary_is_refused() {
        for blank in ["", "   "] {
            let dir = repo();
            let mut empty_summary = args("s43-findings-are-records", 6, 1);
            empty_summary.summary = blank.to_string();
            assert_eq!(run_add(dir.path(), &empty_summary), 2);

            let mut empty_reviewer = args("s43-findings-are-records", 6, 1);
            empty_reviewer.reviewer = blank.to_string();
            assert_eq!(run_add(dir.path(), &empty_reviewer), 2);
            assert!(staged_bodies(dir.path()).is_empty());
        }
    }

    /// `round`/`seq` are 1-based in the model and zero-padded into the
    /// natural key; a `0` would sort ahead of every real finding.
    #[test]
    fn a_zero_round_or_seq_is_refused() {
        let dir = repo();
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 0, 1)), 2);
        assert_eq!(run_add(dir.path(), &args("s43-findings-are-records", 1, 0)), 2);
        assert!(staged_bodies(dir.path()).is_empty());
    }

    #[test]
    fn severity_slug_round_trips_through_parse_severity() {
        for severity in [FindingSeverity::Blocker, FindingSeverity::ShouldFix, FindingSeverity::Note] {
            assert_eq!(parse_severity(severity_slug(severity)), Ok(severity), "{severity:?}");
        }
    }

    #[test]
    fn parse_disposition_accepts_exactly_the_models_own_wire_spellings() {
        for disposition in [FindingDisposition::Open, FindingDisposition::Fixed, FindingDisposition::Rejected, FindingDisposition::Deferred] {
            assert_eq!(parse_disposition(disposition.as_str()), Ok(disposition), "{disposition:?}");
        }
        assert!(parse_disposition("fix-of-fix").is_err(), "fix-of-fix is DERIVED and must never be an authorable disposition");
    }
}
