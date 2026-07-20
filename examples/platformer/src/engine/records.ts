import type { LevelRecord } from './types';
import { LEVELS } from './level';

// localStorage schema per the design doc, verbatim: a single key holding a
// versioned JSON blob with the highest unlocked (1-indexed) level and a
// best-record map keyed by 1-indexed level number.
const STORAGE_KEY = 'rpr.v2.progress';
const SCHEMA_VERSION = 1;

interface StoredBest { acorns: number; timeMs: number; }
interface ProgressData {
  version: number;
  unlockedLevel: number;
  best: Record<string, StoredBest>;
}

function defaultProgress(): ProgressData {
  return { version: SCHEMA_VERSION, unlockedLevel: 1, best: {} };
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isValidUnlockedLevel(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value)
    && value >= 1 && value <= LEVELS.length;
}

function isValidStoredBest(value: unknown): value is StoredBest {
  return isPlainObject(value) && Number.isFinite(value.acorns) && Number.isFinite(value.timeMs);
}

// Drops any malformed individual record instead of discarding the whole
// map -- one corrupted entry must not erase every other level's best.
function sanitizeBest(value: unknown): Record<string, StoredBest> {
  if (!isPlainObject(value)) return {};
  const out: Record<string, StoredBest> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (isValidStoredBest(entry)) out[key] = { acorns: entry.acorns, timeMs: entry.timeMs };
  }
  return out;
}

// Corrupted or nonconforming stored JSON (wrong types, non-finite/
// out-of-range unlockedLevel, a non-object best map) must fall back to
// fresh defaults -- it must never throw and break level completion.
function loadProgress(): ProgressData {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return defaultProgress();
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isPlainObject(parsed)) return defaultProgress();
    if (parsed.version !== SCHEMA_VERSION) return defaultProgress();
    if (!isValidUnlockedLevel(parsed.unlockedLevel)) return defaultProgress();
    if (!isPlainObject(parsed.best)) return defaultProgress();
    return { version: SCHEMA_VERSION, unlockedLevel: parsed.unlockedLevel, best: sanitizeBest(parsed.best) };
  } catch {
    return defaultProgress();
  }
}

function saveProgress(data: ProgressData): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
}

// levelId is the 0-indexed LevelData.id; the stored schema keys "best" by
// the 1-indexed level number shown in the design doc's example payload.
function bestKey(levelId: number): string {
  return String(levelId + 1);
}

export function loadRecord(levelId: number): LevelRecord | null {
  const entry = loadProgress().best[bestKey(levelId)];
  return entry ? { bestAcorns: entry.acorns, bestTimeSeconds: entry.timeMs / 1000 } : null;
}

export function saveRecordIfBetter(levelId: number, result: { acorns: number; timeSeconds: number }): LevelRecord {
  const progress = loadProgress();
  const key = bestKey(levelId);
  const current = progress.best[key];
  const timeMs = Math.round(result.timeSeconds * 1000);
  // acorns (max) and time (min) are independent bests, never replaced as a
  // lexicographically ranked pair -- a faster-but-fewer-acorns run must not
  // discard a prior best time, and vice versa.
  const next: StoredBest = current
    ? { acorns: Math.max(current.acorns, result.acorns), timeMs: Math.min(current.timeMs, timeMs) }
    : { acorns: result.acorns, timeMs };
  progress.best[key] = next;
  saveProgress(progress);
  return { bestAcorns: next.acorns, bestTimeSeconds: next.timeMs / 1000 };
}

export function loadUnlockedLevels(): number[] {
  const progress = loadProgress();
  const count = Math.max(1, progress.unlockedLevel);
  return Array.from({ length: count }, (_, i) => i);
}

export function saveUnlockedLevels(levels: number[]): void {
  const progress = loadProgress();
  progress.unlockedLevel = Math.max(progress.unlockedLevel, levels.length);
  saveProgress(progress);
}
