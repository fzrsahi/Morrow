<script lang="ts">
  // #199/#465 — About, adapted from the macOS About pane. The release link now
  // follows `ReleaseNotesLink.url(forVersion:)` instead of the release list,
  // and the diagnostics button writes a redacted bundle the user can attach to
  // a bug report.
  import { onMount } from 'svelte';
  import { releaseNotesUrl } from '../release-notes';
  import { APP_VERSION } from '../version';

  type Props = {
    settingsFile: string;
    version: string;
    t: (key: string) => string;
    onExport: (destination: string) => Promise<string>;
  };
  let { settingsFile, version, t, onExport }: Props = $props();
  const VERSION = $derived(version || APP_VERSION);
  const releaseNotes = $derived(releaseNotesUrl(VERSION));

  let suggested = $state('Morrow-Diagnostics.json');
  let destination = $state('');
  let exported = $state('');
  let error = $state('');

  // The settings file's folder is the least surprising default: the bundle
  // lands beside the document the report is about, not in an unknown cwd.
  const folder = $derived(settingsFile.replace(/[^/\\]+$/, ''));

  onMount(() => {
    void import('@tauri-apps/api/core').then(({ invoke }) =>
      invoke<string>('diagnostics_suggested_name')
        .then((name) => {
          suggested = name;
          if (!destination) destination = `${folder}${name}`;
        })
        .catch(() => {
          if (!destination) destination = `${folder}${suggested}`;
        }),
    );
  });

  let copied = $state(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(settingsFile);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // A webview without clipboard permission still has the text on screen,
      // selected: the path is the payload, not the button.
      copied = false;
    }
  }

  async function exportBundle() {
    error = '';
    exported = '';
    try {
      exported = await onExport(destination.trim() || `${folder}${suggested}`);
    } catch (cause) {
      error = String(cause);
    }
  }
</script>

<h2>{t('about')}</h2>
<div class="about">
  <div class="mark"><span><i></i><i></i></span></div>
  <div class="name">Morrow</div>
  <div class="version">Version {VERSION}</div>
  <p>
    Real-time AI coding agent status panel for Windows and Linux<br />
    Talks to agents over a Unix socket or a named pipe
  </p>
  <div class="links">
    <a href="https://github.com/fzrsahi/Morrow" target="_blank" rel="noreferrer">GitHub</a>
    <a href="https://github.com/fzrsahi/Morrow/issues" target="_blank" rel="noreferrer">Issues</a>
    <a href={releaseNotes} target="_blank" rel="noreferrer" title={t('release_notes_for_version').replace('%@', VERSION)}>What's New</a>
  </div>
  {#if settingsFile}
    <div class="file">
      <span class="k">Settings file</span>
      <code>{settingsFile}</code>
      <button class="b" onclick={copy}>{copied ? 'Copied' : 'Copy path'}</button>
    </div>
  {/if}
  <button class="b" disabled title="Updates are handled by the package manager on this platform">
    {t('check_for_updates')}
  </button>

  <div class="diag" title={t('export_diagnostics_desc')}>
    <div class="label">{t('export_diagnostics')}</div>
    <input
      aria-label="Diagnostics destination"
      bind:value={destination}
      placeholder={suggested}
      spellcheck="false"
    />
    <button class="b" disabled={!destination.trim()} onclick={exportBundle}>Export</button>
    {#if exported}
      <code class="result">Saved {exported}</code>
    {/if}
    {#if error}
      <code class="result err">{error}</code>
    {/if}
  </div>
</div>
<style>
  .file {
    display: grid;
    gap: 4px;
    width: 100%;
    max-width: 420px;
    text-align: left;
  }
  .file .k {
    font-size: 11px;
    opacity: 0.7;
  }
  .file code {
    overflow-wrap: anywhere;
    font: 11px ui-monospace, monospace;
  }
  .diag {
    display: grid;
    gap: 5px;
    width: 100%;
    max-width: 420px;
    margin-top: 12px;
    text-align: left;
  }
  .diag .label {
    font-size: 12px;
    opacity: 0.85;
  }
  .diag input {
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 5px 7px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: 11px ui-monospace, monospace;
  }
  .result {
    overflow-wrap: anywhere;
    font: 11px ui-monospace, monospace;
    color: #8fe3a1;
  }
  .result.err {
    color: #ff9c9c;
  }
</style>
