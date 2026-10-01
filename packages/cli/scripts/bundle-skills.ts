import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const source = resolve(scriptDir, "../../../canon/skills");
const destination = resolve(scriptDir, "../dist/skills");

if (!existsSync(join(source, "SKILL.src.md"))) {
  throw new Error(`canonical skill source is missing: ${join(source, "SKILL.src.md")}`);
}

rmSync(destination, { recursive: true, force: true });
mkdirSync(dirname(destination), { recursive: true });
cpSync(source, destination, {
  recursive: true,
  filter: (path) => !path.endsWith(".install-lock.json") && !path.endsWith("README.md"),
});
