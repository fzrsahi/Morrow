<script lang="ts">
  // #200 — the mini-peek (canvas #anchoring). Its own small window, not an
  // island state: the island's tight-fit/E2E-1 window is never resized or
  // re-positioned by this notice. 440 × 100, shown once at launch for 5 s;
  // hovering keeps it open until the pointer leaves (#180 tokens, #182 window
  // rules: fixed size, no per-frame resize, no focus).
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    anchoringReason,
    NOTICE_DURATION_MS,
    NoticeLifetime,
    type AnchoringResult,
  } from '../anchoringNotice';

  let reason = $state<string | null>(null);
  const lifetime = new NoticeLifetime();
  let timer: ReturnType<typeof setTimeout> | undefined;

  function close() {
    if (timer !== undefined) clearTimeout(timer);
    // A custom command: the capability file only grants core:default to the
    // island window, so the peek cannot call the core window API itself.
    invoke('close_peek').catch(() => {});
  }

  onMount(() => {
    invoke<AnchoringResult>('anchoring')
      .then((result) => {
        reason = anchoringReason(result);
        // Defensive: the shell only creates this window for Unavailable.
        if (reason === null) close();
      })
      .catch(() => close());
    timer = setTimeout(() => {
      if (lifetime.timeout()) close();
    }, NOTICE_DURATION_MS);
    return () => {
      if (timer !== undefined) clearTimeout(timer);
    };
  });
</script>

<div
  class="peek"
  role="status"
  aria-live="polite"
  onpointerenter={() => lifetime.pointerEnter()}
  onpointerleave={() => {
    if (lifetime.pointerLeave()) close();
  }}
>
  <span class="ic" aria-hidden="true">
    <!-- The #anchoring canvas warning glyph (components.js SV.warn); it is not
         part of the exported pixel font, so it is inlined here. -->
    <svg viewBox="0 0 16 16" width="16" height="16">
      <path d="M8 2.5l6 10.5H2z" />
      <path d="M8 6.5v3M8 11.3h.01" />
    </svg>
  </span>
  <div class="copy">
    <div class="t1">The island is a normal window here</div>
    <div class="t2">{reason ?? ''}</div>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }
  .peek {
    box-sizing: border-box;
    width: 440px;
    height: 100px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--chassis);
    color: var(--tw);
    border-radius: 12px;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.12),
      0 8px 24px rgba(0, 0, 0, 0.45);
    font-family: system-ui, sans-serif;
    text-align: left;
    overflow: hidden;
  }
  .ic {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    flex: none;
    display: grid;
    place-items: center;
    background: rgba(255, 184, 51, 0.14);
    color: var(--amber);
  }
  .ic svg {
    stroke: currentColor;
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .copy {
    min-width: 0;
  }
  .t1 {
    font: 600 var(--f-title) system-ui, sans-serif;
  }
  .t2 {
    font: var(--f) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.65);
    margin-top: 1px;
  }
  @media (prefers-reduced-motion: reduce) {
    .peek {
      transition: none;
    }
  }
</style>
