import { Texture } from 'pixi.js';

export const ASSET_DEFS: { name: string; chroma: boolean }[] = [
  { name: 'red-panda', chroma: true },
  { name: 'squirrel', chroma: true },
  { name: 'acorn', chroma: true },
  { name: 'pinecone', chroma: true },
  { name: 'heart', chroma: true },
  { name: 'tile', chroma: false },
  { name: 'background', chroma: false },
  { name: 'platform', chroma: true },
];

// Zeroes alpha on near-white pixels so pixel-art sprites on a white
// background composite cleanly over the game's warm palette. platform.png
// has white vertical margins around its full-width art that must also key
// out, so it is chroma-keyed like the character sprites.
export function chromaKeyToCanvas(img: HTMLImageElement): HTMLCanvasElement {
  const off = document.createElement('canvas');
  off.width = img.width;
  off.height = img.height;
  const ctx = off.getContext('2d')!;
  ctx.drawImage(img, 0, 0);
  const frame = ctx.getImageData(0, 0, off.width, off.height);
  const d = frame.data;
  for (let i = 0; i < d.length; i += 4) {
    if (d[i] >= 240 && d[i + 1] >= 240 && d[i + 2] >= 240) d[i + 3] = 0;
  }
  ctx.putImageData(frame, 0, 0);
  return off;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  const { promise, resolve, reject } = Promise.withResolvers<HTMLImageElement>();
  const img = new Image();
  img.onload = () => resolve(img);
  img.onerror = reject;
  img.src = src;
  return promise;
}

export async function loadTextures(): Promise<Record<string, Texture>> {
  const entries = await Promise.all(
    ASSET_DEFS.map(async (def) => {
      const img = await loadImage(`assets/${def.name}.png`);
      const source = def.chroma ? chromaKeyToCanvas(img) : img;
      return [def.name, Texture.from(source)] as const;
    })
  );
  return Object.fromEntries(entries);
}
