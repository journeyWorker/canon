// Builds src/data/corpus-figure.json: when each scenario was authored versus
// when it first gained evidence, plus the current gate result.
//
// Reads (all relative to the repo root, two levels above packages/website):
//   - specs/features/**/*.feature    scenario id tags (@<area>.<surface>.<nn>)
//   - git history                    author date of the first commit adding each tag
//   - .canon/ledger/kind=evidence_record/*.json   `scenario_id` + `at` per record
//   - `canon gate check`             exit code and summary line
//
// The JSON output is committed so the Vercel build (a shallow clone without
// git history or the canon binary) never has to run this.
// To regenerate: `bun run figure:data` from packages/website.

import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { join, relative, dirname } from "node:path";

const WEBSITE_DIR = join(import.meta.dir, "..");
const REPO_ROOT = join(WEBSITE_DIR, "..", "..");
const FEATURES_DIR = join(REPO_ROOT, "specs", "features");
const EVIDENCE_DIR = join(REPO_ROOT, ".canon", "ledger", "kind=evidence_record");
const OUT_PATH = join(WEBSITE_DIR, "src", "data", "corpus-figure.json");
const GIT_CONCURRENCY = 8;

// A scenario id tag is exactly three dot-separated segments, the last all digits.
const SCENARIO_TAG = /^@([a-z][a-z0-9_-]*)\.([a-z][a-z0-9_-]*)\.([0-9]+)$/;

type Scenario = {
  id: string;
  file: string;
  authoredAt: string;
  authoredMs: number;
  firstEvidenceAt: string | null;
  firstEvidenceMs: number | null;
  lagHours: number | null;
};

// Date.parse is not guaranteed to accept sub-millisecond fractions
// (the ledger writes microseconds), so trim the fraction to 3 digits first.
function toMs(iso: string): number {
  const ms = Date.parse(iso.replace(/(\.\d{3})\d+/, "$1"));
  if (Number.isNaN(ms)) throw new Error(`unparseable timestamp: ${iso}`);
  return ms;
}

function run(cmd: string[], cwd = REPO_ROOT) {
  const p = Bun.spawnSync(cmd, { cwd, stdout: "pipe", stderr: "pipe" });
  return {
    exitCode: p.exitCode,
    stdout: p.stdout.toString(),
    stderr: p.stderr.toString(),
  };
}

async function runAsync(cmd: string[], cwd = REPO_ROOT) {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(p.stdout).text(),
    new Response(p.stderr).text(),
    p.exited,
  ]);
  if (exitCode !== 0) throw new Error(`${cmd.join(" ")} failed (${exitCode}): ${stderr}`);
  return stdout;
}

async function walk(dir: string, ext: string): Promise<string[]> {
  const out: string[] = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await walk(full, ext)));
    else if (entry.isFile() && entry.name.endsWith(ext)) out.push(full);
  }
  return out.sort();
}

async function collectScenarios(): Promise<Map<string, string>> {
  const ids = new Map<string, string>();
  for (const path of await walk(FEATURES_DIR, ".feature")) {
    const file = relative(REPO_ROOT, path);
    const text = await readFile(path, "utf8");
    for (const line of text.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed.startsWith("@")) continue;
      for (const token of trimmed.split(/\s+/)) {
        const m = SCENARIO_TAG.exec(token);
        if (!m) continue;
        const id = token.slice(1);
        const prev = ids.get(id);
        if (prev === undefined) ids.set(id, file);
        else if (prev !== file) console.warn(`warn: @${id} in ${file} also in ${prev}; keeping ${prev}`);
      }
    }
  }
  return ids;
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// Exact-tag match: `-G` (git compiles it as an extended regex) anchored so that
// @gate.check.1 does not match @gate.check.12, which a plain `-S` would.
async function authoredAt(id: string, file: string): Promise<string> {
  const pattern = `@${escapeRegex(id)}([^0-9A-Za-z_.-]|$)`;
  const base = ["git", "log", "--reverse", "--format=%aI", `-G${pattern}`];
  let first = (await runAsync([...base, "--", file])).split("\n")[0].trim();
  if (!first) {
    console.warn(`warn: @${id} has no adding commit in ${file}; retrying without path restriction`);
    first = (await runAsync(base)).split("\n")[0].trim();
  }
  if (!first) throw new Error(`no commit found that adds @${id}`);
  return first;
}

