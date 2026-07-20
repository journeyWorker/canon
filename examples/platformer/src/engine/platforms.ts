import type { MovingPlatformState, PlatformSpawn } from './types';

export function spawnPlatforms(defs: PlatformSpawn[]): MovingPlatformState[] {
  return defs.map((def, i) => ({
    id: `platform-${i}`,
    x: def.x, y: def.y, w: def.w, h: def.h,
    axis: def.axis, min: def.min, max: def.max, speed: def.speed, dir: 1,
  }));
}

// Moves each platform along its axis between min/max, reversing at the
// bounds, and returns this tick's per-platform delta so player.ts can carry
// a rider by exactly one tick's platform motion.
export function updateMovingPlatforms(platforms: MovingPlatformState[], dt: number): { id: string; dx: number; dy: number }[] {
  return platforms.map((p) => {
    const delta = p.speed * p.dir * dt;
    const before = p.axis === 'x' ? p.x : p.y;
    let next = before + delta;
    if (next <= p.min) { next = p.min; p.dir = 1; }
    else if (next >= p.max) { next = p.max; p.dir = -1; }
    const actualDelta = next - before;
    if (p.axis === 'x') p.x = next; else p.y = next;
    return { id: p.id, dx: p.axis === 'x' ? actualDelta : 0, dy: p.axis === 'y' ? actualDelta : 0 };
  });
}
