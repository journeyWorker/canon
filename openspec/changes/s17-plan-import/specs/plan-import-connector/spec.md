## ADDED Requirements

### Requirement: A plan dialect is one registered adapter behind one frozen PlanAdapter trait
`canon-ingest` SHALL expose a `PlanAdapter` trait (`dialect_id()`,
`resolve_source`, `parse`) plus a static, declaration-ordered
`plan_registry` — mirroring the existing `SessionAdapter`/`registry` and
`ArtifactAdapter`/`artifact_registry` families (a static table, no dynamic
plugin loading, deterministic iteration order). Every `parse` SHALL emit one
shared normalization target, `PlanParseOutcome`: `Change` + `Task` record
candidates, per-construct NAMED unmapped-drop counts, and a malformed count.
Adding a dialect SHALL require exactly one registry entry plus one adapter
module — no change to the trait, the outcome type, the driver, or any other
adapter.

#### Scenario: A second dialect lands as one registry entry
- **WHEN** a new plan dialect adapter (the test-fixture dialect this change
  registers in tests) is added beside the openspec adapter
- **THEN** the diff touches exactly one `plan_registry` entry and one new
  adapter module — the `PlanAdapter` trait, `PlanParseOutcome`, the CLI
  driver, and the openspec adapter are byte-identical before and after

#### Scenario: An unknown dialect id fails loud, naming the registered ids
- **WHEN** `canon ingest plans --dialect no-such-dialect --source /tmp/x`
  runs, or a `canon.yaml` `plans:` source names a dialect absent from the
  registry
- **THEN** the command fails with an error naming the unknown id and the
  registered dialect ids — never a silent zero-source pass
  indistinguishable from "nothing to import"

#### Scenario: Registry iteration order is deterministic
- **WHEN** two `canon ingest plans` passes run over the same configured
  sources
- **THEN** sources and dialects are processed in the same
  declaration/config order both times — never a map-iteration-order
  dependent sequence

### Requirement: canon ingest plans reads a strict canon.yaml plans section with a targeted one-shot override
`canon ingest plans` SHALL resolve its sources from `canon.yaml`'s
top-level `plans:` section (`sources: [{dialect, root}]`, roots resolved
relative to the `canon.yaml` directory). An ABSENT `plans:` section SHALL
resolve to zero sources — a clean, explicit no-op, never a hardcoded
default root. A PRESENT section SHALL parse strictly
(`deny_unknown_fields`): an unknown key or an unregistered dialect id fails
the command loud. A `--dialect <id> --source <path>` pair SHALL bypass the
config for a one-shot import of exactly that source; supplying either flag
without the other SHALL fail loud.

#### Scenario: An absent plans section is a clean no-op
- **WHEN** `canon ingest plans` runs against a repo whose `canon.yaml` has
  no `plans:` section
- **THEN** zero sources are scanned, the summary reports zero sources, and
  the exit is clean — no default path is ever invented

#### Scenario: A typo'd key in a present plans section fails loud
- **WHEN** `canon.yaml` declares `plans: { source: […] }` (a typo for
  `sources:`)
- **THEN** the command fails with an error naming the unknown key — never
  a silent fallback that scans nothing while appearing configured

#### Scenario: The one-shot override requires both flags
- **WHEN** `canon ingest plans --dialect openspec` runs without `--source`
- **THEN** the command fails loud stating both flags are required together,
  and no config-driven scan runs as a fallback

### Requirement: A change_id colliding across two configured sources resolves first-configured-wins, diagnosed
The importer SHALL resolve a `change_id` that collides across two
configured plan sources in one pass first-configured-wins: the source
EARLIER in `canon.yaml`'s `sources:` declaration order wins — its
`Change` + `Task` records are the imported ones — and every later
same-`change_id` occurrence SHALL be skipped with a NAMED
`duplicate-change-id` diagnostic count, never silently importing two
competing histories for one id in a single pass. Across separate PASSES
the append-only fold-latest read still governs; this is the WITHIN-pass,
cross-source tiebreak, mirroring s16's own first-in-config-order rule.

#### Scenario: The first-configured source wins a cross-source change_id collision
- **WHEN** `canon.yaml` configures two `plans:` sources whose trees each
  contain a change dir named `add-widget`, and `canon ingest plans` runs
- **THEN** only the first-configured source's `add-widget` `Change`/`Task`
  records are imported, the second occurrence is skipped and counted under
  a `duplicate-change-id` diagnostic naming the id, and the pass summary
  surfaces that count — never two competing `add-widget` histories from
  one pass

### Requirement: Imported plan records persist only through canon-store's validated tiered write
Every imported record SHALL persist through `TierRegistry::persist` — the
same entry point session/artifact ingest and `canon query` already share —
landing in whatever tier `canon.yaml`'s routing assigns its kind; the
importer SHALL NOT write any record file directly, bypass validation, or
introduce a second write path. When a routed tier is unreachable (e.g.
`task: pg` with no live DSN), the unpersisted candidates SHALL degrade to
the documented `unwritten` output — printed, never silently dropped, and
never fatal to sibling records whose tier writes already succeeded.

#### Scenario: Records land per routing policy
- **WHEN** a plan import runs against a repo routing `change: git` and
  `task: pg` with both tiers reachable
