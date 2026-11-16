import { describe, expect, test } from "bun:test";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

// Which platform package a (platform, arch) pair runs, and where a workspace
// dev build is looked for. The launcher is run as a user runs it (under node),
// with `process.platform` and `process.arch` replaced by a preload, in a
// throwaway tree: the answer is the package it names when it finds no binary
// and the stub it runs when it finds one. Linux is two static musl packages and
// the host libc is never asked, so `CANON_LIBC` (the old override) changes
// nothing.
const launcherSource = join(import.meta.dir, "..", "src", "index.ts");

type Run = { exitCode: number | null; stdout: string; stderr: string };

function runAs(
  platform: string,
  arch: string,
  stubs: string[],
  env: Record<string, string> = {},
): Run {
  const root = mkdtempSync(join(tmpdir(), "canon-platform-"));
  try {
    const cliDir = join(root, "packages", "cli");
    mkdirSync(join(cliDir, "src"), { recursive: true });
    copyFileSync(launcherSource, join(cliDir, "src", "index.ts"));
    writeFileSync(join(cliDir, "package.json"), '{"type":"module"}\n');
    // Each stub is a path under the tree; it prints its own path under `root`.
    const binaryName = platform === "win32" ? "canon.exe" : "canon";
    for (const stub of stubs) {
      const path = join(root, stub, binaryName);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, `#!/bin/sh\necho "ran ${stub}"\n`);
      chmodSync(path, 0o755);
    }
    const preload = join(root, "as-platform.cjs");
    writeFileSync(
      preload,
      `Object.defineProperty(process, "platform", { value: ${JSON.stringify(platform)} });\n` +
        `Object.defineProperty(process, "arch", { value: ${JSON.stringify(arch)} });\n`,
    );
    const result = Bun.spawnSync(["node", "--require", preload, join(cliDir, "src", "index.ts")], {
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, ...env },
    });
    return { exitCode: result.exitCode, stdout: result.stdout.toString(), stderr: result.stderr.toString() };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

describe.skipIf(process.platform === "win32")("launcher platform packages", () => {
  test.each([
    ["darwin", "arm64", "@journeykit/canon-core-darwin-arm64"],
    ["linux", "x64", "@journeykit/canon-core-linux-x64"],
    ["linux", "arm64", "@journeykit/canon-core-linux-arm64"],
    ["win32", "x64", "@journeykit/canon-core-win32-x64"],
  ])("%s/%s runs %s", (platform, arch, pkg) => {
    const run = runAs(platform, arch, []);
    expect(run.exitCode).toBe(1);
    expect(run.stderr).toContain(`canon: no binary found for ${platform}/${arch}`);
    expect(run.stderr).toContain(`expected optional package: ${pkg}`);
  });

  test.each([
    ["darwin", "x64"],
    ["win32", "arm64"],
    ["linux", "ia32"],
    ["linux", "riscv64"],
    ["freebsd", "x64"],
  ])("%s/%s has no package", (platform, arch) => {
    const run = runAs(platform, arch, []);
    expect(run.exitCode).toBe(1);
    expect(run.stderr).toContain(`canon: no binary found for ${platform}/${arch}`);
    expect(run.stderr).toContain("no @journeykit/canon-core-<platform> package exists for this platform/arch yet");
  });

  // The old launcher asked the host's libc (an override, `ldd`, the loaders) and
  // sent a musl host to no package at all: both libcs run the same binary now.
  test.each(["musl", "gnu", "glibc", "nonsense", ""])("CANON_LIBC=%j changes nothing on linux", (libc) => {
    for (const arch of ["x64", "arm64"]) {
      const run = runAs("linux", arch, [], { CANON_LIBC: libc });
      expect(run.stderr).toContain(`expected optional package: @journeykit/canon-core-linux-${arch}`);
    }
  });

  test("a linux dev build is looked for in the musl target directory first", () => {
    const musl = (triple: string) => `target/${triple}/release`;
    const both = runAs("linux", "x64", [musl("x86_64-unknown-linux-musl"), "target/release"]);
    expect(both.stdout.trim()).toBe(`ran ${musl("x86_64-unknown-linux-musl")}`);
    const bothArm = runAs("linux", "arm64", [musl("aarch64-unknown-linux-musl"), "target/release"]);
    expect(bothArm.stdout.trim()).toBe(`ran ${musl("aarch64-unknown-linux-musl")}`);
  });

  test("a linux dev build falls back to target/release", () => {
    const run = runAs("linux", "x64", ["target/release"]);
    expect(run.stdout.trim()).toBe("ran target/release");
  });

  test("the gnu target directory is not looked in any more", () => {
    const run = runAs("linux", "x64", ["target/x86_64-unknown-linux-gnu/release"]);
    expect(run.exitCode).toBe(1);
    expect(run.stderr).toContain("canon: no binary found for linux/x64");
  });
});
