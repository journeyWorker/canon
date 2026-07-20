## ADDED Requirements

### Requirement: `canon feature new` prints a next-step hint on success
`canon feature new <area>.<surface> --title <label>` SHALL print, on a
successful write, an explicit next-step hint naming the exact `canon
scenario new` invocation that would add the file's first scenario and
make it fmt-clean — derived from the SAME `<area>.<surface>` the command
just scaffolded, never a generic or hand-typed example.

#### Scenario: A fresh feature file's success message names the next command
- **WHEN** `canon feature new wall.render --title 'the wall render
  surface'` writes a fresh stub
- **THEN** stdout includes both the existing "wrote `<path>`" confirmation
  and a next-step line naming `canon scenario new wall.render.01 --title
  '<label>' [--feature <path>]` (the exact derived path this change's
  `derived-validated-scenario-feature` capability would resolve to)

### Requirement: An empty-feature-stub LayoutGrammar violation is worded distinctly, without a new failure class
`canon fmt --check` SHALL still report a `.feature` file matching the
exact shape `canon feature new` produces (a `Feature:` header, one
paired provenance comment, and ZERO `@<area>.<surface>.<nn>`-tagged
scenarios anywhere in the file) under `FmtFailureClass::LayoutGrammar`
— no new `FmtFailureClass` variant is introduced, and `FmtFailureClass::
ALL`'s cardinality SHALL remain exactly 11. The violation's rendered
message text SHALL lead with wording identifying it as an empty,
not-yet-authored feature stub (e.g. "empty feature stub (not yet a
valid corpus entry)") rather than generic grammar-mismatch phrasing, so
it reads as an expected work-in-progress state rather than corpus
corruption. `canon fmt --check`'s exit code for this violation SHALL
remain unchanged (nonzero on a `--check` run that finds it) — the
wording change is legibility only, never leniency.

#### Scenario: A fresh feature-new stub's fmt violation reads as WIP, not corruption
- **WHEN** `canon feature new wall.render --title '…'` writes a fresh
  stub, and `canon fmt --check specs` runs immediately after
- **THEN** the reported violation's class is still `layout-grammar`, the
  command still exits `1`, and the violation's message text leads with
  wording identifying an empty/not-yet-authored feature stub rather than
  a bare grammar-mismatch phrase

#### Scenario: A genuinely malformed layout violation keeps its original phrasing
- **WHEN** `canon fmt --check` encounters a `LayoutGrammar` violation for
  a shape OTHER than the empty-feature-stub case (e.g. a flat pre-migration
  `features/idolive/idolive-hub.feature` path, or a partition-key smear)
- **THEN** the violation's message text is UNCHANGED from today's generic
  grammar-mismatch phrasing — the reworded message applies ONLY to the
  exact empty-stub shape, never broadened to other `LayoutGrammar` causes

#### Scenario: FmtFailureClass's cardinality is unchanged
- **WHEN** this change lands
- **THEN** `FmtFailureClass::ALL` still has exactly 11 members and every
  existing selftest oracle asserting that cardinality (and that each of
  the 11 is independently surfaced by the fixture corpus) still passes
  unchanged

### Requirement: The reworded message is a rendering change only, never a classification or scan change
The empty-feature-stub detector SHALL reuse the SAME
`canon_fmt::gherkin::scan` result `canon fmt --check`'s existing layout
check already computes for the file — never a second corpus scan, never a
new file-read pass, and never a change to WHICH files are scanned or
WHICH violations are reported (only the rendered text of one specific,
already-reported violation shape).

#### Scenario: Scan cost and violation count are unchanged
- **WHEN** `canon fmt --check` runs against a corpus containing one
  `feature new`-fresh empty stub among otherwise well-formed files
- **THEN** the total violation count, the set of files checked, and every
  OTHER violation's class/message are byte-identical to a pre-change run
  — only the one empty-stub violation's message text differs
