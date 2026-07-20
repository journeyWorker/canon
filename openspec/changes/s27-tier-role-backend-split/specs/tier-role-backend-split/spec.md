## ADDED Requirements

### Requirement: `canon.yaml` declares tier routing by capability rung, with the backend tagged separately per rung
`canon.yaml`'s `tiers:` section SHALL be keyed by `Rung` (`local`/
`hot`/`cold`), and each rung's entry SHALL declare its own `backend:`
tag (`git`/`postgres`/`s3`) plus that backend's own config fields
(`git`: `root`; `postgres`: `dsn_env`, `schema`; `s3`: `bucket_env`,
`prefix`). `canon.yaml`'s `routing` map (record kind → destination)
and `aging.*.to` (aging destination) SHALL both take RUNG values
(`local`/`hot`/`cold`), never a backend name. `TierPolicy::
from_yaml` SHALL parse this shape and resolve every write/read/age
through it, exactly as `TierPolicy` already does for `TierKind`
(supersedes s2 `tier-policy`'s original `git`/`pg`/`r2`-keyed shape).

#### Scenario: A rung-and-backend-tagged canon.yaml parses successfully
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` containing
  ```yaml
  tiers:
    local: { backend: git, root: canon/ledger }
    hot:       { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }
    cold:      { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
  routing:
    task: hot
    change: local
    handoff: hot
  aging:
    handoff: { after: 0s, to: cold }
  ```
- **THEN** parsing succeeds; `routing[task] == Rung::Hot`,
  `routing[change] == Rung::Local`, `aging[handoff].to ==
  Rung::Cold`, and `tiers[Rung::Hot]` resolves to a Postgres backend
  config carrying `dsn_env: CANON_PG_DSN`, `schema: canon_v1`

#### Scenario: Any rung may be tagged with any backend, not a fixed pairing
- **WHEN** a `canon.yaml` declares `tiers.cold: { backend: postgres,
  dsn_env: CANON_PG_DSN_COLD, schema: canon_cold }` — the cold rung
  backed by Postgres, an unconventional but expressible pairing
- **THEN** `TierPolicy::from_yaml` parses it successfully; nothing in
  `Rung`'s or `BackendConfig`'s type shape statically forbids a
  non-default rung↔backend assignment

### Requirement: A legacy backend name used where a rung is expected fails loud with a rung-vocabulary hint
`TierPolicy::from_yaml` SHALL reject, with a `PolicyError` naming the
rung vocabulary as a hint, any of: a `routing.<kind>` or
`aging.<kind>.to` value of `git`/`pg`/`r2`; a top-level `tiers.<key>`
whose key is `git`/`pg`/`r2` instead of `local`/`hot`/`cold`; a
rung-named `tiers.<rung>` block missing its required `backend:` tag.
None of these SHALL be silently accepted, silently coerced, or parsed
via a backward-compatibility alias — canon.yaml's rung/backend shape
(this capability) is the ONLY shape `from_yaml` accepts.

#### Scenario: A legacy pg routing value fails loud with a hint
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` whose
  `routing.task` value is the legacy backend name `pg` (not a rung)
- **THEN** parsing fails with a `PolicyError` whose message states
  that `pg` is a backend name, not a rung, and names the rung
  vocabulary (`local`/`hot`/`cold`) as the expected value —
  never a silent fallback to any rung, never a panic

#### Scenario: A legacy r2 aging destination fails loud with a hint
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` whose
  `aging.handoff.to` value is the legacy backend name `r2`
- **THEN** parsing fails with the same class of `PolicyError` naming
  `r2` as a backend, not a rung, and pointing at the rung vocabulary

#### Scenario: A legacy git-named top-level tiers key fails loud with the same hint
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` whose
  `tiers:` section has a top-level key `git` (the pre-migration
  backend-named section) instead of a rung name
- **THEN** parsing fails with a `PolicyError` naming `git` as a
  backend, not a rung — the identical hint text class the routing/
  aging legacy-value scenarios above produce, not a separately-worded
  serde error

#### Scenario: A rung block missing its backend tag fails loud naming the required key
- **WHEN** `TierPolicy::from_yaml` parses a `canon.yaml` whose
  `tiers.hot` block has no `backend:` key (e.g. the pre-migration
  `{ dsn_env: ..., schema: ... }` body with no discriminant)
- **THEN** parsing fails with a `PolicyError` explicitly naming the
  required `backend: git|postgres|s3` key — never a raw, unmodified
  `serde_yaml` "missing field" message

### Requirement: canon gate check is unaffected by the rung/backend split
`canon gate check`'s inputs and verdicts SHALL be byte-identical
before and after this change, for any corpus — `canon-gate` reads
nothing from `canon-store`'s tier vocabulary (`Rung`/`Backend`/
`TierPolicy`) or `canon-report`'s tier-boundary derivation, and no
`canon-gate` source file is touched by this change.

#### Scenario: Gate verdicts are byte-identical across the rung/backend migration
- **WHEN** `canon gate check` runs against an unchanged evidence
  ledger/corpus both before and after this change lands (including
  after the corpus's own `canon.yaml` is migrated to the rung/backend
  shape)
- **THEN** `canon gate check`'s verdicts are byte-identical in both
  cases — the migration of `canon.yaml`'s tier vocabulary has no
  observable effect on gate verdicts
