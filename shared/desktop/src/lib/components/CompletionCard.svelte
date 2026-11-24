<script lang="ts">
  // #191 — the #completion canvas: a session row, the reply card and the
  // "N sessions" footer. The reply is drawn from core blocks; no Markdown
  // parsing here. The session row reuses #183's card so the status tier and
  // its colours stay the same as in the agents list.
  import MarkdownReply from './MarkdownReply.svelte';
  import SessionList from './SessionList.svelte';
  import type { Completion, Session } from '../types';

  let {
    completion,
    session,
    sessionCount,
  }: { completion: Completion; session: Session; sessionCount: number } = $props();

  // The canvas's head shows the prompt but not the reply; the reply lives in
  // the card below it.
  let head = $derived({ ...session, last_assistant_message: '' });
</script>

<div class="completion">
  <SessionList
    sessions={[head]}
    filter="all"
    density="comfortable"
    onDelete={() => {}}
    onOpenSettings={() => {}}
  />
  <div class="reply" class:failed={completion.turn_failed}>
    <MarkdownReply blocks={completion.blocks} />
  </div>
  <div class="more">
    {sessionCount} sessions
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">
      <path d="M4.5 6.5L8 10l3.5-3.5" />
    </svg>
  </div>
</div>

<style>
  .completion {
    display: flex;
    flex-direction: column;
  }
  .reply {
    padding: 4px 14px 10px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }
  .reply.failed {
    border-left: 2px solid var(--red);
  }
  .more {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    text-align: center;
    font: 500 var(--f) system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.55);
    padding: 8px 0 2px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    margin: 4px 14px 0;
  }
</style>
