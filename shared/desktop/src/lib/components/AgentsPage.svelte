<script lang="ts">
  // #465 — the Agents page. Every control writes a Swift `SettingsKey` through
  // the same settings_set command; the panel renders theme, font, reply lines,
  // density, row limit and card details. Quota/usage data has no shell reader,
  // so those two controls say so instead of showing an empty chip.
  import {
    AGENT_KEYS,
    type AgentsSettings,
    type QuotaChipMode,
  } from '../agents-settings';

  type Props = {
    settings: AgentsSettings;
    t: (key: string) => string;
    onChange: (key: keyof typeof AGENT_KEYS, value: string | number | boolean) => void;
  };
  let { settings, t, onChange }: Props = $props();

  const sizes = [11, 12, 14, 16] as const;
  const replyLines = [
    { value: 1, label: () => t('1_line_default') },
    { value: 2, label: () => t('2_lines') },
    { value: 3, label: () => t('3_lines') },
    { value: 5, label: () => t('5_lines') },
    { value: 0, label: () => t('unlimited') },
  ] as const;
  const limits = [3, 5, 8, 10, 99] as const;
  const chipModes: Array<{ value: QuotaChipMode; label: () => string }> = [
    { value: 'off', label: () => t('quota_chip_off') },
    { value: 'auto', label: () => t('quota_chip_auto') },
    { value: 'session', label: () => t('quota_chip_session') },
    { value: 'weeklyAll', label: () => t('quota_chip_weekly') },
    { value: 'weeklyScoped', label: () => t('quota_chip_weekly_model') },
  ];
</script>

<h2>{t('agents')}</h2>
<div class="grp">
  <div>
    <span class="n">Enable AI Agents Feature</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.agentsEnabled}
        aria-label="Enable AI Agents Feature"
        aria-pressed={settings.agentsEnabled}
        onclick={() => onChange('agentsEnabled', !settings.agentsEnabled)}
      ></button>
    </span>
  </div>
  <p class="hint">
    Turns off agent session tracking and hides the Agents tab. The desktop feed keeps collecting
    hooks while the tab is hidden, so switching it back on shows the sessions it found.
  </p>
</div>

<div class="grp" class:disabled={!settings.agentsEnabled}>
  <div>
    <span class="n">{t('agents_tab_theme')}</span>
    <span class="s">
      <span class="seg2">
        <button
          class:on={settings.agentsHUDTheme === 'pixelRetro'}
          aria-pressed={settings.agentsHUDTheme === 'pixelRetro'}
          onclick={() => onChange('agentsHUDTheme', 'pixelRetro')}>Pixel Retro</button
        >
        <button
          class:on={settings.agentsHUDTheme === 'modern'}
          aria-pressed={settings.agentsHUDTheme === 'modern'}
          onclick={() => onChange('agentsHUDTheme', 'modern')}>Modern Clean</button
        >
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('content_font_size')}</span>
    <span class="s">
      <span class="seg2">
        {#each sizes as size}
          <button
            class:on={settings.contentFontSize === size}
            aria-pressed={settings.contentFontSize === size}
            onclick={() => onChange('contentFontSize', size)}>{size}</button
          >
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('ai_reply_lines')}</span>
    <span class="s">
      <span class="seg2">
        {#each replyLines as choice}
          <button
            class:on={settings.aiMessageLines === choice.value}
            aria-pressed={settings.aiMessageLines === choice.value}
            onclick={() => onChange('aiMessageLines', choice.value)}>{choice.label()}</button
          >
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('session_list_density')}</span>
    <span class="s">
      <span class="seg2">
        <button
          class:on={settings.sessionListDensity === 'comfortable'}
          aria-pressed={settings.sessionListDensity === 'comfortable'}
          onclick={() => onChange('sessionListDensity', 'comfortable')}
          >{t('density_comfortable')}</button
        >
        <button
          class:on={settings.sessionListDensity === 'compact'}
          aria-pressed={settings.sessionListDensity === 'compact'}
          onclick={() => onChange('sessionListDensity', 'compact')}>{t('density_compact')}</button
        >
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('max_visible_sessions')}</span>
    <span class="s">
      <span class="seg2">
        {#each limits as limit}
          <button
            class:on={settings.maxVisibleSessions === limit}
            aria-pressed={settings.maxVisibleSessions === limit}
            onclick={() => onChange('maxVisibleSessions', limit)}
            >{limit === 99 ? t('unlimited') : limit}</button
          >
        {/each}
      </span>
    </span>
  </div>
</div>

<div class="grp" class:disabled={!settings.agentsEnabled}>
  <div>
    <span class="n">{t('show_agent_details')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showAgentDetails}
        aria-label={t('show_agent_details')}
        aria-pressed={settings.showAgentDetails}
        onclick={() => onChange('showAgentDetails', !settings.showAgentDetails)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('show_tool_status')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showToolStatus}
        aria-label={t('show_tool_status')}
        aria-pressed={settings.showToolStatus}
        onclick={() => onChange('showToolStatus', !settings.showToolStatus)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('show_model_label')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showModelLabel}
        aria-label={t('show_model_label')}
        aria-pressed={settings.showModelLabel}
        onclick={() => onChange('showModelLabel', !settings.showModelLabel)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('show_task_progress')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showTaskProgress}
        aria-label={t('show_task_progress')}
        aria-pressed={settings.showTaskProgress}
        onclick={() => onChange('showTaskProgress', !settings.showTaskProgress)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('show_session_recap')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showSessionRecap}
        aria-label={t('show_session_recap')}
        aria-pressed={settings.showSessionRecap}
        onclick={() => onChange('showSessionRecap', !settings.showSessionRecap)}
      ></button>
    </span>
  </div>
  <p class="hint">
    These five switches change the session cards immediately: prompt and reply rows, tool detail,
    the model tag, the agent checklist and the idle recap.
  </p>
</div>

<div class="grp" class:disabled={!settings.agentsEnabled}>
  <div>
    <span class="n">{t('show_usage_stats')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showUsageStats}
        aria-label={t('show_usage_stats')}
        aria-pressed={settings.showUsageStats}
        onclick={() => onChange('showUsageStats', !settings.showUsageStats)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('show_claude_quota')}</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showClaudeQuota}
        aria-label={t('show_claude_quota')}
        aria-pressed={settings.showClaudeQuota}
        onclick={() => onChange('showClaudeQuota', !settings.showClaudeQuota)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">{t('claude_quota_chip')}</span>
    <span class="s">
      <select
        aria-label={t('claude_quota_chip')}
        disabled={!settings.showClaudeQuota}
        value={settings.claudeQuotaChip}
        onchange={(event) => onChange('claudeQuotaChip', event.currentTarget.value)}
      >
        {#each chipModes as mode (mode.value)}
          <option value={mode.value}>{mode.label()}</option>
        {/each}
      </select>
    </span>
  </div>
  <p class="hint">
    macOS reads plan limits from the Anthropic usage endpoint; the Windows/Linux shell has no such
    reader, so these three keys are stored for parity but draw no quota or token chip here.
  </p>
</div>

<style>
  .hint {
    margin: 6px 0 0;
    font-size: 11px;
    opacity: 0.6;
  }
  .grp.disabled {
    opacity: 0.55;
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
</style>
