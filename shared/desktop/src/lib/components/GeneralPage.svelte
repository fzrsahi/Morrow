<script lang="ts">
  // #462/#465 — the General controls this shell obeys. The language tag is
  // written through settings_set and redraws the window; the two island
  // position switches are the Swift `SettingsKey` spellings and defaults, and
  // `allowHorizontalDrag` is already read by App.svelte's DragGate.
  import { isLanguage, LANGUAGES, type Language } from '../strings';

  type DragField = 'allowHorizontalDrag' | 'avoidMenuBarIcons';
  type Props = {
    stored: string;
    allowHorizontalDrag: boolean;
    avoidMenuBarIcons: boolean;
    t: (key: string) => string;
    onChange: (tag: string) => void;
    onBehaviorChange: (field: DragField, value: boolean) => void;
    onDragDisabled: () => void;
    islandEnabled: boolean;
    displaySummaryText: string;
    onIslandEnabled: (enabled: boolean) => void;
    onConfigureDisplays: () => void;
  };
  let {
    stored,
    allowHorizontalDrag,
    avoidMenuBarIcons,
    t,
    onChange,
    onBehaviorChange,
    onDragDisabled,
    islandEnabled,
    displaySummaryText,
    onIslandEnabled,
    onConfigureDisplays,
  }: Props = $props();

  // Endonyms, not translated labels: macOS's Picker names every language in its
  // own words so the list stays readable whatever the current UI language is.
  const names: Record<Language, string> = {
    en: 'English',
    zh: '简体中文',
    'zh-Hant': '繁體中文',
    de: 'Deutsch',
    ja: '日本語',
    ko: '한국어',
    tr: 'Türkçe',
  };
  const choices = $derived([
    { tag: 'system', name: t('system_language') },
    ...LANGUAGES.map((tag) => ({ tag, name: names[tag] })),
  ]);
  // A malformed stored tag must not leave the picker with no visible selection.
  const selected = $derived(stored === 'system' || isLanguage(stored) ? stored : 'system');

  function toggleDrag() {
    const next = !allowHorizontalDrag;
    onBehaviorChange('allowHorizontalDrag', next);
    // Swift's onChange resets panelHorizontalOffset when the switch goes off.
    // Here the equivalent store is the per-monitor island position.
    if (!next) onDragDisabled();
  }
</script>

<h2>{t('general')}</h2>
<div class="grp">
  <div>
    <span class="n">{t('language')}</span>
    <span class="s">
      <select
        value={selected}
        onchange={(event) => onChange(event.currentTarget.value)}
        aria-label={t('language')}
      >
        {#each choices as choice (choice.tag)}
          <option value={choice.tag}>{choice.name}</option>
        {/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">{t('allow_horizontal_drag')}</span>
    <span class="s">{t('allow_horizontal_drag_desc')}</span>
    <button
      class="sw"
      class:off={!allowHorizontalDrag}
      role="switch"
      aria-checked={allowHorizontalDrag}
      aria-label={t('allow_horizontal_drag')}
      onclick={toggleDrag}
    ></button>
  </div>
  <div>
    <span class="n">{t('avoid_menu_bar_icons')}</span>
    <span class="s">
      {t('avoid_menu_bar_icons_desc')} Windows and Linux have no third-party menu-bar icon layer
      to dodge, so this preference is stored but has no host effect (#240).
    </span>
    <button
      class="sw"
      class:off={!avoidMenuBarIcons}
      role="switch"
      aria-checked={avoidMenuBarIcons}
      aria-label={t('avoid_menu_bar_icons')}
      onclick={() => onBehaviorChange('avoidMenuBarIcons', !avoidMenuBarIcons)}
    ></button>
  </div>
</div>
<h3>{t('displays_section')}</h3>
<div class="grp">
  <div>
    <span class="n">{t('island_enabled')}</span>
    <span class="s">{displaySummaryText}</span>
    <button
      class="sw"
      class:off={!islandEnabled}
      role="switch"
      aria-checked={islandEnabled}
      aria-label={t('island_enabled')}
      onclick={() => onIslandEnabled(!islandEnabled)}
    ></button>
  </div>
  <div>
    <span class="n">{t('active_displays')}</span>
    <span class="s">{displaySummaryText}</span>
    <button class="b" onclick={onConfigureDisplays}>{t('configure_displays')}</button>
  </div>
</div>

<style>
  h3 {
    margin: 14px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  select {
    max-width: 100%;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  .grp > div .s {
    line-height: 1.4;
  }
</style>
