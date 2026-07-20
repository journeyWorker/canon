import { Sprite, Texture } from 'pixi.js';
import type { PlayerState } from '../engine/types';
import { PLAYER_DRAW, SQUASH_DURATION } from '../engine/constants';

function lerp(a: number, b: number, t: number) { return a + (b - a) * t; }

export function createPlayerSprite(textures: Record<string, Texture>): Sprite {
  const sprite = new Sprite(textures['red-panda']);
  sprite.anchor.set(0.5, 1);
  return sprite;
}

const BLINK_HZ = 10;

export function updatePlayerSprite(sprite: Sprite, player: PlayerState, cameraX: number, elapsed: number): void {
  const t = 1 - player.squashTimer / SQUASH_DURATION;
  const eased = 1 - Math.pow(1 - t, 3);
  const squashX = player.squashTimer > 0 ? lerp(1.15, 1, eased) : 1;
  const squashY = player.squashTimer > 0 ? lerp(0.85, 1, eased) : 1;
  // Scale is recomputed relative to the texture's native size every frame
  // (never left as an absolute scale.set()) so squash/facing flips never
  // clobber the PLAYER_DRAW-based sizing established at creation.
  const baseScaleX = PLAYER_DRAW.w / sprite.texture.width;
  const baseScaleY = PLAYER_DRAW.h / sprite.texture.height;
  sprite.x = player.x + player.w / 2 - cameraX;
  sprite.y = player.y + player.h;
  sprite.scale.set(player.facing * squashX * baseScaleX, squashY * baseScaleY);
  // Blink at ~10Hz while invulnerable after a pinecone hit.
  sprite.visible = player.invulnTimer <= 0 || Math.floor(elapsed * BLINK_HZ * 2) % 2 === 0;
}
