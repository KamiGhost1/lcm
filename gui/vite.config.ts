import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// Single source of truth for the version: the top-level VERSION file. We inject
// it at build time so the UI never hardcodes a version (see scripts/set-version.sh).
const appVersion = readFileSync(fileURLToPath(new URL('../VERSION', import.meta.url)), 'utf8').trim();

// Tauri serves the built assets from a relative base; the dev server is also
// what Tauri points its webview at during `tauri dev`. When opened in a plain
// browser (no Tauri), the app falls back to mock data — see src/api/client.ts.
export default defineConfig({
  plugins: [react()],
  base: './',
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  server: {
    host: true,
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    target: 'es2021',
    sourcemap: false,
  },
});
