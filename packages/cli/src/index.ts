#!/usr/bin/env node
// `@journeykit/canon`'s launcher: platform detection, binary resolution,
// argv/exit-code passthrough. Adapted (design D2) from the vendored
// upstream launcher's `resolveTargetPackageName` / search-path order /
// self-reference guarding — this file is dense, adversarially-tested logic
// the upstream launcher already hardened against real install-topology
// bugs; re-deriving it risks reintroducing them. The upstream launcher's
// libc probing is gone: the Linux binaries are static musl and run on any
// libc, so the launcher never asks which one the host has.
//
// Deviation from the upstream launcher (native-launcher spec, "Workspace dev build
// takes priority over the packaged binary"): the search-path order here
// puts the workspace dev build FIRST (`target/<triple>/release/`, then
// `target/release/`), so `bun run dev` iteration inside the canon monorepo
// never silently picks up a stale installed `@journeykit/canon-core-<platform>`
// package. The upstream launcher's own order happens to work the same way in practice
// (its monorepo-dev candidate is rarely populated), but canon makes the
// priority explicit and structural rather than incidental.
import { spawnSync } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { constants as osConstants } from "node:os";
import { resolve, join, basename } from "node:path";
import { fileURLToPath } from "node:url";

const binaryName = process.platform === "win32" ? "canon.exe" : "canon";

const currentDir = fileURLToPath(new URL(".", import.meta.url));
const dirName = basename(currentDir);
// In npm/bun install: currentDir = .../node_modules/@journeykit/canon/dist/
//   cliDir = .../node_modules/@journeykit/canon/
//   scopeDir = .../node_modules/@journeykit/   (scoped: siblings are
//              node_modules/@journeykit/canon-core-<platform>/)
// In monorepo dev (dist): currentDir = .../packages/cli/dist/
//   cliDir = .../packages/cli/
//   scopeDir = .../packages/
// In monorepo dev (src): currentDir = .../packages/cli/src/
//   cliDir = .../packages/cli/
//   scopeDir = .../packages/
const isSubDir = dirName === "dist" || dirName === "src";
const cliDir = isSubDir ? resolve(currentDir, "..") : currentDir;
const scopeDir = resolve(cliDir, "..");
const workspaceRoot = resolve(scopeDir, "..");

// Shipped platforms resolve to a real `@journeykit/canon-core-<platform>`
// package: macOS arm64, Linux x64 and arm64 (static musl binaries, the host
// libc is not asked), Windows x64. Every other platform/arch combination
// returns null and hits the "Unsupported platform fails with an actionable
// error" scenario — adding a target later is a new `packages/core-<platform>/`
// + a CI matrix row, not a change to this function's shape (design
// non-goals). scripts/check-release-workflow-safety.py reads the `core-`
// names below and fails a release matrix row this function cannot resolve.
function resolveTargetPackageName(): string | null {
  const arch = process.arch;

  if (process.platform === "darwin") {
    if (arch === "arm64") return "core-darwin-arm64";
    return null;
  }

  if (process.platform === "linux") {
    if (arch === "x64") return "core-linux-x64";
    if (arch === "arm64") return "core-linux-arm64";
    return null;
  }

  if (process.platform === "win32") {
    if (arch === "x64") return "core-win32-x64";
    return null;
  }

  return null;
}

// The workspace-dev-build search path below wants the full Rust target triple
// regardless of whether a published platform package exists for it. Linux dev
// builds are looked for in the musl directory first (the target the release
// builds), then in `target/release/`.
function resolveRustTargetTriple(): string | null {
  const arch = process.arch;

  if (process.platform === "darwin") {
    if (arch === "arm64") return "aarch64-apple-darwin";
    if (arch === "x64") return "x86_64-apple-darwin";
    return null;
  }

  if (process.platform === "linux") {
    if (arch === "arm64") return "aarch64-unknown-linux-musl";
    if (arch === "x64") return "x86_64-unknown-linux-musl";
    return null;
  }

  if (process.platform === "win32") {
    if (arch === "arm64") return "aarch64-pc-windows-msvc";
    if (arch === "x64") return "x86_64-pc-windows-msvc";
    return null;
  }

  return null;
}

const targetPackage = resolveTargetPackageName();
const rustTargetTriple = resolveRustTargetTriple();
const searchPaths: string[] = [];

// 1. Workspace dev build — always wins over an installed package, so
//    `cargo build` iteration never needs a fresh npm publish (native-launcher
//    spec, "Workspace dev build takes priority over the packaged binary").
if (rustTargetTriple) {
  searchPaths.push(join(workspaceRoot, "target", rustTargetTriple, "release", binaryName));
}
searchPaths.push(join(workspaceRoot, "target", "release", binaryName));

// 2. This package's own bundled bin/ (used when the launcher ships its own
//    binary directly rather than through an optionalDependency).
searchPaths.push(join(cliDir, "bin", binaryName));

