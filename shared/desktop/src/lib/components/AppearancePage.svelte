<script lang="ts">
  // #199 — the Appearance page. Every control writes one top-level key to the
  // existing ~/.codeisland/settings.json through the shell (no new store), so
  // values survive an app upgrade.
  import { SETTINGS_KEYS, type AppearanceSettings } from '../settings';
  import { ACCENTS, isAccent } from '../tokens';

  type Props = {
    appearance: AppearanceSettings;
    t: (key: string) => string;
    onChange: (key: string, value: string | number | boolean) => void;
    // #239 AC 1: the shell's `reset_island_position` forgets the offset stored
    // for *this* monitor and re-fits. Resetting the `position` key alone left the
    // island where it was, which is what the old button actually did.
    onResetPosition: () => void;
  };
  let { appearance, t, onChange, onResetPosition }: Props = $props();

  const sizes = [11, 12, 14, 16] as const;
  const accents = Object.keys(ACCENTS) as Array<keyof typeof ACCENTS>;
  // #465: Swift's notchAnimationSpeed is a multiplier; 1 is the shipped pace.
  const speeds = [
    { value: 0.5, label: 'Slow' },
    { value: 0.75, label: '0.75×' },
    { value: 1, label: '1×' },
    { value: 1.5, label: '1.5×' },
    { value: 2, label: 'Fast' },
  ] as const;
  const selectedAccent = $derived(isAccent(appearance.accent) ? appearance.accent : 'cyan');
</script>

<h2>{t('appearance')}</h2>
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
    <span class="n">{t('content_font_size')}</span>
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
  <div>
    <span class="n">Accent</span>
    <span class="s">
      <span class="seg2">
        {#each accents as accent}
          <button
            class:on={selectedAccent === accent}
            aria-pressed={selectedAccent === accent}
            aria-label="Accent {accent}"
            onclick={() => onChange(SETTINGS_KEYS.accent, accent)}
          >
            <i class="dot" style:background={ACCENTS[accent]}></i>{accent}
          </button>
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('notch_animation_speed')}</span>
    <span class="s">
      <span class="seg2">
        {#each speeds as speed}
          <button
            class:on={appearance.animationSpeed === speed.value}
            aria-pressed={appearance.animationSpeed === speed.value}
            onclick={() => onChange(SETTINGS_KEYS.animationSpeed, speed.value)}>{speed.label}</button
          >
        {/each}
      </span>
    </span>
  </div>
</div>
<div class="grp">
  <div>
    <span class="n">{t('display_mode')}</span>
    <span class="s">
      <span class="seg2">
        <button class:on={appearance.displayMode === 'notch'} aria-pressed={appearance.displayMode === 'notch'} onclick={() => onChange(SETTINGS_KEYS.displayMode, 'notch')}>{t('display_mode_notch')}</button>
        <button class:on={appearance.displayMode === 'dynamicIsland'} aria-pressed={appearance.displayMode === 'dynamicIsland'} onclick={() => onChange(SETTINGS_KEYS.displayMode, 'dynamicIsland')}>{t('display_mode_dynamic_island')}</button>
      </span>
    </span>
  </div>
  <div>
    <span class="n">Position</span>
    <span class="s">{appearance.position}</span>
    <button class="b" onclick={onResetPosition}>Reset</button>
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
    <span class="n">{t('collapse_on_mouse_leave')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!appearance.collapseOnLeave}
        aria-label={t('collapse_on_mouse_leave')}
        aria-pressed={appearance.collapseOnLeave}
        onclick={() => onChange(SETTINGS_KEYS.collapseOnLeave, !appearance.collapseOnLeave)}
      ></button>
    </span>
  </div>
</div>

<style>
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 50%;
    vertical-align: middle;
  }
</style>
