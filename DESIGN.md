# Design direction — Morrow website (#475)

Status: **PROPOSAL**, not yet validated. Three clickable variants live in `prototype/index.html`
(`/prototype/?variant=A|B|C`). A variant must win there before this file becomes the spec.

## Question this answers

What should `https://fzrsahi.github.io/Morrow/` look like, and how honest should the
platform story be while Linux/Windows have no real host evidence?

## Sources

Three Refero style records, read 2026-11-26 via `https://api.refero.design/v1/styles/<uuid>`:

| Ref | Site | Role in the mix |
|---|---|---|
| `5fdd662c-…` | cofounder.co | Warm canvas, two-tone headline in one weight, mono eyebrow, physical controls |
| `5a899c5e-…` | generalintelligencecompany.com | Glass promise panel, floating content-fit nav, optimistic scale |
| `8b0c66e4-…` | teenage.engineering | Technical structure: campaign hero, headline tab, signal band, black spec band |

## Shared thesis

All three agree on four things, and that agreement is the real basis for combining them:

1. **Art leads.** The opening is a full-width scene, not a floating app screenshot on a card.
2. **Canvas stays nearly colourless.** `#f5f5f2` / `#fefffc` / `#f9faf9` dominate.
3. **Colour is a signal, not a theme.** Scene colour lives in the artwork; `#ff5000` or
   `#c2501b` is reserved for one band, one tab, or one small detail.
4. **Controls are small physical objects.** 31–41px high, 4–8px corners, fine border,
   inset highlight. Never a large friendly pill.

## Composition: 60 / 25 / 15, not 33 / 33 / 33

Averaging three styles produces slop. The mix has a hierarchy:

- **60% teenage.engineering — structure.** Morrow is an engineering tool. Use the
  image-led hero, the short headline tab, the full-width signal band, and a decisive
  dark technical band for the platform matrix. `hero-island.svg` is treated as a
  *product object*, not a dashboard tile.
- **25% cofounder.co — warmth and rhythm.** Use `#f5f5f2`, headline in two tones at a
  single weight (400, not bold), 12px uppercase mono eyebrow at `0.12em`, and low-contrast
  raised panels with inset highlights. This stops the technical structure from going cold.
- **15% generalintelligencecompany — one detail.** Use the glass promise panel over the
  hero art and a content-fit floating nav. Keep the theme switch that #24 already approved.

### Deliberately rejected

- Pixel nature parks — no relation to a notch panel.
- `ppmondwest`, `Departure Mono`, `TechnoType` — embedded/paid faces. Keep the existing
  Google fonts (Gabarito, Figtree, DM Mono) and change scale, tracking, and weight instead.
- "Add to bag", ticker-as-sales-pitch, price tiles — e-commerce patterns, not an installer.
- `#ff5000` on small text: ~3.3:1 on near-white. It passes only for large display or as a
  band background with light text. Small links stay in the existing `#c2501b` accent.
- New font files, new dependencies, new build step, and generated pixel art. The four
  existing `shots/*.svg` are original art and are enough.

## Tokens

```css
--canvas:      #f5f5f2;  /* cofounder: warm paper */
--canvas-alt:  #f9faf9;  /* teenage: cool paper, technical chapters only */
--ink:         #171717;  /* strongest small text */
--heading:     #4f4d4c;  /* primary heading tone */
--body:        #646261;
--faint:       #8e8c8b;  /* second half of a two-tone headline */
--edge:        #e7e7e3;
--raised:      #fbfbf8;
--panel:       #e5e5e5;  /* technical tables */
--signal:      #ff5000;  /* band/tab background only */
--signal-soft: #c2501b;  /* existing accent, small links */
--band-text:   #e9e8e1;

--eyebrow: DM Mono 12px / 1.5 / 0.12em / uppercase;
--headline: Gabarito 400 / 46px / 1.08 / -0.03em;
--section-heading: Gabarito 400 / 40px / 1.15;
--body-copy: Figtree 460 / 15px / 1.4 / 0.01em;
--caption: DM Mono 13px / 1.3;

--action: height 41px, radius 8px, 12px padding, fine border,
          inset 0 2px 0 rgba(255,255,255,.35), 0 2px 3px rgba(0,0,0,.06);
--glass: radius 16px, border 1px rgba(255,255,255,.2),
         background linear-gradient(rgba(0,0,0,.12), rgba(0,0,0,.07)),
         backdrop-filter blur(15px);
```

## Section map

| Current | Proposed |
|---|---|
| Header, full-width with Pixel/Modern switch | Content-fit floating nav (54px, top 16px, blur 9px); theme switch kept |
| Hero: notice + headline + 2 CTAs beside SVG | Full-width `hero-island.svg`; glass promise panel left; eyebrow + two-tone headline; honest CTA |
| "The notch panel" 3 cards | Numbered capability rows (`01/02/03`) in mono, `approval.svg` beside them |
| "Morrow on macOS, Linux, Windows" | **Dark technical band**: 3-row platform matrix with status and reason, not equal billing |
| Footer "Download Morrow" | Signal band: macOS installer available now; Linux/Windows preview, with the reason |
| 158 asset files | Only `shots/*.svg` and the handful of CSS files the page actually uses |

## Content honesty rules (non-negotiable)

- No CTA may point at `github.com/fzrsahi/Morrow/releases`: it currently has 0 tags and
  0 releases. Use a real installer URL once one exists, otherwise say so plainly.
- Do not claim "packages ship for every desktop". macOS is the finished product;
  Linux X11 and Windows are ports whose host gates have not run.
- The Windows row must name the named-pipe/`ssh -R` limitation (#451).
- Copyright/artwork note stays: the shots are illustrative panel artwork.

## Accessibility constraints

- Signal band: light text on `#ff5000`, display size only. No small text on that fill.
- Glass panel over art needs a dark enough area behind it; keep the existing white text
  and the 15px blur.
- Theme switch keeps `aria-pressed` and keyboard focus states.
- Respect `prefers-reduced-motion` for the existing enter animation and liquid type.

## Open decisions

1. Which prototype variant wins: A (recommended), B, or C — or a hybrid?
2. Does @fzrsahi want visual change at all, or only the honest content fix (#475 has no body)?
3. What is the real macOS download URL — Homebrew tap (`wxtsky/tap` today), a direct DMG,
   or neither yet? This decides the primary CTA label.
