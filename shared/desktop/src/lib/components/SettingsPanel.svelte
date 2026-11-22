<script lang="ts">
  // #180 — the settings block extracted verbatim from App.svelte. #199 moves
  // it into a separate Settings window; behaviour is unchanged here.
  import type {
    AutostartStatus,
    CapabilityRow,
    HooksSnapshot,
    KimiStatus,
    OpencodeStatus,
    TargetStatus,
    YamlStatus,
  } from '../types';

  type Props = {
    hooks: HooksSnapshot | null;
    capabilities: CapabilityRow[];
    autostart: AutostartStatus | null;
    hooksBusy: boolean;
    hooksError: string | null;
    onRunHooks: (command: 'hooks_install' | 'hooks_uninstall') => void;
    onToggleTarget: (row: TargetStatus) => void;
    onToggleOpencode: (row: OpencodeStatus) => void;
    onToggleKimi: (row: KimiStatus) => void;
    onToggleYaml: (row: YamlStatus) => void;
    onToggleAutostart: () => void;
  };
  let {
    hooks,
    capabilities,
    autostart,
    hooksBusy,
    hooksError,
    onRunHooks,
    onToggleTarget,
    onToggleOpencode,
    onToggleKimi,
    onToggleYaml,
    onToggleAutostart,
  }: Props = $props();

  // Rust sends `Capability` as an externally tagged enum: one key, optionally
  // a reason.
  function describe(row: CapabilityRow): { label: string; reason: string | null } {
    const [kind, reason] = Object.entries(row.capability)[0] ?? ['Supported', null];
    const label =
      kind === 'Supported' ? 'Supported' : kind === 'Degraded' ? 'Limited' : 'Not supported';
    return { label, reason: kind === 'Supported' ? null : String(reason) };
  }

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

