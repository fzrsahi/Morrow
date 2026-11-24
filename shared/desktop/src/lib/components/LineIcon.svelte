<script lang="ts">
  // #202 — the Modern Clean icons are the SF-Symbol stand-ins from the
  // prototype's components.js `ic()`; Pixel 16-bit keeps PixelIcon's bitmaps.
  const PATHS = {
    cpu: '<rect x="4" y="4" width="8" height="8" rx="1.5"/><path d="M6.5 2v2M9.5 2v2M6.5 12v2M9.5 12v2M2 6.5h2M2 9.5h2M12 6.5h2M12 9.5h2"/>',
    note: '<path d="M6 12V3.5l7-1.5v8.5"/><circle cx="4.5" cy="12" r="1.6"/><circle cx="11.5" cy="10.5" r="1.6"/>',
    spk: '<path d="M2.5 6h2.5l3.5-3v10l-3.5-3H2.5z"/><path d="M11 5.5a3.5 3.5 0 0 1 0 5"/>',
    gear: '<circle cx="8" cy="8" r="2.2"/><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M3.6 12.4L5 11M11 5l1.4-1.4"/>',
    power: '<path d="M8 2v5.5"/><path d="M4.8 4.3a5 5 0 1 0 6.4 0"/>',
    branch: '<circle cx="5" cy="4" r="1.6"/><circle cx="5" cy="12" r="1.6"/><circle cx="11" cy="5.5" r="1.6"/><path d="M5 5.6v4.8M11 7.1c0 2.4-6 1.6-6 3.3"/>',
    moon: '<path d="M12.5 10.5A5 5 0 0 1 6 3.8a5 5 0 1 0 6.5 6.7z"/>',
  } as const;

  type IconName = keyof typeof PATHS;
  type Props = { name: IconName; color?: string; size?: number };
  let { name, color = 'currentColor', size = 11 }: Props = $props();
</script>

<svg
  class="ico"
  viewBox="0 0 16 16"
  width={size}
  height={size}
  style="color:{color}"
  aria-hidden="true"
>
  {@html PATHS[name]}
</svg>

<style>
  .ico {
    flex: none;
    stroke: currentColor;
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    vertical-align: middle;
  }
</style>
