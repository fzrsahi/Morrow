<script lang="ts">
  // #241 — the Sound page: the cue table, and one play button per row so the
  // user can hear what a cue sounds like. The per-event switches, the master
  // toggle and quiet hours arrive with the rest of `AttentionPolicy.settle`
  // (#238); until then every cue is on and the page only previews them.
  import { cueLabel } from '../sound';
  import type { SoundCue } from '../types';

  type Props = {
    cues: SoundCue[];
    supported: boolean;
    busy: boolean;
    onPreview: (cue: string) => void;
  };
  let { cues, supported, busy, onPreview }: Props = $props();
</script>

<h2>Sound</h2>
<div class="grp">
  {#each cues as cue (cue.name)}
    <div>
      <span class="n">{cueLabel(cue.name)}</span>
      <span class="s">{cue.file}</span>
      <button class="b" disabled={busy || !supported} onclick={() => onPreview(cue.name)}>
        Play
      </button>
    </div>
  {:else}
    <div><span class="s">Checking…</span></div>
  {/each}
</div>
{#if !supported}
  <p class="note">
    No sound player is available on this machine, so these cues stay silent. The Capability table
    says the same under <b>Sound effects</b>.
  </p>
{/if}

<style>
  /* The rows themselves are `settings.css`'s; only the note is this page's. */
  .note {
    margin: 0;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
