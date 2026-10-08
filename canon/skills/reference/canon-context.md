# canon-context

`canon context` is a capability QUERY over the same schema/policy/
vocabulary registry `canon format`/`canon gate` validate against — never
validation itself. It answers "what CAN I author in this repo, and how?":
record kinds + their envelope fields, enum domains, join-key grammars,
partition layout, policy-derived evidence requirements, the typed
authoring vocabulary, the review vocabulary (`review`: finding severities
and dispositions, Review fields, the active `require_review`; see
`canon-review`), and the CEL binding surface — folded into one
deterministic authoring surface.

## `canon context [--repo <dir>] [--json]`

```bash
canon context                 # compact human outline (prompt-injectable)
canon context --json          # the full machine-readable surface
canon context --repo ../other # resolve a specific repo's surface
```

## Reproducible context packs and prompt bundles

Use an explicit, repository-relative JSON manifest when a run needs a
replayable snapshot. The manifest selects files; Canon copies their bytes into
an immutable content-addressed registry and records only metadata and digests
in the pack manifest.

```bash
canon context-pack create --manifest .canon/context-manifest.json
canon context-pack create --manifest .canon/context-manifest.json --repo ../my-repo --json
canon context-pack show sha256:<pack-digest> --repo . --json
canon context-pack verify sha256:<pack-digest>
```

`create` prints only the pack id by default, so captured content is never
written to concise terminal output. `--json` prints the pack manifest,
including selected-path metadata and content digests. `show` verifies the
manifest and immutable content objects before returning it; `verify` exits
nonzero when the pack is missing, stale, or tampered. Manifest paths must be
relative to the resolved repo, remain inside it, and contain no symlink;
missing or unsafe paths are rejected by the context-pack module.

## Provider-neutral adapter responses

An execution provider may return a versioned response envelope for Canon to
validate. Validation is read-only: `canon` never starts a provider, interprets
provider-specific extension payloads, or enforces declared capabilities.

```bash
canon adapter validate --response response.json
canon adapter validate --response response.json --repo . --json
```

The envelope has `protocol_version: 1`, required `run_id`, `provider`,
`model`, and `context_pack_id`, a closed status vocabulary
(`succeeded`/`failed`/`aborted`), and strict filesystem/network/secrets
capability vocabularies. Evidence references are safe, relative references
with `sha256:<64-hex>` digests. Telemetry timestamps must be RFC3339 with
`ended_at >= started_at`, and counts/cost are nonnegative.

Core fields are closed and unknown fields are rejected. Provider-specific JSON
belongs only under opaque `extensions`; successful JSON output contains the
normalized core summary and extension key names, never extension payloads.
`--repo` can verify the referenced context pack; file-only validation reports
`context_join_verified: null`. A valid declaration is not a sandbox:
providers MUST enforce filesystem, network, and secret capabilities before
execution.

Version prompt inputs separately when several runs should share the same
prompt files:

```bash
canon prompt register --name reviewer --version v1 \
  --manifest .canon/reviewer-prompt.json
canon prompt show --name reviewer --version v1 --json
```

Prompt registration uses the same immutable content registry. `prompt show`
verifies the selected bundle and returns its metadata/digests; neither concise
output form includes captured prompt bytes.

- **A capability query, never validation:** `canon context` ALWAYS exits
  `0` with the full surface — even when `canon format --check` or
  `canon gate` would report diagnostics against the same repo. It reads
  only the schema/policy/vocabulary registry, never the corpus or the
  evidence ledger.
- `--repo` resolves through the same nearest-`canon.yaml`-ancestor walk
  `canon format`/`canon gate`/`canon retrieve` use: omitted or `.` walks up
  from cwd to the project root; any other explicit dir is used as-is.
- `--json` renders the full surface; the default renders a compact per-
  section outline. Both project from the identical resolution (byte-stable
  across runs).

## The surface sections

- **`kinds`** — each record kind's `{schema_version, envelope_fields,
  partition}`.
- **`enums`** — each enum domain's members (verdicts, statuses, lanes,
  roles, polarity, …).
- **`joinKeys`** — the join-spine grammar string per key (`regime_key`,
  `session_id`, `task_id`, …).
- **`policy`** — the resolved `policy.yaml`-derived evidence/trust
  requirements per kind.
- **`vocab`** — the typed authoring vocabulary (directive/enum/evidence-
  kind index) + its content-hash version.
- **`cel`** — the per-kind CEL binding surface: the bindable
  `record.<field>` names + types and the callable function allowlist a
  `policy.yaml` `applies_when:`/predicate may reference. The agent-facing
  answer to "what can a CEL predicate for this kind reference?"

## Single source of truth

`canon context` never re-derives a schema/policy/enum list of its own — it
resolves through the same shared schema, policy, and vocabulary loaders
the validators use. A schema edit propagates to `canon context`'s output
AND `canon format`'s diagnostics from one edit site, never two.

## Reading an "expected one of: …" validator error

When `canon format --check` rejects an out-of-domain enum value it emits:

```
`not-a-real-kind` is not a valid value for `kind` of `run` (expected one of: run, review, divergence, …)
```

The `expected one of:` member list is the SAME enum domain `canon context
--json`'s `enums.<field>` reports for that kind — so to fix the artifact,
pick a member `canon context` lists. Both read the member list off the
same schema, no hand-maintained second copy.

## What this skill does NOT cover

- `canon format --check`'s full violation taxonomy — see `canon-fmt`;
  this skill only covers reading its enum diagnostic against the surface.
- Authoring a typed task atom / handoff body — see
  `canon-vocab`; `canon context`'s `vocab`/`cel` sections
  tell you what's declarable, not how to declare it.
- `policy.yaml` CEL authoring semantics — see `canon-policy`; this skill
  only surfaces the bindable identifier/function set per kind.