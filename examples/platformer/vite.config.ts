import { cpSync } from 'node:fs';
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';

// index.html/src reference sprites as plain `assets/${name}.png` URLs (not
// bundler imports), matching v1's raw fetch approach. Vite's `publicDir`
// convention would require moving the folder under `public/`, but the repo
// constraint keeps assets at `examples/platformer/assets/` verbatim -- so a
// tiny build-time copy step (no extra dependency) mirrors it into
// `dist/assets/` on every production build, alongside the dev server's
// built-in static-file serving of the project root.
function copyAssetsOnBuild(): Plugin {
  return {
    name: 'copy-assets-dir',
    apply: 'build',
    writeBundle() {
      cpSync('assets', 'dist/assets', { recursive: true });
    },
  };
}

export default defineConfig({
  plugins: [react(), copyAssetsOnBuild()],
  base: './',
});
