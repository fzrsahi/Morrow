<script lang="ts">
  // #465 — the Floating Synced Lyrics controls. They write the same Swift
  // settings keys macOS writes; the window itself is a host-gate item (#400)
  // because no real Windows/Linux windowing run is possible from this host.
  import {
    FLOATING_LYRIC_KEYS,
    LYRIC_FONT_DESIGNS,
    LYRIC_ICON_STYLES,
    LYRIC_PRESETS,
    LYRIC_TEXT_COLORS,
    type FloatingLyrics,
  } from '../floating-lyrics';

  type Props = {
    settings: FloatingLyrics;
    t: (key: string) => string;
    onChange: (key: string, value: string | number | boolean) => void;
  };
  let { settings, t, onChange }: Props = $props();
</script>

<h2>{t('floating_synced_lyrics')}</h2>
<p class="note">
  These values are shared with the macOS lyric renderer. This shell has no
  separate always-on-top lyric window yet; its synced lines render inline on
  the island's Music tab.
</p>
<div class="grp">
  <div>
    <span class="n">Preset</span>
    <span class="s">
      <select
        value={settings.preset}
        aria-label="Lyric preset"
        onchange={(event) => onChange(FLOATING_LYRIC_KEYS.preset, event.currentTarget.value)}
      >
        {#each LYRIC_PRESETS as preset (preset)}<option value={preset}>{preset}</option>{/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Icon</span>
    <span class="s">
      <select
        value={settings.iconStyle}
        aria-label="Lyric icon"
        onchange={(event) => onChange(FLOATING_LYRIC_KEYS.iconStyle, event.currentTarget.value)}
      >
        {#each LYRIC_ICON_STYLES as style (style)}<option value={style}>{style}</option>{/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Font</span>
    <span class="s">
      <select
        value={settings.fontDesign}
        aria-label="Lyric font design"
        onchange={(event) => onChange(FLOATING_LYRIC_KEYS.fontDesign, event.currentTarget.value)}
      >
        {#each LYRIC_FONT_DESIGNS as design (design)}<option value={design}>{design}</option>{/each}
      </select>
      <input
        type="number"
        min="8"
        max="32"
        value={settings.fontSize}
        aria-label="Lyric font size"
        onchange={(event) => onChange(FLOATING_LYRIC_KEYS.fontSize, Number(event.currentTarget.value))}
      />
    </span>
  </div>
  <div>
    <span class="n">Colour</span>
    <span class="s">
      <select
        value={settings.textColor}
        aria-label="Lyric text colour"
        onchange={(event) => onChange(FLOATING_LYRIC_KEYS.textColor, event.currentTarget.value)}
      >
        {#each LYRIC_TEXT_COLORS as color (color)}<option value={color}>{color}</option>{/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Line gap</span>
    <span class="s">
      <input
        type="range"
        min="0"
        max="20"
        step="0.5"
        value={settings.verticalGap}
        aria-label="Lyric vertical gap"
        oninput={(event) => onChange(FLOATING_LYRIC_KEYS.verticalGap, Number(event.currentTarget.value))}
      />
      {settings.verticalGap}pt
    </span>
  </div>
  <div>
    <span class="n">Sync offset</span>
    <span class="s">
      <input
        type="range"
        min="-30"
        max="30"
        step="0.5"
        value={settings.syncOffset}
        aria-label="Lyric sync offset"
        oninput={(event) => onChange(FLOATING_LYRIC_KEYS.syncOffset, Number(event.currentTarget.value))}
      />
      {settings.syncOffset}s
    </span>
  </div>
  <div>
    <span class="n">Hide on hover</span>
    <span class="s">Fade the lyric window while the pointer rests on it</span>
    <button
      class="sw"
      class:off={!settings.autoHideOnHover}
      role="switch"
      aria-checked={settings.autoHideOnHover}
      aria-label="Hide lyrics on hover"
      onclick={() => onChange(FLOATING_LYRIC_KEYS.autoHideOnHover, !settings.autoHideOnHover)}
    ></button>
  </div>
</div>

<style>
  .note {
    margin: 0 0 12px;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    line-height: 1.5;
  }
  .grp :is(select, input[type='number']) {
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
    margin-right: 6px;
  }
  .grp input[type='range'] {
    width: 130px;
    vertical-align: middle;
    margin-right: 6px;
  }
</style>
