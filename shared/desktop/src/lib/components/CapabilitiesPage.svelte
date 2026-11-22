<script lang="ts">
  // #199 — the Capability table. Every Limited / Not supported row carries the
  // reason the shell reported (describeCapability supplies one if it did not).
  import { describeCapability } from '../settings';
  import type { AutostartStatus, CapabilityRow } from '../types';

  type Props = {
    capabilities: CapabilityRow[];
    autostart: AutostartStatus | null;
    busy: boolean;
    onToggleAutostart: () => void;
  };
  let { capabilities, autostart, busy, onToggleAutostart }: Props = $props();

  function tone(label: string): string {
    return label === 'Supported' ? 'yes' : label === 'Limited' ? 'lim' : 'no';
  }
</script>

<h2>Capabilities</h2>
<div class="grp">
  {#each capabilities as row (row.feature)}
    {@const described = describeCapability(row)}
    <div>
      <span class="n">{row.feature}</span>
      <span class="s {tone(described.label)}">
        {described.label}{described.reason ? ` — ${described.reason}` : ''}
      </span>
    </div>
  {:else}
    <div><span class="s">Checking…</span></div>
  {/each}
  {#if autostart}
    <div>
      <span class="n">Start at login</span>
      <span class="s" class:yes={autostart.installed}>
        {autostart.installed ? 'Supported · on' : 'Supported · off'}
      </span>
      <button class="b" disabled={busy} onclick={onToggleAutostart}>
        {autostart.installed ? 'Turn off' : 'Turn on'}
      </button>
    </div>
  {/if}
</div>
