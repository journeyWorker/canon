import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import '../src/App.css';
import { App } from '../src/App';
import { HUD } from '../src/hud/HUD';
import { LEVELS } from '../src/engine/level';
import { FIXED_DT } from '../src/engine/constants';
import { GameSimulation } from '../src/engine/simulation';
import { frozen, level, Screen, settle, simOf } from './harness';

vi.mock('../src/render/PixiStage', () => ({ PixiStage: () => null }));

beforeEach(() => localStorage.clear());

let screen: Screen | null = null;
afterEach(() => { screen?.unmount(); screen = null; });

test('platformer.hud.01: Pause freezes the simulation and resume continues it', () => {
  // Given the game is running with the player in motion (right held).
  screen = new Screen(<App />);
  screen.click('Level 1');
  const sim = screen.sim;
  screen.press('ArrowRight');
  screen.frames(30);
  expect(sim.getRenderSnapshot().player.vx).toBeGreaterThan(0);

  // When the pause key is pressed and the pause menu opens.
  screen.press('Escape');
  expect(screen.container.querySelector('.pause-menu')).not.toBeNull();

  // Then the simulation halts and the timer stops advancing.
  const frozenState = frozen(sim.getRenderSnapshot());
  const timer = screen.container.querySelector('.hud-level')?.textContent;
  screen.frames(120);
  expect(frozen(sim.getRenderSnapshot())).toEqual(frozenState);
  expect(screen.container.querySelector('.hud-level')?.textContent).toBe(timer);

  // And resume continues from the exact frozen state: after 30 more frames
  // the game matches one that ran the same 60 frames with no pause at all,
  // so the elapsed time excludes the paused interval.
  screen.click('Resume');
  expect(screen.container.querySelector('.pause-menu')).toBeNull();
  screen.frames(30);
  const reference = new GameSimulation(LEVELS);
  for (let i = 0; i < 60; i++) {
    reference.setInput({ left: false, right: true, jumpBuffered: false, restartRequested: false });
    reference.tick(FIXED_DT);
  }
  expect(frozen(sim.getRenderSnapshot())).toEqual(frozen(reference.getRenderSnapshot()));
  expect(sim.getHudSnapshot().elapsed).toBeCloseTo(60 * FIXED_DT, 9);
});

test('platformer.hud.02: HUD reflects hearts acorns and level in real time', () => {
  // A floor with two acorns; a pinecone pinned on the second one, so one
  // step both costs a heart and collects an acorn.
  const sim = simOf(level([
    '............',
    '............',
    '.S.A..A.....',
    '############',
  ], { enemies: [{ x: 6 * 48 + 10, y: 112, patrolMinX: 6 * 48 + 10, patrolMaxX: 6 * 48 + 10 }] }));
  settle(sim);
  screen = new Screen(<HUD sim={sim} />);
  const hud = screen.container;

  // The hearts row is the HUD's first item, the level label its middle, the
  // acorn count its last, spread across the top edge.
  const items = [...(hud.querySelector('.hud')?.children ?? [])].map((el) => el.className);
  expect(items).toEqual(['hud-hearts', 'hud-level', 'hud-acorns']);
  const bar = getComputedStyle(hud.querySelector('.hud')!);
  expect(bar.position).toBe('absolute');
  expect(bar.top).toBe('0px');
  expect(bar.justifyContent).toBe('space-between');

  const shown = () => ({
    hearts: hud.querySelectorAll('.hud-hearts img.heart-full').length,
    acorns: hud.querySelector('.hud-acorns')?.textContent,
    label: hud.querySelector('.hud-level')?.textContent,
  });
  const expected = () => {
    const s = sim.getHudSnapshot();
    const secs = Math.floor(s.elapsed);
    return {
      hearts: s.hearts,
      acorns: `${s.acorns} / ${s.acornsTotal}`,
      label: `Level ${s.levelIndex + 1} · ${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`,
    };
  };

  // Given mid-level with three hearts and one acorn collected.
  screen.press('ArrowRight');
  while (sim.getHudSnapshot().acorns === 0) screen.frame(undefined, sim);
  expect(shown()).toEqual({ ...expected(), hearts: 3, acorns: '1 / 2' });

  // When a heart is lost and another acorn is collected — on one step.
  // Then the hearts row and acorn count change on that same frame, and no
  // HUD value ever lags the simulation, the label included.
  const labels = new Set<string | undefined>();
  let eventFrame = -1;
  for (let i = 0; i < 90; i++) {
    const before = sim.getHudSnapshot();
    screen.frame(undefined, sim);
    const after = sim.getHudSnapshot();
    expect(shown()).toEqual(expected());
    labels.add(shown().label);
    if (after.hearts !== before.hearts) {
      expect(after.acorns).toBe(before.acorns + 1);
      eventFrame = i;
    }
  }
  expect(eventFrame).toBeGreaterThanOrEqual(0);
  expect(shown()).toMatchObject({ hearts: 2, acorns: '2 / 2' });

  // And the top-center label shows the current level and a running timer.
  expect([...labels].every((l) => l?.startsWith('Level 1 · '))).toBe(true);
  expect(labels.size).toBeGreaterThan(1);
});