// 3. The resolved `@journeykit/canon-core-<platform>` optionalDependency,
//    in every location a package manager might have placed it. The npm
//    package is SCOPED under `@journeykit`; `resolveTargetPackageName`
//    returns the short `core-<platform>` suffix, which doubles as the
//    monorepo `packages/<dir>` directory name, so both the scoped package
//    name and the dir are built from that one suffix here.
const scope = "@journeykit";
// Bare package name within the scope dir — on a scoped install scopeDir
// already IS node_modules/@journeykit/, so the sibling is just this name
// (e.g. "canon-core-darwin-arm64"). Nested/hoisted node_modules paths that
// start from a node_modules root re-add the "@journeykit" scope segment.
const platformBareName = targetPackage ? `canon-${targetPackage}` : null;
const platformPkgName = platformBareName ? `${scope}/${platformBareName}` : null;
if (targetPackage) {
  searchPaths.push(
    // Monorepo development: prebuilt binary staged into packages/core-<platform>/bin
    join(workspaceRoot, "packages", targetPackage, "bin", binaryName),
  );
}
if (platformBareName) {
  searchPaths.push(
    // npm/bun install: sibling scoped package
    //   node_modules/@journeykit/canon-core-<platform>/bin/...
    join(scopeDir, platformBareName, "bin", binaryName),
    // Nested node_modules: non-hoisted / pnpm
    //   .../@journeykit/canon/node_modules/@journeykit/canon-core-<platform>/bin/...
    join(cliDir, "node_modules", scope, platformBareName, "bin", binaryName),
    // Hoisted edge cases
    join(scopeDir, "node_modules", scope, platformBareName, "bin", binaryName),
    join(workspaceRoot, "node_modules", scope, platformBareName, "bin", binaryName),
  );
}

function tryRealpath(p: string): string {
  try {
    return realpathSync(p);
  } catch {
    return p;
  }
}

// Paths that would re-enter this wrapper if executed - using any of these as
// the "real" binary causes infinite recursion (a fork bomb). We compare by
// realpath so symlinks (e.g. npm/bun bin shims) are dereferenced.
const selfPaths = new Set<string>([
  tryRealpath(fileURLToPath(import.meta.url)),
  tryRealpath(join(cliDir, "bin.js")),
]);
if (process.argv[1]) {
  selfPaths.add(tryRealpath(process.argv[1]));
}

const binary = searchPaths.find((p) => existsSync(p) && !selfPaths.has(tryRealpath(p)));

if (!binary) {
  console.error(`canon: no binary found for ${process.platform}/${process.arch}`);
  if (platformPkgName) {
    console.error(`  expected optional package: ${platformPkgName}`);
  } else {
    console.error("  no @journeykit/canon-core-<platform> package exists for this platform/arch yet");
  }
  console.error("  build from source: cargo build --release -p canon-cli");
  process.exit(1);
}

const bundledSkills = join(cliDir, "dist", "skills");
const checkoutSkills = join(workspaceRoot, "canon", "skills");
const skillsSource = existsSync(join(bundledSkills, "SKILL.src.md")) ? bundledSkills : checkoutSkills;
const args = process.argv.slice(2);
const env = { ...process.env, CANON_SKILLS_SOURCE: skillsSource };

// On macOS and Linux with Node 26.1 or later (or a Bun that reports it) the
// launcher replaces itself with the binary (`process.execve`): no node process
// (~39 MB) stays alive beside a long-running canon, the pid stays the one the
// caller started, and signals reach canon directly. Returns only when that is
// not possible here (Windows, an older Node, an exec the system refuses) —
// nothing has run yet, and the child path below runs the binary and passes
// its exit status on.
function replaceWith(bin: string, argv: string[]): void {
  // @types/node ^20 predates process.execve, so it is found with a guard.
  if (process.platform === "win32" || !("execve" in process) || typeof process.execve !== "function") return;
  // Before Node 26.1 a failed execve(2) aborts the process (SIGABRT, exit 134)
  // instead of throwing (nodejs/node#62878); Bun 1.3 (reporting Node 24) too.
  const [major = 0, minor = 0] = process.versions.node.split(".").map(Number);
  if (major < 26 || (major === 26 && minor < 1)) return;
  const execve: Function = process.execve;
  // process.execve is experimental in Node 23.11 to 26 and announces itself
  // with a warning on stderr; every canon command would print it.
  const emitWarning = process.emitWarning;
  process.emitWarning = () => {};
  try {
    execve.call(process, bin, [bin, ...argv], env);
  } catch {
    // Not replaced: the child path below reports the failure.
  } finally {
    process.emitWarning = emitWarning;
  }
}

replaceWith(binary, args);
const result = spawnSync(binary, args, { stdio: "inherit", env });

// The launcher IS the `canon` users run, so the native binary's exit code
// must survive it: scripts, CI, and hooks read only the status. canon's own
// exits are 0, 1 and 2 (atip/1 spec 8); a signal death has no status and
// reads as 128+n, the shell's spelling of it (spec 8.4), never as canon's 1.
// A spawn failure has neither and still reads as failure.
if (result.error) {
  console.error(`canon: failed to run ${binary}: ${result.error.message}`);
}
if (result.status === null && result.signal) {
  process.exit(128 + (osConstants.signals[result.signal] ?? 0));
}
process.exit(result.status ?? 1);
