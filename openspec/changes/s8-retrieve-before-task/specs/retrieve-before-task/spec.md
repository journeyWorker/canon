## ADDED Requirements

### Requirement: Role/regime-scoped advisory retrieval
The system SHALL provide `canon retrieve --role <r> --regime <k>`
returning top-k strategies and guardrails scoped to the given role and
regime, computed via S6's canonical `regime_key(role, repo, area, hash)`
search surface.

#### Scenario: Retrieval returns role-scoped results
- **WHEN** `canon retrieve --role dev --regime <k>` runs against a store
  containing strategies for both `dev` and `content` roles
- **THEN** only `dev`-role strategies appear in the result set

#### Scenario: Retrieval honors the k limit
- **WHEN** `canon retrieve --role dev --regime <k> --k 3` runs against a
  store with more than 3 matching strategies
- **THEN** at most 3 strategies are returned, ranked by similarity

### Requirement: Fail-soft retrieval contract
The system SHALL guarantee that retrieval never blocks a dispatch and
never surfaces a typed failure to the caller: a store outage, timeout, or
malformed row SHALL produce an empty guidance list, never a propagated
error.

#### Scenario: A store outage returns empty guidance, not an error
- **WHEN** `canon retrieve` is invoked while the underlying strategy
  store is unreachable
- **THEN** the command returns an empty guidance list and does not exit
  with an error the caller must specially handle to proceed

#### Scenario: A malformed strategy row is skipped, not fatal
- **WHEN** the store's search results include one malformed row alongside
  otherwise-valid rows
- **THEN** the malformed row is excluded from the returned guidance and
  the valid rows are still returned

### Requirement: Verbatim manifest guidance recording
The system SHALL record every strategy injected into a dispatch as a full
content snapshot (`{strategy_id, title, content}`, never a live pointer)
in the run manifest's `injected_guidance` field, written exactly once at
dispatch time.

#### Scenario: Injected guidance is snapshotted, not referenced
- **WHEN** `canon retrieve` surfaces a strategy at dispatch time and it is
  recorded into `injected_guidance`
- **THEN** the recorded entry carries the strategy's full content at that
  moment, independent of any later edit to the source strategy

#### Scenario: A fixture run's manifest embeds retrieved guidance
- **WHEN** a fixture run dispatches with non-empty retrieved guidance
- **THEN** the resulting manifest's `injected_guidance` field is non-empty
  and matches what `canon retrieve` returned at dispatch time

### Requirement: Replay determinism
The system SHALL guarantee that replaying a manifest re-injects its
recorded `injected_guidance` verbatim via `manifest_guidance_for_replay`,
and SHALL NEVER perform a fresh live retrieval call during replay,
regardless of the live store's state at replay time.

#### Scenario: Replay reproduces guidance after the store changes
- **WHEN** a manifest recorded `injected_guidance` from an original run,
  the underlying store is subsequently modified (strategies added,
  edited, or removed), and the manifest is replayed
- **THEN** the replay's guidance input is byte-identical to the original
  run's `injected_guidance`, unaffected by the store change

#### Scenario: Replay never calls canon retrieve
- **WHEN** a manifest is replayed
- **THEN** no live `canon retrieve` (or equivalent search) call occurs;
  only `manifest_guidance_for_replay` is invoked

### Requirement: Demoted-strategy exclusion
The system SHALL exclude any strategy carrying `status: demoted` from
`canon retrieve` results, while a manifest recorded before the demotion
SHALL continue to replay its originally-injected content unchanged.

#### Scenario: A demoted strategy is excluded from new retrieval
- **WHEN** a strategy is demoted after having previously been retrievable
- **THEN** a subsequent `canon retrieve` call for the same role/regime no
  longer includes it

#### Scenario: An old manifest still replays the demoted content verbatim
- **WHEN** a manifest recorded a now-demoted strategy's content before the
  demotion occurred, and that manifest is replayed
- **THEN** the replay still injects the originally-recorded content —
  demotion does not retroactively alter a past manifest

### Requirement: Pre-dispatch hook wiring
The system SHALL ship a generic pre-dispatch hook script invoking `canon
retrieve` and reuse the same hook-seam entry shape S5 establishes for
`.claude/settings.json` / `.codex/hooks.json`, and SHALL wire it into the donor monorepo
additively alongside the existing pre-edit pattern-lookup hook.

#### Scenario: The pre-dispatch hook entry matches S5's shape
- **WHEN** the pre-dispatch hook is installed into a consumer repo's
  `.claude/settings.json`
- **THEN** the emitted entry uses the same `{matcher, hooks: [{type:
  "command", command, timeout}]}` shape as S5's `canon gate task` entries

#### Scenario: The donor monorepo's existing hook is unaffected
- **WHEN** the pre-dispatch hook entry is added to the donor monorepo's
  `.claude/settings.json`
- **THEN** the existing `pre-edit-pattern-lookup.ts`-driven hook entry
  remains present and unmodified
