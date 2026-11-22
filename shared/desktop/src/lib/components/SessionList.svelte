<script lang="ts">
  // #183 — status, running tool, last prompt and last reply of every session,
  // drawn like the canvas sections #list-comfortable and #list-status. The
  // shell already sends them in tier order (needs you, working, the rest).
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import PixelText from './PixelText.svelte';
  import { displayName } from '../approval';
  import { MASCOT_SIZES } from '../mascots';
  import { STATUS_META, mascotStatus, timeLabel, toolLabel } from '../session-cards';
  import { PALETTE } from '../tokens';
  import type { Card, Decision, Session } from '../types';

  type Props = {
    sessions: Session[];
    /** Pending, non-hidden requests (#approval-queue: inline in the list). */
    approvals?: Card[];
    onAnswer?: (id: number, decision: Decision) => void;
  };
  let { sessions, approvals = [], onAnswer }: Props = $props();
</script>

<ul class="list">
  {#each sessions as session (session.session_id)}
    {@const meta = STATUS_META[session.status]}
    {@const approval = approvals.find((card) => card.session_id === session.session_id)}
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
        {#if approval}
          <div class="papp">
            <div class="r">
              <PixelText text="[!]" size={1.2} color={PALETTE.amber} />
              <PixelText text="APPROVAL REQUIRED:" size={1.1} color={PALETTE.amber} />
              <PixelText text={displayName(approval.tool_name)} size={1.1} color={PALETTE.textWhite} />
            </div>
            <div class="r">
              <button class="p16 acc" onclick={() => onAnswer?.(approval.id, 'allow')}>
                <PixelIcon name="checkmark" size={1} color="#000" />
                <PixelText text="APPROVE" size={1.05} color="#000" />
              </button>
              <button class="p16" onclick={() => onAnswer?.(approval.id, 'deny')}>
                <PixelText text="✕ DENY" size={1.05} color={PALETTE.deny} />
              </button>
            </div>
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
  {:else}
    <li class="empty">No sessions</li>
  {/each}
</ul>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 4px;
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
  .papp {
    align-self: flex-start;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--amber) 8%, transparent);
  }
  .papp .r {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .p16 {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3.5px 7px;
    border: 0;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.06);
    font: inherit;
  }
  .p16:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .p16.acc {
    background: var(--cyan);
  }
  .empty {
    color: var(--tm);
    padding: 8px;
  }
</style>
