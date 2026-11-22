<script lang="ts">
  // #186 — the approval card from sections #approval / #approval-queue /
  // #approval-gone. The shell still owns the decision; this component only
  // spells the request in the macOS copy and forwards the button press.
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import {
    GONE_NOTICE,
    SHORTCUTS,
    approvalBody,
    approvalTitle,
    queueLabel,
    shortcutHint,
  } from '../approval';
  import { MASCOT_SIZES } from '../mascots';
  import { PALETTE } from '../tokens';
  import type { Card, Decision, Session } from '../types';

  type Props = {
    card: Card;
    session?: Session | null;
    /** 1-based place in the pending queue. */
    position?: number;
    total?: number;
    hotkeySupported?: boolean;
    /** `answer_permission` said false: show the notice, then drop the card. */
    gone?: boolean;
    onAnswer: (id: number, decision: Decision) => void;
    onHide: (id: number) => void;
  };
  let {
    card,
    session = null,
    position = 1,
    total = 1,
    hotkeySupported = false,
    gone = false,
    onAnswer,
    onHide,
  }: Props = $props();

  let title = $derived(approvalTitle(card.tool_name, card.tool_input));
  let body = $derived(approvalBody(card.tool_name, card.tool_input, card.summary));
  let queue = $derived(queueLabel(position, total));
  let hideKey = $derived(shortcutHint(SHORTCUTS.hide, hotkeySupported));
  let denyKey = $derived(shortcutHint(SHORTCUTS.deny, hotkeySupported));
  let allowKey = $derived(shortcutHint(SHORTCUTS.allow, hotkeySupported));
</script>

<!-- data-card / .approve / .deny are the E2E-1 smoke seam: keep them intact. -->
<section class="card" class:dim={gone} data-card={card.id}>
  <div class="ctx">
    <Mascot source={session?.source ?? card.source} status="waitingApproval" size={MASCOT_SIZES.compact} />
    <PixelIcon name="workspace" size={0.85} color={PALETTE.textWhite} />
    <span class="cwd">{session?.cwd_name || card.session_id}</span>
    {#if queue}<span class="queue">{queue}</span>{/if}
  </div>
  {#if gone}
    <div class="note info">
      <svg class="ico" viewBox="0 0 16 16" width="11" height="11" aria-hidden="true">
        <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.4" />
        <path d="M8 7v4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        <circle cx="8" cy="4.6" r="0.9" fill="currentColor" />
      </svg>
      <span>{GONE_NOTICE}</span>
    </div>
  {/if}
  <div class="ttl">
    <span class="tagp">PERMISSION</span>
    <span class="tt">{title}</span>
    {#if body.server}<span class="tsrv">MCP · {body.server}</span>{/if}
  </div>
  <div class="code">
    {#each body.lines as line, i (i)}
      {#if line.kind === 'kv'}
        <div><span class="k">{line.key}</span> {line.value}</div>
      {:else}
        <div class={line.kind}>{line.text}</div>
      {/if}
    {/each}
  </div>
  <div class="acts">
    <span class="sp"></span>
    <button
      class="btn quiet"
      disabled={gone}
      onclick={() => onHide(card.id)}
    >Hide{#if hideKey}<kbd>{hideKey}</kbd>{/if}</button>
    <button
      class="btn d deny"
      disabled={gone}
      onclick={() => onAnswer(card.id, 'deny')}
    >Deny{#if denyKey}<kbd>{denyKey}</kbd>{/if}</button>
    <button
      class="btn p approve"
      disabled={gone}
      onclick={() => onAnswer(card.id, 'allow')}
    >Allow once{#if allowKey}<kbd>{allowKey}</kbd>{/if}</button>
  </div>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
  }
  .card.dim {
    opacity: 0.45;
    filter: saturate(0.4);
  }
  .ctx {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 -6px;
    padding: 3px 6px;
    border-radius: 5px;
    font: 700 var(--f) ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.9);
  }
  .ctx:hover {
    background: rgba(255, 255, 255, 0.09);
  }
  .cwd {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .queue {
    margin-left: auto;
    font: 600 var(--f-sec) ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.6);
  }
  .note {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 7px 10px;
    border-radius: 6px;
    font: var(--f-sec) system-ui, sans-serif;
  }
  .note.info {
    background: rgba(115, 184, 255, 0.1);
    color: #bfd9ff;
  }
  .ico {
    flex: none;
    margin-top: 1px;
  }
  .ttl {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tagp {
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(255, 153, 51, 0.14);
    color: var(--tag);
    font: 700 var(--f-badge) ui-monospace, monospace;
    letter-spacing: 0.02em;
  }
  .tt {
    font: 600 var(--f-title) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.92);
  }
  .tsrv {
    font: var(--f-sec) system-ui, sans-serif;
    color: #99b3e6;
  }
  .code {
    min-height: calc(var(--f) * 3.75 + 16px);
    padding: 8px 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    font: var(--f) / 1.25 ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.9);
    white-space: pre-wrap;
  }
  .code .comment {
    color: rgba(255, 255, 255, 0.5);
  }
  .code .add {
    color: var(--green);
  }
  .code .rem {
    color: #ff6666;
  }
  .code .k {
    display: inline-block;
    min-width: 46px;
    color: #9aa6ba;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .acts .sp {
    flex: 1;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 26px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: var(--f-btn);
  }
  .btn:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .quiet {
    padding: 6px 8px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.75);
  }
  .quiet:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.07);
  }
  .d {
    padding: 6px 11px;
    font-weight: 600;
    color: var(--deny);
    box-shadow: inset 0 0 0 1px rgba(235, 97, 97, 0.7);
  }
  .d:hover:not(:disabled) {
    background: rgba(235, 97, 97, 0.14);
    box-shadow: inset 0 0 0 1px rgba(235, 97, 97, 0.9);
  }
  .p {
    padding: 6px 14px;
    font-weight: 600;
    color: #fff;
    background: var(--allow);
  }
  .p:hover:not(:disabled) {
    background: #2b8742;
  }
  kbd {
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.1);
    font: max(8.5px, calc(var(--f) - 3px)) ui-monospace, monospace;
    opacity: 0.85;
  }
</style>
