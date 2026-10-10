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
canon skills install                         # detect .claude/.agents|.codex/.omp/.pi
canon skills install --providers=claude,codex,omp,pi
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics, never deletes
```

The canonical projection is one directory-shaped `canon` skill per selected
provider: Claude uses `.claude/skills/canon/SKILL.md`, Codex uses
`.agents/skills/canon/SKILL.md` (the directory Codex scans for skills), and
OMP/Pi use `.omp/skills/canon/SKILL.md` and `.pi/skills/canon/SKILL.md`. Every
selected provider receives lazy `reference/**` and `scripts/**` sidecars.
Without `--providers`, an existing `.agents` or `.codex` directory selects
Codex. OMP/Pi are project-local passive bundles; the retrieve pre-dispatch
script is a sidecar, not a native hook.

Canon 0.13.0 and earlier wrote Codex to `.codex/skills/canon.md` and
`.codex/skills/canon/**`, which Codex never reads. `canon skills install`
removes the legacy files whose bytes still match the hash in
`.canon/skills/.install-lock.json`, keeps everything else (including your own
files under `.codex/skills`), and drops `.codex/skills` only if it ends up
empty. `skills check` and `skills doctor` report any leftover legacy Codex
projection as a remnant with the command that migrates it.

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

## 0.14.0 release

Version 0.14.0 teaches agents how to work with canon: the skill is a working
loop, `canon init` sets that loop up, bound evidence stays provable, units
and reviews carry a session, and `canon status` says where a repo stands and
what to run next.

**The working loop.** The canon skill now opens with an 8-step loop: brief,
subject, features and scenarios with failure paths, units (one actor id and
session each), implement, evidence, independent review in a different
session, status transition. It names the human stop points (approving the
brief and the policy), and the command reference follows the loop. Its
description now claims build, implement, add-a-feature, change-a-feature and
fix requests in any repo with `canon.yaml`, ahead of generic site, app or
game builder skills; it used to describe itself as a CLI companion.

**`canon init` sets up the loop.** Next to `canon.yaml`, `init` now writes a
starter `.canon/policy.yaml` (`spec_coverage` with `require_evidence: true`,
`require_cases: [failure]` and `require_review: {}`), a canon block in
`AGENTS.md` between `<!-- canon:begin -->` / `<!-- canon:end -->` (created,
or merged without touching text outside the markers), and an `openspec`
plans source rooted at `.` with `openspec/changes/` created. `--no-agents-md`
and `--no-policy` opt out. Rerunning `init` on an existing repo leaves
`canon.yaml` alone, refreshes only the AGENTS.md block and exits 0 (it used
to exit 2); a rerun with nothing to do (`--no-agents-md`) still refuses.

**`canon change new`.** `canon change new <slug> --subject <id> --title <t>`
scaffolds `openspec/changes/<slug>/{proposal,tasks}.md`, imports the change
and adopts it into the subject, with no hand edits to `canon.yaml`. It is
all or nothing up to the adoption, and refuses an existing slug or an
unknown subject. The adopt write it shares with `canon subject adopt` writes
the subject record first (the gate reads adopted changes from it, so a
half-done adoption still has its open blockers counted), writes only the
side lacking the link, and writes nothing for a linked pair. That makes
`canon subject adopt <change> --subject <id>` an idempotent repair: a
failure between the two writes exits 2 and prints that exact command.

**`canon gate check` says when it is not enforcing.** When the corpus has
scenarios and `.canon/policy.yaml` has no `spec_coverage`, the gate prints
`advisory: spec_coverage is off — N scenario(s) are not checked for
evidence; see .canon/policy.yaml` after its result. The exit code is
unchanged.

**Bound evidence stays provable.** `canon evidence add --artifact/--report`
copies each bound file into `.canon/artifacts/sha256/<hex>`, which is
tracked in git. `canon gate check` re-checks every file bound to a latest
evidence record: an intact stored blob is clean even after the working-tree
file is rewritten; a missing blob with a working-tree file that still
matches is clean with an advisory naming `canon evidence vault`; anything
else is `stale-evidence`, naming the path and both digests. `canon evidence
vault` stores the bytes for pre-0.14 records whose files still match. Files
over 25 MiB are refused; `--max-artifact-mib <N>` raises the limit on
`evidence add` and `evidence vault`.

**Evidence is not shaped by tasks.** `--summary` is allowed on
scenario-only evidence. `--report-case` is repeatable and adds cases; every
case whose name carries the scenario id is bound, and a `faithful` verdict
is refused when any bound case failed. A flipped task row's note aggregates
every record bound to the task (`— ✅ 3 evidence records (2 faithful, 1
not-applicable); latest: <summary>`), and `gate task` re-ingests the plan
source so `canon query --kind task` agrees with `tasks.md`. A record keyed
by both `--task` and `--scenario-id` now counts for both cells in
`gate check`, the shipped gate and `canon status`.

**Sessions.** `--session-id` on `evidence add`, `review add`, `finding add`
and `finding close` fills `actor.session_id`.
`require_review.distinct_actor` now also rejects a review whose session
equals an evidence session on that scenario, when both are set.
`finding add --introduced-by self` is refused (`introduced_by` is a commit
SHA); an author records a defect in its own work with `--reviewer <me>`,
and its open blocker still blocks.

**Write mode in every write command.** Each write's success line now says
how it landed: `evidence add`, `finding add` and `finding close` end with
``— run `canon gate promote` to commit it``; `divergence stage` names
`canon divergence promote`; every other write (`review add`,
`divergence resolve|defer`, `subject new|adopt|status`, `change new`,
`scenario new`, `feature new`) ends with `— written directly; nothing to
promote`.

**`canon status`.** The new `canon status [--json]` lists subjects by
status with their scenario, evidence and review counts, gaps and policy
warnings, and ends with a `next:` list of concrete commands. `canon
context` now opens with a header pointing to `canon status`, plus a
`warning:` line when no `spec_coverage` is in force; `--json` is
unchanged.

*Migration:*

- Repos initialized before 0.14 keep `plans.sources: []`; `canon change
  new` refuses and names the entry to add, `{ dialect: openspec, root: . }`
  under `plans.sources`.
- Commit `.canon/artifacts/` with the ledger.
- Run `canon evidence vault` once to store bytes for bound records written
  before 0.14, before their working-tree files change.
- Scripts that parse write-command output see the new write-mode suffixes.
- Re-run `canon skills install` to project the new skill.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.14.0`. Existing cases re-blessed:

- `evidence-add-refused-report-contradiction` — the refusal now says a
  faithful verdict needs every bound case to pass.
- `gate-check-evidence-binding-off`, `gate-check-evidence-binding-require`,
  `gate-check-evidence-binding-require-report`,
  `gate-check-evidence-binding-warn`, `gate-check-malformed-evidence`,
  `gate-check-red-uncovered-cell` — the `spec_coverage is off` advisory.
- `review-gate-distinct-review-passes`, `review-gate-malformed-finding`,
  `review-gate-open-blocker`, `review-gate-override-recorded`,
  `review-gate-self-review-not-counted`, `review-gate-unreviewed-refused` —
  the guard line now reads "a review by an actor, and from a session, other
  than its evidence's".
- `review-gate-distinct-review-passes`, `review-gate-open-blocker`,
  `review-gate-override-recorded`, `subject-ship-allowed` — `subject status`
  ends with `— written directly; nothing to promote`.

New cases: `change-new`, `init-scaffold-and-rerun`,
`evidence-vault-stores-unstored`, `evidence-vault-reports-unstorable`,
`gate-check-artifact-stale-evidence`,
`gate-check-artifact-stored-survives-rewrite`,
`gate-check-artifact-unstored-advisory`, `finding-add-self-found-blocker`,
`review-gate-same-session-not-counted`, `status-gaps`, `status-no-policy`,
`status-ship-ready` (the `status-*` cases print the canon version and were
re-blessed with it). The `gate-check-evidence-binding-*` fixtures also gained
their stored artifact blobs. Every other case is unchanged.

## 0.13.1 fix

0.13.0 and earlier installed the Codex skill as a flattened
`.codex/skills/canon.md`, a directory Codex never reads, so Codex agents never
discovered canon. `canon skills install` now projects Codex the same way as
every other provider, to `.agents/skills/canon/SKILL.md` with its `reference/**`
and `scripts/**` sidecars, and an existing `.agents` or `.codex` directory
selects Codex when `--providers` is omitted.

*Migration:* after upgrading, re-run `canon skills install`; Codex does not
see canon until you do. That run migrates the legacy `.codex/skills/canon.md`
and `.codex/skills/canon/**` projection automatically, but only for files the
lock proves canon wrote: it removes a legacy file only if it is recorded in
`.canon/skills/.install-lock.json` and its bytes still match the recorded
hash, keeps edited, replaced, symlinked, and user-authored files, and removes
`.codex/skills` only if it ends up empty. On unix the cleanup is race-safe:
every directory below the repo is opened without following symlinks and each
file is hashed and unlinked through its parent's handle, so swapping a legacy
directory for a symlink, before or during the run, cannot make it read or
delete a file outside the repo (other platforms check for symlinks first,
which narrows that window without closing it). `skills check` (exit 1) and
`skills doctor` report whatever legacy projection remains, with the fix
command. The developer-only `canon/skills-dev` materializer now writes
`.agents/skills/<name>/SKILL.md` instead of `.codex/skills/<name>.md`; its lock
records no output hashes, so delete old `.codex/skills/<name>.md` files by hand.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.13.1`. Every other case is unchanged.

## 0.13.0 release

Version 0.13.0 makes `canon context --json` parseable, makes an unknown
`policy.yaml` key fail the gate, and has canon pass its own review
requirement.

**Typed policy summary in `canon context --json`.** `spec_coverage`,
`evidence_binding`, and `review.requireReview` are now JSON objects
instead of one-line summary strings. `spec_coverage` and
`evidence_binding` are keyed like `policy.yaml`, with booleans as booleans
and lists as arrays (`{"require_evidence": true, "scope": ["verifying",
"shipped"], "exclude_lanes": [], "require_cases": ["failure"],
"require_review": null}`); `review.requireReview` is
`{"scope": [...], "distinctActor": true, "blockOnFindings": true}` or
`null`. A present but unusable section is `{"invalid": "<detail>"}`
instead of an `INVALID — …` string. `capabilityVersion` is 5. The text
outline is unchanged. *Migration:* a consumer that parsed the old strings
(`require_evidence=true scope=verifying, shipped …`) reads the fields
directly, and checks for an `invalid` key instead of an `INVALID` prefix.

**Unknown `policy.yaml` keys fail the gate.** A top-level key canon does
not know, such as the typo `spec_coverag`, used to be ignored, so a
misspelled section read as not opted in. The new `policy-keys` check makes
`canon gate check` report it as `uncovered-cell <key>`, naming the known
keys: `adapter_capabilities`, `allowed_signers`, `approval`,
`experimental`, `query`, `risk_approvals`, `risk_routing`, `risk_tiers`,
`schema`, `spec_coverage`, `staleness`, `trust_required`, `trust_sample`.
Parsing of the known keys is unchanged. *Migration:* fix or remove any
unknown top-level key before upgrading.

**Canon reviews its own corpus.** Six independent reviewers, none of them
the evidence actor, read each of canon's 257 scenarios, its proving test,
and the code under test. They withheld 17 sign-offs and raised 16 findings
(3 blocker, 13 should-fix). All 16 are fixed, mostly by strengthening the
tests, plus three corrected spec claims (`learn.promotion.01`,
`query.read.04`/`05`, `report.generation.07`); the withheld scenarios were
re-reviewed and every finding is closed as fixed.
`spec_coverage.require_review` is now on in canon's own policy, and review
records (`.canon/ledger/kind=review/`) are tracked in git, so a fresh
checkout, CI included, sees the same reviews.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.13.0`. `context-json-policy-summary` was re-blessed for the typed
`spec_coverage` object, `evidence_binding` as `{"invalid": …}`, and
`capabilityVersion` 5. New case `gate-check-unknown-policy-key` pins the
`uncovered-cell spec_coverag` violation and exit 1. Every other case is
unchanged.

**Website.** The docs site is redesigned as the "Checks Panel" (a
PR-checks hero and a real HTML comparison table, no grid hairlines).

## 0.12.0 release

Version 0.12.0 pins canon's command-line behavior in a conformance corpus,
adds an opt-in independent-review gate, and makes the ledger commands fail
loud on an invalid `canon.yaml`.

**Conformance corpus.** [`conformance/`](conformance/README.md) replays
real `canon` invocations against checked-in fixture repos and compares
normalized stdout, stderr, and exit code byte for byte;
`conformance/regenerate.sh` is the only bless path, and any changed
expectation is a contract change named in the release notes. The case
groups and what they pin:

- `version` — `canon --version` output.
- `gate-check-*` — exit 0 on a clean corpus, 1 on a red cell, 2 on an
  invalid `canon.yaml`; `malformed-evidence` naming the file; the
  `spec_coverage` evidence and `require_cases` golden-path checks; and
  experimental evidence binding off / warn / require / require-report /
  malformed section.
- `usage-error-unreadable-canon-yaml` — exit 2 with the parse error when a
  command cannot read `canon.yaml`.
- `evidence-add-refused-*` — refusals for a file outside the repo, newline
  injection, no matching report case, no subject key, and a report that
  contradicts the verdict.
- `subject-ship-*` and `subject-status-off-chain-transition` — ship refused
  on no tagged scenarios, a missing verdict, or a `require_cases` gap;
  ship allowed; an off-chain status transition.
- `context-json-policy-summary`, `inventory-sync-tag-diagnostics`,
  `query-subject-json` — the read surfaces: `canon context --json` policy
  summary, inventory tag diagnostics, `canon query --kind subject --json`.
- `review-gate-*` — the review gate below: unreviewed promotion refused,
  self-review not counted, distinct review passes, open blocker, recorded
  override, unreadable finding blocks.

**Fail loud on an invalid `canon.yaml`.** The commands that read or write
the ledger (`gate`, `inventory sync`, `evidence`, `review`, `finding`,
`divergence`, `subject`, `report`, `dashboard`, and the rest) now exit 2
naming `<repo>/canon.yaml` and the parse error, where they used to fall
back silently to the default ledger. One deliberate exception:
`canon dispatch begin`/`end` still exit 0, writing the run manifest and
printing the unpersisted ledger write, with the parse error, as a stderr
warning, because failing a live dispatch would lose the run's provenance.
`canon retrieve`, which reads only the learn store, still falls back to its
defaults. *Migration:* a `canon.yaml` that only
worked because of that fallback now refuses — notably a legacy
`tiers: git:` layout; rewrite it to named tiers
(`tiers: { local: { backend: git, root: .canon/ledger } }`) before
upgrading.

**Smaller fixes.** A `malformed-evidence` violation now names the ledger
file for schema-invalid records too (for example `verdict: maybe`), not only
for unparseable JSON. Under `experimental.evidence_binding` with
`strength: report`, the remediation hint names `--report`, never
`--artifact`, since an artifact cannot reach that strength. `canon context`
reports `capabilityVersion` 4.

**Independent review before "done"** ([journeyWorker/canon#2](https://github.com/journeyWorker/canon/issues/2)).
Until now, a subject could reach `verifying` or `shipped` on evidence the
same agent wrote, and open review findings did not affect the gate: nothing
asked whether anyone other than the author had looked at the work. The new
opt-in `spec_coverage.require_review` policy (`scope`, default
`[verifying, shipped]`; `distinct_actor`, default `true`;
`block_on_findings`, default `true`) makes `canon gate check` report each
in-scope scenario with no `canon review add` record by someone other than
its evidence actor as `unreviewed-promotion`, and adds a ninth failure
class, `open-blocker`, for an open `blocker` finding on a change the
subject adopted. `canon subject status` runs the same checks when moving
into a scoped status, prints which ones it ran or skipped, and refuses on
gaps; `--override-reason "<one line>"` (with `--actor-id`) waives only
the violations of those two classes it reports, records exactly those on
the subject, and leaves them listed as advisories by `canon gate check`
until the next status change; a gap that appears later is not covered.
With `block_on_findings`, a finding record canon cannot read (unparseable,
schema-invalid, or misfiled) is a `malformed-evidence` violation naming
its ledger file, in `canon gate check` and in `canon subject status`, and
no override waives it: it might be an open blocker. An unreadable review
record already fails closed, because it counts as no review.
`canon context` now lists finding severities, dispositions, Review fields,
and the active `require_review` setting. Nothing changes for a repo that
does not set `require_review`; canon's own policy leaves it off for now.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.12.0`. `usage-error-unreadable-canon-yaml` `1.err` now reads
``parsing `<repo>/canon.yaml`: canon.yaml TierPolicy: …`` from the shared
fail-loud loader instead of the old inventory-only message.
`context-json-policy-summary` was re-blessed because `canon context --json`
now carries the `review` object, `capabilityVersion` 4, and the Subject
kind's optional `status_override` field. Every other case is unchanged.

**Website.** The docs site is redesigned as the "Grid Manual" (EN and KO
landing, re-themed Starlight docs, a build-time corpus figure).

## 0.11.0 release (experimental evidence binding)

Version 0.11.0 adds an **experimental, off-by-default** way to bind
evidence to files the team's own runner or agent already produced. Canon
still never runs a test. `canon evidence add --artifact <path>` binds any
repository file by sha256 (a trace, screenshots, an agent QA log);
`--report junit:<path>` or `--report cucumber:<path>` also parses the report,
records the case matched to the scenario and its outcome, and refuses a
`faithful` verdict over a failed case. Enforcement is a policy switch,
`experimental.evidence_binding` with `mode: off | warn | require`, a minimum
`strength` (`artifact` or `report`), and optional `case`/`lane`/`scope`
filters; `warn` only lists advisories after `canon gate check`. Nothing
changes for a repo that does not set it. The API and policy shape may change
while experimental.

## 0.10.0 release

Version 0.10.0 makes golden-path-only specs visible. A scenario can carry a
`@case:<value>` tag (`happy`, `failure`, `edge` in the base vocabulary;
`canon scenario new --case`), indexed onto `Scenario.case` by
`canon inventory sync`. The opt-in `spec_coverage.require_cases: [failure]`
then reports every feature surface (`<area>.<surface>`) whose in-scope
scenarios specify no failure path, and `canon subject status <id> shipped`
refuses on the same rule. Nothing changes until a repo sets
`require_cases`; when it does, untagged scenarios satisfy no case, so tag
existing failure-path scenarios first. `canon context` now also prints
`exclude_lanes` and `require_cases`.

## 0.9.1 fix

The `@journeykit/canon` launcher in 0.8.0–0.9.0 exited 0 regardless of the
native binary's result, so refusals such as a blocked `verifying → shipped`
or a red `canon gate check` looked successful to scripts, CI, and hooks.
0.9.1 passes the binary's exit code through again; a spawn failure or signal
death exits 1. Upgrade if anything relies on `canon`'s exit status.

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
