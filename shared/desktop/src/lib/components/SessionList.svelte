<script lang="ts">
  // #183 — status, running tool, last prompt and last reply of every session;
  // #184 — git branch, tasks, subagents and idle recap. Drawn like the canvas
  // sections #list-comfortable and #list-status. The shell already sends the
  // sessions in tier order (needs you, working, the rest).
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import PixelText from './PixelText.svelte';
  import { MASCOT_SIZES } from '../mascots';
  import {
    STATUS_META,
    branchLabel,
    mascotStatus,
    subLabel,
    taskGauge,
    taskLabel,
    timeLabel,
    toolLabel,
  } from '../session-cards';
  import { PALETTE } from '../tokens';
  import type { Session } from '../types';

  let { sessions }: { sessions: Session[] } = $props();
</script>

<ul class="list">
  {#each sessions as session (session.session_id)}
    {@const meta = STATUS_META[session.status]}
    {@const branch = branchLabel(session.git_branch, session.git_is_worktree)}
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
          {#if branch}
            <PixelText text="//" size={1.05} color={PALETTE.textDim} />
            <svg class="ico" viewBox="0 0 16 16" width="10" height="10" style="color:{meta.color}" aria-hidden="true">
              <circle cx="5" cy="4" r="1.6" /><circle cx="5" cy="12" r="1.6" />
              <circle cx="11" cy="5.5" r="1.6" /><path d="M5 5.6v4.8M11 7.1c0 2.4-6 1.6-6 3.3" />
            </svg>
            <span class="branch" title={branch}>
              <PixelText text={session.git_branch} size={0.95} color={meta.color} max={22} />
              {#if session.git_is_worktree}
                <!-- PixelText has no ⧉ glyph; Swift draws it beside the branch. -->
                <svg class="wtmark" viewBox="0 0 9 9" width="9" height="9" style="color:{meta.color}" aria-hidden="true">
                  <rect x="0.75" y="0.75" width="5.5" height="5.5" rx="0.4" />
                  <rect x="2.75" y="2.75" width="5.5" height="5.5" rx="0.4" />
                </svg>
              {/if}
            </span>
          {/if}
          {#if session.subagent_count > 0}
            <span class="pb" style="--c:{PALETTE.cyan}">
              <PixelText
                text={subLabel(session.subagent_count)}
                size={1.05}
                color={PALETTE.cyan}
              />
            </span>
          {/if}
          <span class="sp"></span>
          <PixelText
            text={timeLabel(session.updated_at, Date.now())}
            size={1.05}
            color={PALETTE.textDim}
          />
        </div>
        {#if session.tasks_total > 0}
          <div class="ptasks">
            <PixelText
              text={taskLabel(session.tasks_done, session.tasks_total)}
              size={0.95}
              color={session.tasks_done === session.tasks_total ? PALETTE.cyan : PALETTE.textWhite}
            />
            <span class="gauge">
              {#each taskGauge(session.tasks_done, session.tasks_total) as segment, i (i)}
                <i class={segment}></i>
              {/each}
            </span>
            {#if session.task_active}
              <PixelText
                text={'▶ ' + session.task_active}
                size={0.95}
                color={PALETTE.textMuted}
                max={40}
              />
            {/if}
          </div>
        {/if}
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
        {#if session.recap}
          <div class="precap"><b>↻</b><span>{session.recap}</span></div>
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
  /* #184 — canvas #list-comfortable: branch line icon, 12-segment gauge,
     subagent count badge and the idle recap line. */
  .ico {
    width: 10px;
    height: 10px;
    flex: none;
    stroke: currentColor;
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    vertical-align: middle;
  }
  .branch {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    min-width: 0;
  }
  .wtmark {
    flex: none;
    stroke: currentColor;
    fill: none;
    stroke-width: 1;
  }
  .ptasks {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .gauge {
    display: flex;
    gap: 1.5px;
    width: 84px;
  }
  .gauge i {
    flex: 1;
    height: 2.5px;
    background: rgba(255, 255, 255, 0.12);
  }
  .gauge i.on {
    background: var(--cyan);
    box-shadow: inset 0 0.8px 0 rgba(255, 255, 255, 0.35);
  }
  .gauge i.cur {
    background: var(--amber);
    animation: pulse 1s steps(2) infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.45;
      transform: scale(0.88);
    }
  }
  .precap {
    display: inline-flex;
    align-self: flex-start;
    gap: 6px;
    padding: 3px 6px;
    border-radius: 2.5px;
    background: rgba(255, 255, 255, 0.04);
    font: 500 calc(var(--f) * 0.77) ui-monospace, monospace;
    color: var(--tm);
  }
  @media (prefers-reduced-motion: reduce) {
    .gauge i.cur {
      animation: none;
    }
  }
  .empty {
    color: var(--tm);
    padding: 8px;
  }
</style>
