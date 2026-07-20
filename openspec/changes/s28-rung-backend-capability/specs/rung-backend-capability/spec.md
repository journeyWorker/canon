## ADDED Requirements

### Requirement: A rung's configured backend must belong to that rung's expected `BackendClass`
Every `Backend` SHALL belong to exactly one `BackendClass`
(`LocalFile`/`LiveDb`/`ObjectStore`, via `Backend::class()`), and
every `Rung` SHALL declare exactly one expected `BackendClass` (via
`Rung::expected_backend_class()`): `Local`→`LocalFile`,
`Hot`→`LiveDb`, `Cold`→`ObjectStore`. `TierPolicy::from_yaml` SHALL
reject, with a `PolicyError`, any `tiers.<rung>` entry whose
configured backend's class does not equal that rung's expected class.
The error message SHALL name the rung key, the configured backend, its
actual class, the rung's expected class, and one example backend of
that expected class. This SHALL supersede s27
`tier-role-backend-split`'s original "any rung may be tagged with any
backend" behavior.

#### Scenario: A class-mismatched tiers.<rung> entry fails loud with a hint
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` containing
  `tiers.cold: { backend: postgres, dsn_env: ..., schema: ... }` — the
  `cold` rung (expects `ObjectStore`) tagged with `postgres` (a
  `LiveDb` backend)
- **THEN** parsing fails with a `PolicyError` whose message names
  `tiers.cold`, states that `postgres` is a live-database backend, and
  states that the `cold` rung expects an object-store backend (`s3`)
- **AND** this is the SAME class of loud rejection for any other
  class-mismatched pairing (e.g. `tiers.local: { backend: postgres }`,
  `tiers.hot: { backend: git }`, `tiers.hot: { backend: s3 }`)

#### Scenario: Every class-correct rung/backend pairing parses successfully
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` whose
  `tiers:` section pairs `local` with `git`, `hot` with `postgres`,
  and `cold` with `s3` — today's default, class-correct pairing
- **THEN** parsing succeeds for each pairing independently and in
  combination; `policy.tiers.get(&rung).unwrap().backend()` returns
  the configured backend for each

#### Scenario: The backend field stays an explicit, swappable declaration, not a rung-inferred default
- **WHEN** a `canon.yaml` author considers whether `tiers.<rung>` could
  omit `backend:` and have it inferred from the rung (e.g. `local`
  implying `git`)
- **THEN** `TierPolicy::from_yaml` SHALL continue to require an
  explicit `backend:` tag on every `tiers.<rung>` entry (unchanged
  from s27 D1) — D1's class check narrows WHICH backends are
  acceptable per rung, it does not reintroduce a fixed, code-level
  rung→backend pairing

### Requirement: canon gate check is unaffected by the backend-class validation
`canon gate check`'s inputs and verdicts SHALL be byte-identical
before and after this change, for any corpus — `canon-gate` reads
nothing from `canon-store`'s tier vocabulary (`Rung`/`Backend`/
`BackendClass`/`TierPolicy`) or `canon-report`'s tier-boundary
derivation, and no `canon-gate` source file is touched by this change.

#### Scenario: Gate verdicts are byte-identical across the backend-class validation
- **WHEN** `canon gate check` runs against an unchanged evidence
  ledger/corpus both before and after this change lands
- **THEN** `canon gate check`'s verdicts are byte-identical in both
  cases — the new class-compatibility validation has no observable
  effect on gate verdicts
