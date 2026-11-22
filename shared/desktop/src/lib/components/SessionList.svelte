<script lang="ts">
  // #183 — status, running tool, last prompt and last reply of every session,
  // drawn like the canvas sections #list-comfortable and #list-status. The
  // shell already sends them in tier order (needs you, working, the rest).
  // #185 — 34 pt compact rows (#list-compact), ALL/STA/AGT grouping, a capped
  // scroll viewport with an "N more sessions" footer and the hooks empty state
  // (#list-empty). Comfortable cards stay available for #183/#184/#192.
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import PixelText from './PixelText.svelte';
  import { MASCOT_SIZES } from '../mascots';
  import { STATUS_META, mascotStatus, timeLabel, toolColor, toolLabel } from '../session-cards';
  import { COMFORTABLE_SESSION_LIMIT, VISIBLE_SESSION_LIMIT, groupSessions, splitOverflow } from '../session-list';
  import type { SessionDensity, SessionFilter } from '../panel';
  import { PALETTE } from '../tokens';
  import type { Session } from '../types';

  type Props = {
    sessions: Session[];
    filter: SessionFilter;
    density?: SessionDensity;
    onDelete: (sessionId: string) => void;
    onOpenSettings: () => void;
  };
  let {
    sessions,
    filter,
    density = 'compact',
    onDelete,
    onOpenSettings,
  }: Props = $props();

  const limit = $derived(
    density === 'compact' ? VISIBLE_SESSION_LIMIT : COMFORTABLE_SESSION_LIMIT,
  );
  const groups = $derived(groupSessions(sessions, filter));
  // Every row stays in the DOM: the scroller shows the first `limit` of them
  // and the footer counts the rest. Hiding the rest would leave nothing to
  // scroll to — this is a viewport, not a truncation.
  const overflow = $derived(splitOverflow(groups.flatMap((group) => group.sessions), limit));

  let scroller: HTMLDivElement;

  function more() {
    scroller?.scrollBy({ top: scroller.clientHeight, behavior: 'smooth' });
  }
</script>

