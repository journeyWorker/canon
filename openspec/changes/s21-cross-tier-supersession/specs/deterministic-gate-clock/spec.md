## ADDED Requirements

### Requirement: The gate authority reads the wall clock at exactly one CLI dispatch boundary, never inside a GateCheck
`canon gate check` and `canon gate task` SHALL call `Utc::now()` exactly once per invocation, at the CLI dispatch boundary (`canon-cli/src/gate.rs`), and thread the resulting value into `GateContext::load` as an explicit `now: DateTime<Utc>` parameter. No `GateCheck` implementation (including `StalenessCheck` and `ReleaseTrustCheck`) SHALL call `Utc::now()` internally; each SHALL read `ctx.now` from the loaded `GateContext` instead.

#### Scenario: A single gate run's clock-dependent checks agree on the same instant
- **WHEN** `canon gate check` runs against a ledger containing evidence a `StalenessCheck` age-check and a `ReleaseTrustCheck` age-check both evaluate
- **THEN** both checks evaluate against the identical `now` value — the one `GateContext.now` set for this invocation — never two independently-read, millisecond-apart wall-clock values

#### Scenario: GateContext::load requires an explicit now, never a default to the live clock
- **WHEN** `GateContext::load` is called
- **THEN** its signature requires a `now: DateTime<Utc>` argument from the caller — there is no code path inside `canon-gate` that reads `Utc::now()` on `GateContext`'s behalf

### Requirement: A time-bearing gate policy is deterministic — identical (ledger, policy, injected now) always produces an identical gate report
Given a fixed ledger corpus, a fixed `policy.yaml` (including one containing a CEL predicate over `age_days(...)` or any other time-bearing expression), and a fixed injected `now`, two independent `GateContext::load` + `GateCheck::run` passes SHALL produce byte-identical gate reports.

#### Scenario: Two gate-context loads at the same injected now produce byte-identical reports
- **WHEN** a `policy.yaml` with an `age_days(...)` CEL predicate is loaded twice via `GateContext::load` at the SAME injected `now`, and every registered `GateCheck` runs over each loaded context
- **THEN** the two resulting gate reports are byte-identical — same violations, same order, same content

#### Scenario: A time-bearing verdict changes only when the injected now changes, never on repeated evaluation at a fixed now
- **WHEN** the same fixed ledger and policy are evaluated three times in a row, all three passes using the identical injected `now`
- **THEN** all three passes report the identical verdict for every time-bearing check — no run-to-run drift

### Requirement: A time-bearing policy predicate is load-bearing — its verdict changes when the injected clock crosses its threshold
The gate suite SHALL include at least one test proving a time-bearing CEL predicate (`age_days(...)` feeding `staleness.max_commits_behind` or a release `trust_required` threshold) actually changes the reported verdict when the injected `now` moves across the predicate's threshold, holding all ledger content fixed — proving the injected clock is consumed, not merely plumbed through inertly.

#### Scenario: Evidence ages from clean to a staleness/trust violation purely by advancing the injected now
- **WHEN** a fixed evidence record and a fixed `age_days`-based policy are evaluated once with an injected `now` BEFORE the policy's threshold and once with an injected `now` AFTER it, all other inputs held fixed
- **THEN** the first pass reports no violation for that record and the second pass reports the expected violation (`stale-evidence` or `trust-below-required`) — the verdict tracks the injected clock, not any change in the ledger or policy source

### Requirement: Injecting the clock preserves connector-never-authority byte-identity
Injecting `now` at the CLI boundary SHALL NOT change `canon gate check`'s verdicts for any existing corpus at a fixed clock reading compared to the pre-injection behavior evaluated at that same instant — the existing byte-identity acceptance tests (gate verdicts with/without a plugin sync, with/without a plan import) SHALL continue to pass unmodified.

#### Scenario: Plugin-sync and plan-import byte-identity tests remain green after clock injection
- **WHEN** the existing `canon-cli` acceptance tests asserting gate-verdict byte-identity with/without a plugin sync, and with/without a plan import, are re-run after this change lands
- **THEN** both continue to pass with no change to their expected assertions — clock injection changed WHERE `now` comes from, not WHAT any check decides given a fixed `now`
