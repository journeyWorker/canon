// Shared drivers for the scenario tests. Everything here calls the game's
// own engine and React components; nothing re-implements game rules.
import { act, type ReactElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { GameSimulation, runFrame } from '../src/engine/simulation';
import { InputCapture, type InputSnapshot } from '../src/engine/input';
import { FIXED_DT } from '../src/engine/constants';
import type { LevelData, PlayerState, Rect } from '../src/engine/types';

declare global {
  // React reads this to know act() wraps every update (no act warnings).
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

export const IDLE: InputSnapshot = { left: false, right: false, jumpBuffered: false, restartRequested: false };

/** A test level from a grid in the game's own tile format (see LevelData). */
export function level(grid: string[], extra: Partial<Omit<LevelData, 'grid'>> = {}): LevelData {
  return { id: 0, enemies: [], platforms: [], ...extra, grid };
}

export function simOf(...levels: LevelData[]): GameSimulation {
  return new GameSimulation(levels);
}

/** Advance exactly one fixed physics step with `input` held. */
export function step(sim: GameSimulation, input: Partial<InputSnapshot> = {}): void {
  sim.setInput({ ...IDLE, ...input });
  sim.tick(FIXED_DT);
}

export function steps(sim: GameSimulation, n: number, input: Partial<InputSnapshot> = {}): void {
  for (let i = 0; i < n; i++) step(sim, input);
}

export function player(sim: GameSimulation): PlayerState {
  return sim.getRenderSnapshot().player;
}

/** Step idle until the player rests on a surface. */
export function settle(sim: GameSimulation): void {
  for (let i = 0; i < 240 && !player(sim).onGround; i++) step(sim);
  if (!player(sim).onGround) throw new Error('player never landed');
}

/**
 * Put the player's hitbox on `target`, at rest: standing on its bottom edge
 * (a pinecone, the squirrel), or centred on it (an acorn, which floats
 * mid-cell; centred, the player fits inside that open cell and touches no
 * solid tile around it).
 */
export function placeOn(sim: GameSimulation, target: Rect, align: 'bottom' | 'centre' = 'bottom'): void {
  const p = player(sim);
  p.x = target.x + target.w / 2 - p.w / 2;
  p.y = align === 'bottom' ? target.y + target.h - p.h : target.y + target.h / 2 - p.h / 2;
  p.vx = 0;
  p.vy = 0;
}

/** A JSON copy, so later steps cannot mutate what was captured. */
export function frozen<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function key(type: 'keydown' | 'keyup', code: string): void {
  window.dispatchEvent(new KeyboardEvent(type, { code }));
}

/**
 * The browser side of the game: <App/> mounted in jsdom, with real keyboard
 * input captured by the game's InputCapture and fed to the simulation one
 * render frame at a time by runFrame, exactly as PixiStage's ticker does.
 */
export class Screen {
  readonly root: Root;
  readonly container: HTMLDivElement;
  readonly input = new InputCapture();

  constructor(element: ReactElement) {
    this.container = document.createElement('div');
    document.body.appendChild(this.container);
    this.root = createRoot(this.container);
    act(() => this.root.render(element));
  }

  get sim(): GameSimulation {
    const sim = window.__sim;
    if (!sim) throw new Error('App did not expose its simulation');
    return sim;
  }

  frame(deltaMS = FIXED_DT * 1000, sim: GameSimulation = this.sim): void {
    act(() => runFrame(sim, this.input, deltaMS));
  }

  frames(n: number, deltaMS?: number): void {
    for (let i = 0; i < n; i++) this.frame(deltaMS);
  }

  press(code: string): void {
    act(() => key('keydown', code));
  }

  release(code: string): void {
    act(() => key('keyup', code));
  }

  click(text: string): void {
    const button = this.button(text);
    if (!button) throw new Error(`no button "${text}" in: ${this.container.textContent}`);
    act(() => button.click());
  }

  button(text: string): HTMLButtonElement | undefined {
    return [...this.container.querySelectorAll('button')].find((b) => b.textContent?.trim().startsWith(text));
  }

  unmount(): void {
    act(() => this.root.unmount());
    this.input.dispose();
    this.container.remove();
  }
}
