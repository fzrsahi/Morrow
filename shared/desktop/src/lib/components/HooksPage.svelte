<script lang="ts">
  // #199 — the Hooks page of the Settings window. The hook logic came from
  // SettingsPanel.svelte; the shell still owns every install decision.
  // #462 — socket health, config roots and extra roots are read from the shell
  // too; the page only maps controls onto commands.
  import {
    configRootLabel,
    coverage,
    extraDirLabel,
    extraDirsFor,
    socketLabel,
  } from '../hooks';
  import type {
    ConfigRootStatus,
    ExtraConfigDirStatus,
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
    onRunHooks: (command: 'hooks_install' | 'hooks_uninstall') => void;
    onToggleTarget: (row: TargetStatus) => void;
    onToggleOpencode: (row: OpencodeStatus) => void;
    onToggleKimi: (row: KimiStatus) => void;
    onToggleYaml: (row: YamlStatus) => void;
    onToggleConfigRoot: (row: ConfigRootStatus) => void;
    onAddExtraDir: (cli: string, path: string) => void;
    onRemoveExtraDir: (id: string) => void;
    onToggleExtraDir: (row: ExtraConfigDirStatus, enabled: boolean) => void;
  };
  let {
    hooks,
    busy,
    error,
    onRunHooks,
    onToggleTarget,
    onToggleOpencode,
    onToggleKimi,
    onToggleYaml,
    onToggleConfigRoot,
    onAddExtraDir,
    onRemoveExtraDir,
    onToggleExtraDir,
  }: Props = $props();

  let newCli = $state('claude');
  let newPath = $state('');

  function addExtraDir() {
    const path = newPath.trim();
    if (!path) return;
    onAddExtraDir(newCli, path);
    newPath = '';
  }

  function toggleExtraDir(row: ExtraConfigDirStatus, event: Event) {
    onToggleExtraDir(row, (event.currentTarget as HTMLInputElement).checked);
  }
</script>

