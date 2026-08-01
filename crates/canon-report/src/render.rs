//! Markdown rendering: [`render`] assembles the digest header + six
//! mart panels into one byte-stable markdown document — "generated,
//! never hand-edited" (design D16), a leading `<!-- GENERATED … DO NOT
//! EDIT -->` comment (task 1.3), and the [`crate::digest::DigestHeader`]
//! table (TIMESTAMP-FREE, decision 11 — no `generated_at` field
//! anywhere below). Every panel section is produced by straight,
//! deterministic string formatting over already-sorted mart rows
//! (`crate::marts`'s `ORDER BY` clauses) — no `HashMap`/wall-clock/
//! random-order input anywhere in this module, the byte-stability
//! property `crate::check` relies on.

use canon_model::envelope::RecordKind;

use crate::digest::{self, DigestHeader};
use crate::marts::MartResult;
use crate::tier_boundary;

/// The path a rendered report is conventionally written to when a
/// caller (this crate's own `[[bin]]`, or the future `canon-cli`
/// `canon report` arm, part2) does not have a `canon.yaml`-configured
/// override — mirrors `canon-learn`'s own `.canon/`-prefixed default
/// convention (`DEFAULT_LEARN_ROOT`, `DEFAULT_STRATEGIES_ROOT`).
pub const DEFAULT_REPORT_PATH: &str = canon_model::paths::REPORT_FILE;

/// The `## Session costs` panel's prose.
///
/// s42 (`close-the-open-loops`) round-9 re-review, two corrections:
///
/// 1. The fold. All three of `mart_session_costs`' CTEs are folded to
///    the latest version per natural key precisely so a re-ingested
///    CORRECTION supersedes the figure it corrects instead of being
///    summed with it. That is the panel's headline number changing
///    meaning, and it had reached only a SQL comment.
/// 2. `workspace_label`. This paragraph used to call it "the closest
///    available stand-in" for a repo. It is not, and it is not even
///    the strongest field available here. It is
///    `canon_ingest::normalize::workspace_label_from_key` — the last
///    non-empty path segment of the `token_usage` event's workspace
///    key and nothing more — so it SPLITS one repo whose worktrees sit
///    in differently-named directories (`/a/canon` vs
///    `/a/canon-wt/review`) and MERGES two different repos sharing a
///    directory name (`/a/canon` vs `/b/canon`). Two stronger fields
///    are in reach and neither is read: the same event's own
///    `workspace_key` (the full key, at this panel's exact grain), and
///    `Session.project_key`, which `canon-cli` stamps to the MAIN
///    worktree's key precisely so linked worktrees aggregate as one
///    project (`crates/canon-model/src/records.rs`' `Session` doc).
///    Naming the better fields is the honest ending; softening the
///    adjective would not be. The GROUP BY deliberately stays on
///    `workspace_label` — `mart_session_costs`' multi-workspace row
///    split is pinned by its own test — so this is a named, explicit
///    gap, not a pending change.
pub const SESSION_COSTS_PANEL: &str = "Token/cost grouped by session/client/role/`workspace_label` (`mart_session_costs`). Totals fold each `run`/`session`/`token_usage` record to its latest version first, so a re-ingested corrected cost REPLACES the superseded figure instead of being summed with it. `workspace_label` is NOT a repo identity: it is the last non-empty path segment of the `token_usage` event's workspace key and nothing more, so it SPLITS one repo whose worktrees sit in differently-named directories and MERGES two different repos sharing a directory name. Two stronger fields exist and this mart reads neither: the same event's own `workspace_key`, at this panel's exact grain, and `Session.project_key`, which `canon-cli` stamps to the main worktree's key so a repo's linked worktrees aggregate as one project.\n\n";

/// The `## Role memory` panel's prose.
///
/// s42 (`close-the-open-loops`) re-review: the one-liner used to read
/// "Strategies, hit rate, effect per role namespace", which names two
/// quantities the view does not compute. `hit_rate` is the NOT-demoted
/// fraction of a namespace's distilled rows, and there is no effect
/// column at all — `avg_source_trajectories` stands in for one. The
/// view's own comment (`crates/canon-store/sql/views.sql`, panel 3)
/// already said so; the panel a reader actually sees did not, which is
/// the same defect class as the funnel below.
pub const ROLE_MEMORY_PANEL: &str = "Per-`(role, regime_key)` strategy counts (`mart_role_memory`). `hit_rate` is NOT a retrieval hit rate: it is the fraction of that namespace's distilled strategies carrying no `demotion` flag, i.e. exactly `active_count / strategy_count`. `avg_source_trajectories` is the mean number of source trajectories a strategy was distilled from — an explicitly-named stand-in, because canon records no per-strategy reward or effect metric.\n\n";

/// The `## Review burn-down` panel's prose.
///
/// s39 (`joined-evidence-grounding`): the panel is a per-day TREND over
/// raw `Divergence.status` events, and `divergence_open_running_total`
/// is a running `opened - resolved` count of those events — NOT how
/// many divergences are open now. The two differ whenever one
/// `resolved` record closes several findings on a scenario, which is
/// normal: `fold_to_current_state` ranks a scenario's records by
/// `run_seq`, so the latest wins per `(project_id, scenario_id)`. On
/// canon's own corpus this panel reads `2` while every scenario is in
/// fact resolved. The column name alone invites reading the trend as
/// current state, so the pointer to the surface that answers that
/// question ships in the panel itself rather than only in
/// `canon_report::divergence`'s module doc.
pub const REVIEW_BURNDOWN_PANEL: &str = "Review-feedback burn-down over time (`mart_review_burndown`) — a per-day trend over raw `Divergence.status` events, so `divergence_open_running_total` is a running `opened - resolved` event count, NOT the number open now. For current state per scenario, run `canon divergence status`.\n\n";

