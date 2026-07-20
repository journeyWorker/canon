## ADDED Requirements

### Requirement: canon scenario new generates an S11-conformant .feature stub, writing no ledger record
`canon scenario new <area>.<surface>.<nn>` SHALL append (creating the
file if absent) a `.feature` entry carrying the `# canon:` provenance
comment, a `@<area>.<surface>.<nn>` tag immediately preceding a
`Scenario: <label>` header, and a placeholder step block — the exact
tag-then-header shape `canon-fmt::gherkin::scan` already reads. The
command SHALL write NO ledger record of any kind; its only output is
the `.feature` file.

#### Scenario: A generated scenario stub round-trips through canon fmt clean
- **WHEN** `canon scenario new world.hotdeal.42 --title "Apply a
  hotdeal coupon" --feature specs/features/world/hotdeal.feature` runs,
  and `canon fmt --check` runs against that root afterward
- **THEN** `canon fmt --check` reports zero violations for the
  generated entry

#### Scenario: A generated scenario stub is indexed by inventory sync
- **WHEN** `canon inventory sync` runs against the root after the
  scaffold above
- **THEN** it materializes exactly one `Scenario` record for
  `world.hotdeal.42` with `title` derived from the generated header,
  exactly as it would for a hand-authored entry

#### Scenario: The scaffold command writes no ledger record itself
- **WHEN** `canon scenario new` runs
- **THEN** no `Scenario`/`Review`/`Divergence`/or any other
  `RecordKind` file is written by the command — only the `.feature`
  file on disk changes

#### Scenario: A duplicate tag is rejected, never silently appended twice
- **WHEN** `canon scenario new world.hotdeal.42 ...` runs a second
  time against a `.feature` file that already carries a
  `@world.hotdeal.42` tag
- **THEN** the command fails loud, naming the existing tag, and the
  `.feature` file is left unchanged

### Requirement: canon feature new scaffolds a fresh .feature file for a not-yet-started surface
`canon feature new <area>.<surface> --title <label>` SHALL create a
NEW `.feature` file carrying the `# canon:` provenance comment and a
bare feature header, with zero scenarios — a starting point for
subsequent `canon scenario new` calls against the same file. It SHALL
fail loud if the target file already exists, never silently
overwriting it.

#### Scenario: A fresh feature file is created with the provenance comment and no scenarios
- **WHEN** `canon feature new world.checkout --title "Checkout flow"`
  runs against a root with no existing `world/checkout.feature`
- **THEN** a new `.feature` file exists carrying the `# canon:`
  provenance comment and a `Feature: Checkout flow` header, with zero
  `Scenario:` blocks

#### Scenario: An existing feature file is never overwritten
- **WHEN** `canon feature new world.checkout ...` runs against a root
  where `world/checkout.feature` already exists
- **THEN** the command fails loud and the existing file's content is
  unchanged
