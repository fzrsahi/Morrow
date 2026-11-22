<script lang="ts">
  // #188/#189 — the Question card of sections #question / #question-multi.
  // The shell holds the hook and writes the answer; this component draws the
  // options (or the typed field) and forwards the pick / Skip / Hide.
  import { tick } from 'svelte';
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { GONE_NOTICE, SHORTCUTS, queueLabel, shortcutHint } from '../approval';
  import { MASCOT_SIZES } from '../mascots';
  import { PALETTE } from '../tokens';
  import { canSubmitTyped, OTHER_LABEL, questionInput } from '../question';
  import type { QuestionCardView } from '../question';
  import type { Session } from '../types';

  type Props = {
    card: QuestionCardView;
    session?: Session | null;
    /** 1-based place in the pending queue. */
    position?: number;
    total?: number;
    hotkeySupported?: boolean;
    /** The label being answered (click or number key): held for the in-flight reply. */
    selected?: string | null;
    gone?: boolean;
    onPick: (id: number, label: string) => void;
    onSkip: (id: number) => void;
    onHide: (id: number) => void;
    /** #189: the field was clicked; the panel asks the island for keyboard focus. */
    onFieldFocus: (id: number) => void;
  };
  let {
    card,
    session = null,
    position = 1,
    total = 1,
    hotkeySupported = false,
    selected = null,
    gone = false,
    onPick,
    onSkip,
    onHide,
    onFieldFocus,
  }: Props = $props();

  let hovered = $state(-1);
  let draft = $state('');
  let showOther = $state(false);
  let inputEl: HTMLInputElement;
  let queue = $derived(queueLabel(position, total));
  let hideKey = $derived(shortcutHint(SHORTCUTS.hide, hotkeySupported));
  let typed = $derived(questionInput(card));
  let canSubmit = $derived(canSubmitTyped(draft) && !gone && selected === null);

  // The Other row reveals the field and focuses it; the input's onfocus is
  // the one place that asks the shell to make the island keyboard-accessible.
  async function revealOther() {
    if (gone || selected !== null) return;
    showOther = true;
    await tick();
    inputEl?.focus();
  }

  function submit() {
    if (!canSubmit) return;
    onPick(card.id, draft);
  }

  function onFieldKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    submit();
  }
</script>

