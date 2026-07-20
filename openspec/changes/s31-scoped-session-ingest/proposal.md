# Why

Dogfooding S3 on a live machine (2026-07-14) proved the session-ingest
loop works — 5,040 sessions / 432,673 events, 0 malformed — and exposed
four defects that make it impractical and aim it at the wrong corpus:

1. **The watermark gate is source-granular** (S3 §3): one growing
   transcript (the operator's live conversation) dirties the whole
   `omp` source, so every pass re-reads, re-digests, re-parses 4,667
   files / ~304k rows and re-attempts ~300k dedup-rejected writes. A
   "steady-state" pass takes 15+ minutes; `--watch` is unusable.
2. **Persist is one `INSERT` round trip per record** (~550 rows/s):
   the full first pass took 23 minutes, nearly all of it in
   `PgTier::write_row`.
3. **The scan corpus is the whole machine** (`$HOME`), but canon's
   unit of accounting is the PROJECT. A repo's main worktree plus its
   linked git worktrees are one project and should be ingested and
   aggregated as one; other projects' sessions are noise (and bulk).
4. **Only `token_usage` events are captured.** The operator's actual
   goal is accumulation-driven improvement: USER DIRECTIVES (what the
   operator asked, corrected, and re-corrected) are the raw material
   the S6/S7 flywheel should eventually distill — and today they are
   discarded at parse time.

# What Changes

- File-granular watermark: a pass re-parses only new/changed files
  (cursor shape unchanged — only the gate's all-or-nothing comparison
  is replaced by a per-file diff).
- Batched tier writes: `Tier::write_batch` (default = loop) with a
  chunked multi-row `INSERT … ON CONFLICT` override in `PgTier`.
- Project-scoped ingest BY DEFAULT: current repo root + its linked
  `git worktree` roots = one project; cwd-partitioned adapter roots
  (omp, claude-code) are pruned before read/digest, the rest filter
  rows post-parse. `--all-workspaces` restores the machine-wide scan.
  `Session` records gain `workspace_key`/`workspace_label`/
  `project_key` so queries aggregate by project.
- User-directive capture: adapters emit user-role messages as
  `DirectiveRow`s; normalize maps them to `Event` records with
  `label: "user_directive"` (full text in `detail.text`), queryable
  via `canon query --kind event`. Distillation into strategy memory is
  a named follow-up, NOT this change.

# Impact

- Affected specs: `session-ingest-scope` (new capability).
- Affected code: `crates/canon-store` (tier trait + pg tier + cursor
  gate helper), `crates/canon-ingest` (adapter contract + 4 adapters +
  normalize), `crates/canon-cli` (ingest pass, flags, summary),
  `canon/skills/canon-session-ingest/SKILL.md`, website CLI page.
- Not affected: record-kind closure (reuses `Session`/`Run`/`Event`),
  gate/trust surfaces, report boundary (s28 wording still true).
