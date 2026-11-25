<script lang="ts">
  // #199 — the Appearance page. Every control writes one top-level key to the
  // existing ~/.codeisland/settings.json through the shell (no new store), so
  // values survive an app upgrade.
  import { SETTINGS_KEYS, type AppearanceSettings } from '../settings';
  import { ACCENTS, isAccent } from '../tokens';
  import {
    TOP_OFFSET_DEFAULT,
    TOP_OFFSET_MAX,
    TOP_OFFSET_MIN,
    clampTopOffset,
    displayMetadata,
    displayTitle,
    type DisplaySnapshot,
  } from '../display';

  type Props = {
    appearance: AppearanceSettings;
    t: (key: string) => string;
    onChange: (key: string, value: string | number | boolean) => void;
    // #239 AC 1: the shell's `reset_island_position` forgets the offset stored
    // for *this* monitor and re-fits. Resetting the `position` key alone left the
    // island where it was, which is what the old button actually did.
    onResetPosition: () => void;
    // #465: the per-display rows mirror Swift's DisplayConfigRow. The shell
    // resolves the stored map against the connected screens before it gets here.
    displays: DisplaySnapshot;
    onDisplaySet: (id: string, enabled: boolean, mode: string, offset: number) => void;
    onDisplayActivate: (id: string) => void;
    onIslandEnabled: (enabled: boolean) => void;
  };
  let {
    appearance,
    t,
    onChange,
    onResetPosition,
    displays,
    onDisplaySet,
    onDisplayActivate,
    onIslandEnabled,
  }: Props = $props();

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

<h3>{t('displays_section')}</h3>
<div class="grp">
  <div>
    <span class="n">{t('island_enabled')}</span>
    <span class="s">{t('active_displays')}: {displays.displays.length}</span>
    <button
      class="sw"
      class:off={!displays.islandEnabled}
      role="switch"
      aria-checked={displays.islandEnabled}
      aria-label={t('island_enabled')}
      onclick={() => onIslandEnabled(!displays.islandEnabled)}
    ></button>
  </div>
</div>
{#each displays.displays as display (display.id)}
  {@const config = displays.configs[display.id] ?? {
    enabled: false,
    displayMode: appearance.displayMode,
    dynamicIslandTopOffset: TOP_OFFSET_DEFAULT,
  }}
  <div class="grp display">
    <div>
      <span class="n">{displayTitle(display, t)}</span>
      <span class="s">{displayMetadata(display, t)}</span>
      <button
        class="sw"
        class:off={!config.enabled}
        role="switch"
        aria-checked={config.enabled}
        aria-label={displayTitle(display, t)}
        onclick={() =>
          onDisplaySet(
            display.id,
            !config.enabled,
            config.displayMode,
            config.dynamicIslandTopOffset,
          )}
      ></button>
    </div>
    {#if config.enabled}
      <div>
        <span class="n">{t('display_mode')}</span>
        <span class="s">
          <span class="seg2">
            <button
              class:on={config.displayMode === 'notch'}
              aria-pressed={config.displayMode === 'notch'}
              onclick={() => onDisplaySet(display.id, true, 'notch', config.dynamicIslandTopOffset)}
            >{t('display_mode_notch')}</button>
            <button
              class:on={config.displayMode === 'dynamicIsland'}
              aria-pressed={config.displayMode === 'dynamicIsland'}
              onclick={() => onDisplaySet(display.id, true, 'dynamicIsland', config.dynamicIslandTopOffset)}
            >{t('display_mode_dynamic_island')}</button>
          </span>
        </span>
      </div>
      {#if config.displayMode === 'dynamicIsland'}
        <div>
          <span class="n">{t('dynamic_island_top_offset')}</span>
          <span class="s">
            <input
              type="range"
              min={TOP_OFFSET_MIN}
              max={TOP_OFFSET_MAX}
              step="1"
              value={clampTopOffset(config.dynamicIslandTopOffset)}
              aria-label={t('dynamic_island_top_offset')}
              oninput={(event) =>
                onDisplaySet(
                  display.id,
                  true,
                  config.displayMode,
                  clampTopOffset(Number(event.currentTarget.value)),
                )}
            />
            {clampTopOffset(config.dynamicIslandTopOffset)}pt
          </span>
        </div>
      {/if}
      <div>
        <span class="n">{t('display')}</span>
        <span class="s">
          {#if displays.primaryEnabledDisplayId === display.id}Active{:else}Available{/if}
        </span>
        <button class="b" onclick={() => onDisplayActivate(display.id)}>Move island here</button>
      </div>
    {:else}
      <div>
        <span class="s">{t('display_hidden_hint').replace('%d', String(display.index + 1))}</span>
      </div>
    {/if}
  </div>
{:else}
  <div class="grp"><div><span class="s">{displays.readerReason ?? t('active_displays')}</span></div></div>
{/each}

<style>
  h3 {
    margin: 14px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .display input[type='range'] {
    width: 130px;
    vertical-align: middle;
    margin-right: 6px;
  }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 50%;
    vertical-align: middle;
  }
</style>
