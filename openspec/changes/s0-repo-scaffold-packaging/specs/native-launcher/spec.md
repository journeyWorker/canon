## ADDED Requirements

### Requirement: Platform binary resolution
The `canon` npm alias and `@canon/cli` JS launcher SHALL resolve, on
invocation, to the platform- and architecture-matched `@canon/core-<platform>`
optionalDependency binary (or a local workspace dev build, when present) and
execute it, forwarding `process.argv.slice(2)` and the child's exit code
unchanged — adapting the vendored upstream launcher's
`resolveTargetPackageName`/search-path order (D2).

#### Scenario: Supported platform runs the packaged binary
- **WHEN** `bunx canon --version` runs on macOS arm64 or Linux x64 with
  `@canon/core-<platform>` installed as a resolved optionalDependency
- **THEN** the launcher execs that platform's `canon` binary, the binary's
  stdout/stderr pass through unmodified, and the launcher's own exit code
  equals the binary's exit code.

#### Scenario: Workspace dev build takes priority over the packaged binary
- **WHEN** the launcher runs inside the canon monorepo and
  `target/release/canon` (or `target/<triple>/release/canon`) exists
- **THEN** the launcher execs that local build instead of any installed
  `@canon/core-<platform>` package, so `bun run dev` iteration never requires
  a fresh npm publish.

#### Scenario: Unsupported platform fails with an actionable error
- **WHEN** `canon` runs on a platform/arch combination with no matching
  `@canon/core-<platform>` package and no local dev build present
- **THEN** the launcher exits non-zero and prints the platform/arch it
  detected plus the exact optionalDependency package name it looked for,
  never a bare stack trace or "command not found".

#### Scenario: Self-reference is never executed
- **WHEN** the launcher's own resolved argv[1] path (a symlinked bin shim)
  appears among its own binary search-path candidates
- **THEN** the launcher skips that candidate (realpath-compared) rather than
  re-invoking itself, preventing an infinite recursion / fork bomb.