{#if sessions.length === 0}
  <div class="empty">
    <Mascot source="claude" status="idle" size={32} hidden />
    <PixelText text="NO SESSIONS YET" size={1.3} color={PALETTE.textMuted} />
    <p>Start an agent in a terminal. Agents without our hook do not show up: open Settings → Hooks.</p>
    <button class="open-settings" data-open-settings onclick={onOpenSettings}>
      <PixelText text="Open Settings" size={1.05} color={PALETTE.textWhite} />
    </button>
  </div>
{:else}
  <div class="scroller" class:compact={density === 'compact'} bind:this={scroller}>
    <ul class="list" class:compact={density === 'compact'}>
      {#each groups as group (group.key)}
        {#if group.label}
          <li class="ghead">
            <PixelText text={group.label} size={1.05} color={group.color} />
            <PixelText text={String(group.sessions.length)} size={1.05} color={PALETTE.textDim} />
          </li>
        {/if}
        {#each group.sessions as session (session.session_id)}
          {@const meta = STATUS_META[session.status]}
          {#if density === 'compact'}
            <li class="crow" style="--sc:{meta.color}">
              <Mascot
                source={session.source}
                status={mascotStatus(session.status)}
                size={MASCOT_SIZES.compact}
              />
              <span class="cname">
                <PixelText
                  text={session.cwd_name || session.session_id}
                  size={1.05}
                  color={PALETTE.textWhite}
                  max={24}
                />
              </span>
              <span class="cdetail">
                {#if session.current_tool}
                  <PixelText
                    text={'[' + toolLabel(session.current_tool) + ']'}
                    size={0.95}
                    color={toolColor(session.current_tool)}
                  />
                {:else if session.status === 'thinking'}
                  <span class="thinkdots" aria-hidden="true"><i></i><i></i><i></i></span>
                {:else if session.last_user_prompt}
                  <span class="quote">› {session.last_user_prompt}</span>
                {/if}
              </span>
              <span class="pb" class:fill={meta.fill} style="--c:{meta.color}">
                <PixelText text={meta.label} size={1.05} color={meta.fill ? '#000' : meta.color} />
              </span>
              <span class="ctail">
                <button
                  class="del"
                  aria-label="Remove {session.cwd_name || session.session_id}"
                  title="Remove session"
                  onclick={() => onDelete(session.session_id)}
                >✕</button>
                <PixelText
                  text={timeLabel(session.updated_at, Date.now())}
                  size={1.05}
                  color={PALETTE.textDim}
                />
              </span>
            </li>
          {:else}
            <li class="pcard" style="--sc:{meta.color}">
              <div class="pslot">
                <Mascot
                  source={session.source}
                  status={mascotStatus(session.status)}
                  size={MASCOT_SIZES.card}
                />
                <span class="leds"><i></i><i></i><i></i></span>
              </div>
              <div class="pbody">
                <div class="prow">
                  <span class="pb" class:fill={meta.fill} style="--c:{meta.color}">
                    <PixelText text={meta.label} size={1.05} color={meta.fill ? '#000' : meta.color} />
                  </span>
                  <span class="pb" style="--c:{PALETTE.textMuted}">
                    <PixelText text={session.source} size={1.05} color={PALETTE.textMuted} />
                  </span>
                  <PixelIcon name="workspace" size={0.85} color={PALETTE.textWhite} />
                  <PixelText
                    text={session.cwd_name || session.session_id}
                    size={1.05}
                    color={PALETTE.textWhite}
                    max={26}
                  />
                  <span class="sp"></span>
                  <button
                    class="del"
                    aria-label="Remove {session.cwd_name || session.session_id}"
                    title="Remove session"
                    onclick={() => onDelete(session.session_id)}
                  >✕</button>
                  <PixelText
                    text={timeLabel(session.updated_at, Date.now())}
                    size={1.05}
                    color={PALETTE.textDim}
                  />
                </div>
                {#if session.current_tool}
                  <div class="ptool">
                    <span class="pb" style="--c:{meta.color}">
                      <PixelText
                        text={'▸ ' + toolLabel(session.current_tool)}
                        size={1.05}
                        color={meta.color}
                      />
                    </span>
                    <span class="detail">{session.tool_description}</span>
                  </div>
                {/if}
                {#if session.last_user_prompt}
                  <div class="pline">
                    <b class="prompt">›</b><span>{session.last_user_prompt}</span>
                  </div>
                {/if}
                {#if session.last_assistant_message}
                  <div class="pline reply">
                    <b style="color:{meta.color}">●</b><span>{session.last_assistant_message}</span>
                  </div>
                {/if}
              </div>
            </li>
          {/if}
        {/each}
      {/each}
    </ul>
  </div>
  {#if overflow.remaining > 0}
    <button class="more" data-more onclick={more}>
      {overflow.remaining} more sessions
      <svg class="chev" viewBox="0 0 16 16" width="10" height="10" aria-hidden="true">
        <path d="M4.5 6.5L8 10l3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.4" />
      </svg>
    </button>
  {/if}
{/if}

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 4px;
  }
  .list.compact {
    gap: 3px;
  }
  .scroller {
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
  }
  /* Four 34 pt rows plus their 3 pt gaps; two ~90 pt comfortable cards. */
  .scroller.compact {
    max-height: 145px;
  }
  .scroller:not(.compact) {
    max-height: 186px;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 2px;
  }
  .ghead::after {
    content: '';
    flex: 1;
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
  }
  .more {
    box-sizing: border-box;
    width: calc(100% - 28px);
    margin: 4px 14px 0;
    padding: 8px 0 2px;
    border: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    background: none;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    font: 500 calc(var(--f) * 1) ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.55);
    cursor: default;
  }
  .more:hover {
    color: var(--tw);
  }
  .chev {
    flex: none;
  }
  .empty {
    padding: 26px 14px 22px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .empty p {
    margin: 0;
    font: calc(var(--f) * 0.95) ui-monospace, monospace;
    color: var(--tm);
    max-width: 360px;
  }
  .open-settings {
    min-height: 26px;
    padding: 6px 11px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.28);
    display: inline-flex;
    align-items: center;
    cursor: default;
  }
  .open-settings:hover {
    background: rgba(255, 255, 255, 0.08);
  }
  .crow {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 15px minmax(0, 230px) minmax(0, 1fr) auto 30px;
    gap: 8px;
    align-items: center;
    height: 34px;
    padding: 0 7px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.02);
  }
  .crow:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .cname,
  .cdetail,
  .ctail {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .cdetail {
    overflow: hidden;
    white-space: nowrap;
  }
  .ctail {
    justify-content: flex-end;
    gap: 3px;
  }
  .quote {
    overflow: hidden;
    text-overflow: ellipsis;
    font: 500 calc(var(--f) * 0.86) ui-monospace, monospace;
    color: var(--tw);
  }
  .thinkdots {
    display: inline-flex;
    gap: 2px;
  }
  .thinkdots i {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--cyan);
  }
  .pcard {
    display: grid;
    grid-template-columns: 34px 1fr;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
  }
  .pcard:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .pslot {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding-top: 2px;
  }
  .leds {
    display: flex;
    gap: 2px;
  }
  .leds i {
    width: 5px;
    height: 2px;
    background: var(--sc);
    opacity: 0.35;
  }
  .leds i:first-child {
    opacity: 1;
  }
  .pbody {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .prow {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }
  .prow .sp {
    flex: 1;
  }
  .pb {
    display: inline-flex;
    align-items: center;
    padding: 2.5px 5px;
    border-radius: 2.5px;
    background: color-mix(in srgb, var(--c) 12%, transparent);
    flex: none;
  }
  .pb.fill {
    background: var(--c);
  }
  .del {
    padding: 0 2px;
    border: 0;
    background: none;
    color: var(--red);
    font: 900 calc(var(--f) * 0.9) ui-monospace, monospace;
    opacity: 0;
    pointer-events: none;
    cursor: default;
  }
  .crow:hover .del,
  .pcard:hover .del,
  .del:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }
  .ptool {
    display: flex;
    align-items: center;
    gap: 6px;
    font: calc(var(--f) * 0.78) ui-monospace, monospace;
    color: var(--tw);
    min-width: 0;
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pline {
    display: flex;
    gap: 6px;
    font: 500 calc(var(--f) * 0.86) ui-monospace, monospace;
    color: var(--tw);
    min-width: 0;
  }
  .pline b {
    font-weight: 900;
    flex: none;
  }
  .pline span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pline.reply {
    font-weight: 400;
    font-size: calc(var(--f) * 0.82);
  }
  .prompt {
    color: rgba(0, 245, 212, 0.8);
  }
</style>
