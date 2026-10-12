import { expect, test } from 'vitest';
import { LEVELS } from '../src/engine/level';
import { aabbOverlap } from '../src/engine/collision';
import {
  FIXED_DT, GRAVITY, MAX_FALL, MOVE_SPEED, PLATFORM_AMPLITUDE, PLATFORM_SPEED,
} from '../src/engine/constants';
import type { GameSimulation } from '../src/engine/simulation';
import type { PlatformSpawn } from '../src/engine/types';
import { level, settle, simOf, step } from './harness';

/**
 * An open sky (no tiles) holding one of the game's real moving platforms,
 * with the spawn marker directly above it so the player drops onto it.
 */
function riding(platform: PlatformSpawn, spawnCol = Math.floor(platform.x / 48)): GameSimulation {
  const rows = Array.from({ length: 24 }, () => '.'.repeat(24).split(''));
  rows[Math.floor(platform.y / 48) - 2][spawnCol] = 'S';
  const sim = simOf(level(rows.map((r) => r.join('')), { platforms: [platform] }));
  settle(sim);
  const { player: p, platforms: [pl] } = sim.getRenderSnapshot();
  expect(p.y + p.h).toBe(pl.y);
  return sim;
}

test('platformer.moving.01: A moving platform carries the standing player', () => {
  // Given a platform oscillating 120px at 60px/s (level one's own platform),
  // and the player standing still on top of it.
  const def = LEVELS[0].platforms[0];
  expect(def.max - def.min).toBe(PLATFORM_AMPLITUDE);
  expect(def.speed).toBe(PLATFORM_SPEED);

  // Level three's vertical platform must carry its rider the same way.
  for (const platform of [def, LEVELS[2].platforms[1]]) {
    const sim = riding(platform);
    const { player: p, platforms: [pl] } = sim.getRenderSnapshot();
    const offsetX = p.x - pl.x;
    // When the platform advances one simulation step — and every step after,
    // through both of its turnarounds.
    let moved = 0;
    for (let i = 0; i < 300; i++) {
      const before = { px: p.x, py: p.y, lx: pl.x, ly: pl.y };
      step(sim);
      // Then the player is displaced by the same delta as the platform.
      expect(p.x - before.px).toBeCloseTo(pl.x - before.lx, 9);
      expect(p.y - before.py).toBeCloseTo(pl.y - before.ly, 9);
      moved += Math.abs(pl.x - before.lx) + Math.abs(pl.y - before.ly);
      // And stays planted: no sinking, no sliding along it.
      expect(p.onGround).toBe(true);
      expect(p.y + p.h).toBeCloseTo(pl.y, 9);
      expect(p.x - pl.x).toBeCloseTo(offsetX, 9);
    }
    expect(moved).toBeGreaterThan(2 * PLATFORM_AMPLITUDE);
  }
});

test('platformer.moving.02: Stepping off a platform edge drops cleanly', () => {
  // Given the player stands on a moving platform near its right edge.
  const platform = LEVELS[0].platforms[0];
  const sim = riding(platform, Math.floor((platform.x + platform.w) / 48) - 1);
  const { player: p, platforms: [pl] } = sim.getRenderSnapshot();
  expect(pl.x + pl.w - (p.x + p.w)).toBeLessThan(p.w * 2);

  // When the player walks past the platform edge into open air.
  const trace: { x: number; y: number; vy: number; onGround: boolean; overlaps: boolean; platformDx: number }[] = [];
  for (let i = 0; i < 60; i++) {
    const before = { x: p.x, y: p.y, lx: pl.x };
    step(sim, { right: true });
    trace.push({ x: p.x - before.x, y: p.y - before.y, vy: p.vy, onGround: p.onGround, overlaps: aabbOverlap(p, pl), platformDx: pl.x - before.lx });
  }
  const lost = trace.findIndex((f) => !f.onGround);
  expect(lost).toBeGreaterThan(0);
  const after = trace.slice(lost + 1);

  // Then the player leaves the platform and falls under normal gravity.
  expect(after.every((f) => !f.onGround)).toBe(true);
  for (let i = 1; i < after.length; i++) {
    expect(after[i].vy).toBeCloseTo(Math.min(after[i - 1].vy + GRAVITY * FIXED_DT, MAX_FALL), 9);
  }

  // And the position changes smoothly: every step's motion stays within
  // one step of walking plus platform carry, and of falling — no warp or
  // snap — and the player never clips into the platform once off it.
  const walk = MOVE_SPEED * FIXED_DT;
  for (const f of trace) {
    expect(Math.abs(f.x)).toBeLessThanOrEqual(walk + PLATFORM_SPEED * FIXED_DT + 1e-9);
    expect(f.y).toBeGreaterThanOrEqual(0);
    expect(f.y).toBeLessThanOrEqual(MAX_FALL * FIXED_DT + 1e-9);
  }
  expect(trace.slice(lost).every((f) => !f.overlaps)).toBe(true);

  // And it is no longer carried once contact is lost: its x moves at
  // walking speed alone while the platform keeps moving.
  for (const f of after) {
    expect(f.x).toBeCloseTo(walk, 9);
    expect(f.platformDx).not.toBe(0);
  }
});
