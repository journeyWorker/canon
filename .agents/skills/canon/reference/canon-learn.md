# canon-learn

canon's role-namespaced strategy memory turns accumulated verdict
trajectories into retrievable, promotable insights. Two tiers:

- **Trajectories (warm, local).** Raw verdict-keyed rows under
  `<repo>/.canon/learn/trajectories/`, written by ingest. Rebuildable,
  gitignored, never hand-committed.
- **Strategies (durable, git).** Distilled, reviewable
  `<repo>/.canon/strategies/<role>/<id>.md` files — the tier a promotion
  writes into and a demotion soft-flags.

## Rebuilding strategy memory

Strategy memory is re-derived from the current trajectory set as part of
`canon ingest artifacts` (see `canon-artifact-ingest`): after it
persists a repo's verdict trajectories, it re-derives the distilled
strategies. This is NON-DESTRUCTIVE — a rebuild never drops a
hand-promoted strategy the trajectory rows don't touch. There is no
standalone `canon learn` rebuild command; the only subcommand is
`promote`.

# `canon learn promote <strategy_id> [--repo <dir>] [--evaluation <path> --approval <path>] [--signature <path>] [--dry-run]`

The production activation path is a signed, paired-evaluation workflow:

1. Run `canon ingest artifacts`. Ingest resolves covering verdicts and rebuilds
   newly distilled strategies as `lifecycle: quarantined`.
2. Check eligibility with the configured `occurrence` or `crn` gate. This is
   only a prerequisite; occurrence alone MUST NOT activate a strategy.
3. An honest external evaluator writes a `PromotionEvaluation` JSON bundle.
   It MUST bind the candidate's id, version, quarantined lifecycle, strategy
   digest, source trajectory ids, baseline and candidate result digests, and
   non-empty corpus/evaluator/context/policy/model/tool provenance.
4. Require paired evidence: baseline and candidate digest arrays MUST be
   non-empty, equal-length, disjoint, and duplicate-free. The evaluation's
   finite `paired_metrics` MUST include `pairs`, `uplift`, and `regressions`;
   `pairs` MUST equal the array length, `uplift` MUST be strictly positive,
   `regressions` MUST be zero, `passed` MUST match those derived checks, and
   `decision` MUST be `pass`, `promote`, or `approved`. Missing or unknown
   metrics are rejected; the repository currently publishes **no measured
   uplift result**, so this command is not evidence that retrieval improves
   agent outcomes.
5. Produce the unsigned approval payload:

   ```bash
   canon learn approve <strategy_id> --evaluation evaluation.json \
     --principal alice --repo .
   # `request` is an exact alias:
   canon learn request <strategy_id> --evaluation evaluation.json \
     --principal alice --repo .
   ```

   The JSON output contains the evaluation digest, RFC3339 `approved_at`,
   subject `strategy:<strategy_id>`, namespace
   `canon-learning-approval-v1`, and `payload_hex`. `approve`/`request` never
   read a private key and never set `verified`.
6. Decode `payload_hex` to the exact canonical JSON bytes. The signed object
   is:

   ```json
   {
     "namespace": "canon-learning-approval-v1",
     "subject": "strategy:<strategy_id>",
     "project": null,
     "artifact_sha": "<evaluation_digest>",
     "run_id": null,
     "surface": [],
     "effects": [],
     "actor": "<principal>",
     "timestamp": "<RFC3339 nanosecond timestamp>"
   }
   ```

   For rollback, the corresponding signed object uses subject
   `strategy:<id>:rollback:<sha256(reason)>`, `artifact_sha` equal to that
   reason digest, and the rollback actor/timestamp. Sign the decoded bytes
   externally with an SSH key using the `canon-learning-approval-v1`
   namespace (for example, `ssh-keygen -Y sign -n canon-learning-approval-v1
   -f <private-key>`). Put the armored detached signature in
   `PromotionApproval.signature` and set `signer_key` to the same principal.
   The approval JSON has exactly these fields: `schema_version`,
   `candidate_strategy_id`, `evaluation_digest`, `approver_identity`,
   `approver_role` (`human`), `verified` (non-authoritative display data),
   `approved_at`, optional `signature`, optional `signer_key`, and
   `integrity_digest`.
7. Configure the verifier in the repository policy:

   ```yaml
   approval:
     allowed_signers: .canon/allowed_signers
   ```

   The path is policy-pinned (relative paths resolve from the repository).
   The verifier runs `ssh-keygen -Y verify` against that file; an absent,
   unreadable, or non-matching signer fails closed. A detached signature is
   authentication of the configured principal, not proof that the principal
   is a human or is otherwise authorized outside this policy.
8. Activate only with both evidence files:

   ```bash
   canon learn promote <strategy_id> --repo . \
     --evaluation evaluation.json --approval approval.json
   # Or attach an armored signature file and recompute the approval digest:
   canon learn promote <strategy_id> --repo . \
     --evaluation evaluation.json --approval approval.json \
     --signature approval.sig
   ```

   `--evaluation` and `--approval` MUST be supplied together. A quarantined
   candidate without valid paired evaluation, policy-pinned SSH verification,
   and human approval remains quarantined. `--dry-run` previews the render but
   does not activate or write.

