import { describe, expect, test } from "bun:test";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// The launcher is the `canon` every npm/bun user runs. Scripts, CI, and
// hooks see only its exit status, so it must be the native binary's —
// 0.8.0–0.9.0 shipped a launcher that exited 0 on every refusal.
//
// The launcher resolves `<cli>/bin/canon` when no workspace build exists,
// so a throwaway tree with a stub binary there runs the real launcher
// source without a cargo build.
const launcherSource = join(import.meta.dir, "..", "src", "index.ts");

// The launcher replaces itself with the binary (`process.execve`) where the
// runtime can do it safely (Node 26.1 or later): a long-running canon keeps
// no node process beside it. Elsewhere (Windows, an older Node) it runs the
// binary as a child. The launcher runs under `node`, as `bin.js` does for
// every npm user (the test and publish workflows install Node 26).
function runStub(
  script: string,
  opts: { stdin?: Buffer; withoutExecve?: boolean; executable?: boolean } = {},
): { exitCode: number | null; signalCode: string | null; stdout: string; stderr: string } {
  const root = mkdtempSync(join(tmpdir(), "canon-launcher-"));
  try {
    const cliDir = join(root, "packages", "cli");
    mkdirSync(join(cliDir, "src"), { recursive: true });
    mkdirSync(join(cliDir, "bin"), { recursive: true });
    copyFileSync(launcherSource, join(cliDir, "src", "index.ts"));
    // An ES module package, as the published one is: node reads the launcher without a warning.
    writeFileSync(join(cliDir, "package.json"), '{"type":"module"}\n');
    const stub = join(cliDir, "bin", "canon");
    writeFileSync(stub, `#!/bin/sh\n${script}\n`);
    chmodSync(stub, opts.executable === false ? 0o644 : 0o755);
    // A preload that takes process.execve away: the launcher's child path.
    const preload = join(root, "no-execve.cjs");
    writeFileSync(preload, "process.execve = undefined;\n");
    const runtime = ["node", ...(opts.withoutExecve ? ["--require", preload] : [])];
    const result = Bun.spawnSync([...runtime, join(cliDir, "src", "index.ts"), "gate", "check"], {
      stdin: opts.stdin ?? "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });
    return {
      exitCode: result.exitCode,
      signalCode: result.signalCode ?? null,
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

describe.skipIf(process.platform === "win32")("launcher exit status", () => {
  test.each([0, 1, 2])("passes the native binary's exit code %d through", (code) => {
    expect(runStub(`exit ${code}`).exitCode).toBe(code);
  });

  // The launcher became the binary, so the caller sees the signal death
  // itself (a shell reads it as 137), never canon's 1 and never success.
  test("a binary killed by a signal is the caller's signal death, never canon's 1 and never success", () => {
    const r = runStub("kill -9 $$");
    expect(r.exitCode).toBeNull();
    expect(r.signalCode).toBe("SIGKILL");
  });
});

// atip/1 15.6: the launcher reads no member of canon's JSON, only the exit status. A document with members
// and enum strings it has never heard of (a later canon's `contractVersion`, `reason`, new kinds) passes
// through byte for byte and the status is still the binary's.
describe.skipIf(process.platform === "win32")("launcher output", () => {
  const document = JSON.stringify({
    ok: false,
    error: { code: "INPUT_INVALID", reason: "A_REASON_FROM_A_LATER_CANON", message: "m", retryable: false, "x-future": { a: 1 } },
    meta: { schemaVersion: "9.9", tool: "canon", toolVersion: "9.9.9", contract: "atip/1", contractVersion: "9.9", futureMember: [1, 2] },
  });

  test("an envelope with members and enum strings it does not know goes through unchanged, with the binary's exit", () => {
    const root = mkdtempSync(join(tmpdir(), "canon-launcher-"));
    try {
      const cliDir = join(root, "packages", "cli");
      mkdirSync(join(cliDir, "src"), { recursive: true });
      mkdirSync(join(cliDir, "bin"), { recursive: true });
      copyFileSync(launcherSource, join(cliDir, "src", "index.ts"));
      const stub = join(cliDir, "bin", "canon");
      writeFileSync(stub, `#!/bin/sh\ncat <<'EOF'\n${document}\nEOF\nexit 2\n`);
      chmodSync(stub, 0o755);
      const result = Bun.spawnSync([process.execPath, join(cliDir, "src", "index.ts"), "scenario", "new", "--json"], { stdout: "pipe", stderr: "pipe" });
      expect(result.exitCode).toBe(2);
      expect(result.stdout.toString()).toBe(`${document}\n`);
      expect(result.stderr.toString()).toBe("");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe.skipIf(process.platform === "win32")("launcher process", () => {
  test("the binary replaces the launcher: its parent is the caller, not a node process", () => {
    const r = runStub('echo "ppid:$PPID args:$*"');
    expect(r.exitCode).toBe(0);
    expect(r.stdout.trim()).toBe(`ppid:${process.pid} args:gate check`);
  });

  test("stdin reaches the binary and the launcher writes nothing to stderr (an LSP or MCP server's stdio)", () => {
    const message = '{"jsonrpc":"2.0","id":1,"method":"initialize"}\n';
    const r = runStub("cat", { stdin: Buffer.from(message) });
    expect(r.exitCode).toBe(0);
    expect(r.stdout).toBe(message);
    expect(r.stderr).toBe("");
  });

  test("the binary still gets CANON_SKILLS_SOURCE", () => {
    const r = runStub('echo "skills:${CANON_SKILLS_SOURCE:-unset}"');
    expect(r.stdout.trim()).toMatch(/^skills:.*skills$/);
  });

  test("a binary the system cannot execute is the launcher's error (exit 1), never an abort", () => {
    const r = runStub("exit 0", { executable: false });
    expect(r.signalCode).toBeNull();
    expect(r.exitCode).toBe(1);
    expect(r.stderr).toContain("canon: failed to run");
  });

  describe("without process.execve (Windows, Node < 26.1): the child path", () => {
    test("runs the binary as a child of the launcher", () => {
      const r = runStub('echo "ppid:$PPID"', { withoutExecve: true });
      expect(r.exitCode).toBe(0);
      expect(r.stdout.trim()).not.toBe(`ppid:${process.pid}`);
    });

    test.each([0, 1, 2])("passes the native binary's exit code %d through", (code) => {
      expect(runStub(`exit ${code}`, { withoutExecve: true }).exitCode).toBe(code);
    });

    test("a binary killed by a signal exits 128+n (SIGKILL: 137)", () => {
      expect(runStub("kill -9 $$", { withoutExecve: true }).exitCode).toBe(137);
    });
  });
});
