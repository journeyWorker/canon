---
status: Accepted
---

# Rust coding rules

- **Inputs:** the same four external guides lute's note uses, analyzed
  2026-10-23: Microsoft *Pragmatic Rust Guidelines* (ids `M-*`), Google *Rust
  Style Guide*, *Rust Design Patterns*, and Ed Page's *Rust Style* (ids
  `P-*`, `F-*`). The analyses live outside the repo, under
  `projects/lute/2026-10-23-rust-guidelines-research/` in the `journey-temp`
  artifact area. Lute's decisions (`journeyWorker/lute`,
  `docs/design/rust-coding-rules.md`, Accepted) are the starting point; this
  note keeps them where canon's code agrees and says so where it does not.
- **Scope:** the Rust code under `crates/` (11 crates, 315 source files,
  125k lines). canon's own `canon fmt` / doc lint check spec corpora and are
  not covered here.
- **Schedule:** see the 0.29/0.30 release-order note
  (`docs/superpowers/notes/2026-10-24-canon-0.29-0.30-release-order.md`,
  removed in 0.30; `git show 0977bce1:<path>`) and the 0.30 changes in
  `.canon/changes/canon-0-30-*`.
  0.29 restructures how `canon gate check`, `canon status` and evidence
  batches derive data from the corpus (§7). Once this note is `Accepted`, new
  0.29 code follows it, but existing code is not swept. 0.30 applies the
  rules to all the code in one behaviour-preserving pass (§9). `canon-gate`
  and the gate/status half of `canon-cli` are what 0.29 rewrites, so a sweep
  first would polish code about to be replaced.
- **Measured at:** HEAD `0642815c` (canon 0.28.0), toolchain 1.99.1,
  `cargo clippy --workspace --all-targets`. Re-measured at 0.29.0 for the
  sweep: the plain clippy run then had 114 default-level sites (105 src,
  9 tests), one of them a `never_loop` error; the earlier "7 warnings" figure
  was wrong.

## 0. Decisions

