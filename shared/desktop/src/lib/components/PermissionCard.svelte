<script lang="ts">
  import type { Card, Decision } from '../types';

  type Props = { card: Card; onAnswer: (id: number, decision: Decision) => void };
  let { card, onAnswer }: Props = $props();
</script>

<!-- data-card / .approve / .deny are the E2E-1 smoke seam: keep them intact. -->
<section class="card" data-card={card.id}>
  <div class="tool"><span class="source">{card.source}</span> {card.tool_name}</div>
  {#if card.summary}<div class="summary">{card.summary}</div>{/if}
  <div class="actions">
    <button class="deny" onclick={() => onAnswer(card.id, 'deny')}>Deny</button>
    <button class="approve" onclick={() => onAnswer(card.id, 'allow')}>Approve</button>
  </div>
</section>

<style>
  .card {
    padding: 8px 0 0;
    border-top: 1px solid #222;
  }
  .tool {
    font-weight: 600;
  }
  .source {
    color: #d0d0d0;
  }
  .summary {
    margin-top: 4px;
    color: #c7c7cc;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
  .actions button {
    width: 84px;
    height: 28px;
    border: 0;
    border-radius: 14px;
    font: inherit;
    font-weight: 600;
    cursor: default;
  }
  .deny {
    background: #3a3a3c;
    color: #f2f2f2;
  }
  .approve {
    background: #f2f2f2;
    color: #000;
  }
</style>
