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

function runWithStubExit(code: number | "signal"): { status: number | null } {
  const root = mkdtempSync(join(tmpdir(), "canon-launcher-"));
  try {
    const cliDir = join(root, "packages", "cli");
    mkdirSync(join(cliDir, "src"), { recursive: true });
    mkdirSync(join(cliDir, "bin"), { recursive: true });
    copyFileSync(launcherSource, join(cliDir, "src", "index.ts"));
    const stub = join(cliDir, "bin", "canon");
    writeFileSync(stub, code === "signal" ? "#!/bin/sh\nkill -9 $$\n" : `#!/bin/sh\nexit ${code}\n`);
    chmodSync(stub, 0o755);
    const result = Bun.spawnSync([process.execPath, join(cliDir, "src", "index.ts"), "gate", "check"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    return { status: result.exitCode };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

describe.skipIf(process.platform === "win32")("launcher exit status", () => {
  test.each([0, 1, 2])("passes the native binary's exit code %d through", (code) => {
    expect(runWithStubExit(code).status).toBe(code);
  });

  test("a binary killed by a signal exits 128+n (SIGKILL: 137), never canon's 1 and never success", () => {
    expect(runWithStubExit("signal").status).toBe(137);
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