| Topic | Decision | Same as lute? |
|---|---|---|
| Lints | Declared once in `[workspace.lints]`; every crate sets `lints.workspace = true`. `clippy.toml` holds the configuration. The pedantic group is not enabled as a whole; the lints in §1 are. A new `clippy` CI job runs with `-D warnings` on a pinned toolchain. | yes, plus the CI job canon lacks |
| Module files | `foo/mod.rs` for every module with submodules. Already true: 10 `mod.rs`, 0 self-named. The lint keeps it that way. | yes |
| Visibility | Private, `pub(crate)`, `pub`. No `pub(super)` or `pub(in …)`. | yes |
| Error types | User mistakes are `CliError`s with stable codes. Rust error types are per failure domain, implement `std::error::Error`, and are written with `thiserror`. A `pub fn` does not return `Result<_, String>`. No `anyhow`. | differs: canon keeps `thiserror` |
| Panics | A panic means a bug. No `unwrap()` outside tests. `expect`/`unreachable!` say why the failure cannot happen. Poisoned locks propagate. No new `catch_unwind`. | yes |
| `unsafe` | `unsafe_code = "deny"`; each existing block gets `#[expect(unsafe_code, reason = "…")]` and a `SAFETY:` comment. | differs: canon has blocks today |
| Newtypes | Required for confusable values, units, invariants. Wire bytes do not change. | yes |
| Shared derived data | Data derived from the whole corpus is computed once per command and borrowed; a per-item query never refolds a kind (§7). | canon-specific (lute #173 analogue) |

## 1. Lints

### Configuration

- The workspace root `Cargo.toml` gets `[workspace.lints.rust]` and
  `[workspace.lints.clippy]`, and each of the 11 crates gets
  `[lints] workspace = true` (MS M-CARGO-WORKSPACE).
- Every lint is `warn` except `unsafe_code`, which is `deny`. The CI job's
  `-D warnings` turns warnings into failures; no crate-level `#![deny]` is
  added (*Design Patterns* "`#![deny(warnings)]`").
- canon's CI has no clippy job today (`.github/workflows/test.yml` runs
  `cargo test` and canon's own gate on `stable`, unpinned). 0.30 adds a
  `clippy` job pinned to 1.99.1, the toolchain these counts were taken on.
  When the pin moves, the lists below are re-checked in the same change.
- A local exception is `#[expect(lint, reason = "…")]`, never `#[allow]`
  (MS M-LINT-OVERRIDE-EXPECT), enforced by `allow_attributes*`.

The lint block is lute's, unchanged, so the two repos read the same:

```toml
[workspace.lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
redundant_lifetimes = "warn"
trivial_numeric_casts = "warn"
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "warn"
unused_lifetimes = "warn"

[workspace.lints.clippy]
# restriction
allow_attributes = "warn"
allow_attributes_without_reason = "warn"
as_pointer_underscore = "warn"
assertions_on_result_states = "warn"
clone_on_ref_ptr = "warn"
deref_by_slicing = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
map_err_ignore = "warn"
redundant_type_annotations = "warn"
renamed_function_params = "warn"
self_named_module_files = "warn"
semicolon_outside_block = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_result_ok = "warn"
# pedantic, selected
cast_lossless = "warn"
cast_possible_truncation = "warn"
cast_possible_wrap = "warn"
cast_sign_loss = "warn"
cloned_instead_of_copied = "warn"
enum_glob_use = "warn"
explicit_iter_loop = "warn"
implicit_clone = "warn"
manual_let_else = "warn"
map_unwrap_or = "warn"
needless_for_each = "warn"
redundant_closure_for_method_calls = "warn"
semicolon_if_nothing_returned = "warn"
uninlined_format_args = "warn"
unnested_or_patterns = "warn"
wildcard_imports = "warn"
```

`clippy.toml` at the workspace root:

```toml
disallowed-methods = [
  { path = "core::iter::Iterator::for_each", reason = "side effects belong in a `for` loop" },
  { path = "core::iter::Iterator::try_for_each", reason = "side effects belong in a `for` loop" },
]
```

`clippy::unwrap_used` is not in `[workspace.lints]`: integration-test helpers
under `crates/*/tests/` are outside clippy's `allow-unwrap-in-tests`. The
`clippy` job runs a second command over production targets only:

```sh
cargo clippy --workspace --lib --bins --locked -- -D warnings -D clippy::unwrap_used
```

### Measured cost

`src` includes inline `#[cfg(test)]` modules; `tests` is `crates/*/tests/`.

| Lint | Source | src | tests |
|---|---|---:|---:|
| `missing_debug_implementations` | MS M-PUBLIC-DEBUG | 263 | 0 |
| `cast_possible_truncation` | §5 panic policy | 151 | 1 |
| `cast_sign_loss` | §5 panic policy | 131 | 1 |
| `redundant_closure_for_method_calls` | pedantic | 102 | 34 |
| `assertions_on_result_states` | MS | 100 | 3 |
| `map_unwrap_or` | pedantic | 60 | 6 |
| `redundant_imports` | MS | 47 | 0 |
| `implicit_clone` | pedantic | 45 | 0 |
| `clone_on_ref_ptr` | MS M-AVOID-WRAPPERS | 23 | 19 |
| `allow_attributes_without_reason` | MS M-LINT-OVERRIDE-EXPECT | 30 | 4 |
| `wildcard_imports` | Google, MS M-NO-GLOB-REEXPORTS | 31 | 0 |
| `allow_attributes` | MS M-LINT-OVERRIDE-EXPECT | 30 | 0 |
| `redundant_type_annotations` | MS | 1 | 24 |
| `map_err_ignore` | MS M-FROM-ERROR | 20 | 0 |
| `manual_let_else` | pedantic | 15 | 0 |
| `unwrap_used` (production job) | Google, MS M-PANIC-ON-BUG | 14 | — |
| `unsafe_code` | MS M-UNSAFE | 10 | 0 |
| `unneeded_field_pattern` | MS | 9 | 0 |
| `disallowed_methods` (`for_each`) | Google, Ed Page F-COMBINATOR | 7 | 0 |
| `undocumented_unsafe_blocks` | MS M-UNSAFE | 5 | 0 |
| `unnested_or_patterns` | pedantic | 5 | 0 |
| `uninlined_format_args` | pedantic | 4 | 1 |
| `explicit_iter_loop` | pedantic | 4 | 1 |
| `cast_possible_wrap` | §5 panic policy | 3 | 2 |
| `semicolon_outside_block` | MS | 4 | 0 |
| `semicolon_if_nothing_returned` | pedantic | 3 | 0 |
| `as_pointer_underscore` | MS | 3 | 0 |
| `if_then_some_else_none` | MS | 2 | 0 |
| `enum_glob_use` | Google | 2 | 0 |
| `deref_by_slicing`, `needless_for_each`, `unnecessary_safety_comment` | | 1 each | 0 |
| the remaining lints in the block | | 0 | 0 |

Total: 1,113 sites in `src` plus 96 in `tests`, plus the 14 production
`unwrap()` calls. `canon-cli` holds 748 of the 1,113; `canon-ingest` 103,
`canon-gate` 66, `canon-model` 56, every other crate under 50.

### Not enabled, and why

| Lint or group | src sites | Reason |
|---|---:|---|
| `clippy::pedantic` as a group | 5,820 (all candidates) | Most of it is the lints below; the useful pedantic lints are enabled one by one. |
| `indexing_slicing` | 1,141 | Indexes whose bounds the code itself establishes; `get(..).expect(..)` adds noise without finding bugs. |
| `too_long_first_doc_paragraph` | 738 | Rustdoc is not published; the long module docs are deliberate design records. |
| `must_use_candidate`, `return_self_not_must_use` | 725 / 47 | Crates have no external consumers (§4). |
| `expect_used`, `panic`, `unreachable` | 611 / 85 / 8 | The sanctioned panic forms (§5). |
| `missing_errors_doc`, `missing_panics_doc` | 334 / 67 | Internal crates; conditions are documented where not obvious. |
| `doc_markdown` | 272 | Prose-heavy docs quote spec terms and ids that are not code. |
| `assert_is_empty` | 179 | Test-only style. |
| `format_push_string` | 146 | `write!` into a `String` returns a `Result` §5 would then need an `expect` for. Hot spots are found by profiling. |
| `result_large_err` | 43 | Not in the block, but on by default. 0.30 fixed it at the root rather than allowing it: `CliError` keeps its hot fields inline and boxes the optional detail, and the few other large `Err`s are boxed. |
| `cast_precision_loss` | 17 | Integer-to-float feeds metrics and report ratios; the narrowing directions are covered. |
| `clippy::cargo` | — | Crates are not published to crates.io; the npm package ships the binary. |
| `mod_module_files` | — | The opposite of §2. |

## 2. Module files

- A module with submodules lives in `foo/mod.rs` (Ed Page P-MOD).
  `clippy::self_named_module_files` enforces it. Today: 10 `mod.rs`, 0
  self-named; nothing moves.
- A `mod.rs` or `lib.rs` with submodules holds declarations, re-exports, the
  module's namesake item and the code that ties the submodules together.
  0.30 does not move existing logic out of `lib.rs`; new modules follow the
  rule.
- No `#[path]` attribute except for `build.rs` output (Ed Page P-PATH-MOD).

## 3. Visibility

- An item is private, `pub(crate)` or `pub` (Ed Page P-VISIBILITY).
- Today there are 20 `pub(super)` sites, all in `canon-cli`:
  `run/top.rs` 10, `skills.rs` 6, `dispatch.rs` 3 (plus one doc comment that
  names the form). 0.30 rewrites them to `pub(crate)`; a type newly reachable
  through a signature becomes `pub(crate)` too, never re-hidden.
- Reason: the crate is the unit of encapsulation (cargo, the crate split).
  A module that must be walled off from its crate is a reason for a crate.
- No clippy lint forbids `pub(super)`. 0.30 adds a `scripts/` check that
  rejects `pub(super)` and `pub(in` in `crates/*/src`; the `clippy` job runs
  it. `clippy::redundant_pub_crate` (nursery) contradicts the rule and stays
  off.

## 4. Error types

The crates ship only inside the `canon` binary (npm launcher); semver rules
for library errors (MS M-ERRORS-CANONICAL-STRUCTS) do not apply.

- **User mistakes are `CliError`s.** A bad flag, an unreadable config, a
  refused transition reach the user as `canon_cli::cli_error::CliError` with
  a stable code (`USAGE`, `CONFIG_UNREADABLE`, …) and the JSON envelope. A
  Rust error type never carries a user mistake past the CLI boundary
  unmapped.
- **Rust error types are for everything else:** I/O, malformed ledger
  records, tier backends, policy compilation, internal failures between
  crates.
  - One type per failure domain, named `<Domain>Error`. 44 exist today.
  - A `pub enum` stays the default: callers match variants to pick a code.
  - **Written with `thiserror`.** Lute has no error crate; canon already
    derives `thiserror::Error` on 33 of its 44 types across 9 crates, so the
    majority form wins and the remaining types move to it.
  - Every error type implements `Debug`, `Display` and `std::error::Error`,
    and `source()` returns the cause (`#[source]` / `#[from]`). Today 8 types
    lack both `Error` and `Display`: `CliError`, `AdoptionWriteError`,
    `BindError` (`canon-cli`), `ResolveError`, `AssembleError`, `LoadError`
    (`canon-vocab`), `LoadError` (`canon-plugin`), `ResolveError`
    (`canon-fmt`). 0.30 adds them.
  - Conversions `?` needs are `From` impls (`#[from]`), not repeated
    `map_err` (MS M-FROM-ERROR).
- **No `Result<_, String>` from a `pub fn`.** 176 `Result<_, String>`
  signatures exist; 65 are `pub fn`, almost all in `canon-cli`. 0.30 gives
  the `pub` ones typed errors. A private helper may return a `String` that
  becomes the user-facing message itself.
- **No `anyhow`.** It is declared in `[workspace.dependencies]` but no crate
  depends on it; 0.30 removes the declaration.

## 5. Panics, `expect` and `unwrap`

| Situation | Response |
|---|---|
| User input: flags, `canon.yaml`, spec files, briefs, policy | A `CliError` or a diagnostic. Never a panic. |
| Input from outside: ledger records, session logs, tier backends, git, file system | An `Err` of a typed error (§4). |
| A broken internal invariant (a bug) | A panic whose message states the invariant. |
| Tests | `unwrap()` is fine. |

- **No `unwrap()` outside tests.** 14 production sites today:
  `canon-cli/src/evidence.rs` 8, `canon-store/src/r2_tier.rs` 3, and one
  `selftest.rs` each in `canon-gate`, `canon-policy`, `canon-vocab`.
- **`expect` says why the failure cannot happen,** not what went wrong.
- **Narrowing `as` casts are checked:** `try_from(..).expect("…")`, or
  `#[expect(clippy::cast_possible_truncation, reason = "…")]` where the range
  is proven nearby (151 + 131 sites).
- **Poisoned locks propagate the panic:** `lock().expect("… lock
  poisoned")`, never `into_inner` on the `PoisonError`. 7 production sites
  recover today (memo caches in `inventory.rs`, `artifact_store.rs`,
  `git_tier.rs`; the approval lock in `dashboard.rs`); 6 more are in test
  modules, which may keep them.
- **`catch_unwind`:** the two production uses stay —
  `canon-policy/src/eval.rs` and `validate.rs` contain `cel` parser and
  interpreter panics on malformed expressions, the same justification as
  lute's `lute_cel`. No new use recovers from canon's own bug (MS
  M-PANIC-CONTINUATION).

## 6. Newtypes

Same rule as lute (Google "newtypes vs aliases", MS M-STRONG-TYPES):

1. **Confusable values** in one signature or struct: ids, digests, shas.
   Existing examples are `SubjectId`, `RoleId`, `RegimeKey`.
2. **A unit or coordinate system:** days vs seconds, byte offset vs column.
3. **An invariant:** private field, construction through `TryFrom`/`FromStr`,
   no infallible `From` from the underlying type.

Wire bytes do not change: a serialized newtype is `#[serde(transparent)]`,
and the ledger, conformance cases and `docs/cli/canon.cli.json` stay
byte-identical. 0.30 does not introduce newtypes across the codebase; the
rule binds new code and code being changed.

## 7. Shared derived data

canon's analogue of lute #173. `GateContext::load` reads each record kind
once per command, which is right; the cost is in what callers derive from it
per item. Measured by reading the code at `0642815c` (no benchmark exists
yet):

- `spec_coverage::latest_by_key` converts every record to JSON and hashes it
  on every call; it is called once per subject or scenario
  (subjects × scenarios, subjects × changes, scenarios × reviews).
- `review_gate`, `freshness` and `acceptance` filter all reviews, evidence
  and divergences once per scenario.
- `reverify::chore_gaps` recomputes `git_rev::code_tree` (temp index,
  `git add -A`, `write-tree`) per subject, although `gate.rs` already holds
  it.
- Evidence batch landing promotes each record separately, each one
  re-reading the whole evidence history.
- `history::evidence_history` deep-clones `ctx.evidence` and re-reads the
  archive per scenario; `inventory` resolves the vocabulary snapshot per
  tagged scenario.

Rules:

- A value derived from a whole record kind (latest-by-key folds, indexes by
  subject / scenario / change, the archive, the code tree, the vocabulary
  snapshot) is computed at most once per command, held by the command's
  context (`OnceCell` where it is lazy), and borrowed by callers.
- A per-item lookup goes through an index or a key-range query, not a scan of
  the whole kind.
- Caches return `&` or `Arc`, never a deep clone of the cached value.
- A design states its scale axes (subjects S, scenarios Sc, evidence E,
  reviews R, batch size B) and how time and memory grow when one grows 10×.
  A product of two axes is a design change, not a tuning task.
- A performance claim carries CPU (user+sys), peak RSS and input size, from
  a release build.

## 8. Not decided here

- Edition 2024 (MS M-LATEST-EDITION). It turns `std::env::set_var` in the
  ingest adapter tests into `unsafe`, so it is its own change.
- `imports_granularity`: nightly-only in rustfmt.
- Moving unit tests to `tests/` (MS M-INTEGRATION-TESTS).
- A faster hasher for internal maps (MS M-FAST-HASHER); only with a
  benchmark.

## 9. 0.30 sweep

Behaviour-preserving, on top of 0.29, with no other branch open. Each step is
its own commit and on its own passes `cargo test --workspace`, the byte-identity
comparison against 0.29.0 outputs, and the conformance cases. The sweep is the
canon change `canon-0-30-rust-sweep`; the pinned `clippy` CI job and the
`scripts/` visibility check are the separate change `canon-0-30-clippy-ci`,
attached to the sweep's last commit, because the job can only pass once the
sweep is done.

1. Lint configuration (§1), `[lints] workspace = true` in every crate,
   `clippy.toml`, `cargo clippy --fix` for the autofix lints, and the
   `#[expect(unsafe_code, reason)]` + `SAFETY:` on the `unsafe` sites (so
   `unsafe_code = "deny"` builds from the first commit).
2. Root causes in `canon-cli`: `CliError` detail boxed (`result_large_err`),
   exit codes as `u8` (the ~120 `as u8` casts).
3. The remaining sites by hand: casts, `Debug` impls, `allow` → `expect`,
   `Arc::clone`, `wildcard_imports`, `for_each`, the production `unwrap()`
   calls and poisoned-lock recovery; the 19 `pub(super)` sites to
   `pub(crate)` (§3).
4. Typed errors for the `pub fn … Result<_, String>` (52 at 0.29.0);
   `Display` and `Error` for the 8 types that lack them; crate-unique error
   names; drop `anyhow` from the workspace (§4).

Done means the `clippy` job, including the production `unwrap_used` command,
passes with the configuration above, and the counts in §1, §3, §4 and §5 are
zero.
