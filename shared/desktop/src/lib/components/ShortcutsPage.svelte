<script lang="ts">
  // #462 — the bindings the shell can actually grab. The rows come from
  // `shortcut_list`, so the page and the grabber read one document. The island's
  // number keys stay below: they are local card input, not global chords.
  import { chordFromEvent, conflictingBinding, type ShortcutBinding } from '../settings';

  type Props = {
    bindings: ShortcutBinding[];
    /** Whether a global grab actually succeeded (#101, L-007). */
    hotkeySupported: boolean;
    /** The composer's answer when it did not, so the page can name the fallback. */
    hotkeyDetail: string | null;
    t: (key: string) => string;
    /** Writes `shortcut_set`; resolves false when the shell refused the write. */
    onSet: (action: string, chord: string | null) => Promise<boolean>;
  };
  let { bindings, hotkeySupported, hotkeyDetail, t, onSet }: Props = $props();

  let recording = $state<string | null>(null);
  let message = $state<string | null>(null);

  function begin(action: string) {
    recording = action;
    message = null;
  }

  async function clear(row: ShortcutBinding) {
    message = null;
    if (!(await onSet(row.action, null))) message = `Could not clear ${row.label}.`;
  }

  function onKeyDown(event: KeyboardEvent) {
    if (recording === null) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      recording = null;
      return;
    }
    const chord = chordFromEvent(event);
    if (chord === null) {
      message = 'Hold Ctrl, Alt or Super and press a key.';
      return;
    }
    const action = recording;
    recording = null;
    void onSet(action, chord).then((ok) => {
      if (!ok) message = `Could not save ${chord}.`;
    });
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<h2>{t('shortcuts')}</h2>
<p class="hint">Click a chord to record it. Escape cancels; × clears it.</p>
<div class="grp">
  {#each bindings as row (row.action)}
    {@const conflict = conflictingBinding(row, bindings)}
    <div>
      <span class="n">
        {row.label}
        {#if conflict}
          <span class="badge" title={`Same chord as ${conflict.label}`}>
            ⚠ conflict: {conflict.label}
          </span>
        {/if}
      </span>
      <span class="s">
        {#if recording === row.action}
          <kbd class="recording" aria-live="polite">press a chord…</kbd>
        {:else}
          <button
            type="button"
            class="chord"
            aria-label={`Change ${row.label} shortcut`}
            onclick={() => begin(row.action)}>{row.chord ?? 'unbound'}</button
          >
          {#if row.chord}
            <button
              type="button"
              class="clear"
              aria-label={`Clear ${row.label} shortcut`}
              onclick={() => clear(row)}>×</button
            >
          {/if}
          {#if !hotkeySupported && row.action === 'toggleIsland'}
            <span class="off">not registered</span>
          {/if}
        {/if}
      </span>
    </div>
  {/each}
  {#if message}
    <div><span class="n">Result</span><span class="s msg" role="alert">{message}</span></div>
  {/if}
  {#if !hotkeySupported}
    <div>
      <span class="n">Fallback</span>
      <span class="s">
        <!-- Wayland has no global grab at all, so the honest answer is the
             desktop's own binding of the CLI, not a key this app cannot take. -->
        bind <code>morrow toggle</code> in your desktop's keyboard settings
      </span>
    </div>
  {/if}
  {#if hotkeyDetail}
    <div><span class="n">Why not</span><span class="s">{hotkeyDetail}</span></div>
  {/if}
  <div>
    <span class="n">Answer a question</span>
    <span class="s"><kbd>1</kbd>–<kbd>9</kbd> in the island (fixed)</span>
  </div>
  <div>
    <span class="n">Toggle a row</span>
    <span class="s">the same <kbd>1</kbd>–<kbd>9</kbd>, on a multi-select or wizard card (fixed)</span>
  </div>
</div>

<style>
  .hint {
    margin: -4px 0 10px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 11.5px;
  }
  kbd,
  .chord {
    display: inline-block;
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
    font: 600 11px ui-monospace, monospace;
  }
  .chord {
    color: inherit;
    cursor: pointer;
  }
  .chord:hover,
  .chord:focus-visible {
    border-color: rgba(255, 255, 255, 0.45);
    background: rgba(255, 255, 255, 0.12);
  }
  .recording {
    color: var(--amber, #e8a33d);
    border-color: var(--amber, #e8a33d);
  }
  .clear {
    margin-left: 6px;
    border: 0;
    background: none;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
  }
  .clear:hover,
  .clear:focus-visible {
    color: #fff;
  }
  .badge {
    margin-left: 6px;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(232, 163, 61, 0.16);
    color: var(--amber, #e8a33d);
    font: 600 10px ui-monospace, monospace;
    white-space: nowrap;
  }
  .off,
  .msg {
    color: var(--amber, #e8a33d);
    margin-left: 8px;
  }
  code {
    font: 11px ui-monospace, monospace;
  }
</style>
