## ADDED Requirements

### Requirement: Fixture corpora use two-sided exact-set oracles
The s15 selftest SHALL register fixture corpora whose expected-
violations oracle is compared against the actual violation set with a
TWO-SIDED exact-set match — a mismatch SHALL be reported both when the
actual set is MISSING an entry the oracle expects AND when the actual
set contains an EXTRA entry the oracle does not expect; a one-sided
"still catches the known-bad case" assertion is insufficient.

#### Scenario: A missing expected violation is reported
- **WHEN** a fixture's actual violation set omits an entry present in
  its checked-in expected-oracle file
- **THEN** `canon selftest` reports that fixture as failed, naming the
  missing entry

#### Scenario: An extra, unexpected violation is reported
- **WHEN** a fixture's actual violation set contains an entry NOT
  present in its checked-in expected-oracle file (over-triggering)
- **THEN** `canon selftest` reports that fixture as failed, naming the
  extra entry — an over-triggering check is treated as a failure, not
  silently accepted because it is a superset of what was expected

### Requirement: Rebindable-roots SyncCtx runs the selftest offline
Sync/inventory-materialization logic SHALL be driven through a
`SyncCtx`-shaped seam carrying every rebindable root — mirroring
`GateCtx`'s `from_repo`/`from_fixture` pattern — with TWO constructors:
one binding to a real repo's configured roots, and one binding to a
fixture directory built fresh in a tempdir, so the selftest suite
exercises the identical sync code path fully offline, with no network
and no dependency on this repo's own checkout layout.

#### Scenario: The fixture constructor runs fully offline against a tempdir
- **WHEN** `SyncCtx::from_fixture` builds a fresh fixture corpus in a
  tempdir and inventory-sync logic runs against it
- **THEN** the selftest run completes with no network access and no
  dependency on any path outside the tempdir it created

#### Scenario: Both constructors drive the identical downstream sync logic
- **WHEN** sync logic is invoked once via `SyncCtx::from_repo` and once
  via `SyncCtx::from_fixture`
- **THEN** no downstream sync code branches on which constructor built
  its `ctx` — the same functions run either way

### Requirement: A frozen-incident slot pins a known past fold case as a regression oracle
The selftest suite SHALL include at least one "frozen-incident" fixture
that reproduces a specific, previously-encountered divergence/fold case
(e.g. a real `run_seq`/`round` ordering, or a resolved-then-invalidated
binding this change's design identified), pinned with a checked-in
expected outcome, so a future regression in `fold_to_current_state` or
the sync pipeline is caught even if no other fixture happens to
exercise that exact shape.

#### Scenario: The frozen-incident fixture's fold output matches its pinned oracle
- **WHEN** the frozen-incident fixture's corpus runs through
  `fold_to_current_state`
- **THEN** the resulting `FoldedState` matches the fixture's
  checked-in expected outcome exactly

#### Scenario: A regression in fold ordering breaks the frozen-incident fixture
- **WHEN** `fold_to_current_state`'s ordering logic is changed to rank
  by something other than `run_seq` as sole primary
- **THEN** the frozen-incident fixture's oracle comparison fails,
  surfacing the regression
