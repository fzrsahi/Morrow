import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://fzrsahi.github.io',
  base: '/Morrow',
  outDir: '../dist-web',
  emptyOutDir: true,
  build: { format: 'file' },
});
