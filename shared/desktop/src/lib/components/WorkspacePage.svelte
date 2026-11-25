<script lang="ts">
  // #462 — the five keys Swift's Workspace settings page writes. Four are
  // obeyed by this shell; the album-art palette has no reader here, so that row
  // states the limit instead of offering a switch that does nothing.
  import { WORKSPACE_KEYS, type WorkspaceSettings } from '../workspace';

  type Props = {
    settings: WorkspaceSettings;
    onChange: (key: string, value: string | boolean) => void;
  };
  let { settings, onChange }: Props = $props();
</script>

<h2>Workspace</h2>
<div class="grp">
  <div>
    <span class="n">Enable Workspace &amp; Git</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.enabled}
        aria-label="Enable Workspace and Git"
        aria-pressed={settings.enabled}
        onclick={() => onChange(WORKSPACE_KEYS.enabled, !settings.enabled)}
      ></button>
    </span>
  </div>
  <p class="hint">
    Monitors branches, changed files and recent commits. Turning this off hides the Workspace tab
    and stops its git reads.
  </p>
</div>

<div class="grp" class:missing={!settings.enabled}>
  <div>
    <span class="n">Workspace HUD theme</span>
    <span class="s">
      <span class="seg2">
        <button
          class:on={settings.hudTheme === 'modern'}
          aria-pressed={settings.hudTheme === 'modern'}
          disabled={!settings.enabled}
          onclick={() => onChange(WORKSPACE_KEYS.hudTheme, 'modern')}>Modern</button
        >
        <button
          class:on={settings.hudTheme === 'pixelRetro'}
          aria-pressed={settings.hudTheme === 'pixelRetro'}
          disabled={!settings.enabled}
          onclick={() => onChange(WORKSPACE_KEYS.hudTheme, 'pixelRetro')}>Pixel Retro</button
        >
      </span>
    </span>
  </div>
  <div>
    <span class="n">Adaptive album art palette</span>
    <span class="s">
      Stored: {settings.syncAlbumArt ? 'On' : 'Off'}. This shell has no album-art colour reader, so
      the Workspace accent does not follow the current track.
    </span>
  </div>
  <div>
    <span class="n">Show project name</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showProjectName}
        aria-label="Show project name"
        aria-pressed={settings.showProjectName}
        disabled={!settings.enabled}
        onclick={() => onChange(WORKSPACE_KEYS.showProjectName, !settings.showProjectName)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Show Git branch</span>
    <span class="s">
      <button
        class="sw"
        class:off={!settings.showGitBranch}
        aria-label="Show Git branch"
        aria-pressed={settings.showGitBranch}
        disabled={!settings.enabled}
        onclick={() => onChange(WORKSPACE_KEYS.showGitBranch, !settings.showGitBranch)}
      ></button>
    </span>
  </div>
</div>

<div class="grp">
  <div>
    <span class="n">Monitored repositories</span>
    <span class="s">
      The tab follows the active session's working directory. Switch projects in your terminal or
      IDE and its branch, changed files and recent commits are read again.
    </span>
  </div>
</div>

<style>
  .hint {
    margin: 0;
    padding: 0 12px 9px;
    font-size: 11px;
    opacity: 0.6;
  }
</style>
