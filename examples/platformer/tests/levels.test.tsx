import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { App } from '../src/App';
import { FIXED_DT } from '../src/engine/constants';
import { placeOn, Screen } from './harness';

vi.mock('../src/render/PixiStage', () => ({ PixiStage: () => null }));

beforeEach(() => localStorage.clear());

let screen: Screen | null = null;
afterEach(() => { screen?.unmount(); screen = null; });

/** A browser reload: the page's React tree and simulation start over; localStorage stays. */
function reload(): Screen {
  screen?.unmount();
  screen = new Screen(<App />);
  return screen;
}

function unlocked(s: Screen): boolean[] {
  return ['Level 1', 'Level 2', 'Level 3'].map((name) => s.button(name)?.disabled === false);
}

/**
 * From the menu, play level one: collect `acorns` acorns, then reach the
 * squirrel on the step that makes the level's elapsed time `seconds`.
 */
function clearLevelOne(s: Screen, acorns: number, seconds: number): void {
  s.click('Level 1');
  const sim = s.sim;
  for (const acorn of sim.getRenderSnapshot().acorns.slice(0, acorns)) {
    placeOn(sim, acorn, 'centre');
    s.frame();
  }
  const total = Math.round(seconds / FIXED_DT);
  s.frames(total - acorns - 1);
  placeOn(sim, sim.getRenderSnapshot().squirrel);
  s.frame();
  const result = sim.getHudSnapshot().levelResult;
  expect(result?.acorns).toBe(acorns);
  expect(result?.timeSeconds).toBeCloseTo(seconds, 6);
}

test('platformer.levels.01: Completing a level unlocks the next in the menu', () => {
  // Given only level one is unlocked in the level-select menu.
  const s = reload();
  expect(unlocked(s)).toEqual([true, false, false]);

  // When the player reaches the squirrel goal and completes level one.
  s.click('Level 1');
  const sim = s.sim;
  for (const acorn of sim.getRenderSnapshot().acorns.slice(0, 2)) {
    placeOn(sim, acorn, 'centre');
    s.frame();
  }
  s.frames(40);
  placeOn(sim, sim.getRenderSnapshot().squirrel);
  s.frame();

  // Then the level-complete screen shows collected acorns and elapsed time.
  const result = sim.getHudSnapshot().levelResult!;
  expect(result.acorns).toBe(2);
  expect(result.timeSeconds).toBeCloseTo(43 * FIXED_DT, 9);
  const complete = s.container.querySelector('.level-complete')?.textContent ?? '';
  expect(complete).toContain(`Acorns: 2 / ${result.acornsTotal}`);
  expect(complete).toContain(`Time: ${result.timeSeconds.toFixed(1)}s`);

  // And level two becomes selectable in the menu.
  s.click('Level Select');
  expect(unlocked(s)).toEqual([true, true, false]);

  // And returning to the menu keeps level two unlocked: after playing it
  // and quitting back, and after a reload.
  s.click('Level 2');
  expect(s.sim.getHudSnapshot().levelIndex).toBe(1);
  s.press('Escape');
  s.click('Quit to Menu');
  expect(unlocked(s)).toEqual([true, true, false]);
  expect(unlocked(reload())).toEqual([true, true, false]);
});

test('platformer.levels.02: Best acorns and time persist per level across reloads', () => {
  const best = (s: Screen) => s.button('Level 1')?.textContent;

  // Given level one was cleared with a best of eleven acorns in fifty seconds.
  let s = reload();
  clearLevelOne(s, 11, 50);
  expect(s.container.querySelector('.level-complete')?.textContent).toContain('(best: 11)');

  // When the browser is reloaded and the menu is reopened.
  s = reload();

  // Then level one still shows eleven acorns and fifty seconds as its best.
  expect(best(s)).toContain('best 11 acorns / 50.0s');

  // And clearing it again with fewer acorns or a slower time leaves the best unchanged.
  clearLevelOne(s, 9, 50);
  s.click('Level Select');
  expect(best(s)).toContain('best 11 acorns / 50.0s');
  clearLevelOne(s, 11, 55);
  s.click('Level Select');
  s = reload();
  expect(best(s)).toContain('best 11 acorns / 50.0s');

  // And a faster time or more acorns replaces the stored best.
  clearLevelOne(s, 11, 45);
  s.click('Level Select');
  expect(best(s)).toContain('best 11 acorns / 45.0s');
  clearLevelOne(s, 12, 60);
  s.click('Level Select');
  expect(best(reload())).toContain('best 12 acorns / 45.0s');
});

test('platformer.levels.03: Corrupted saved progress falls back to a fresh start without breaking the menu', () => {
  const store = (value: unknown) => localStorage.setItem('rpr.v2.progress', typeof value === 'string' ? value : JSON.stringify(value));
  const best = { '1': { acorns: 5, timeMs: 50000 } };

  // Given saved progress that is not JSON, that unlocks a level beyond the
  // last, or that carries an unknown version.
  for (const saved of [
    'not json {',
    { version: 1, unlockedLevel: 99, best },
    { version: 1, unlockedLevel: 0, best },
    { version: 2, unlockedLevel: 2, best },
    { version: 1, unlockedLevel: 2, best: 'nope' },
  ]) {
    store(saved);
    // When the game starts and the level-select menu is shown.
    const s = reload();
    // Then only level one is unlocked and no best is shown.
    expect(unlocked(s)).toEqual([true, false, false]);
    expect(s.button('Level 1')?.textContent).not.toContain('best');
  }

  // And saved progress holding one valid and one malformed best keeps the
  // valid best and unlocks the saved level.
  store({ version: 1, unlockedLevel: 2, best: { ...best, '2': 'bad' } });
  const s = reload();
  expect(unlocked(s)).toEqual([true, true, false]);
  expect(s.button('Level 1')?.textContent).toContain('best 5 acorns / 50.0s');
  expect(s.button('Level 2')?.textContent).not.toContain('best');
});
