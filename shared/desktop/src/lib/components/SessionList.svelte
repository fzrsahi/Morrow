<script lang="ts">
  // #181 — each session card carries its own source mascot at 28 pt. The
  // layout stays minimal until #183 fills in status/tool/prompt/reply.
  import Mascot from './Mascot.svelte';
  import { MASCOT_SIZES } from '../mascots';
  import type { Session } from '../types';

  let { sessions }: { sessions: Session[] } = $props();
</script>

<ul class="list">
  {#each sessions as session (session.session_id)}
    <li class="card">
      <Mascot source={session.source} status="working" size={MASCOT_SIZES.card} />
      <div class="body">
        <div class="source">{session.source}</div>
        <div class="cwd">{session.cwd_name || session.session_id}</div>
        <div class="event">{session.last_event}</div>
      </div>
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
  .card {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-top: 1px solid #222;
  }
  .body {
    min-width: 0;
    flex: 1;
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
