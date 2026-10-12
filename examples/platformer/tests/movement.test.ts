import { expect, test } from 'vitest';
import { findSpawn, LEVELS } from '../src/engine/level';
import { COYOTE_TIME, FIXED_DT, GRAVITY, JUMP_VELOCITY, MAX_FALL } from '../src/engine/constants';
import { spawnPlayer } from '../src/engine/player';
import { level, player, settle, simOf, step, steps } from './harness';

test('platformer.movement.01: Left world edge clamps the player', () => {
  const sim = simOf(LEVELS[0]);
  settle(sim);
  // Given: level one's spawn sits near the world's left edge.
  expect(player(sim).x).toBeLessThan(3 * 48);

  // When left is held for two seconds, x clamps at the boundary and never passes it.
  const xs: number[] = [];
  for (let i = 0; i < 120; i++) { step(sim, { left: true }); xs.push(player(sim).x); }
  expect(Math.min(...xs)).toBe(0);
  expect(player(sim).x).toBe(0);

  // Spamming jump against the wall: no horizontal drift on any frame, and the jumps do fire.
  let lowestTop = player(sim).y;
  for (let i = 0; i < 120; i++) {
    step(sim, { left: true, jumpBuffered: i % 6 === 0 });
    expect(player(sim).x).toBe(0);
    lowestTop = Math.min(lowestTop, player(sim).y);
  }
  expect(lowestTop).toBeLessThan(436 - 48);
});

test('platformer.movement.02: Ceiling contact cancels upward motion without sticking', () => {
  // Given the player stands under a platform tile (row 2, col 3).
  const sim = simOf(level([
    '.......',
    '.......',
    '...#...',
    '.......',
    '...S...',
    '#######',
  ]));
  settle(sim);
  const startX = player(sim).x;
  const tileUnderside = 3 * 48;

  // When the player jumps into the tile's underside.
  step(sim, { jumpBuffered: true });
  const trace: { x: number; y: number; vy: number; onGround: boolean }[] = [];
  for (let i = 0; i < 90; i++) {
    step(sim);
    const p = player(sim);
    trace.push({ x: p.x, y: p.y, vy: p.vy, onGround: p.onGround });
  }
  const contact = trace.findIndex((f) => f.y === tileUnderside);
  expect(contact).toBeGreaterThan(0);

  // Then vertical velocity cancels the instant the head touches: rising the
  // step before, zero on the contact step.
  expect(trace[contact - 1].vy).toBeLessThan(0);
  expect(trace[contact].vy).toBe(0);

  // And the player falls straight back: never sticks at the underside,
  // descends every step until landing, with no horizontal drift.
  const landed = trace.findIndex((f, i) => i > contact && f.onGround);
  expect(landed).toBeGreaterThan(contact);
  for (let i = contact + 1; i <= landed; i++) {
    expect(trace[i].y).toBeGreaterThan(trace[i - 1].y);
  }
  expect(trace.every((f) => f.x === startX)).toBe(true);
  expect(trace[landed].y).toBe(5 * 48 - player(sim).h);
});

// A ledge (cols 0-4) over a gap, a lower floor four rows down.
const GAP = level([
  '..............',
  '..............',
  '..............',
  '.S............',
  '#####.........',
  '..............',
  '..............',
  '..............',
  '##############',
]);

/** Walk right from spawn until the step the player leaves the ledge. */
function walkOffLedge() {
  const sim = simOf(GAP);
  settle(sim);
  for (let i = 0; i < 120 && player(sim).onGround; i++) step(sim, { right: true });
  expect(player(sim).onGround).toBe(false);
  return sim;
}

