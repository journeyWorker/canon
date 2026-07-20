## MODIFIED Requirements

### Requirement: `canon.yaml` declares tier routing by capability rung, with a CLASS-COMPATIBLE backend tagged separately per rung
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
The configured backend's `BackendClass` SHALL match the rung's
expected class (`rung-backend-capability` design D1) — this
SUPERSEDES this capability's original "any rung may be tagged with
any backend" scenario, which is retracted: a `local` rung backed by
`postgres` or `s3`, a `hot` rung backed by `git` or `s3`, or a `cold`
rung backed by `git` or `postgres` no longer parses.

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

#### Scenario: A rung's backend must belong to that rung's expected capability class
- **WHEN** a `canon.yaml` declares `tiers.cold: { backend: postgres,
  dsn_env: CANON_PG_DSN_COLD, schema: canon_cold }` — the `cold` rung
  (expects an object-store backend) tagged with `postgres` (a
  live-database backend) — the SAME pairing this capability's
  original "any rung may be tagged with any backend" scenario used to
  accept
- **THEN** `TierPolicy::from_yaml` now REJECTS it with a `PolicyError`
  naming the class mismatch (`rung-backend-capability` design D1) —
  it no longer parses
- **AND** the `backend:` field remains an explicit, required
  declaration for every configured rung (unchanged from this
  capability's original shape) — only WHICH backends are acceptable
  per rung narrows, a future same-class backend swap (e.g. a second
  live-database vendor for `hot`) remains a config-only change
