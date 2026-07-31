# s39 joined-evidence-grounding — tasks

## 1. Antecedent findings join

- [ ] 1.1 Collect the prose-bearing NON-verdict events by `join_key`
      before the accumulation loop drops them. Only events whose
      `evidence_line` carries real prose qualify — a bare kind label is
      not evidence.
- [ ] 1.2 A verdict-bearing event absorbs the antecedents on its own
      `join_key` whose `at` is at or before its own. Strictly ordered by
      `(at, evidence_line)`, deduped, and capped, so content stays
      deterministic. Depends on 1.1.
- [ ] 1.3 Antecedents are visibly distinct from the verdict's own line in
      the rendered `context`, so a reader can tell the finding from the
      outcome. Depends on 1.2.
- [ ] 1.4 An idempotence test: two passes over the same events derive
      byte-identical `task`/`context`, with antecedents present.
      Depends on 1.2.

## 2. Scenario title join

- [ ] 2.1 Read the `Scenario` ledger index once through the tier registry
      and build a `(project_id, scenario_id) -> title` index. A missing or
      unreadable scenario tier degrades to an empty index — never a fatal
      error, matching the per-adapter degrade contract.
- [ ] 2.2 A trajectory whose `join_key` is a scenario names the scenario's
      title in its `task`. Absent title falls back to today's text
      unchanged. Depends on 2.1.

## 3. Correct the shipped ceiling claim

- [ ] 3.1 Both `strategy-memory` pages (en + ko) assert the pin-sha
      ceiling as a model-level limit. Replace with what is actually true:
      the record carries no prose, and the join spine is how canon reaches
      it anyway. Depends on 1.3 and 2.2, so the docs describe shipped
      behavior rather than an intention.

## 4. Verification

- [ ] 4.1 `cargo test --workspace` green.
- [ ] 4.2 `canon retrieve` on this repo returns at least one strategy
      quoting a real SHIP-BLOCKER finding alongside its resolution, and
      review-derived items name their scenario in human terms.
      Depends on 1.3 and 2.2.
- [ ] 4.3 `canon ingest artifacts` run twice in a row reports the second
      pass as fully deduped — no double-persisted trajectory.
