# Morrow website (Astro)

Source for `https://fzrsahi.github.io/Morrow/`. The site on `main` is a static build;
the `gh-pages` branch holds only the published output.

This is a stack migration of the existing page, not a redesign: markup, CSS, the
Pixel/Modern switch, and the illustrations are carried over unchanged.

## Commands

```sh
cd web
npm install
npm run dev      # http://localhost:4321/Morrow/
npm run build    # writes ../dist-web/
npm run preview  # serves the built site
```

## Publishing

Build first, then copy `dist-web/` to the root of the `gh-pages` branch and commit
there. Pages is configured to publish from that branch's root. Do not add a GitHub
Actions workflow without @aazhrt's approval — CI files are owned separately.

## Asset paths

`astro.config.mjs` sets `base: '/Morrow'`. Every local asset in `index.astro` is
prefixed with `import.meta.env.BASE_URL`; do not use root-absolute `/assets/...`
paths, because they break on the project pages URL.

## Performance budget

The brief for #475 asks for a load under two seconds. The page ships no framework
runtime and no client-side routing; the only JavaScript is the theme switch and the
existing moment script. Keep it that way: no new dependency, no hydration, and
compress raster assets before adding them to `public/`.
