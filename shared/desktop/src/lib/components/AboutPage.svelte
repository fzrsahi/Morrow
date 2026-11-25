<script lang="ts">
  // #199 — About, adapted from the macOS About pane in the prototype's
  // settings-about reference to the Windows/Linux port.
  //
  // #239 AC 1: the shell's `settings_path` was registered for a "reveal" button
  // that was never built. Showing the path is the part that earns its keep — the
  // file is one the user may want to back up, edit or copy into a bug report —
  // and it needs no desktop-integration plugin.
  type Props = {
    settingsFile: string;
    t: (key: string) => string;
  };
  let { settingsFile, t }: Props = $props();
  const VERSION = '0.1.0';

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
    <a href="https://github.com/fzrsahi/Morrow/releases" target="_blank" rel="noreferrer">What's New</a>
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
</style>