- **THEN** the `Change` record lands under the git tier's `kind=change/`
  partition and the `Task` records land in pg — each via
  `TierRegistry::persist`, byte-validated like any native record

#### Scenario: An unreachable pg tier degrades to the unwritten seam
- **WHEN** a plan import runs with `task: pg` routed but `CANON_PG_DSN`
  unset
- **THEN** git-routed `Change` records persist normally, the `Task`
  candidates are emitted through the `unwritten` output with the reason,
  the exit is non-fatal, and the source's watermark cursor is NOT advanced
  (the pass was not fully durable)

### Requirement: Plan import is deterministic and idempotent — an unchanged source writes zero new records
Record-body derivation SHALL be a pure function of the source snapshot:
envelope `at` derives from the source (file mtimes), NEVER `Utc::now()`;
the actor is a fixed `Actor::new_unattributed("canon-plan-import-
<dialect>")` per dialect. Each configured source SHALL be gated by a
persisted content-digest watermark cursor (`canon-store`'s `SourceCursor`,
source-granular): a source whose present file set is byte-identical to the
cursor is skipped wholesale. Persistence SHALL treat a byte-identical
resubmission as a successful no-op (`DuplicatePath`-tolerant on git,
native dedup on pg/r2). Net: re-importing an unchanged foreign plan writes
ZERO new records; a changed plan writes exactly the refreshed records,
which supersede in fold-latest reads.

#### Scenario: A repeat import over an unchanged source writes nothing
- **WHEN** `canon ingest plans` runs twice over a source whose bytes did
  not change between runs
- **THEN** the second pass skips the source at the cursor gate (no parse,
  no persist) and the ledger gains zero new records

#### Scenario: mtime churn without byte churn never reaches the write path
- **WHEN** every file under an already-imported source is re-stamped
  (e.g. a `git checkout` or `touch`) without any byte change
- **THEN** the cursor's content-digest match still skips the source
  wholesale — no new record bodies (whose `at` would differ) are ever
  derived

#### Scenario: A changed plan supersedes in fold-latest reads
- **WHEN** one checkbox row flips in a source's `tasks.md` and the import
  re-runs
- **THEN** exactly the refreshed records (the flipped `Task`, and the
  `Change` when its derived status changed) are appended with a newer
  source-derived `at`, and fold-latest reads return the new state — prior
  records remain, append-only, never rewritten

#### Scenario: Two runs over one snapshot produce byte-identical bodies
- **WHEN** the same unchanged source is parsed twice (cursor cleared
  between runs)
- **THEN** every derived record body is byte-identical across the two runs
  — fixed actor, source-derived `at`, no wall-clock input — so the
  idempotent persist dedupes them all

### Requirement: Plan import is a connector, never an authority, and never widens the closed kind set
A plan import SHALL map foreign constructs ONTO the existing twelve
`RecordKind`s or drop them with a NAMED per-construct diagnostic count —
never a thirteenth kind, never a new core field, never an untyped payload
escape hatch. No gate, coverage, promotion, or learn code path SHALL read
anything importer-specific: imported `Change`/`Task` records are ordinary
records, distinguishable only by their actor provenance, and no consumer
SHALL branch on that provenance to grant or deny authority.

#### Scenario: Gate verdicts are byte-identical with and without a plan import
- **WHEN** `canon gate check` runs before and after a `canon ingest plans`
  pass over the same repo
- **THEN** every gate verdict (uncovered-cell and all others) is
  byte-identical — the importer feeds the ledger's plan rows, never the
  gate's decisions

#### Scenario: The kind closure survives the import layer
- **WHEN** this change lands
- **THEN** `RecordKind::ALL` still has exactly 12 members, every existing
  structural assertion of that closure is untouched, and no plan-import
  code path constructs a record kind outside the twelve

#### Scenario: An unmappable construct is dropped with a named diagnostic
- **WHEN** a dialect encounters a construct with no home among the twelve
  kinds (e.g. an openspec spec-delta scenario block)
- **THEN** the construct produces no record and increments a diagnostic
  count NAMED for that construct kind in the pass summary — never a
  silent skip, never an invented mapping

### Requirement: Malformed plan sources fail soft per construct, loud per configuration
A plan-import pass SHALL skip AND count a malformed or unreadable
individual construct (one change dir, one row), leaving sibling constructs
to import normally — malformed evidence is no evidence, never a crash. A
malformed CONFIGURATION (unparseable `plans:` section, unknown dialect,
unusable source root) SHALL fail the command loud before any scan — a
misconfiguration is an operator error, not a degradable record.

#### Scenario: One malformed change dir does not sink the pass
- **WHEN** one change dir under a source has an unreadable proposal.md
  while its siblings are well-formed
- **THEN** that dir is skipped and counted in the malformed tally, every
  sibling imports normally, and the pass exits cleanly with the count
  visible in the summary

#### Scenario: A nonexistent configured source root fails loud
- **WHEN** `canon.yaml` configures a `plans:` source whose `root` does not
  exist on disk
- **THEN** the command reports the missing root as an error naming the
  source — never a silent empty scan indistinguishable from an empty
  plan tree