/// The `## Flywheel funnel` panel's prose, named so the honesty
/// property this string carries is testable on its own rather than
/// only through a fixture-backed full render (s42
/// (`close-the-open-loops`) re-review). Every sentence describes the
/// EXACT relation `mart_flywheel_funnel` computes — see the call site
/// in [`render`] for why the earlier, causal wording was wrong.
pub const FLYWHEEL_FUNNEL_PANEL: &str = "Verdicts → distilled → retrieved → applied (`mart_flywheel_funnel`) — the last three stages count STRATEGIES, so the funnel narrows by construction. `retrieved` counts the distinct strategies some run recorded in its `injected_guidance` that are still distilled today; strategy ids are derived from a strategy's own content, so re-ingesting unchanged evidence re-derives the same ids and leaves this stage intact. `applied` is that same set narrowed to the ones whose recipient run has something recorded about how it ended — NOT how many trajectories were resolved — and `applied_attributed`/`applied_proxy` say WHICH rule earned each count, partitioning `applied` exactly. Both rules are CO-OCCURRENCE inside one run, never causation. `applied_attributed`: the strategy is named in some run's `injected_guidance`, and that SAME run has at least one trajectory of the SAME role stamped with that run id (`Trajectory.run_id`, set only by `canon ingest artifacts --run`) whose `outcome` is resolved (`success`/`failure`/`rolled-back`). `applied_proxy`: no such trajectory exists, so all that is recorded is the recipient run's own terminal `Run.status` (`succeeded`/`failed`/`aborted`). Attribution is the stronger of the two — it requires a judged outcome out of that run, not merely that the run reached a terminal state — and is still weaker than tying the outcome TO the guidance: canon stores no edge from a strategy to a verdict, so this join cannot separate guidance that was followed from guidance that was ignored in an otherwise identical run, and a second still-distilled strategy of the same role injected into the same run counts identically. Closing that gap needs a record change canon has not made — a `StrategyId` stamped on the `Trajectory`/`VerdictRow` at judgment time, joined here instead of `run_id` alone — not a re-reading of this one. A repo that never passes `canon ingest artifacts --run` reads its whole `applied` under `applied_proxy`. `applied` can never exceed `retrieved`, and `retrieved 0` means no run's recorded guidance names a strategy that exists now.\n\n";

/// The ONE sentence every surface states about `fix_of_fix`, verbatim.
///
/// s43 round 2, findings 5 and 7: seven surfaces described this count
/// and each described it differently — `records.rs` and the CLI help
/// called it a FLOOR, `marts.rs` called it "a readable FLOOR rather
/// than a total", the dashboard's static markup called it "a derived
/// floor", and this panel had already retired the word while quietly
/// replacing it with a per-row attribution verdict the query cannot
/// reach. Paraphrase is how that happens: each surface restates the
/// claim in its own words and one of the restatements is wrong.
///
/// So there is exactly one sentence, and every surface repeats it
/// character for character. It carries all four things the join
/// supports — that the count bounds nothing, the two reasons it
/// under-counts, the reason it over-counts, and that no INDIVIDUAL
/// match is attributable either way — because a surface that quotes
/// half of it is back to paraphrasing.
///
/// Pinned here (`review_rounds_panel_states_the_one_canonical_sentence`),
/// in `canon-cli`'s help test, in `canon-model`'s own source guard, and
/// across the dashboard/skill copies by
/// `packages/dashboard/test/panel-copy.test.ts`, which reads this
/// declaration out of this file and asserts it on every other surface.
pub const FIX_OF_FIX_MEANING: &str = "`fix_of_fix` bounds NOTHING — not from below, not from above: it UNDER-counts, because an unsourced finding is never counted and a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all; it OVER-counts, because a `resolution_sha` commit may carry work BEYOND the fix and every finding recording that commit is counted regardless; and for any individual match the data cannot say whether the fix or the other work in that commit introduced the defect.";

