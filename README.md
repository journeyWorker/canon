# canon

Harness knowledge substrate: systematic spec planning, machine-enforced task
completion, unified agent-session logging, and accumulation-driven harness
improvement — one Rust core, distributed as `bunx @journeykit/canon` (installed bin:
`canon`), usable from any repo.

Named for the musical **canon**: one subject, taken up by many voices, each
developing it in its own register — the same shared backbone on which every
specialist agent role (planning / design / dev / test / review) evolves its own
strategy memory.

## Quick start

```bash
bunx @journeykit/canon --help    # or: cargo install --path crates/canon-cli
canon init                       # write a starter canon.yaml in your repo
canon demo init --repo /tmp/canon-demo   # or try the evidence loop in a sandbox
canon format spec                # validate a spec corpus
canon gate check                 # run the evidence gate
```

Run `canon <command> --help` for any explicit CLI command. Install the
provider-neutral user-facing companion bundle with:

```bash
canon skills install                         # detect .claude/.codex/.omp/.pi
canon skills install --providers=claude,codex,omp,pi
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics, never deletes
```

The canonical projection is one `canon` skill per selected provider:
Claude uses `.claude/skills/canon/SKILL.md`, Codex uses flattened
`.codex/skills/canon.md`, and OMP/Pi use directory-shaped
`.omp/skills/canon/SKILL.md` and `.pi/skills/canon/SKILL.md`. Every selected
provider receives lazy `reference/**` and `scripts/**` sidecars. OMP/Pi are
project-local passive bundles; the retrieve pre-dispatch script is a sidecar,
not a native hook.

`canon/skills-dev/` is contributor-only legacy tooling. It is not part of the
user-facing install; maintainers may materialize it explicitly when developing
Canon with `canon skills install --source canon/skills-dev`.

## Safety boundaries

Canon records and verifies intent, evidence, policy, approvals, and run
lineage; it does not execute agent actions or enforce provider capabilities.
Adapter v1 validation is strict and read-only. Filesystem, network, and secret
capabilities require enforcement by the external provider sandbox. ContextPack
and prompt-registry inputs are captured as immutable selected bytes, while Run
lineage records the pack, provider, model, skill, and policy used.

Risk tiers require current effect/path binding and policy-pinned SSH approval.
Learning is quarantined until paired evaluation and signed approval, and can be
rolled back. Capability authorization defaults to deny (`execution=false`).
Quality and cost outcomes are currently unknown/null; Canon does not claim that
retrieved guidance improves outcomes.

Knowledge-map and check scripts govern canonical-source/projection ownership;
they are not substitutes for implementation or runtime enforcement.

## Status

Public pre-alpha. Core workflows are implemented and dogfooded; interfaces
and storage formats may change. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for
the current architecture contract and [`canon/knowledge-index.json`](canon/knowledge-index.json)
for the machine-readable source/projection/memory map.

## 0.9.0 release and migration

Version 0.9.0 makes the Gherkin `@subject:<id>` tag the only subject ↔
scenario link. `canon report`'s Subjects panel and the `verifying → shipped`
gate now count the scenarios whose latest `canon inventory sync` generation
carries the tag; the never-populated `Subject.scenario_ids` list is gone
(older records still read). Shipping is now refused when a subject owns no
tagged scenario, so tag each subject's scenarios and re-run
`canon inventory sync` before moving it to `shipped`.

## 0.8.0 release and migration

Version 0.8.0 consolidates provider projections into a single `canon` skill,
with stricter signed-approval and quarantine boundaries. Ingest is now
metadata-only: source bytes remain external while Canon records metadata and
provenance. Update integrations to the current parser generations before
migrating, and review existing approval/quarantine workflows for the stricter
requirements.

## Layout

```
crates/            Rust workspace (canon-model, canon-store, canon-ingest,
                   canon-gate, canon-learn, canon-report, canon-cli)
packages/          Bun workspace (cli launcher + prebuilt native binaries)
docs/              Design docs and specs
openspec/          Imported plan dialect (openspec change dirs) — scaffolded at plan time
```

## Live tiers: env contract

`crates/canon-store`'s hot (Postgres) and cold (S3-compatible) tiers
resolve their credentials from these env vars — never hardcoded, never
committed to `canon.yaml` (which only names WHICH var to read, e.g.
`tiers.hot.dsn_env: CANON_PG_DSN`, keeping the config file itself
commit-safe).

| Var | Purpose |
|---|---|
| `CANON_PG_DSN` | Full Postgres DSN URL (`postgres://user:pass@host:port/db`) for the hot tier. A URL, not split user/pass/host fields: one atomic secret to rotate, and the env-name indirection (`canon.yaml`'s `dsn_env` names this var, never the DSN itself) is what keeps `canon.yaml` safe to commit. |
| `CANON_R2_BUCKET` | Cold-tier bucket name (`canon.yaml`'s `tiers.cold.bucket_env` default). |
| `CANON_S3_ENDPOINT` | S3-compatible endpoint URL (MinIO, Cloudflare R2, real S3, …). |
| `CANON_S3_ACCESS_KEY` | S3-compatible access key. |
| `CANON_S3_SECRET_KEY` | S3-compatible secret key. |
| `CANON_S3_REGION` | S3 region; defaults to `us-east-1` in every build (a wrong region is not the silent-misdirection risk a defaulted endpoint/credential pair is). |

**Debug builds** default every `CANON_S3_*` var to the local
`docker-compose.yml` MinIO stack (`http://127.0.0.1:59000`,
`canon`/`canoncanon`) when unset — zero exported env vars needed for
local dev/CI. **Release builds** (`cargo build --release`) REQUIRE
`CANON_S3_ENDPOINT`, `CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY`
to be set explicitly (s29 `store-hardening` D1); a missing one fails
attachment loud, naming every unset var, rather than silently
attaching to the loopback dev stack.

`docker compose up -d --wait postgres minio` starts the local stack:
Postgres on `127.0.0.1:55432` (`canon`/`canon`, database `canon_v1`),
MinIO on `127.0.0.1:59000` (`canon`/`canoncanon`). See
`docker-compose.yml`'s own header comment for the full quick-start
(including `docker compose up minio-init` to create the bucket).
