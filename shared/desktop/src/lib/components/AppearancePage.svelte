<script lang="ts">
  // #199 — the Appearance page. Every control writes one top-level key to the
  // existing ~/.codeisland/settings.json through the shell (no new store), so
  // values survive an app upgrade.
  import { DEFAULT_APPEARANCE, SETTINGS_KEYS, type AppearanceSettings } from '../settings';

  type Props = {
    appearance: AppearanceSettings;
    onChange: (key: string, value: string | number | boolean) => void;
  };
  let { appearance, onChange }: Props = $props();

  const sizes = [11, 12, 14, 16] as const;
</script>

<h2>Appearance</h2>
<div class="grp">
  <div>
    <span class="n">Theme</span>
    <span class="s">
      <span class="seg2">
        <button class:on={appearance.theme === 'pixel'} aria-pressed={appearance.theme === 'pixel'} onclick={() => onChange(SETTINGS_KEYS.theme, 'pixel')}>Pixel 16-bit</button>
        <button class:on={appearance.theme === 'modern'} aria-pressed={appearance.theme === 'modern'} onclick={() => onChange(SETTINGS_KEYS.theme, 'modern')}>Modern Clean</button>
      </span>
    </span>
  </div>
  <div>
    <span class="n">Content font size</span>
    <span class="s">
      <span class="seg2">
        {#each sizes as size}
          <button
            class:on={appearance.fontSize === size}
            aria-pressed={appearance.fontSize === size}
            onclick={() => onChange(SETTINGS_KEYS.fontSize, size)}>{size}</button
          >
        {/each}
      </span>
    </span>
  </div>
  <div><span class="n">Accent</span><span class="s">Cyan</span></div>
  <div><span class="n">Animation speed</span><span class="s">{appearance.animationSpeed.toFixed(1)}×</span></div>
</div>
<div class="grp">
  <div>
    <span class="n">Display mode</span>
    <span class="s">
      <span class="seg2">
        <button class:on={appearance.displayMode === 'notch'} aria-pressed={appearance.displayMode === 'notch'} onclick={() => onChange(SETTINGS_KEYS.displayMode, 'notch')}>Notch</button>
        <button class:on={appearance.displayMode === 'dynamicIsland'} aria-pressed={appearance.displayMode === 'dynamicIsland'} onclick={() => onChange(SETTINGS_KEYS.displayMode, 'dynamicIsland')}>Dynamic Island</button>
      </span>
    </span>
  </div>
  <div>
    <span class="n">Position</span>
    <span class="s">{appearance.position}</span>
    <button class="b" onclick={() => onChange(SETTINGS_KEYS.position, DEFAULT_APPEARANCE.position)}>Reset</button>
  </div>
  <div>
    <span class="n">Expand on hover</span>
    <span class="s">
      <button
        class="sw"
        class:off={!appearance.expandOnHover}
        aria-label="Expand on hover"
        aria-pressed={appearance.expandOnHover}
        onclick={() => onChange(SETTINGS_KEYS.expandOnHover, !appearance.expandOnHover)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Collapse when the pointer leaves</span>
    <span class="s">
      <button
        class="sw"
        class:off={!appearance.collapseOnLeave}
        aria-label="Collapse when the pointer leaves"
        aria-pressed={appearance.collapseOnLeave}
        onclick={() => onChange(SETTINGS_KEYS.collapseOnLeave, !appearance.collapseOnLeave)}
      ></button>
    </span>
  </div>
</div>
