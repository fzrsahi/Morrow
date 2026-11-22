<script lang="ts">
  // #182 — header bar matched 1:1 to the prototype's #header section. Only
  // Agents and Music tabs: the other four have no user story in #82.
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { PALETTE } from '../tokens';
  import type { PanelTab, PanelVariant, SessionFilter } from '../panel';

  type Props = {
    tab: PanelTab;
    filter: SessionFilter;
    muted: boolean;
    quitArmed: boolean;
    variant: PanelVariant;
    onTab: (tab: PanelTab) => void;
    onFilter: (filter: SessionFilter) => void;
    onMute: () => void;
    onSettings: () => void;
    onQuit: () => void;
  };
  let {
    tab,
    filter,
    muted,
    quitArmed,
    variant,
    onTab,
    onFilter,
    onMute,
    onSettings,
    onQuit,
  }: Props = $props();

  const tabs: Array<{ id: PanelTab; label: string; icon: string }> = [
    { id: 'agents', label: 'AGENTS', icon: 'agents' },
    { id: 'music', label: 'MUSIC', icon: 'music' },
  ];
  const filters: Array<{ id: SessionFilter; label: string }> = [
    { id: 'all', label: 'ALL' },
    { id: 'sta', label: 'STA' },
    { id: 'agt', label: 'AGT' },
  ];
  const inactive = 'rgba(255,255,255,.55)';
</script>

<header class="hdr" class:edge={variant !== 'B'} data-header={tab}>
  <div class="tabs" role="tablist" aria-label="Island tabs">
    {#each tabs as item (item.id)}
      <button
        class="tab"
        class:on={tab === item.id}
        role="tab"
        aria-selected={tab === item.id}
        data-tab={item.id}
        onclick={() => onTab(item.id)}
      >
        <PixelIcon name={item.icon} size={1.35} color={tab === item.id ? PALETTE.cyan : inactive} />
        {#if tab === item.id}
          <PixelText text={item.label} size={1.15} color={PALETTE.cyan} />
        {/if}
      </button>
    {/each}
  </div>
  <span class="hsp"></span>
  {#if tab === 'agents'}
    <div class="filt" role="group" aria-label="Session filter">
      {#each filters as item (item.id)}
        <button
          class="fbtn"
          class:on={filter === item.id}
          aria-pressed={filter === item.id}
          data-filter={item.id}
          onclick={() => onFilter(item.id)}
        >
          <PixelText text={item.label} size={1.05} color={filter === item.id ? PALETTE.cyan : inactive} />
        </button>
      {/each}
    </div>
  {/if}
  <button class="hbtn" class:muted data-mute aria-label={muted ? 'Unmute' : 'Mute'} onclick={onMute}>
    <PixelIcon name={muted ? 'volumeMute' : 'volume'} size={1.35} color={muted ? PALETTE.red : PALETTE.textWhite} />
  </button>
  <button class="hbtn" data-settings aria-label="Settings" onclick={onSettings}>
    <PixelIcon name="settings" size={1.35} color={PALETTE.textWhite} />
  </button>
  {#if quitArmed}
    <button class="quit" data-quit aria-label="Confirm quit" onclick={onQuit}>
      <PixelText text="QUIT?" size={1.15} color="#000" />
    </button>
  {:else}
    <button class="hbtn power" data-quit aria-label="Quit Morrow" onclick={onQuit}>
      <PixelIcon name="power" size={1.35} color={PALETTE.red} />
    </button>
  {/if}
</header>

<style>
  .hdr {
    box-sizing: border-box;
    height: 40px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
  }
  .hdr.edge {
    padding-top: 2px;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tab {
    box-sizing: border-box;
    height: 20px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 6px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: rgba(255, 255, 255, 0.55);
    font: inherit;
    cursor: default;
    transition: background 0.12s ease-out;
  }
  .tab.on {
    padding: 0 8px;
    background: color-mix(in srgb, var(--cyan) 18%, transparent);
  }
  .hsp {
    flex: 1;
  }
  .filt {
    display: inline-flex;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 4px;
    padding: 2px;
    margin-right: 10px;
  }
  .fbtn {
    height: 16px;
    display: inline-flex;
    align-items: center;
    padding: 0 5px;
    border: 0;
    border-radius: 2.5px;
    background: none;
    cursor: default;
    transition: background 0.12s ease-out;
  }
  .fbtn.on {
    background: color-mix(in srgb, var(--cyan) 18%, transparent);
  }
  .hbtn {
    box-sizing: border-box;
    width: 20px;
    height: 20px;
    margin-left: 4px;
    border: 0;
    border-radius: 3px;
    display: inline-grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.08);
    cursor: default;
    transition:
      background 0.12s ease-out,
      transform 0.12s ease-out;
  }
  .hbtn:hover,
  .fbtn:hover,
  .tab:hover {
    background: rgba(255, 255, 255, 0.22);
  }
  .hbtn:hover {
    transform: scale(1.08);
  }
  .hbtn.muted {
    background: color-mix(in srgb, var(--red) 20%, transparent);
  }
  .hbtn.power {
    background: color-mix(in srgb, var(--red) 8%, transparent);
  }
  .quit {
    box-sizing: border-box;
    height: 20px;
    padding: 0 8px;
    margin-left: 4px;
    border: 0;
    border-radius: 4px;
    background: var(--red);
    display: inline-flex;
    align-items: center;
    cursor: default;
  }
  :focus-visible {
    outline: 2px solid var(--cyan);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .hbtn,
    .fbtn,
    .tab {
      transition: none;
    }
  }
</style>
