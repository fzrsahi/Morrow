import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri embeds dist/ (src-tauri/tauri.conf.json `build.frontendDist`). The
// island and the lazily created Settings window are separate HTML entries, so
// their JS never loads in the other window.
export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: { input: { main: 'index.html', settings: 'settings.html' } },
  },
});
