<script lang="ts">
  // #182 — header bar matched 1:1 to the prototype's #header section. Only
  // Agents and Music tabs: the other four have no user story in #82.
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import LineIcon from './LineIcon.svelte';
  import { PALETTE, type Theme } from '../tokens';
  import { visiblePanelTabs } from '../gestures';
  import type { PanelTab, PanelVariant, SessionFilter } from '../panel';

  type Props = {
    theme?: Theme;
    tab: PanelTab;
    /** #465: the Agents switch hides this tab rather than leaving a dead one. */
    agentsEnabled?: boolean;
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
    theme = 'pixel',
    tab,
    agentsEnabled = true,
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

  const allTabs: Array<{
    id: PanelTab;
    label: string;
    icon: string;
    line: 'cpu' | 'note';
    modern: string;
  }> = [
    { id: 'agents', label: 'AGENTS', icon: 'agents', line: 'cpu', modern: 'Agents' },
    { id: 'music', label: 'MUSIC', icon: 'music', line: 'note', modern: 'Music' },
  ];
  const visible = $derived(new Set(visiblePanelTabs(agentsEnabled)));
  const tabs = $derived(allTabs.filter((item) => visible.has(item.id)));
  const filters: Array<{ id: SessionFilter; label: string; modern: string }> = [
    { id: 'all', label: 'ALL', modern: 'All' },
    { id: 'sta', label: 'STA', modern: 'Status' },
    { id: 'agt', label: 'AGT', modern: 'Agent' },
  ];
  const inactive = 'rgba(255,255,255,.55)';
</script>

<header class="hdr" class:edge={variant !== 'B'} class:modern={theme === 'modern'} data-header={tab}>
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
        {#if theme === 'modern'}
          <LineIcon name={item.line} color={tab === item.id ? '#fff' : inactive} size={11} />
          {#if tab === item.id}<span class="tl">{item.modern}</span>{/if}
        {:else}
          <PixelIcon name={item.icon} size={1.35} color={tab === item.id ? PALETTE.cyan : inactive} />
          {#if tab === item.id}
            <PixelText text={item.label} size={1.15} color={PALETTE.cyan} />
          {/if}
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
          {#if theme === 'modern'}
            <span class="fb">{item.modern}</span>
          {:else}
            <PixelText text={item.label} size={1.05} color={filter === item.id ? PALETTE.cyan : inactive} />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
  <button class="hbtn" class:muted data-mute aria-label={muted ? 'Unmute' : 'Mute'} onclick={onMute}>
    {#if theme === 'modern'}
      <LineIcon name="spk" color="#fff" size={11} />
    {:else}
      <PixelIcon name={muted ? 'volumeMute' : 'volume'} size={1.35} color={muted ? PALETTE.red : PALETTE.textWhite} />
    {/if}
  </button>
  <button class="hbtn" data-settings aria-label="Settings" onclick={onSettings}>
    {#if theme === 'modern'}
      <LineIcon name="gear" color="#fff" size={11} />
    {:else}
      <PixelIcon name="settings" size={1.35} color={PALETTE.textWhite} />
    {/if}
  </button>
  {#if quitArmed}
    <button class="quit" data-quit aria-label="Confirm quit" onclick={onQuit}>
      <PixelText text="QUIT?" size={1.15} color="#000" />
    </button>
  {:else}
    <button class="hbtn power" data-quit aria-label="Quit Morrow" onclick={onQuit}>
      {#if theme === 'modern'}
        <LineIcon name="power" color={PALETTE.red} size={11} />
      {:else}
        <PixelIcon name="power" size={1.35} color={PALETTE.red} />
      {/if}
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
  /* #202 — canvas.css:141-149,152: Modern Clean's white-on-glass tab. */
  .modern .tab.on {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }
  .tl {
    font: 600 10px var(--ui, system-ui, sans-serif);
  }
  .fb {
    font: 500 9.5px var(--ui, system-ui, sans-serif);
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
  .modern .filt {
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 0.6px rgba(255, 255, 255, 0.12);
    border-radius: 7px;
  }
  .modern .fbtn {
    height: auto;
    padding: 3px 6px;
    border-radius: 5px;
    color: rgba(255, 255, 255, 0.6);
  }
  .modern .fbtn.on {
    font-weight: 700;
    color: #fff;
    background: rgba(255, 255, 255, 0.25);
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
  .modern .hbtn {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.8);
  }
  .modern .hbtn.muted {
    background: rgba(255, 255, 255, 0.08);
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