{#snippet answerField()}
  <!-- #189: a real input; the blue :focus-within ring is the canvas' .ans.focus. -->
  <div class="ans">
    <b aria-hidden="true">&gt;</b>
    <input
      bind:this={inputEl}
      class="field"
      type="text"
      placeholder="Type your answer..."
      aria-label="Your answer"
      bind:value={draft}
      disabled={gone || selected !== null}
      onfocus={() => onFieldFocus(card.id)}
      onkeydown={onFieldKeydown}
    />
  </div>
{/snippet}

<!-- data-card is the E2E-1 seam, shared with the approval card. -->
<section class="card" class:dim={gone} data-card={card.id} aria-label="Question">
  <div class="ctx">
    <Mascot
      source={session?.source ?? card.source}
      status="waitingQuestion"
      size={MASCOT_SIZES.compact}
    />
    <PixelIcon name="workspace" size={0.85} color={PALETTE.textWhite} />
    <span class="cwd">{session?.cwd_name || card.session_id}</span>
    {#if queue}<span class="q">{queue}</span>{/if}
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
  <div class="qh">
    <span class="qmark">?</span>
    {#if card.question.header}<span class="tagq">{card.question.header}</span>{/if}
    <span class="qt">{card.question.question}</span>
  </div>
  {#if typed === 'free'}
    {@render answerField()}
  {:else}
    <div class="opts">
      {#each card.question.options as option, i (i)}
        <button
          class="opt"
          class:hov={hovered === i}
          class:sel={selected === option.label}
          disabled={gone || selected !== null}
          onpointerenter={() => (hovered = i)}
          onpointerleave={() => (hovered = -1)}
          onclick={() => onPick(card.id, option.label)}
        >
          <span class="mk">▸</span>
          <span class="ix">{i + 1}.</span>
          <span class="body">
            <span class="ol">{option.label}</span>
            {#if option.description}<span class="od">{option.description}</span>{/if}
          </span>
        </button>
      {/each}
      <button
        class="opt"
        class:hov={hovered === -2}
        class:sel={showOther}
        disabled={gone || selected !== null}
        onpointerenter={() => (hovered = -2)}
        onpointerleave={() => (hovered = -1)}
        onclick={revealOther}
      >
        <span class="mk">▸</span>
        <span class="ix">…</span>
        <span class="body"><span class="ol">{OTHER_LABEL}</span></span>
      </button>
    </div>
    {#if showOther}
      {@render answerField()}
    {/if}
  {/if}
  <div class="acts">
    <span class="sp"></span>
    <button class="btn q" disabled={gone} onclick={() => onHide(card.id)}
      >Hide{#if hideKey}<kbd>{hideKey}</kbd>{/if}</button>
    <button class="btn s" disabled={gone} onclick={() => onSkip(card.id)}>Skip</button>
    {#if typed === 'free' || showOther}
      <button class="btn p" disabled={!canSubmit} onclick={submit}>Submit</button>
    {/if}
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
  .q {
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
  .qh {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .qmark {
    font: 700 var(--f-title) system-ui, sans-serif;
    color: var(--qblue);
  }
  .tagq {
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(102, 179, 255, 0.14);
    color: var(--qblue);
    font: 700 var(--f-badge) ui-monospace, monospace;
  }
  .qt {
    font: 500 var(--f-title) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.92);
  }
  .opts {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 72px;
  }
  .opt {
    display: grid;
    grid-template-columns: 10px auto 1fr;
    gap: 6px;
    align-items: baseline;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .opt:disabled {
    cursor: default;
  }
  .opt.hov {
    background: rgba(255, 255, 255, 0.08);
    box-shadow: inset 0 0 0 1px rgba(102, 179, 255, 0.2);
  }
  .opt.sel {
    background: rgba(102, 179, 255, 0.08);
    box-shadow: inset 0 0 0 1px rgba(102, 179, 255, 0.4);
  }
  .mk {
    color: var(--cyan);
    font: 700 10px ui-monospace, monospace;
    visibility: hidden;
  }
  .opt.hov .mk {
    visibility: visible;
  }
  .ix {
    font: 600 var(--f-sec) ui-monospace, monospace;
    color: rgba(102, 179, 255, 0.8);
  }
  .body {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .ol {
    font: var(--f) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.8);
  }
  .opt.hov .ol,
  .opt.sel .ol {
    color: #fff;
    font-weight: 600;
  }
  .od {
    margin-top: 2px;
    font: var(--f-cap) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.58);
  }
  /* #189 — canvas .ans: padding 6 × 10, white 0.05 fill, white 0.10 stroke. */
  .ans {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    font: var(--f-link) system-ui, sans-serif;
  }
  .ans:focus-within {
    box-shadow: inset 0 0 0 1px rgba(102, 179, 255, 0.55);
  }
  .ans b {
    color: var(--green);
    font: 700 var(--f-sec) ui-monospace, monospace;
  }
  .field {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    outline: 0;
    background: none;
    color: #fff;
    font: inherit;
  }
  .field::placeholder {
    color: rgba(255, 255, 255, 0.4);
  }
  .field:disabled {
    cursor: default;
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
  .q.btn {
    padding: 6px 8px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.75);
  }
  .q.btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.07);
  }
  .s {
    padding: 6px 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.28);
  }
  .s:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
  }
  .p {
    padding: 6px 14px;
    font-weight: 600;
    color: #fff;
    background: var(--allow);
  }
  .p:hover:not(:disabled) {
    /* canvas --allow-h / PALETTE.allowHover */
    background: #2b8742;
  }
  .p:disabled {
    background: rgba(41, 128, 61, 0.22);
    color: rgba(255, 255, 255, 0.35);
    cursor: default;
  }
  kbd {
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.1);
    font: max(8.5px, calc(var(--f) - 3px)) ui-monospace, monospace;
    opacity: 0.85;
  }
</style>
