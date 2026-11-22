<script lang="ts">
  // Render and input only (docs/plan-windows-linux.md §5.3): the shell owns
  // the sessions, the permission requests and the window; this file draws
  // what it is sent, reports the island's size so the shell can fit the
  // window to it, and passes on the user's Approve / Deny.
  import { onMount } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import CollapsedBar from './lib/components/CollapsedBar.svelte';
  import SessionList from './lib/components/SessionList.svelte';
  import PermissionCard from './lib/components/PermissionCard.svelte';
  import PixelIcon from './lib/components/PixelIcon.svelte';
  import { PALETTE } from './lib/tokens';
  import type { Card, Decision, Session, Snapshot } from './lib/types';

  let sessions = $state<Session[]>([]);
  let cards = $state<Card[]>([]);
  let expanded = $state(false);
  let island: HTMLElement;

  function answer(id: number, decision: Decision) {
    invoke('answer_permission', { id, decision });
  }

  // #199 — Settings moved to its own window; the gear only asks the shell to
  // create or raise it, lazily on first use.
  function openSettings() {
    invoke('open_settings');
  }

  // For the E2E-1 gate (the shell prints them only with MORROW_SMOKE=1).
  function reportCards() {
    const rect = (el: Element | null) => {
      const r = el!.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    };
    for (const el of island.querySelectorAll<HTMLElement>('[data-card]')) {
      invoke('smoke_card', {
        id: Number(el.dataset.card),
        approve: rect(el.querySelector('.approve')),
        deny: rect(el.querySelector('.deny')),
      });
    }
  }

  onMount(() => {
    const channel = new Channel<Snapshot>();
    channel.onmessage = (snapshot) => {
      sessions = snapshot.sessions;
      cards = snapshot.permissions;
    };
    invoke('subscribe', { channel });

    // Tight-fit window: the window is as big as the island and no bigger.
    // Measured after every layout change (collapse, expand, a row, a card).
    const observer = new ResizeObserver(() => {
      invoke('set_island_size', { width: island.offsetWidth, height: island.offsetHeight }).then(
        reportCards,
      );
    });
    observer.observe(island);
    return () => observer.disconnect();
  });
</script>

<div class="island" class:wide={expanded || cards.length > 0} bind:this={island}>
  <button class="toggle" onclick={() => (expanded = !expanded)}>
    {#if expanded || cards.length > 0}
      <div class="pill">
        <span class="dot" class:live={sessions.length > 0}></span>
        {#if sessions.length > 0}
          <span class="count">{sessions.length}</span>
          <span class="source">{sessions[0].source}</span>
        {:else}
          <span class="source">Morrow</span>
        {/if}
      </div>
    {:else}
      <CollapsedBar sessionCount={sessions.length} />
    {/if}
    {#if expanded}
      <div class="padded"><SessionList {sessions} /></div>
    {/if}
  </button>
  {#if expanded || cards.length > 0}
    <button class="gear" aria-label="Settings" title="Settings" onclick={openSettings}>
      <PixelIcon name="settings" size={1} color={PALETTE.textMuted} />
    </button>
  {/if}
  {#each cards as card (card.id)}
    <div class="padded"><PermissionCard {card} onAnswer={answer} /></div>
  {/each}
</div>

<style>
  /* Fixed widths, so the measured size never depends on the window's. */
  .island {
    position: absolute;
    top: 0;
    left: 0;
    box-sizing: border-box;
    width: 240px;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 0 0 17px 17px;
    background: #000;
    color: #f2f2f2;
    font: 13px/1.2 system-ui, sans-serif;
    text-align: left;
    cursor: default;
  }
  .island.wide {
    width: 360px;
    padding-bottom: 10px;
    border-radius: 0 0 20px 20px;
  }
  .toggle {
    display: block;
    width: 100%;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: inherit;
    cursor: default;
  }
  .padded {
    padding: 0 14px;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
  }
  .dot.live {
    background: #34c759;
  }
  .count {
    font-weight: 600;
  }
  .source {
    color: #d0d0d0;
  }
  /* Absolutely placed so the gear never changes the island's measured size. */
  .gear {
    position: absolute;
    top: 9px;
    right: 10px;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    background: none;
    cursor: default;
  }
</style>
