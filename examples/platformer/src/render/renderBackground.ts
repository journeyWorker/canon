import { Container, Sprite, Texture, Graphics } from 'pixi.js';
import { CANVAS_W, CANVAS_H, BG_SCALE, BG_PARALLAX, HILLS_PARALLAX } from '../engine/constants';

export function createBackgroundLayer(textures: Record<string, Texture>): { view: Container; update: (cameraX: number) => void } {
  const view = new Container();
  const sky = new Sprite(Texture.WHITE);
  sky.tint = 0xF6C89F; sky.width = CANVAS_W; sky.height = CANVAS_H;
  const bg = new Sprite(textures['background']);
  bg.width = textures['background'].width * BG_SCALE;
  bg.height = textures['background'].height * BG_SCALE;
  bg.y = -(bg.height - CANVAS_H) * 0.35;
  const hills = new Graphics();
  view.addChild(sky, bg, hills);
  const panMax = bg.width - CANVAS_W;

  function update(cameraX: number) {
    bg.x = -Math.max(0, Math.min(panMax, cameraX * BG_PARALLAX));
    const hillsOffset = cameraX * HILLS_PARALLAX;
    hills.clear();
    hills.moveTo(0, CANVAS_H);
    const step = 40;
    for (let x = -step; x <= CANVAS_W + step; x += step) {
      const worldX = x + hillsOffset;
      hills.lineTo(x, 380 + Math.sin(worldX * 0.01) * 18);
    }
    hills.lineTo(CANVAS_W, CANVAS_H);
    hills.closePath();
    hills.fill({ color: 0xE8A15C, alpha: 0.6 });
  }

  return { view, update };
}
