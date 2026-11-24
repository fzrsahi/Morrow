import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri embeds dist/ (src-tauri/tauri.conf.json `build.frontendDist`). The
// island, the anchoring peek (#200) and the lazily created Settings window are
// separate HTML entries, so their JS never loads in another window.
export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: { main: 'index.html', peek: 'peek.html', settings: 'settings.html' },
    },
  },
});
