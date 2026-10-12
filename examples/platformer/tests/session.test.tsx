import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { App } from '../src/App';
import { LevelCompleteScreen } from '../src/hud/LevelCompleteScreen';
import { LEVELS } from '../src/engine/level';
import { saveRecordIfBetter } from '../src/engine/records';
import { GameSimulation } from '../src/engine/simulation';
import { frozen, level, placeOn, player, Screen, settle, simOf, step } from './harness';

vi.mock('../src/render/PixiStage', () => ({ PixiStage: () => null }));

beforeEach(() => localStorage.clear());

let screen: Screen | null = null;
afterEach(() => { screen?.unmount(); screen = null; vi.restoreAllMocks(); });

/** Start level one from the menu, collect `acorns` acorns, and reach the squirrel. */
function winLevelOne(s: Screen, acorns: number): GameSimulation {
  const sim = s.sim;
  for (const acorn of sim.getRenderSnapshot().acorns.slice(0, acorns)) {
    placeOn(sim, acorn, 'centre');
    s.frame();
  }
  s.frames(20);
  placeOn(sim, sim.getRenderSnapshot().squirrel);
  s.frame();
  expect(s.container.querySelector('.level-complete')).not.toBeNull();
  return sim;
}

test('platformer.session.01: Collecting every acorn shows a full counter', () => {
  // Given a fresh level with ten acorns, all on the way to the squirrel.
  const sim = simOf(level([
    '..............',
    '.SAAAAAAAAAA.Q',
    '##############',
  ]));
  settle(sim);
  expect(sim.getHudSnapshot()).toMatchObject({ acorns: 0, acornsTotal: 10 });

  // When the player collects every acorn and reaches the squirrel.
  for (let i = 0; i < 300 && !sim.getHudSnapshot().levelComplete; i++) step(sim, { right: true });
  const result = sim.getHudSnapshot().levelResult!;
  expect(result).toMatchObject({ acorns: 10, acornsTotal: 10 });

  // Then the counter reads ten out of ten on the win overlay.
  screen = new Screen(
    <LevelCompleteScreen
      result={result}
      record={saveRecordIfBetter(0, result)}
      hasNext={false}
      onNext={() => {}}
      onMenu={() => {}}
    />,
  );
  expect(screen.container.querySelector('.level-complete')?.textContent).toContain('Acorns: 10 / 10');
});

test('platformer.session.02: Win overlay freezes gameplay input', () => {
  // Given the win overlay is showing.
  screen = new Screen(<App />);
  screen.click('Level 1');
  const sim = winLevelOne(screen, 0);
  const at = frozen(player(sim));

  // When movement and jump keys are held (and jump re-pressed).
  screen.press('ArrowRight');
  screen.press('KeyD');
  for (let i = 0; i < 120; i++) {
    if (i % 10 === 0) for (const jump of ['Space', 'ArrowUp', 'KeyW']) { screen.release(jump); screen.press(jump); }
    screen.frame();
  }

  // Then the player's position does not change.
  expect(frozen(player(sim))).toEqual(at);
  expect(screen.container.querySelector('.level-complete')).not.toBeNull();
});

test('platformer.session.03: R restarts to a clean initial state', () => {
  const initial = new GameSimulation(LEVELS);
  const clean = { world: frozen(initial.getRenderSnapshot()), hud: frozen(initial.getHudSnapshot()) };
  screen = new Screen(<App />);
  screen.click('Level 1');

  for (let cycle = 1; cycle <= 2; cycle++) {
    // Given a won game with a nonzero score (the camera has scrolled to the squirrel).
    const sim = winLevelOne(screen, 3);
    expect(sim.getHudSnapshot().acorns).toBe(3);
    expect(sim.getRenderSnapshot().camera.x).toBeGreaterThan(0);

    // When R is pressed.
    screen.press('KeyR');
    screen.frame();

    // Then the player, score, camera, and every acorn reset to the initial
    // state, and play resumes from the overlay. A second win-and-restart
    // cycle resets just as cleanly.
    expect(frozen(sim.getRenderSnapshot())).toEqual(clean.world);
    expect(sim.getHudSnapshot()).toMatchObject({
      acorns: 0, elapsed: 0, hearts: clean.hud.hearts, levelComplete: false, levelResult: null,
    });
    expect(screen.container.querySelector('.level-complete')).toBeNull();
    expect(screen.container.querySelector('.hud-acorns')?.textContent).toBe(`0 / ${clean.hud.acornsTotal}`);
  }
});

test('platformer.session.04: Idle and resize sessions stay error-free', () => {
  const errors = vi.spyOn(console, 'error');
  const warnings = vi.spyOn(console, 'warn');
  screen = new Screen(<App />);
  screen.click('Level 1');
  const sim = screen.sim;

  // Given a running game left idle for sixty seconds, while the viewport is
  // resized repeatedly (each resize also costs a long, clamped frame).
  screen.frames(60);
  const settled = frozen({ x: player(sim).x, y: player(sim).y });
  let resizes = 0;
  for (let i = 0; sim.getHudSnapshot().elapsed < 60; i++) {
    if (i % 120 === 0) {
      Object.defineProperty(window, 'innerWidth', { value: 640 + (resizes % 5) * 160, configurable: true });
      Object.defineProperty(window, 'innerHeight', { value: 360 + (resizes % 3) * 180, configurable: true });
      window.dispatchEvent(new Event('resize'));
      resizes++;
      screen.frame(100);
    } else {
      screen.frame();
    }
    // Then the player's settled position shows no drift.
    expect({ x: player(sim).x, y: player(sim).y }).toEqual(settled);
  }
  expect(resizes).toBeGreaterThan(20);

  // And the console stays free of errors and warnings.
  expect(errors).not.toHaveBeenCalled();
  expect(warnings).not.toHaveBeenCalled();
  expect(screen.container.querySelector('.hud')).not.toBeNull();
});
