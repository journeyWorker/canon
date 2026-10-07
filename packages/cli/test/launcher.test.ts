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
  test.each([0, 1, 2, 3])("passes the native binary's exit code %d through", (code) => {
    expect(runWithStubExit(code).status).toBe(code);
  });

  test("a binary killed by a signal reads as failure, never success", () => {
    expect(runWithStubExit("signal").status).toBe(1);
  });
});
