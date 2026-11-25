<script lang="ts">
  // #239 AC 3 — Shortcuts has content because the shortcuts exist: one global
  // binding the shell registers, and the island's own number keys. Anything not
  // in this table would be a story, so the table is deliberately short.
  import { DEFAULT_HOTKEY } from '../settings';

  type Props = {
    /** Whether the global hotkey actually registered (#101, L-007). */
    hotkeySupported: boolean;
    /** The composer's answer when it did not, so the page can name the fallback. */
    hotkeyDetail: string | null;
    t: (key: string) => string;
  };
  let { hotkeySupported, hotkeyDetail, t }: Props = $props();
</script>

<h2>{t('shortcuts')}</h2>
<div class="grp">
  <div>
    <span class="n">{t('shortcut_togglePanel')}</span>
    <span class="s">
      {#if hotkeySupported}
        <kbd>{DEFAULT_HOTKEY}</kbd>
      {:else}
        <span class="off">not registered</span>
      {/if}
    </span>
  </div>
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
  <div><span class="n">{t('question_answer')}</span><span class="s"><kbd>1</kbd>–<kbd>9</kbd></span></div>
  <div>
    <span class="n">Toggle a row</span>
    <span class="s">the same <kbd>1</kbd>–<kbd>9</kbd>, on a multi-select or wizard card</span>
  </div>
</div>

<style>
  kbd {
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
    font: 600 11px ui-monospace, monospace;
  }
  code {
    font: 11px ui-monospace, monospace;
  }
  .off {
    color: var(--amber, #e8a33d);
  }
</style>