<div class="hooks">
  <div class="hooks-title">
    Claude Code hooks{#if hooks?.version}&nbsp;· {hooks.version}{/if}
  </div>
  {#if hooks}
    <div class="hooks-line">
      {coverage().installed} of {coverage().detected} agents found here are hooked
    </div>
  {/if}
  {#if hooks}
    <div class="hooks-line">
      {hooks.status.hooks_installed ? 'Installed' : 'Not installed'}
      {#if hooks.status.script_installed}&nbsp;· script{/if}
      {#if hooks.status.bridge_installed}&nbsp;· bridge{/if}
    </div>
    <div class="hooks-actions">
      <button class="hooks-button" disabled={hooksBusy} onclick={() => onRunHooks('hooks_install')}>
        {hooks.status.hooks_installed ? 'Repair' : 'Install'}
      </button>
      {#if hooks.status.hooks_installed || hooks.status.script_installed}
        <button class="hooks-button" disabled={hooksBusy} onclick={() => onRunHooks('hooks_uninstall')}>
          Remove
        </button>
      {/if}
    </div>
    {#if hooks.targets.length > 0}
      <ul class="hooks-targets">
        {#each hooks.targets as row (row.source)}
          <li class:missing={!row.detected}>
            <span class="hooks-name">{row.name}</span>
            <span class="hooks-state">
              {row.installed ? 'Installed' : row.detected ? 'Not installed' : 'Not detected'}
            </span>
            {#if row.detected}
              <button class="hooks-button" disabled={hooksBusy} onclick={() => onToggleTarget(row)}>
                {row.installed ? 'Remove' : 'Install'}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
    {#if hooks.opencode}
      <div class="hooks-targets">
        <ul>
          <li class:missing={!hooks.opencode.detected}>
            <span class="hooks-name">OpenCode</span>
            <span class="hooks-state">
              {hooks.opencode.plugin_installed && hooks.opencode.registered
                ? 'Installed'
                : hooks.opencode.detected
                  ? 'Not installed'
                  : 'Not detected'}
            </span>
            {#if hooks.opencode.detected}
              <button
                class="hooks-button"
                disabled={hooksBusy}
                onclick={() => onToggleOpencode(hooks!.opencode)}
              >
                {hooks.opencode.plugin_installed ? 'Remove' : 'Install'}
              </button>
            {/if}
          </li>
        </ul>
      </div>
    {/if}
    {#each hooks.yaml as row (row.source)}
      <div class="hooks-targets">
        <ul>
          <li class:missing={!row.detected}>
            <span class="hooks-name">{row.name}</span>
            <span class="hooks-state">
              {row.refusal
                ? 'Needs a manual merge'
                : row.installed
                  ? 'Installed'
                  : row.detected
                    ? 'Not installed'
                    : 'Not detected'}
            </span>
            {#if row.detected && !row.refusal}
              <button class="hooks-button" disabled={hooksBusy} onclick={() => onToggleYaml(row)}>
                {row.installed ? 'Remove' : 'Install'}
              </button>
            {/if}
          </li>
        </ul>
      </div>
    {/each}
    {#if autostart}
      <div class="hooks-targets">
        <ul>
          <li>
            <span class="hooks-name">Start at login</span>
            <span class="hooks-state">{autostart.installed ? 'On' : 'Off'}</span>
            <button class="hooks-button" disabled={hooksBusy} onclick={onToggleAutostart}>
              {autostart.installed ? 'Turn off' : 'Turn on'}
            </button>
          </li>
        </ul>
      </div>
    {/if}
    {#if capabilities.length > 0}
      <div class="hooks-title">Adapters here</div>
      <div class="hooks-targets">
        <ul>
          {#each capabilities as row (row.feature)}
            {@const described = describe(row)}
            <li class:missing={described.label === 'Not supported'}>
              <span class="hooks-name">{row.feature}</span>
              <span class="hooks-state" title={described.reason ?? ''}>
                {described.label}{described.reason ? ` — ${described.reason}` : ''}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    {#if hooks.kimi}
      <div class="hooks-targets">
        <ul>
          <li class:missing={!hooks.kimi.detected}>
            <span class="hooks-name">Kimi</span>
            <span class="hooks-state">
              {hooks.kimi.installed
                ? 'Installed'
                : hooks.kimi.detected
                  ? 'Not installed'
                  : 'Not detected'}
            </span>
            {#if hooks.kimi.detected}
              <button
                class="hooks-button"
                disabled={hooksBusy}
                onclick={() => onToggleKimi(hooks!.kimi)}
              >
                {hooks.kimi.installed ? 'Remove' : 'Install'}
              </button>
            {/if}
          </li>
        </ul>
      </div>
    {/if}
  {:else}
    <div class="hooks-line">Checking…</div>
  {/if}
  {#if hooksError}<div class="hooks-error">{hooksError}</div>{/if}
</div>

<style>
  .hooks {
    margin: 6px 0 2px;
    padding-top: 6px;
    border-top: 1px solid #262626;
  }
  .hooks-title {
    color: #9a9a9a;
    font-size: 11px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .hooks-line {
    margin: 4px 0 6px;
    font-size: 12px;
    color: #d0d0d0;
  }
  .hooks-actions {
    display: flex;
    gap: 6px;
  }
  .hooks-button {
    padding: 3px 10px;
    border: 1px solid #3a3a3a;
    border-radius: 6px;
    background: #1c1c1c;
    color: #f2f2f2;
    font: inherit;
    font-size: 12px;
    cursor: default;
  }
  .hooks-button:disabled {
    opacity: 0.5;
  }
  .hooks-targets {
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  .hooks-targets li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 0;
    font-size: 12px;
  }
  .hooks-targets li.missing {
    color: #6a6a6a;
  }
  .hooks-name {
    flex: 0 0 84px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .hooks-state {
    flex: 1;
    color: #9a9a9a;
  }
  .hooks-error {
    margin-top: 6px;
    font-size: 11px;
    color: #ff6b6b;
  }
</style>
