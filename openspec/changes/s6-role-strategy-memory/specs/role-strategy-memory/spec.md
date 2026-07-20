## ADDED Requirements

### Requirement: Role-namespaced trajectory store
The system SHALL store every raw trajectory tagged with an open `role`
value drawn from a registered set (`planning|design|dev|test|review|
content|sim` built in, extensible per `canon.yaml`), and SHALL reject a
write carrying an unregistered role at write time.

#### Scenario: A trajectory writes successfully with a built-in role
- **WHEN** a caller stores a trajectory tagged `role: "dev"`
- **THEN** the trajectory is persisted and retrievable by role-scoped
  search

#### Scenario: An unregistered role is rejected at write time
- **WHEN** a caller stores a trajectory tagged with a role not present in
  the repo's registered role set
- **THEN** the write is rejected with a schema-validation error, and no
  trajectory row is persisted

### Requirement: Non-destructive distillation
The system SHALL keep raw trajectories and distilled strategy items in
separate stores. Rebuilding the strategy layer for a role SHALL delete and
re-derive only that role's `StrategyItem` rows, and SHALL NEVER modify or
delete any `Trajectory` row.

#### Scenario: Rebuild leaves raw trajectories untouched
- **WHEN** `rebuild_strategies(role)` runs for a role with N stored
  trajectories and M distilled strategies
- **THEN** after the rebuild, all N trajectories are still present
  unmodified, and the strategy layer is re-derived from them

#### Scenario: Distillation failure never blocks the primary write
- **WHEN** a trajectory is stored and its distiller raises an error
- **THEN** the trajectory write itself already succeeded before
  distillation ran, and the store reports zero strategy rows contributed
  by the failed distillation — the trajectory is not rolled back

### Requirement: Canonical regime key
The system SHALL derive a `regime_key` from `(role, repo, area, hash)`
using exactly one canonical serialization function, called identically at
write time (tagging a trajectory) and at read time (constructing a
retrieval query), so a trajectory recorded under a regime is always the
top candidate for a later same-regime lookup.

#### Scenario: Write key and read key never diverge
- **WHEN** a trajectory is written under regime `(role="dev", repo="donor",
  area="auth", hash="abc123")` and a caller later queries the identical
  regime tuple
- **THEN** the query's derived key is byte-identical to the key recorded
  on the trajectory

#### Scenario: A different role never collides on the retrieval key
- **WHEN** two regimes differ only in `role` (all other fields identical)
- **THEN** their derived `regime_key` values are distinct strings

### Requirement: Git-tier strategy promotion with provenance
The system SHALL support promoting a distilled `StrategyItem` into a
human-reviewable file under `canon/strategies/<role>/` in the consumer
repo's git tree, carrying a provenance block naming the source trajectory
ids the strategy was distilled from.

#### Scenario: A promoted strategy appears as a git-tier file
- **WHEN** `canon learn promote <strategy_id>` runs for a strategy
  distilled from trajectories `[t1, t2]`
- **THEN** a file exists at `canon/strategies/<role>/<strategy_id>.md`
  whose provenance block lists exactly `[t1, t2]`

#### Scenario: Promotion is a new commit, never a rewrite
- **WHEN** an already-promoted strategy is promoted again with updated
  content
- **THEN** the change lands as a new, reviewable diff to the existing
  file — no other promoted strategy file is touched

### Requirement: Similarity search scoping
The system SHALL scope trajectory and strategy similarity search to a
single role by default, never returning cross-role results unless a
caller explicitly requests a cross-role query.

#### Scenario: A role-scoped query excludes other roles
- **WHEN** a caller searches strategies for `role="dev"`
- **THEN** no `content`-role or `sim`-role strategy appears in the results

### Requirement: Store, distill, rebuild, search round-trip
The system SHALL support a full round-trip over its fixture corpus:
storing a trajectory, distilling it into a strategy, rebuilding the
strategy layer from raw trajectories, and searching both layers by role,
producing consistent results across the cycle.

#### Scenario: Fixture round-trip is consistent
- **WHEN** the fixture corpus's store→distill→rebuild→search cycle runs
- **THEN** the strategy set found by search after `rebuild_strategies`
  matches the strategy set found by search before the rebuild, for the
  same source trajectories
