import type { PlayerState } from './types';
import { CANVAS_W } from './constants';

export function updateCamera(camera: { x: number }, player: PlayerState, worldW: number): void {
  const target = player.x + player.w / 2 - CANVAS_W / 2;
  camera.x = Math.max(0, Math.min(target, worldW - CANVAS_W));
}