/// The `## Review rounds` panel's prose (s43 `findings-are-records`).
///
/// This change exists because a hand-written release summary claimed
/// "49 real issues, four of them defects in the previous round's fix"
/// and both numbers were wrong. So this panel's ONE job is to be a
/// number worth copying — which it only is if the prose beside it
/// claims exactly what `mart_review_rounds` computes.
///
/// Written from the SQL, clause by clause:
///
/// - "one row per `(change_id, round)`" — the view's `GROUP BY
///   change_id, "round"`.
/// - "folded to the latest version of each … finding first" — the
///   `QUALIFY row_number() … PARTITION BY change_id, round, seq`.
/// - "`introduced_by` equals the `resolution_sha` of a finding earlier
///   in the SAME change by `(round, seq)`" — the `EXISTS` clause,
///   field for field, including its `g.change_id = f.change_id` scope.
/// - "strictly" — `(g.round < f.round OR (g.round = f.round AND g.seq
///   < f.seq))`, which is irreflexive.
/// - "a commit-id equality join" — `g.resolution_sha =
///   f.introduced_by` is the whole predicate; there is no git read, no
///   blame, and no diff anywhere in the path.
/// - "NOT counted … reviewing a different change" — the same-change
///   scope again, stated as the cost it is.
/// - "`introduced_by_unsourced` … never as not-a-fix-of-fix" — the two
///   complementary `count(*) FILTER (WHERE introduced_by IS
///   [NOT] NULL)` columns.
/// - "bounds NOTHING" — follows from the clauses above, in BOTH
///   directions. UNDER: an unsourced `introduced_by` fails
///   `f.introduced_by IS NOT NULL`, and the `g.change_id = f.change_id`
///   scope drops the cross-change case. OVER: the `EXISTS` predicate is
///   `g.resolution_sha = f.introduced_by` and nothing else, so a
///   resolution commit that ALSO carries work other than the fix
///   matches every finding recording it. The count is exact; what it
///   counts is a commit-id coincidence. This bullet used to read
///   "FLOOR", which is the misreading the panel was written to
///   prevent, shipped by the panel itself.
/// - "for any individual match the data cannot say whether the fix or
///   the other work in that commit introduced the defect" — the same
///   `EXISTS` predicate, read one row at a time. s43 round 2: the
///   panel had upgraded the population claim into a per-row verdict,
///   saying round 9's mixed-commit matches were NOT attributable
///   while rounds 10 and 11's pure-fix matches WERE. The SQL
///   establishes neither. A fix-only `resolution_sha` narrows what the
///   commit CONTAINED and still leaves the match unattributable,
///   because the predicate compared two RECORDED ids and canon never
///   checks that `introduced_by` names the true cause. `tasks.md` 4.2
///   is the authoritative wording: a mixed commit's findings "cannot
///   be attributed either way".
/// - "counts the rounds that FOUND something, never the rounds RUN" —
///   `FROM stg_records WHERE kind = 'finding'` is the only source. A
///   round that found nothing wrote no `Finding` and cannot appear, and
///   canon has no round-completion record to supply the missing rows.
/// - "`reviewed_sha` is the greatest value any of the round's findings
///   recorded" — `max(reviewed_sha)`, and the panel does NOT say the
///   round's findings agree on it, nor that a blank one PROVES a
///   worktree round, because the view checks neither.
///
/// What the panel deliberately does NOT say: that a counted finding
/// was CAUSED by the earlier fix. The join reads two recorded SHA
/// fields for equality; it cannot distinguish a correctly-sourced
/// `introduced_by` from a careless one, and canon never infers the
/// field. Asserting causation here would be the ninth surface on this
/// release line to claim a relationship its query never computed
/// (s39's model-level ceiling, s40's funnel columns, s41's
/// burn-down-as-current-state, s42's applied split, and this panel's
/// own retired FLOOR — s43 round 1, seq 1), which is
/// precisely the failure this change exists to stop.
pub const REVIEW_ROUNDS_PANEL: &str = "Findings per review round (`mart_review_rounds`) — one row per `(change_id, round)` over `Finding` records, folded to the latest version of each `{change_id}__{round}__{seq}` finding first, so a finding re-authored from `open` to `fixed` is counted once and in one disposition bucket. A round that found NOTHING wrote no `Finding` and so has no row here: this table counts the rounds that FOUND something, never the rounds RUN. s42's round 12 returned MERGEABLE with zero findings and is absent from canon's own rows. canon has no record kind for a review round — `Review` is a per-scenario attestation, not a round — so the number of rounds RUN is not derivable from this corpus at all; it would take a record written when a round completes, and canon has none today.\n\n`fix_of_fix` counts the findings in this round whose SOURCED `introduced_by` equals the `resolution_sha` of a finding earlier in the SAME change, ordered strictly by the natural key's own `(round, seq)` pair — strictly, so no finding is ever matched against its own `resolution_sha`. That is a commit-id equality join over two recorded fields and nothing more: it reports that the commit which closed an earlier finding is the commit a later finding RECORDS as its introducing commit. It reads no git history, computes no blame, and is exactly as sound as the sourcing of `introduced_by` — a field canon never infers and a reviewer may leave unsourced, which is a discipline canon asks for and cannot enforce. Rounds are ordered by `round`, never by `reviewed_sha` (a round that reviewed an uncommitted working tree has none, and that is the common case) and never by `at` (that is when the record was WRITTEN — a backfill authors many rounds in one sitting, in any order). `introduced_by_unsourced` is the UNKNOWN bucket: a finding with no sourced `introduced_by` is counted there and NEVER as not-a-fix-of-fix, because absence of a sourced commit is not evidence that no earlier fix was involved.\n\n`fix_of_fix` bounds NOTHING — not from below, not from above: it UNDER-counts, because an unsourced finding is never counted and a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all; it OVER-counts, because a `resolution_sha` commit may carry work BEYOND the fix and every finding recording that commit is counted regardless; and for any individual match the data cannot say whether the fix or the other work in that commit introduced the defect. It is an EXACT count of a commit-id coincidence, and that coincidence errs in both directions — `round` restarts at 1 per change and canon holds no cross-change round order, and the join sees one commit id on both sides and cannot tell a fix-only commit from a commit that also carried the feature. Both directions are live in canon's own v0.4.0 review rounds — the `s42-close-the-open-loops` rows, which are the rows below in canon's own report. Round 9's six counted findings record `f438c610`, which closed round 8 AND shipped s42's whole feature, so they cannot be attributed either way; rounds 10 and 11's introducing commits (`49d3eb4f`, `b22fe8f5`) held no work but the fix, which narrows what those commits contained and still leaves their matches unattributable, because the predicate compared two RECORDED ids and whether `introduced_by` names the right commit is the author's sourcing, not the query's. Nothing in the table separates a mixed commit from a fix-only one, and reading the commits settles only what they contained. Round 9 also carries one unsourced finding, which could belong to the count and does not. So read the number as exactly what it joins — findings whose recorded introducing commit is a recorded earlier resolution commit of the same change — and as no statement whatever about how many defects this change's fixes introduced.\n\n`introduced_by_sourced + introduced_by_unsourced = findings` and `fix_of_fix <= introduced_by_sourced`, both by construction, so the size of the unknown is readable against the count. `reviewed_sha` is the greatest value any of the round's findings recorded, and reads `—` when NO finding in the round recorded one; a round that reviewed an uncommitted working tree is the usual reason, but the view cannot distinguish that from a round whose findings simply left the field unrecorded, and it neither requires a round's findings to agree on the value nor claims they do.\n\n";

