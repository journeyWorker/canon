# Decisions (phase 8, 2026-10-21)

Operator chose bundles A (skill text) and B (CLI and records); bundle 0 (measure first) and C (unscheduled) are deferred. Source: `CANON-ADOPTION-BRIEF.md`.

| # | Pattern | Decision | Bundle | Rationale |
| --- | --- | --- | --- | --- |
| X1 | Fail-first proof for fixes | ADOPT | A | skill text: implementer/reviewer fail-first + closest repro |
| X2 | Reviewer proof standards | ADOPT | A | extend role-reviewer 'check the checks' |
| X3 | Anti-rationalization / red-flag blocks | ADOPT | A | short canon-specific blocks per role |
| X4 | Harness capability matrix + spawn contract | ADOPT (text) / DEFER (attestation) | A / C | prose now; session attestation stays with #14/#39 |
| X5 | skills install/check prune dropped files | ADOPT | B | silent stale projection |
| X6 | Authoring rules + reachability check | DEFER | 0 | measure-first bundle not chosen |
| X7 | Dogfood eval methods | DEFER | 0 | measure-first bundle not chosen |
| X8 | Reasoned finding dispositions | ADOPT | B | finding close --reason; prerequisite for #18 |
| X9 | Brief durability | ADOPT | A | current vs desired behavior; no stale paths |
| X10 | Units as flow-step slices | ADOPT | A | planner/implementer text |
| X11 | Expand/contract for migration chores | ADOPT | A (+B template hint) | eligible migrations only |
| X12 | Long-run record / continuation | ADOPT (text) / DEFER (CLI) | A / C | pointer-not-copy + redaction text; CLI with #54 |
| X13 | Repo-specific verifier recipe | DEFER | C | blocked on #20's precondition |
| X14 | Scope bar + rejection records | DEFER | 0 | no declined proposal yet |
| X15 | Intent-to-first-command routing | ADOPT | A | fold into existing route-by-task list |
