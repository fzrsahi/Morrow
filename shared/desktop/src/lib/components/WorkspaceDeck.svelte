<script lang="ts">
  // #462 — the Workspace tab: branch, changed-file counts and recent commits
  // from the active session's checkout. All four states are drawn here; the
  // shell owns the git process and this card never guesses a value.
  import LineIcon from './LineIcon.svelte';
  import PixelText from './PixelText.svelte';
  import { PALETTE, type Theme } from '../tokens';
  import {
    changeCount,
    visibleBranch,
    visibleProject,
    type WorkspaceSettings,
    type WorkspaceState,
  } from '../workspace';

  type Props = {
    theme?: Theme;
    state: WorkspaceState;
    settings: WorkspaceSettings;
    onRefresh: () => void;
  };
  let { theme = 'pixel', state, settings, onRefresh }: Props = $props();

  let status = $derived(state.kind === 'ready' ? state.status : null);
  let project = $derived(status ? visibleProject(status, settings) : '');
  let branch = $derived(status ? visibleBranch(status, settings) : '');
  let changes = $derived(status ? changeCount(status) : 0);
</script>

<div class="deck" class:modern={theme === 'modern'} data-workspace={state.kind}>
  <div class="top">
    {#if theme === 'modern'}
      <span class="title">Workspace</span>
    {:else}
      <PixelText text="WORKSPACE" size={1.1} color={PALETTE.cyan} />
    {/if}
    <span class="hsp"></span>
    <button class="refresh" aria-label="Refresh workspace" onclick={onRefresh}>
      {#if theme === 'modern'}<span class="glyph">↻</span>{:else}<PixelText text="⟳" size={1} color={PALETTE.textWhite} />{/if}
    </button>
  </div>

  {#if state.kind === 'loading'}
    <p class="note">Reading git…</p>
  {:else if state.kind === 'error'}
    <p class="note error" role="status">{state.message}</p>
  {:else if state.kind === 'no-session'}
    <p class="note">Open a session to see its repository.</p>
  {:else if state.kind === 'not-repo'}
    <p class="note">The active session's folder is not a git checkout.</p>
  {:else if status}
    <div class="body">
      <div class="repo">
        {#if project}<div class="project" title={status.cwd}>{project}</div>{/if}
        {#if branch}
          <div class="branch"><LineIcon name="branch" size={12} /><span>{branch}{status.isWorktree ? ' ⧉' : ''}</span></div>
        {/if}
        <div class="counts" aria-label="Changed files">
          <span><b>{status.staged}</b> staged</span>
          <span><b>{status.modified}</b> modified</span>
          <span><b>{status.untracked}</b> untracked</span>
        </div>
        <div class="total">{changes} changed files</div>
      </div>
      <div class="commits">
        <div class="label">RECENT COMMITS</div>
        {#each status.commits as commit (commit.hash)}
          <div class="commit">
            <code>{commit.hash}</code><span>{commit.subject}</span>
          </div>
        {:else}
          <div class="empty">No commits yet.</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .deck {
    box-sizing: border-box;
    padding: 4px 16px 12px;
    color: rgba(255, 255, 255, 0.86);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 22px;
  }
  .title {
    font: 600 11px var(--ui, system-ui, sans-serif);
    letter-spacing: 0.01em;
  }
  .hsp { flex: 1; }
  .refresh {
    width: 22px;
    height: 20px;
    border: 0;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: inherit;
    cursor: default;
    display: inline-grid;
    place-items: center;
  }
  .refresh:hover { background: rgba(255, 255, 255, 0.16); }
  .refresh:focus-visible, .commit:focus-visible { outline: 2px solid var(--cyan); }
  .glyph { font: 14px var(--ui, system-ui, sans-serif); line-height: 1; }
  .note {
    margin: 12px 0 4px;
    font: 11px/1.5 var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.6);
  }
  .error { color: var(--red, #ff6b6b); }
  .body {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-top: 8px;
  }
  .project {
    font: 600 13px var(--ui, system-ui, sans-serif);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .branch {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 5px;
    font: 11px var(--ui, system-ui, sans-serif);
    color: var(--cyan);
    overflow: hidden;
  }
  .branch span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .counts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    margin-top: 10px;
    font: 10px var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.6);
  }
  .counts b { color: rgba(255, 255, 255, 0.92); font-weight: 600; }
  .total {
    margin-top: 5px;
    font: 10px var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.45);
  }
  .label {
    font: 600 9px var(--ui, system-ui, sans-serif);
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.45);
    margin-bottom: 5px;
  }
  .commit {
    display: flex;
    gap: 7px;
    align-items: baseline;
    padding: 3px 0;
    font: 11px/1.35 var(--ui, system-ui, sans-serif);
    border-top: 1px solid rgba(255, 255, 255, 0.07);
  }
  .commit code { color: var(--cyan); font-size: 10px; }
  .commit span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { font: 11px var(--ui, system-ui, sans-serif); color: rgba(255, 255, 255, 0.5); }
  .modern .project { font-weight: 600; }
</style>
