# s38 evidence-bearing-memory — tasks

## 1. canon-ingest — evidence surface on ArtifactEvent

- [ ] 1.1 `ArtifactJoinKey::as_str` — the concrete scenario/handoff/task
      id string, so a trajectory can name its artifact.
- [ ] 1.2 `ArtifactEventKind::label` — one human phrase per variant.
      These strings land inside retrieved agent guidance, so they read as
      prose, never as enum names.
- [ ] 1.3 `ArtifactEvent::evidence_line` — the kind label plus the most
      salient free text the event actually carries (`detail.detail`,
      `detail.pin`, `detail.evidence`, `detail.reason`, in that priority
      order), falling back to `status`/`state` and then the label alone.
      NEVER serializes the whole `detail` blob, normalizes whitespace,
      and caps the quoted text so one pathological record cannot bloat
      every retrieved strategy. Depends on 1.2.

## 2. canon-cli — trajectories carry that evidence

- [ ] 2.1 `canon ingest artifacts` stops collapsing its accumulator to
      `(RegimeKey, VerdictRow, DateTime)` and discarding the event; the
      salient text is carried through on a named local struct.
      Depends on 1.3.
- [ ] 2.2 A trajectory's `task` names its concrete artifact plus the
      deduped kind labels, and its `context` is the deduped evidence
      lines. `regime_hash` derives from `join_key`, so one regime group is
      one join key — asserted rather than assumed, degrading to something
      honest if a group ever spans keys. Both strings deterministic, so
      `trajectory_content_digest` idempotence survives. Depends on 2.1.

## 3. canon-store — deterministic supersession

- [ ] 3.1 `fold_latest_by_key` gains a `schema` accessor and orders by
      `(at, schema, digest)`. Doc explains WHY schema sits between them:
      byte-stable `at` lets two generations tie, and a lexicographic
      digest then picks arbitrarily per row.
- [ ] 3.2 Every call site threads the record's real `envelope.schema` —
      never a constant. Sites: canon-report `divergence`, canon-gate
      `ledger`/`staleness`, canon-plugin `project`, canon-cli
      `subject`/`query`/`inventory`/`artifact_ingest`, canon-store
      `tests/pg_tier_live`. Depends on 3.1.
- [ ] 3.3 Fold tests: equal `at` + differing `schema` → greater schema
      wins, constructed so the LOWER schema carries the GREATER digest
      (otherwise the test proves nothing); equal `at` + equal `schema` →
      the digest tie-break still holds; strictly greater `at` still wins
      even with a lower schema. Depends on 3.1.

## 4. canon-model — Task is a new generation

- [ ] 4.1 `Task`'s `Envelope.schema` → `2` (it gained `depends_on`, and
      `Task` is a byte-stable-`at` kind). Every other kind stays at `1`;
      the bump's doc records that the rule is "bump when a kind's records
      can tie on `at`", not "bump on every field addition".
      Depends on 3.1.
- [ ] 4.2 `fixtures/well-formed/task.json` and the `"schema": 1`
      expectations covering Task updated; `cargo xtask write` regenerates
      `schemas/task.schema.json`. Depends on 4.1.

## 5. canon-ingest + canon-cli — a parser change forces a re-parse

- [ ] 5.1 `PlanAdapter::parse_version() -> u32`, required rather than
      defaulted so a new dialect decides deliberately. Both shipped
      dialects return `2` — each gained dependency extraction, so their
      output for an identical source genuinely changed.
- [ ] 5.2 `plan_source_cursor_id` folds the parse version into the cursor
      identity, so a bump yields a different id, finds no prior cursor,
      and re-parses exactly as if the source had been edited.
      Depends on 5.1.
- [ ] 5.3 Report whether the session-ingest cursor has the identical
      exposure. Investigation only — fixing it belongs to its own change.

## 6. Verification

- [ ] 6.1 `cargo test --workspace` green, including the generated-output
      drift check.
- [ ] 6.2 `canon retrieve` on this repo returns strategies naming real
      scenarios and quoting real reviewer/divergence prose — no plumbing
      description, no `detail` JSON blob. Depends on 2.2.
- [ ] 6.3 `canon ingest plans` re-parses on the first run after this
      lands instead of reporting `skipped unchanged`, and `canon query
      --kind task` then shows `depends_on` on EVERY row the corpus
      declares one for, with no `touch` required. Depends on 4.1 and 5.2.