test('platformer.movement.03: No double jump; coyote and buffered jumps fire', () => {
  // Given the player is airborne over a gap (jumped off the ledge's end).
  const air = simOf(GAP);
  settle(air);
  while (player(air).x < 4 * 48) step(air, { right: true });
  step(air, { right: true, jumpBuffered: true });
  expect(player(air).vy).toBeLessThan(0);
  // When jump is pressed repeatedly mid-air, over the gap: no second jump fires,
  // so velocity only ever grows under gravity until the landing.
  let vy = player(air).vy;
  for (let i = 0; i < 200 && !player(air).onGround; i++) {
    step(air, { right: true, jumpBuffered: i % 3 === 0 && player(air).x >= 5 * 48 });
    if (player(air).onGround) break;
    expect(player(air).vy).toBeCloseTo(Math.min(vy + GRAVITY * FIXED_DT, MAX_FALL), 9);
    vy = player(air).vy;
  }
  expect(player(air).onGround).toBe(true);

  // A jump pressed within coyote time after leaving a ledge still fires.
  const coyote = walkOffLedge();
  steps(coyote, 2, { right: true });
  expect(player(coyote).onGround).toBe(false);
  expect(3 * FIXED_DT).toBeLessThan(COYOTE_TIME);
  step(coyote, { right: true, jumpBuffered: true });
  expect(player(coyote).vy).toBeCloseTo(JUMP_VELOCITY + GRAVITY * FIXED_DT, 9);

  // ...and once coyote time has run out it does not.
  const late = walkOffLedge();
  steps(late, 8, { right: true });
  const before = player(late).vy;
  step(late, { right: true, jumpBuffered: true });
  expect(player(late).vy).toBeGreaterThan(before);

  // A jump buffered just before landing fires on the landing frame. A dry
  // run finds the landing step L (and that, unpressed, the player stays
  // grounded after it); the real run presses jump on step L-3 (0.05s
  // early), stays airborne until L, lands on L, and is jumping on L+1 —
  // the first step that begins on the ground — with no grounded step between.
  const dry = walkOffLedge();
  let landing = 0;
  while (!player(dry).onGround) { step(dry); landing++; }
  step(dry);
  expect(player(dry).onGround).toBe(true);

  const buffered = walkOffLedge();
  steps(buffered, landing - 4);
  step(buffered, { jumpBuffered: true });
  steps(buffered, 2);
  expect(player(buffered).onGround).toBe(false);
  step(buffered);
  expect(player(buffered).onGround).toBe(true);
  step(buffered);
  expect(player(buffered).onGround).toBe(false);
  expect(player(buffered).vy).toBeCloseTo(JUMP_VELOCITY + GRAVITY * FIXED_DT, 9);
});

test('platformer.movement.04: Falling into a pit respawns without touching score', () => {
  // A ledge with two acorns, ending at x=288 over a bottomless pit.
  const pit = level([
    '..........',
    '.S.A.A....',
    '######....',
  ]);
  const sim = simOf(pit);
  settle(sim);
  // Given the player has collected some acorns.
  while (player(sim).x < 300) step(sim, { right: true });
  expect(sim.getHudSnapshot().acorns).toBe(2);
  const collected = sim.getRenderSnapshot().acorns.map((a) => a.collected);

  // When the player falls more than 200 pixels below the world.
  const threshold = pit.grid.length * 48 + 200;
  const spawn = spawnPlayer(findSpawn(pit, 'S'));
  let prev = { ...player(sim) };
  let respawned = false;
  for (let i = 0; i < 240 && !respawned; i++) {
    step(sim);
    const p = player(sim);
    respawned = p.y < prev.y;
    if (!respawned) {
      // Not respawned while still within 200px below the world.
      expect(p.y).toBeLessThanOrEqual(threshold);
      prev = { ...p };
    }
  }
  expect(respawned).toBe(true);
  // It respawned on the step that would have carried it past the threshold.
  const fallVy = Math.min(prev.vy + GRAVITY * FIXED_DT, MAX_FALL);
  expect(prev.y + fallVy * FIXED_DT).toBeGreaterThan(threshold);

  // Then the player respawns at the spawn point.
  const p = player(sim);
  expect({ x: p.x, y: p.y, vx: p.vx, vy: p.vy }).toEqual({ x: spawn.x, y: spawn.y, vx: 0, vy: 0 });

  // And the score and collected acorns are unchanged.
  expect(sim.getHudSnapshot().acorns).toBe(2);
  expect(sim.getRenderSnapshot().acorns.map((a) => a.collected)).toEqual(collected);
  steps(sim, 30);
  expect(sim.getHudSnapshot().acorns).toBe(2);
});