<h2>Hooks</h2>
{#if !hooks}
  <div class="grp"><div><span class="s">Checking…</span></div></div>
{:else}
  <div class="grp">
    <div>
      <span class="n">Hook socket</span>
      <span
        class="s"
        class:yes={hooks.socket.listening}
        class:no={!hooks.socket.listening}>{socketLabel(hooks.socket)}</span
      >
    </div>
    <div>
      <span class="n">Claude Code</span>
      <span class="s" class:yes={hooks.status.hooks_installed}>
        {[
          hooks.status.hooks_installed ? 'Installed' : 'Not installed',
          hooks.status.script_installed ? 'script' : '',
          hooks.status.bridge_installed ? 'bridge' : '',
          hooks.version,
        ]
          .filter(Boolean)
          .join(' · ')}
      </span>
      <button class="b" disabled={busy} onclick={() => onRunHooks('hooks_install')}>
        {hooks.status.hooks_installed ? 'Repair' : 'Install'}
      </button>
    </div>
    <div>
      <span class="n">Coverage</span>
      <span class="s">{coverage(hooks).installed} of {coverage(hooks).detected} agents found here are hooked</span>
    </div>
  </div>
  <div class="grp" class:missing={hooks.targets.length === 0}>
    {#each hooks.targets as row (row.source)}
      <div class:missing={!row.detected}>
        <span class="n">{row.name}</span>
        <span class="s" class:yes={row.installed}>
          {row.installed ? 'Installed' : row.detected ? 'Not installed' : 'Not detected'}
        </span>
        {#if row.detected}
          <button class="b" disabled={busy} onclick={() => onToggleTarget(row)}>
            {row.installed ? 'Remove' : 'Install'}
          </button>
        {/if}
      </div>
    {:else}
      <div><span class="n">Agents</span><span class="s">No other hook targets on this host</span></div>
    {/each}
  </div>
  <div class="grp">
    <div class:missing={!hooks.opencode.detected}>
      <span class="n">OpenCode</span>
      <span class="s" class:yes={hooks.opencode.plugin_installed}>
        {hooks.opencode.plugin_installed && hooks.opencode.registered
          ? 'Installed'
          : hooks.opencode.detected
            ? 'Not installed'
            : 'Not detected'}
      </span>
      {#if hooks.opencode.detected}
        <button class="b" disabled={busy} onclick={() => onToggleOpencode(hooks!.opencode)}>
          {hooks.opencode.plugin_installed ? 'Remove' : 'Install'}
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
              ? 'Not installed'
              : 'Not detected'}
        </span>
        {#if hooks.kimi.detected}
          <button class="b" disabled={busy} onclick={() => onToggleKimi(hooks!.kimi)}>
            {hooks.kimi.installed ? 'Remove' : 'Install'}
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
                ? 'Not installed'
                : 'Not detected'}
        </span>
        {#if row.detected && !row.refusal}
          <button class="b" disabled={busy} onclick={() => onToggleYaml(row)}>
            {row.installed ? 'Remove' : 'Install'}
          </button>
        {/if}
      </div>
    {/each}
  </div>

  <div class="grp">
    <div><span class="n">Config roots</span><span class="s">Primary root and its extra roots, per CLI</span></div>
    {#each hooks.config_roots as row (row.cli)}
      <div class:missing={!row.ready}>
        <span class="n">{row.name}</span>
        <span class="s" class:yes={row.ready && row.installed}>{configRootLabel(row)}</span>
        <span class="s path">{row.path}</span>
        {#if row.ready}
          <button class="b" disabled={busy} onclick={() => onToggleConfigRoot(row)}>
            {row.installed ? 'Remove' : 'Install'}
          </button>
        {/if}
      </div>
      {#each extraDirsFor(hooks.extra_dirs, row.cli) as dir (dir.id)}
        <div class:missing={!dir.ready || !dir.enabled}>
          <span class="n">{row.name} extra</span>
          <span
            class="s"
            class:yes={dir.ready && dir.installed && !dir.problem}
            class:lim={!!dir.problem}>{extraDirLabel(dir)}</span
          >
          <span class="s path">{dir.path}</span>
          {#if dir.ready && !dir.problem}
            <button class="b" disabled={busy} onclick={() => onToggleConfigRoot({
              cli: row.cli,
              name: row.name,
              path: dir.path,
              ready: dir.ready,
              installed: dir.installed,
            })}>
              {dir.installed ? 'Remove' : 'Install'}
            </button>
          {/if}
          <label class="toggle">
            <input
              type="checkbox"
              checked={dir.enabled}
              disabled={busy}
              onchange={(event) => toggleExtraDir(dir, event)}
            /> Enabled
          </label>
          <button class="b" disabled={busy} onclick={() => onRemoveExtraDir(dir.id)}>Forget</button>
        </div>
      {/each}
    {/each}
  </div>
  <div class="grp">
    <div>
      <span class="n">Add config root</span>
      <select class="b" bind:value={newCli} disabled={busy} aria-label="CLI">
        {#each hooks.config_roots as row (row.cli)}
          <option value={row.cli}>{row.name}</option>
        {/each}
      </select>
      <input
        class="path-input"
        type="text"
        bind:value={newPath}
        placeholder="~/.claude-work"
        aria-label="Config directory"
        onkeydown={(event) => event.key === 'Enter' && addExtraDir()}
      />
      <button class="b" disabled={busy || !newPath.trim()} onclick={addExtraDir}>Add</button>
    </div>
  </div>
  {#if hooks.note}
    <div class="grp missing"><div><span class="s">{hooks.note}</span></div></div>
  {/if}
{/if}
{#if error}<div class="grp"><div><span class="s no">{error}</span></div></div>{/if}

<style>
  .path {
    font-family: ui-monospace, monospace;
    overflow-wrap: anywhere;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
    font-size: 12px;
  }
  .path-input {
    flex: 1;
    min-width: 120px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.2);
    color: #fff;
    padding: 4px 8px;
    font: inherit;
    font-size: 12px;
  }
</style>
