<script lang="ts">
  // #199 — the Hooks page of the Settings window. The hook logic came from
  // SettingsPanel.svelte; the shell still owns every install decision.
  import type {
    HooksSnapshot,
    KimiStatus,
    OpencodeStatus,
    TargetStatus,
    YamlStatus,
  } from '../types';

  type Props = {
    hooks: HooksSnapshot | null;
    busy: boolean;
    error: string | null;
    t: (key: string) => string;
    onRunHooks: (command: 'hooks_install' | 'hooks_uninstall') => void;
    onToggleTarget: (row: TargetStatus) => void;
    onToggleOpencode: (row: OpencodeStatus) => void;
    onToggleKimi: (row: KimiStatus) => void;
    onToggleYaml: (row: YamlStatus) => void;
  };
  let {
    hooks,
    busy,
    error,
    t,
    onRunHooks,
    onToggleTarget,
    onToggleOpencode,
    onToggleKimi,
    onToggleYaml,
  }: Props = $props();

  function coverage(): { installed: number; detected: number } {
    if (!hooks) return { installed: 0, detected: 0 };
    const rows = [
      ...hooks.targets.map((row) => ({ detected: row.detected, installed: row.installed })),
      { detected: hooks.opencode.detected, installed: hooks.opencode.plugin_installed },
      { detected: hooks.kimi.detected, installed: hooks.kimi.installed },
      ...hooks.yaml.map((row) => ({ detected: row.detected, installed: row.installed })),
    ];
    return {
      installed: rows.filter((row) => row.installed).length,
      detected: rows.filter((row) => row.detected).length,
    };
  }
</script>

<h2>{t('hooks')}</h2>
{#if !hooks}
  <div class="grp"><div><span class="s">Checking…</span></div></div>
{:else}
  <div class="grp">
    <div>
      <span class="n">Claude Code</span>
      <span class="s" class:yes={hooks.status.hooks_installed}>
        {[
          hooks.status.hooks_installed ? 'Installed' : t('not_installed'),
          hooks.status.script_installed ? 'script' : '',
          hooks.status.bridge_installed ? 'bridge' : '',
          hooks.version,
        ]
          .filter(Boolean)
          .join(' · ')}
      </span>
      <button class="b" disabled={busy} onclick={() => onRunHooks('hooks_install')}>
        {hooks.status.hooks_installed ? t('reinstall') : 'Install'}
      </button>
    </div>
    <div>
      <span class="n">Coverage</span>
      <span class="s">{coverage().installed} of {coverage().detected} agents found here are hooked</span>
    </div>
  </div>
  <div class="grp" class:missing={hooks.targets.length === 0}>
    {#each hooks.targets as row (row.source)}
      <div class:missing={!row.detected}>
        <span class="n">{row.name}</span>
        <span class="s" class:yes={row.installed}>
          {row.installed ? 'Installed' : row.detected ? t('not_installed') : t('not_detected')}
        </span>
        {#if row.detected}
          <button class="b" disabled={busy} onclick={() => onToggleTarget(row)}>
            {row.installed ? t('uninstall') : 'Install'}
          </button>
        {/if}
      </div>
    {:else}
      <div><span class="n">{t('agents')}</span><span class="s">No other hook targets on this host</span></div>
    {/each}
  </div>
  <div class="grp">
    <div class:missing={!hooks.opencode.detected}>
      <span class="n">OpenCode</span>
      <span class="s" class:yes={hooks.opencode.plugin_installed}>
        {hooks.opencode.plugin_installed && hooks.opencode.registered
          ? 'Installed'
          : hooks.opencode.detected
            ? t('not_installed')
            : t('not_detected')}
      </span>
      {#if hooks.opencode.detected}
        <button class="b" disabled={busy} onclick={() => onToggleOpencode(hooks!.opencode)}>
          {hooks.opencode.plugin_installed ? t('uninstall') : 'Install'}
        </button>
      {/if}
    </div>
    {#if hooks.kimi}
      <div class:missing={!hooks.kimi.detected}>
        <span class="n">Kimi</span>
        <span class="s" class:yes={hooks.kimi.installed}>
          {hooks.kimi.installed
            ? 'Installed'
            : hooks.kimi.detected
              ? t('not_installed')
              : t('not_detected')}
        </span>
        {#if hooks.kimi.detected}
          <button class="b" disabled={busy} onclick={() => onToggleKimi(hooks!.kimi)}>
            {hooks.kimi.installed ? t('uninstall') : 'Install'}
          </button>
        {/if}
      </div>
    {/if}
    {#each hooks.yaml as row (row.source)}
      <div class:missing={!row.detected}>
        <span class="n">{row.name}</span>
        <span class="s" class:lim={!!row.refusal} class:yes={row.installed && !row.refusal}>
          {row.refusal
            ? 'Needs a manual merge: the file has a hooks block we did not write'
            : row.installed
              ? 'Installed'
              : row.detected
                ? t('not_installed')
                : t('not_detected')}
        </span>
        {#if row.detected && !row.refusal}
          <button class="b" disabled={busy} onclick={() => onToggleYaml(row)}>
            {row.installed ? t('uninstall') : 'Install'}
          </button>
        {/if}
      </div>
    {/each}
  </div>
  {#if hooks.note}
    <div class="grp missing"><div><span class="s">{hooks.note}</span></div></div>
  {/if}
{/if}
{#if error}<div class="grp"><div><span class="s no">{error}</span></div></div>{/if}
