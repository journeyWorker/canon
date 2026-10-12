import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { Sprite } from 'pixi.js';
import { App } from '../src/App';
import { aabbOverlap } from '../src/engine/collision';
import { FIXED_DT, HIT_INVULN_DURATION, MAX_HEARTS } from '../src/engine/constants';
import { saveUnlockedLevels } from '../src/engine/records';
import { updatePlayerSprite } from '../src/render/renderPlayer';
import type { GameSimulation } from '../src/engine/simulation';
import { level, placeOn, player, Screen, settle, simOf, step } from './harness';

// WebGL is not available in jsdom; the stage only draws what the simulation
// holds. renderPlayer's blink rule is pure; only its Sprite import is Pixi's.
vi.mock('../src/render/PixiStage', () => ({ PixiStage: () => null }));
vi.mock('pixi.js', () => ({ Sprite: class {} }));

beforeEach(() => localStorage.clear());

let screen: Screen | null = null;
afterEach(() => { screen?.unmount(); screen = null; });

test('platformer.enemy.01: Pinecone contact costs a heart and grants brief invulnerability', () => {
  // Given three hearts and a pinecone patrolling the ledge toward the player.
  const sim = simOf(level([
    '..........',
    '..........',
    '....S.....',
    '##########',
  ], { enemies: [{ x: 96, y: 112, patrolMinX: 96, patrolMaxX: 200 }] }));
  settle(sim);
  expect(sim.getHudSnapshot().hearts).toBe(MAX_HEARTS);
  const p = player(sim);
  const enemy = sim.getRenderSnapshot().enemies[0];

  // When its hitbox overlaps the player while not invulnerable.
  let frames = 0;
  while (sim.getHudSnapshot().hearts === MAX_HEARTS && frames++ < 300) {
    expect(p.invulnTimer).toBe(0);
    step(sim);
  }
  // Then one heart is lost and the counter drops to two.
  expect(sim.getHudSnapshot().hearts).toBe(2);
  expect(p.invulnTimer).toBe(HIT_INVULN_DURATION);

  // And invulnerability lasts one second, the sprite blinking while it lasts.
  // During that window the player is pushed back onto the pinecone every
  // step: a second overlap costs no further heart. On the window's last
  // step the player is parked clear of the patrol, so the expiry itself
  // is not an overlap.
  const fake = { texture: { width: 76, height: 76 }, scale: { set() {} }, x: 0, y: 0, visible: true };
  // updatePlayerSprite reads only the texture size and writes x/y/scale/visible.
  const sprite = fake as unknown as Sprite;
  const visibility: boolean[] = [];
  let overlapped = 0;
  let windowSteps = 0;
  while (p.invulnTimer > 0) {
    if (p.invulnTimer > 1.5 * FIXED_DT) placeOn(sim, enemy);
    else p.x = 8 * 48;
    step(sim);
    windowSteps++;
    if (aabbOverlap(p, enemy)) overlapped++;
    expect(sim.getHudSnapshot().hearts).toBe(2);
    const snap = sim.getRenderSnapshot();
    updatePlayerSprite(sprite, p, snap.camera.x, snap.elapsed);
    if (p.invulnTimer > 0) visibility.push(fake.visible);
  }
  expect(overlapped).toBeGreaterThan(30);
  expect(Math.abs(windowSteps * FIXED_DT - HIT_INVULN_DURATION)).toBeLessThanOrEqual(FIXED_DT + 1e-9);
  expect(visibility).toContain(true);
  expect(visibility).toContain(false);
  const snap = sim.getRenderSnapshot();
  updatePlayerSprite(sprite, p, snap.camera.x, snap.elapsed);
  expect(fake.visible).toBe(true);

  // Once the second is up, the next overlap costs a heart again.
  placeOn(sim, enemy);
  step(sim);
  expect(sim.getHudSnapshot().hearts).toBe(1);
});

/** Push the player onto level two's first pinecone until a heart is lost. */
function hitOnce(s: Screen, sim: GameSimulation): void {
  const hearts = sim.getHudSnapshot().hearts;
  for (let i = 0; i < 120 && sim.getHudSnapshot().hearts === hearts; i++) {
    placeOn(sim, sim.getRenderSnapshot().enemies[0]);
    s.frame();
  }
  expect(sim.getHudSnapshot().hearts).toBe(hearts - 1);
}

test('platformer.enemy.02: Losing every heart shows game over and returns to menu', () => {
  // Level two is the first with pinecones; unlock it, then play it.
  saveUnlockedLevels([0, 1]);
  screen = new Screen(<App />);
  screen.click('Level 2');
  const sim = screen.sim;

  // Given one heart remaining during play.
  hitOnce(screen, sim);
  hitOnce(screen, sim);
  expect(screen.container.querySelectorAll('.hud-hearts img.heart-full')).toHaveLength(1);
  expect(screen.container.querySelector('.game-over')).toBeNull();

  // When a pinecone contact removes that last heart.
  hitOnce(screen, sim);

  // Then the hearts counter reads zero and a game over screen appears.
  expect(sim.getHudSnapshot().hearts).toBe(0);
  expect(screen.container.querySelectorAll('.hud-hearts img.heart-full')).toHaveLength(0);
  expect(screen.container.querySelectorAll('.hud-hearts img.heart-empty')).toHaveLength(MAX_HEARTS);
  expect(screen.container.querySelector('.game-over')?.textContent).toContain('Game Over');

  // And the simulation stops accepting movement input: held arrows and
  // jumps from the real keyboard handler move nothing.
  const frozenAt = { ...player(sim) };
  screen.press('ArrowRight');
  for (let i = 0; i < 60; i++) {
    if (i % 10 === 0) { screen.release('Space'); screen.press('Space'); }
    screen.frame();
  }
  expect({ ...player(sim) }).toEqual(frozenAt);

  // And dismissing game over returns to the level-select menu.
  screen.click('Return to Menu');
  expect(screen.container.querySelector('.game-over')).toBeNull();
  expect(screen.container.querySelector('.hud')).toBeNull();
  expect(screen.container.querySelector('.level-select')).not.toBeNull();
  expect(screen.button('Level 1')?.disabled).toBe(false);
});
