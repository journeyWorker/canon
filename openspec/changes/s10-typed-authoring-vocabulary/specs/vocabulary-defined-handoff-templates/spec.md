## ADDED Requirements

### Requirement: Each handoff domain is declared as a directive
A handoff body template SHALL be declared as one directive per domain (e.g.
`handoff-dev`, `handoff-design`, `handoff-content`, `handoff-test`) in canon's
vocabulary manifest, whose attrs are the domain's required and optional body
fields. A handoff's `domain` field SHALL select the directive tag used to
validate and render its body.

#### Scenario: A domain directive declares the body schema
- **GIVEN** a `handoff-dev` directive declaring required attrs `summary` and
  `verification`
- **WHEN** the vocabulary is resolved
- **THEN** the capability snapshot's directive index contains `handoff-dev` with
  `summary` and `verification` marked required

### Requirement: Handoff bodies are validated through the same pipeline as task atoms
A handoff body SHALL be validated against its domain's directive using the same
resolve-snapshot → validate-attrs pipeline typed task atoms use: an unrecognized
domain SHALL report `E-UNKNOWN-DIRECTIVE`, an undeclared body field SHALL report
`E-UNKNOWN-ATTR`, and an absent required field SHALL report `E-MISSING-ATTR`.

#### Scenario: A handoff missing a required body field is rejected
- **GIVEN** a `handoff-dev` directive requiring `verification`
- **WHEN** a handoff with `domain: dev` omits `verification` from its body
- **THEN** validation reports `E-MISSING-ATTR` naming `verification`

#### Scenario: A handoff for an undeclared domain is rejected
- **WHEN** a handoff declares `domain: marketing` and no `handoff-marketing`
  directive is declared in the resolved vocabulary
- **THEN** validation reports `E-UNKNOWN-DIRECTIVE`

### Requirement: The handoff state-machine fields stay canonical while the body stays vocabulary-defined
Vocabulary-defining a handoff's body SHALL NOT alter the S1 `Handoff` state-
machine fields (`id`, `state`, `chainId`, `parentHandoffId`, `seq`, `claimedBy`,
`openspecChangeSlug`) or their wire-compatibility with the donor monorepo's `handoffs` table.
Only the body content is validated against the resolved domain directive.

#### Scenario: State-machine fields are unaffected by body validation
- **GIVEN** a handoff whose body fails vocabulary validation
- **WHEN** the handoff record is inspected
- **THEN** its state-machine fields (`id`, `state`, `chainId`, `parentHandoffId`,
  `seq`, `claimedBy`, `openspecChangeSlug`) are present and unchanged by the
  body-validation outcome