The command resolves the repo via the nearest-`canon.yaml`-ancestor walk,
evaluates the role gate before any write, and runs advisory lints
(content-length ceiling and literal-absolute-path rejection). There is no
`--force`. Promotion re-renders the whole git-tier file with YAML front matter
(`status`, `regime_key`, `role`, `title`, `source_trajectory_ids`,
`recorded_at`) plus evaluation and approval digests; a re-promote does not
preserve manual edits.

An unknown `strategy_id` fails nonzero. A candidate's lifecycle defaults to
`quarantined` when newly distilled and quarantine is excluded from retrieval.
The old public writer is retained only for already-`active` items; it is not a
way to activate new or legacy candidates.

### The eligibility gates: `crn` vs `occurrence`

A role's eligibility gate is a `canon.yaml` `learn:`-section choice:

```yaml
# canon.yaml
learn:
  promotion:
    dev:
      mode: occurrence
      n_min: 8            # optional — defaults to 5
      window_days: 14     # optional — defaults to 30
    sim:
      mode: crn
  demotion:
    hard_delete: false                 # optional — soft-flag is the default
    strategies_root: .canon/strategies # optional
```

A role with no explicit `promotion.<role>` entry defaults to
`mode: occurrence, n_min: 5, window_days: 30`.

- **`occurrence`** — for roles whose domain does NOT support deterministic
  replay. Within the trailing window, at least `n_min` resolved `Success`
  trajectories for the same `regime_key` are required and no `Failure` or
  `RolledBack` trajectory may be present. A contradiction resets the count;
  it is never averaged away. This eligibility does not replace paired
  evaluation or signed approval.
- **`crn`** — for roles that CAN run a deterministic simulator. A paired
  common-random-number statistical eligibility gate uses the same
  `crn:config=<label>` and `crn:panel=<index>` panels across compared
  configurations. Insufficient panels/noise evidence is not a pass. CRN
  eligibility still does not replace the external `PromotionEvaluation` and
  signature.

Both gates read resolved `verdict_record.outcome`, not raw verdict rows.
`Pending` trajectories neither corroborate nor contradict. `canon ingest
artifacts` is therefore always first:

```bash
canon ingest artifacts
canon learn promote <id> --evaluation evaluation.json --approval approval.json
```

### Demotion versus authenticated rollback

A later ordinary contradictory `Failure`/`RolledBack` trajectory demotes an
already-active strategy when a non-dry-run promotion observes it. The default
is append-only soft-flagging of the existing git file with `status: demoted`
and `reason:`; `demotion.hard_delete: true` deletes that file instead. This
ordinary contradiction path is not an approval and does not create a rollback
record.

An operator-requested rollback is a distinct authenticated action:

```bash
canon learn rollback <strategy_id> --reason "<reason>" --actor alice \
  --signature-file rollback.sig --approved-at 2026-10-02T12:00:00Z \
  [--contradicting-trajectory-id <trajectory_id>] --repo .
```

`--signature-file` is an armored SSH detached signature over the
`canon-learning-approval-v1` payload bound to
`strategy:<id>:rollback:<sha256(reason)>`, actor, and `--approved-at`.
`actor` MUST be listed in policy-pinned `approval.allowed_signers`; verification
uses that file and `ssh-keygen -Y verify`. Successful rollback writes durable
provenance, sets lifecycle `rolled_back`, and records the optional contradicting
trajectory. Missing policy, signer, timestamp, or signature fails closed.

### What retrieval can and cannot serve

Retrieval serves only exact role/regime matches and excludes demoted,
quarantined, rejected, and rolled-back strategies. Legacy rows with
`lifecycle: null` remain readable for backward compatibility, but they have no
quarantine/evaluation/signature provenance and MUST NOT be treated as proof of
the current activation workflow. Retrieval is fail-soft and advisory; it does
not upgrade legacy rows or establish measured uplift.

## How verdicts feed scoring

Verdicts arrive from `canon ingest artifacts` (see
`canon-artifact-ingest`), persisted regime-keyed
(`<role>/<repo>/<area>/<hash>`). Each verdict scores its covering
trajectory into `Success` / `Failure` / `RolledBack` / `Pending`; the
eligibility gate reads a regime's accumulated resolved verdicts, while the
paired evaluation and signed approval decide whether a quarantined strategy
may become active. A contradicting trajectory for an already-promoted
strategy triggers ordinary demotion; an explicit rollback is separate and
authenticated.

## Reading a demoted or rolled-back strategy

An ordinary contradicting trajectory demotes an active strategy append-only:
the default soft-flag merges `status: demoted` + `reason: <text>` into the
existing front matter and leaves the body byte-unchanged. Set
`demotion.hard_delete: true` to delete the file instead. A strategy demoted
before it reached the git tier has no file to flag — that is not an error.
`canon retrieve` skips demoted and rolled-back strategies. An authenticated
rollback is different: it records signed operator provenance and sets the
stored lifecycle to `rolled_back`.

## The flywheel

```
ingest → resolved trajectories → quarantined strategies
   ↑                                  │
verdicts                 eligibility + paired evaluation + SSH approval
                                      │ (activate)
                                      ▼
                      active git strategies → advisory retrieve
```

## What this skill does NOT cover

- Retrieval at dispatch time (`canon retrieve`, the pre-dispatch hook) —
  see the `canon-retrieve` skill.
- Producing the verdict trajectories promotion reads — see the
  `canon-artifact-ingest` skill.