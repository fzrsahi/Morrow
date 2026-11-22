<script lang="ts">
  import type { Session } from '../types';

  let { sessions }: { sessions: Session[] } = $props();
</script>

<ul class="list">
  {#each sessions as session (session.session_id)}
    <li>
      <span class="source">{session.source}</span>
      <span class="cwd">{session.cwd_name || session.session_id}</span>
      <span class="event">{session.last_event}</span>
    </li>
  {:else}
    <li class="empty">No sessions</li>
  {/each}
</ul>

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .list li {
    display: grid;
    grid-template-columns: 70px 1fr auto;
    gap: 8px;
    padding: 6px 0;
    border-top: 1px solid #222;
  }
  .source {
    color: #d0d0d0;
  }
  .cwd {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .event {
    color: #8e8e93;
  }
  .empty {
    color: #8e8e93;
  }
</style>
