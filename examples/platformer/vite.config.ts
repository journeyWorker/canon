/// <reference types="vitest/config" />
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
  // One test per scenario in specs/, named with the scenario id, so
  // `canon evidence add --report junit:reports/junit.xml --all-scenarios`
  // binds each scenario to its own cases. jsdom hosts the React HUD and
  // screens. It has no WebGL, so tests stub PixiStage where they mount
  // <App/>, and enemy.test stubs pixi.js's Sprite class.
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.{ts,tsx}'],
    // The HUD's placement is App.css's; load it so layout can be read back.
    css: true,
  },
});
