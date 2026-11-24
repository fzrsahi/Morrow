<script lang="ts">
  // #308 — Settings page for the collapsed-island quota choice. The picker
  // writes one key of the existing settings document through the shell.
  import { QUOTA_MODES, type QuotaMode } from '../settings';

  type Props = {
    mode: QuotaMode;
    onChange: (mode: QuotaMode) => void;
  };
  let { mode, onChange }: Props = $props();

  const labels: Record<QuotaMode, string> = {
    off: 'Nothing',
    auto: 'Auto (weekly; 5-hour when pressing)',
    session: '5-hour window',
    weeklyAll: 'Weekly',
    weeklyScoped: 'Weekly (model-specific)',
  };
</script>

<h2>Claude quota</h2>
<div class="grp">
  <div>
    <span class="n">Collapsed island shows</span>
    <span class="s">
      <span class="seg2">
        {#each QUOTA_MODES as choice}
          <button
            class:on={mode === choice}
            aria-pressed={mode === choice}
            onclick={() => onChange(choice)}>{labels[choice]}</button
          >
        {/each}
      </span>
    </span>
  </div>
</div>