fn cell(row: &crate::query::Row, column: &str) -> String {
    match row.get(column) {
        None | Some(serde_json::Value::Null) => "—".to_string(),
        Some(serde_json::Value::Bool(b)) => if *b { "✓" } else { "·" }.to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn render_table(out: &mut String, mart: &MartResult) {
    if mart.rows.is_empty() {
        out.push_str("_No rows._\n\n");
        return;
    }
    out.push('|');
    for column in mart.columns {
        out.push_str(&format!(" {column} |"));
    }
    out.push('\n');
    out.push('|');
    for _ in mart.columns {
        out.push_str("---|");
    }
    out.push('\n');
    for row in &mart.rows {
        out.push('|');
        for column in mart.columns {
            out.push_str(&format!(" {} |", cell(row, column)));
        }
        out.push('\n');
    }
    out.push('\n');
}

/// The eight panels this report renders, in design D5's own declared
/// order, with s24's `scope_status`, s36's `subjects` and s43's
/// `review_rounds` appended last.
pub struct ReportMarts {
    pub trust_matrix: MartResult,
    pub session_costs: MartResult,
    pub role_memory: MartResult,
    pub flywheel_funnel: MartResult,
    pub review_burndown: MartResult,
    pub scope_status: MartResult,
    pub subjects: MartResult,
    pub review_rounds: MartResult,
}

/// Renders the full report markdown. Pure formatting over already-
/// computed inputs (`digest`, `marts`, `kinds_not_read_directly`) —
/// deterministic, so calling this twice with identical inputs produces
/// byte-identical output (the property `crate::check::check`'s drift
/// gate and this crate's own byte-stability test both exercise).
/// `kinds_not_read_directly` is config-derived
/// (`crate::tier_boundary::kinds_not_read_directly`, `canon.yaml`'s
/// static `routing`/`tiers` tables), never a live, non-directly-
/// readable-backend read (s28 design D2) — an empty slice renders NO
/// `## Kinds not read directly` section at all (design D2), so every
/// fixture whose `canon.yaml` routes nothing to a not-directly-read
/// backend renders byte-identically to before this parameter existed.
pub fn render(digest: &DigestHeader, marts: &ReportMarts, kinds_not_read_directly: &[RecordKind]) -> String {
    let mut out = String::new();
    out.push_str("# canon report\n\n");
    out.push_str("<!-- GENERATED by `canon report` (design doc D16/S9 decision 11). DO NOT EDIT.\n");
    out.push_str("     Run `canon report --check` at the same inputs to verify freshness; a mismatch is drift, not a merge conflict to resolve by hand. -->\n\n");

    out.push_str("## Inputs (digest)\n\n");
    out.push_str("| input | digest |\n|---|---|\n");
    out.push_str(&format!("| corpus ({}) | `{}` |\n", digest::corpus_coverage_label(), digest.corpus_hash));
    out.push_str(&format!("| policy | `{}` |\n", digest.policy_hash));
    out.push_str(&format!("| ledger head ({}) | `{}` |\n", digest::ledger_coverage_label(), digest.ledger_hash));
    out.push('\n');

    if let Some(note) = tier_boundary::render_note(kinds_not_read_directly) {
        out.push_str(&note);
    }
    out.push_str("## Trust matrix\n\n");
    out.push_str("Change/task coverage × green × who (`mart_trust_matrix`).\n\n");
    render_table(&mut out, &marts.trust_matrix);

    out.push_str("## Session costs\n\n");
    out.push_str(SESSION_COSTS_PANEL);
    render_table(&mut out, &marts.session_costs);

    out.push_str("## Role memory\n\n");
    out.push_str(ROLE_MEMORY_PANEL);
    render_table(&mut out, &marts.role_memory);

    out.push_str("## Flywheel funnel\n\n");
    // s40 (`plan-vs-actual-diff`): `applied` used to count resolved
    // trajectories with no reference to retrieval at all, so it could
    // — and on canon's own corpus did — exceed `retrieved`, which is
    // an impossible reading for a funnel. It is now the retrieved set
    // narrowed by what is recorded about the recipient run, and the
    // last three stages share one unit (strategies). The panel says so
    // itself rather than leaving the reader to infer it from bare
    // column names, the same posture as the burn-down panel below.
    //
    // s42 (`close-the-open-loops`, task 3.3): s40 could only offer the
    // recipient run's terminal status, because the trajectory feeding
    // this panel carried no run id — so s40 task 3.1's own wording ("a
    // resolved trajectory joined to its own run") stayed open. It is
    // closed now, and the prose has to distinguish the two rules
    // rather than let one `applied` number stand for both: an
    // attribution and a proxy make DIFFERENT claims, and quietly
    // adding them is the same defect class as the burn-down panel that
    // read as current state. The split is quantified in the table's own
    // `applied_attributed`/`applied_proxy` columns, so this paragraph
    // only has to say what each rule means and which is the weaker.
    //
    // s42 re-review: the first wording of that split still overreached
    // — it said a stamped trajectory showed "the guidance was in
    // context AND that context produced a judged outcome". The SQL
    // joins `(run_id, role)` and nothing else, so ANY resolved
    // trajectory of a role admits EVERY still-distilled strategy of
    // that role injected into that run; there is no strategy→outcome
    // edge in any record kind, hence no basis for a causal reading.
    // This is the fourth panel on this release line to assert a
    // relationship its query never computed (s39's model-level
    // ceiling, s40's funnel columns, s41's burn-down-as-current-state),
    // so the fix is not a softer adjective: the prose below states the
    // exact join, ranks it against the proxy, and names the record
    // change real attribution would require. Keep it that way.
    //
    // The two retrieval stages join a `Run.injected_guidance` snapshot
    // against the CURRENT distilled rows, so the prose may claim no
    // more than "still distilled today" — it deliberately does NOT say
    // "ever injected". That stronger reading was false while a rebuild
    // re-keyed the strategy layer, and is only near-true now that
    // `StrategyId` is content-derived (`crates/canon-learn/src/ids.rs`);
    // a strategy re-derived from CHANGED evidence still lands under a
    // new id and drops out, which the wording has to leave room for.
    out.push_str(FLYWHEEL_FUNNEL_PANEL);
    render_table(&mut out, &marts.flywheel_funnel);

    out.push_str("## Review burn-down\n\n");
    out.push_str(REVIEW_BURNDOWN_PANEL);
    render_table(&mut out, &marts.review_burndown);

    out.push_str("## Scope status\n\n");
    out.push_str("Task done × evidence-verified × spec-covered, per declared scenario ref (`mart_scope_status`).\n\n");
    render_table(&mut out, &marts.scope_status);

    out.push_str("## Subjects\n\n");
    out.push_str("Per-domain subject rollup: status × scenario coverage (`mart_subjects`).\n\n");
    render_table(&mut out, &marts.subjects);

    out.push_str("## Review rounds\n\n");
    // s43 (`findings-are-records`): the number this panel exists to
    // replace was typed from memory into a published git tag ("49 real
    // issues, four of them defects in the previous round's fix" — both
    // wrong). A generated number only beats a remembered one if the
    // prose beside it is exact, so `REVIEW_ROUNDS_PANEL` is written
    // from the view's own clauses, not from what a fix-of-fix count is
    // FOR: the join is commit-id equality between a recorded
    // `introduced_by` and an earlier finding's recorded
    // `resolution_sha`, scoped to one change and ordered strictly by
    // `(round, seq)`. It reads no git history and establishes no
    // causal claim, and the panel says both.
    //
    // It also says, since s43 round 1 found the panel claiming
    // otherwise about itself, that the count bounds NOTHING. Calling it
    // a FLOOR was true only of the two UNDER-counts (the UNSOURCED
    // bucket, the uncounted cross-change case) and blind to the
    // OVER-count sitting in the very table it introduces: `f438c610`
    // closed s42's round 8 AND shipped s42's feature, so round 9's six
    // counted findings are a commit-id coincidence, not defects
    // attributable to round 8's fixes. A directional word is a claim
    // like any other, and this one was the eighth on this release line.
    out.push_str(REVIEW_ROUNDS_PANEL);
    render_table(&mut out, &marts.review_rounds);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marts;

    /// Phrases that assert the injected guidance DROVE the run's
    /// outcome. `mart_flywheel_funnel` joins `(run_id, role)` and holds
    /// no strategy -> outcome edge, so none of these may appear in a
    /// panel a reader takes as the view's meaning (s42
    /// (`close-the-open-loops`) re-review).
    const CAUSAL_PHRASES: &[&str] =
        &["produced", "caused", "acted on", "led to", "resulted in", "drove", "thanks to", "influenced", "brought about", "was effective"];

    /// Phrases that assert `fix_of_fix` BOUNDS the number of defects a
    /// change's fixes introduced. It bounds nothing: the same-change
    /// scope and the UNSOURCED bucket make it miss, and a
    /// `resolution_sha` commit carrying work other than the fix makes
    /// it over-count (s43 `findings-are-records` round 1, seq 1 — the
    /// panel shipped "FLOOR" while the corpus underneath it disproved
    /// the direction). Same idea as `CAUSAL_PHRASES` one field up: a
    /// word the query cannot earn, banned by spelling rather than left
    /// to a reviewer's eye.
    ///
    /// Deliberately banned OUTRIGHT rather than only when asserted. A
    /// panel that says "not a floor" invites the reader to weigh the
    /// denial against the word, and the word wins — the honest sentence
    /// names the two directions instead.
    const DIRECTIONAL_PHRASES: &[&str] = &[
        "floor",
        "lower bound",
        "lower-bound",
        "bounds from below",
        "minimum",
        "at least",
        "no fewer than",
        "conservative",
        "understates",
        "underestimate",
        "under-estimate",
        "upper bound",
        "ceiling",
        "at most",
        "no more than",
        "overstates",
    ];

    fn empty(columns: &'static [&'static str]) -> MartResult {
        MartResult { columns, rows: Vec::new() }
    }

    fn empty_marts() -> ReportMarts {
        ReportMarts {
            trust_matrix: empty(marts::TRUST_MATRIX_COLUMNS),
            session_costs: empty(marts::SESSION_COSTS_COLUMNS),
            role_memory: empty(marts::ROLE_MEMORY_COLUMNS),
            flywheel_funnel: empty(marts::FLYWHEEL_FUNNEL_COLUMNS),
            review_burndown: empty(marts::REVIEW_BURNDOWN_COLUMNS),
            scope_status: empty(marts::SCOPE_STATUS_COLUMNS),
            subjects: empty(marts::SUBJECTS_COLUMNS),
            review_rounds: empty(marts::REVIEW_ROUNDS_COLUMNS),
        }
    }

    fn rendered() -> String {
        let digest = DigestHeader { corpus_hash: "c".to_string(), policy_hash: "p".to_string(), ledger_hash: "l".to_string() };
        render(&digest, &empty_marts(), &[])
    }

    /// s43 round 3, finding 5. The two `## Inputs (digest)` labels were
    /// hand-typed and still read `change/task/scenario` and
    /// `evidence/review/divergence` two kinds after `Subject` and
    /// `Finding` joined the digest — a header naming a strict subset of
    /// what it fingerprints. They are now generated from
    /// `crate::digest`'s own partition, and this asserts the RENDERED
    /// lines against that partition kind by kind: each digested kind
    /// names itself on its own row and on no other, and each excluded
    /// kind appears on neither. A subset, a superset, or a stale
    /// spelling (`evidence` for `evidence_record`) all fail here.
    #[test]
    fn the_inputs_labels_state_exactly_what_the_digest_hashes() {
        let report = rendered();
        let corpus_line = report.lines().find(|line| line.starts_with("| corpus ")).expect("a corpus digest row");
        let ledger_line = report.lines().find(|line| line.starts_with("| ledger head ")).expect("a ledger head digest row");

        let corpus_kinds = digest::corpus_kinds();
        let ledger_kinds = digest::ledger_kinds();
        for kind in RecordKind::ALL {
            let name = kind.as_str();
            assert_eq!(corpus_line.contains(name), corpus_kinds.contains(&kind), "corpus label disagrees with the corpus digest about `{name}`: {corpus_line}");
            assert_eq!(ledger_line.contains(name), ledger_kinds.contains(&kind), "ledger label disagrees with the ledger digest about `{name}`: {ledger_line}");
        }

        // The open half is not enumerable per-kind, so the label has to
        // say it is covered rather than list it (`digest`'s
        // `overlay_texts`).
        assert!(corpus_line.contains("namespaced overlay"), "the corpus label must state that overlay kinds are hashed too: {corpus_line}");
        assert!(!ledger_line.contains("overlay"), "overlays are corpus-side; the ledger label must not claim them: {ledger_line}");
    }

    /// The panel must describe the join the SQL actually performs —
    /// injection into a run PLUS a resolved trajectory of the same role
    /// stamped with that same run — and must not imply the guidance
    /// drove the outcome. The prose it replaced said a stamped
    /// trajectory showed "that context produced a judged outcome",
    /// which this test would have failed on.
    #[test]
    fn flywheel_funnel_panel_states_the_join_and_never_claims_causation() {
        // Space-prefixed so a phrase only matches at a word boundary:
        // "distilled today" contains "led to" as a substring, and a
        // false positive there would make this test unmaintainable.
        let panel = format!(" {}", FLYWHEEL_FUNNEL_PANEL.to_ascii_lowercase());
        for phrase in CAUSAL_PHRASES {
            assert!(!panel.contains(&format!(" {phrase}")), "flywheel funnel panel implies causation via {phrase:?}: {FLYWHEEL_FUNNEL_PANEL}");
        }
        for required in [
            "co-occurrence",
            "same run has at least one trajectory of the same role",
            "trajectory.run_id",
            "canon stores no edge from a strategy to a verdict",
        ] {
            assert!(panel.contains(required), "flywheel funnel panel omits {required:?}");
        }
        assert!(rendered().contains(FLYWHEEL_FUNNEL_PANEL), "the rendered report must carry the funnel panel verbatim");
    }

    /// Ranking is part of the claim: attribution is STRONGER than the
    /// terminal-status proxy and WEAKER than attributing the outcome to
    /// the guidance, and the panel has to name the record change the
    /// stronger claim would require rather than leave it as a mood.
    #[test]
    fn flywheel_funnel_panel_ranks_both_rules_and_names_what_real_attribution_needs() {
        let panel = FLYWHEEL_FUNNEL_PANEL.to_ascii_lowercase();
        assert!(panel.contains("stronger of the two"), "the panel must rank attribution above the proxy");
        assert!(panel.contains("weaker than tying the outcome to the guidance"), "the panel must rank attribution below real attribution");
        assert!(
            panel.contains("`strategyid` stamped on the `trajectory`/`verdictrow` at judgment time"),
            "the panel must name the record change real attribution requires"
        );
    }

    /// `mart_role_memory` has no effect column and its `hit_rate` is the
    /// not-demoted share, never a retrieval hit rate. The one-liner used
    /// to advertise both ("Strategies, hit rate, effect per role
    /// namespace"); the panel now says what the view computes.
    #[test]
    fn role_memory_panel_describes_hit_rate_as_the_not_demoted_share() {
        let report = rendered();
        assert!(!report.contains("hit rate, effect per role namespace"), "the role memory panel must not advertise an effect column the view lacks");
        assert!(report.contains("`hit_rate` is NOT a retrieval hit rate"), "the role memory panel must correct the `hit_rate` reading");
        assert!(report.contains("exactly `active_count / strategy_count`"), "the role memory panel must give `hit_rate`'s exact formula");
        assert!(report.contains("canon records no per-strategy reward or effect metric"), "the role memory panel must say no effect metric exists");
    }

    /// The named panel constants are the bridge the dashboard's
    /// cross-surface test (`packages/dashboard/test/panel-copy.test.ts`)
    /// binds to, and that binding is only worth anything if every
    /// constant is actually EMITTED. Keeping a constant while dropping
    /// its `push_str` would otherwise leave both surfaces agreeing about
    /// text no reader ever sees.
    #[test]
    fn every_named_panel_constant_reaches_the_rendered_report_verbatim() {
        let report = rendered();
        for (name, panel) in [
            ("SESSION_COSTS_PANEL", SESSION_COSTS_PANEL),
            ("ROLE_MEMORY_PANEL", ROLE_MEMORY_PANEL),
            ("FLYWHEEL_FUNNEL_PANEL", FLYWHEEL_FUNNEL_PANEL),
            ("REVIEW_BURNDOWN_PANEL", REVIEW_BURNDOWN_PANEL),
            ("REVIEW_ROUNDS_PANEL", REVIEW_ROUNDS_PANEL),
        ] {
            assert!(report.contains(panel), "{name} is declared but never rendered");
        }
    }

    /// The whole point of s43: `fix_of_fix` must read as what
    /// `mart_review_rounds` computes — commit-id equality between a
    /// recorded `introduced_by` and an EARLIER finding's recorded
    /// `resolution_sha` — and never as proof that the earlier fix
    /// caused the later defect. The join reads two stored SHA fields;
    /// canon performs no blame analysis and never infers
    /// `introduced_by`, so a causal reading is exactly the class of
    /// claim this release line has shipped eight times.
    #[test]
    fn review_rounds_panel_states_the_join_and_never_claims_causation() {
        // Space-prefixed for the same word-boundary reason the funnel
        // check is.
        let panel = format!(" {}", REVIEW_ROUNDS_PANEL.to_ascii_lowercase());
        for phrase in CAUSAL_PHRASES {
            assert!(!panel.contains(&format!(" {phrase}")), "review rounds panel implies causation via {phrase:?}: {REVIEW_ROUNDS_PANEL}");
        }
        for required in [
            "commit-id equality join",
            "the commit a later finding records as its introducing commit",
            "reads no git history",
            "as sound as the sourcing of `introduced_by`",
        ] {
            assert!(panel.contains(required), "review rounds panel omits {required:?}");
        }
    }

    /// `introduced_by = None` is UNSOURCED, not "no earlier fix
    /// involved". The panel must give that bucket its own name and
    /// refuse the not-a-fix-of-fix reading outright. A count that reads
    /// as global while computing something narrower is the defect, not
    /// the narrowness.
    #[test]
    fn review_rounds_panel_names_the_unknown_bucket_rather_than_folding_it_into_a_zero() {
        let report = rendered();
        assert!(report.contains("`introduced_by_unsourced` is the UNKNOWN bucket"), "the panel must name the unsourced bucket as unknown");
        assert!(
            report.contains("counted there and NEVER as not-a-fix-of-fix"),
            "the panel must refuse folding unsourced findings into not-a-fix-of-fix"
        );
        assert!(
            report.contains("`introduced_by_sourced + introduced_by_unsourced = findings`"),
            "the panel must state the arithmetic that makes the unknown readable against the count"
        );
    }

    /// s43 round 1, seq 1 — found by reading this panel against the
    /// corpus it renders. The panel said `fix_of_fix` was a FLOOR
    /// "twice over", naming both UNDER-counts and neither OVER-count.
    /// The corpus in the table disproves the direction: `f438c610`
    /// closed s42's round 8 AND shipped s42's whole feature, so round
    /// 9's six counted findings came from the feature, and the join —
    /// commit-id equality, one id on each side — cannot separate them
    /// from the fix. So the panel must name BOTH directions and claim
    /// no bound at all.
    #[test]
    fn review_rounds_panel_calls_the_fix_of_fix_count_no_kind_of_bound() {
        // Space-prefixed for the same word-boundary reason the causal
        // check is.
        let panel = format!(" {}", REVIEW_ROUNDS_PANEL.to_ascii_lowercase());
        for phrase in DIRECTIONAL_PHRASES {
            assert!(!panel.contains(&format!(" {phrase}")), "review rounds panel bounds `fix_of_fix` via {phrase:?}: {REVIEW_ROUNDS_PANEL}");
        }

        let report = rendered();
        assert!(
            report.contains(FIX_OF_FIX_MEANING),
            "the panel must state the one canonical sentence verbatim, not a paraphrase of it"
        );
        assert!(
            report.contains("first found while reviewing a DIFFERENT change is not counted at all"),
            "the panel must name the cross-change case its same-change scope misses"
        );
        assert!(
            report.contains("a `resolution_sha` commit may carry work BEYOND the fix"),
            "the panel must name the OVER-count: a resolution commit carrying non-fix work"
        );
        assert!(
            report.contains("`f438c610`, which closed round 8 AND shipped s42's whole feature"),
            "the panel must work the over-count through the live row a reader is looking at"
        );
    }

    /// s43 round 2, findings 5 and 7 — the round-1 correction reached
    /// this panel and stopped there. `records.rs`, the CLI help,
    /// `marts.rs` and the dashboard's static markup each kept their own
    /// paraphrase, and this panel, having retired the word FLOOR,
    /// replaced it with a per-row attribution verdict the query cannot
    /// reach: round 9's mixed-commit matches were "NOT attributable"
    /// while rounds 10 and 11's pure-fix matches were. The SQL
    /// establishes neither. So the claim is now ONE sentence
    /// ([`FIX_OF_FIX_MEANING`]) that every surface repeats verbatim —
    /// asserted here on the markdown side, in `canon-cli`'s help test,
    /// in `canon-model`'s source guard, and across the dashboard/skill
    /// copies by `packages/dashboard/test/panel-copy.test.ts`.
    #[test]
    fn review_rounds_panel_states_the_one_canonical_sentence() {
        assert!(
            REVIEW_ROUNDS_PANEL.contains(FIX_OF_FIX_MEANING),
            "REVIEW_ROUNDS_PANEL must carry FIX_OF_FIX_MEANING verbatim: {REVIEW_ROUNDS_PANEL}"
        );

        // The retired per-row verdict, banned by spelling for the same
        // reason DIRECTIONAL_PHRASES are: a reader weighing "NOT
        // attributable to round 8's fixes" against "cannot be
        // attributed either way" takes the first as the finer-grained
        // truth, and it is the false one.
        for retired in ["NOT attributable to", "so theirs are", "therefore bounds NOTHING"] {
            assert!(
                !REVIEW_ROUNDS_PANEL.contains(retired),
                "the panel re-acquired the retired wording {retired:?}: {REVIEW_ROUNDS_PANEL}"
            );
        }
        assert!(
            REVIEW_ROUNDS_PANEL.contains("cannot be attributed either way"),
            "the panel must use `tasks.md` 4.2's authoritative wording for the mixed-commit rows"
        );
    }

    /// s43 round 1, seq 2. A review round that finds nothing writes no
    /// `Finding`, so it has no row: the table's unit is rounds THAT
    /// FOUND SOMETHING. s42's round 12 returned MERGEABLE with zero
    /// findings and is absent. The panel must say so, and must not
    /// imply canon can supply the missing rows — no record kind marks a
    /// review round as run.
    #[test]
    fn review_rounds_panel_says_a_round_that_found_nothing_has_no_row() {
        let report = rendered();
        assert!(
            report.contains("counts the rounds that FOUND something, never the rounds RUN"),
            "the panel must state that its unit is rounds that found something"
        );
        assert!(
            report.contains("s42's round 12 returned MERGEABLE with zero findings"),
            "the panel must name the live clean round the table cannot show"
        );
        assert!(
            report.contains("canon has no record kind for a review round"),
            "the panel must say the rounds-run count is unavailable, not merely unrendered"
        );
    }

    /// Round ORDER is the one ordinal this derivation has, and both
    /// alternatives are actively wrong: `reviewed_sha` is absent for a
    /// round that reviewed a worktree (the common case, and why the key
    /// is not sha-based), and `at` is the record's authoring instant,
    /// which a backfill sets in whatever order it walked the rounds.
    /// The panel has to say which it uses and why, or a reader cannot
    /// tell a strict derivation from a plausible one.
    #[test]
    fn review_rounds_panel_says_rounds_are_ordered_by_round_and_not_by_sha_or_at() {
        let panel = REVIEW_ROUNDS_PANEL;
        assert!(panel.contains("ordered strictly by the natural key's own `(round, seq)` pair"), "the panel must state the ordering the join uses");
        assert!(panel.contains("no finding is ever matched against its own `resolution_sha`"), "the panel must state what strictness buys");
        assert!(panel.contains("never by `reviewed_sha`"), "the panel must rule out sha ordering");
        assert!(panel.contains("never by `at`"), "the panel must rule out `at` ordering");
    }

    /// `workspace_label` is `workspace_label_from_key`'s last non-empty
    /// path segment, so it SPLITS one repo across differently-named
    /// worktree directories and MERGES two repos sharing a directory
    /// name. It is not "the closest available stand-in" for a repo and
    /// not even the strongest field on this join: the same `token_usage`
    /// event carries `workspace_key` at the identical grain, and
    /// `Session.project_key` carries repo identity outright. The panel
    /// must also state the fold, which is what makes its totals a
    /// corrected figure rather than a sum over superseded versions.
    #[test]
    fn session_costs_panel_never_calls_workspace_label_a_repo_identity() {
        let report = rendered();
        assert!(!report.contains("the closest available stand-in"), "the session costs panel must not rank `workspace_label` as the best available repo proxy");
        assert!(report.contains("`workspace_label` is NOT a repo identity"), "the session costs panel must deny `workspace_label` repo identity");
        assert!(
            report.contains("SPLITS one repo whose worktrees sit in differently-named directories"),
            "the session costs panel must name the split limitation, and condition it on the directory names"
        );
        assert!(report.contains("MERGES two different repos sharing a directory name"), "the session costs panel must name the merge limitation");
        assert!(report.contains("Two stronger fields exist and this mart reads neither"), "the session costs panel must say better fields are in reach and unread");
        assert!(report.contains("the same event's own `workspace_key`, at this panel's exact grain"), "the session costs panel must name `workspace_key`, the stronger field at this grain");
        assert!(
            report.contains("`Session.project_key`, which `canon-cli` stamps to the main worktree's key"),
            "the session costs panel must name the field that carries repo identity outright"
        );
        assert!(
            report.contains("a re-ingested corrected cost REPLACES the superseded figure"),
            "the session costs panel must state the fold that makes its totals a corrected figure"
        );
    }
}