async function mapLimit<T, R>(items: T[], limit: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results = new Array<R>(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

// Older ledger records are keyed by `task_id` (plan task) rather than
// `scenario_id`; they attest no scenario, so they are skipped.
async function firstEvidence(): Promise<Map<string, { at: string; ms: number }>> {
  const first = new Map<string, { at: string; ms: number }>();
  const names = (await readdir(EVIDENCE_DIR)).filter((n) => n.endsWith(".json")).sort();
  let taskKeyed = 0;
  for (const name of names) {
    const rec = JSON.parse(await readFile(join(EVIDENCE_DIR, name), "utf8"));
    if (rec.scenario_id === undefined) {
      taskKeyed++;
      continue;
    }
    if (typeof rec.scenario_id !== "string" || typeof rec.at !== "string") {
      throw new Error(`evidence record ${name} has non-string scenario_id/at`);
    }
    const ms = toMs(rec.at);
    const prev = first.get(rec.scenario_id);
    if (!prev || ms < prev.ms) first.set(rec.scenario_id, { at: rec.at, ms });
  }
  console.log(`evidence: ${names.length - taskKeyed} scenario-keyed records, ${taskKeyed} task-keyed skipped`);
  return first;
}

function gateCheck() {
  const direct = Bun.which("canon");
  let cmd: string[];
  let runner: string;
  if (direct) {
    cmd = [direct, "gate", "check"];
    runner = run([direct, "--version"]).stdout.trim() || "canon";
  } else {
    cmd = ["bunx", "@journeykit/canon", "gate", "check"];
    runner = "bunx @journeykit/canon";
  }
  const checkedAt = new Date().toISOString();
  const res = run(cmd);
  const output = `${res.stdout}\n${res.stderr}`;
  const summary =
    output
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.startsWith("canon gate check:")) ?? null;
  let violations: number | null = null;
  if (summary) {
    const clean = /clean \((\d+) violations?\)/.exec(summary);
    const failing = /:\s*(\d+) violations?/.exec(summary);
    violations = clean ? Number(clean[1]) : failing ? Number(failing[1]) : null;
  }
  if (summary === null || violations === null) {
    console.warn(`warn: could not parse gate summary (exit ${res.exitCode}):\n${output}`);
  }
  return {
    command: "canon gate check",
    runner,
    exitCode: res.exitCode,
    violations,
    clean: res.exitCode === 0 && violations === 0,
    summary,
    checkedAt,
  };
}

const sourceCommit = run(["git", "rev-parse", "HEAD"]).stdout.trim();
const ids = await collectScenarios();
const evidence = await firstEvidence();
const entries = [...ids.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
const authored = await mapLimit(entries, GIT_CONCURRENCY, ([id, file]) => authoredAt(id, file));

const scenarios: Scenario[] = entries.map(([id, file], i) => {
  const authoredMs = toMs(authored[i]);
  const ev = evidence.get(id) ?? null;
  return {
    id,
    file,
    authoredAt: authored[i],
    authoredMs,
    firstEvidenceAt: ev?.at ?? null,
    firstEvidenceMs: ev?.ms ?? null,
    lagHours: ev ? Math.round((ev.ms - authoredMs) / 360_000) / 10 : null,
  };
});
scenarios.sort((a, b) => a.authoredMs - b.authoredMs || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));

const withEv = scenarios.filter((s) => s.firstEvidenceMs !== null);
const startS = scenarios.reduce((m, s) => (s.authoredMs < m.authoredMs ? s : m));
let end = { at: startS.authoredAt, ms: startS.authoredMs };
for (const s of scenarios) {
  if (s.authoredMs > end.ms) end = { at: s.authoredAt, ms: s.authoredMs };
  if (s.firstEvidenceMs !== null && s.firstEvidenceMs > end.ms) end = { at: s.firstEvidenceAt!, ms: s.firstEvidenceMs };
}

const figure = {
  generatedAt: new Date().toISOString(),
  sourceCommit,
  scenarioCount: scenarios.length,
  withEvidence: withEv.length,
  evidenceAtOrBeforeAuthoring: withEv.filter((s) => s.firstEvidenceMs! <= s.authoredMs).length,
  evidenceAfterAuthoring: withEv.filter((s) => s.firstEvidenceMs! > s.authoredMs).length,
  withoutEvidence: scenarios.length - withEv.length,
  range: { start: startS.authoredAt, end: end.at, startMs: startS.authoredMs, endMs: end.ms },
  gate: gateCheck(),
  scenarios,
};

await mkdir(dirname(OUT_PATH), { recursive: true });
await writeFile(OUT_PATH, `${JSON.stringify(figure, null, 2)}\n`);
console.log(
  `wrote ${relative(WEBSITE_DIR, OUT_PATH)}: ${figure.scenarioCount} scenarios, ` +
    `${figure.withEvidence} with evidence (${figure.evidenceAtOrBeforeAuthoring} at/before authoring, ` +
    `${figure.evidenceAfterAuthoring} after), gate: ${figure.gate.summary}`,
);
