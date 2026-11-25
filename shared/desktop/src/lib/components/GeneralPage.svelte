<script lang="ts">
  // #462 — the one General control this shell obeys: the UI language. The tag
  // is written through the same settings_set command as every other page, and
  // the caller's translator is built from resolveLanguage, so the window
  // redraws in the new language without a restart.
  import { isLanguage, LANGUAGES, type Language } from '../strings';

  type Props = {
    stored: string;
    t: (key: string) => string;
    onChange: (tag: string) => void;
  };
  let { stored, t, onChange }: Props = $props();

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
</div>

<style>
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
</style>
