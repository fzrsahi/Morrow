<script lang="ts">
  // #183 — status, tool, prompt, reply; #184 — git branch, tasks, subagents,
  // recap; #185 — 34 pt compact rows, ALL/STA/AGT grouping, capped scroller
  // with an "N more sessions" footer and the hooks empty state. Drawn like the
  // canvas sections #list-comfortable, #list-status, #list-compact, #list-empty.
  import Mascot from './Mascot.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import PixelText from './PixelText.svelte';
  import LineIcon from './LineIcon.svelte';
  import { displayName } from '../approval';
  import { MASCOT_SIZES } from '../mascots';
  import {
    STATUS_META,
    branchLabel,
    liveReply,
    mascotStatus,
    modernStatus,
    subLabel,
    taskGauge,
    taskLabel,
    timeLabel,
    toolColor,
    toolLabel,
  } from '../session-cards';
  import {
    COMFORTABLE_SESSION_LIMIT,
    VISIBLE_SESSION_LIMIT,
    groupSessions,
    splitOverflow,
  } from '../session-list';
  import type { SessionDensity, SessionFilter } from '../panel';
  import { PALETTE, type Theme } from '../tokens';
  import type { Card, Decision, Session } from '../types';

  type Props = {
    theme?: Theme;
    sessions: Session[];
    filter: SessionFilter;
    density?: SessionDensity;
    maxVisible?: number;
    details?: import('../agents-settings').CardDetails;
    onDelete: (sessionId: string) => void;
    onOpenSettings: () => void;
    /** Pending, non-hidden requests (#approval-queue: inline in the list). */
    approvals?: Card[];
    onAnswer?: (id: number, decision: Decision) => void;
  };
  let {
    theme = 'pixel',
    sessions,
    filter,
    density = 'compact',
    maxVisible = 8,
    details,
    onDelete,
    onOpenSettings,
    approvals = [],
    onAnswer,
  }: Props = $props();

  const limit = $derived(
    density === 'compact' ? VISIBLE_SESSION_LIMIT : COMFORTABLE_SESSION_LIMIT,
  );
  const groups = $derived(groupSessions(sessions, filter));
  // Every row stays in the DOM: the scroller shows the first `limit` of them
  // and the footer counts the rest. Hiding the rest would leave nothing to
  // scroll to — this is a viewport, not a truncation.
  const overflow = $derived(splitOverflow(groups.flatMap((group) => group.sessions), limit));

  let scroller = $state<HTMLDivElement>();

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
          {@const approval = approvals.find((card) => card.session_id === session.session_id)}
          {@const reply = liveReply(session)}
          {@const branch = branchLabel(session.git_branch, session.git_is_worktree)}
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
            {#if theme === 'modern'}
              {@const modern = modernStatus(session.status)}
              <li class="mcard" style="--sc:{modern.color}">
                <i class="rail"></i>
                <Mascot
                  source={session.source}
                  status={mascotStatus(session.status)}
                  size={MASCOT_SIZES.card}
                />
                <div class="mbody">
                  <div class="mrow">
                    <span class="chip">{modern.label}</span>
                    <b class="mcwd">{session.cwd_name || session.session_id}</b>
                    {#if session.git_branch}
                      <span class="mbr"><LineIcon name="branch" size={10} />{session.git_branch}</span>
                    {/if}
                    {#if session.subagent_count > 0}
                      <span class="mtag">+{session.subagent_count} Sub</span>
                    {/if}
                    <span class="hsp"></span>
                    <span class="msrc">{session.source}</span>
                    <span class="mt">{timeLabel(session.updated_at, Date.now()).toLowerCase()}</span>
                  </div>
                  {#if session.current_tool}
                    <div class="mtool">
                      <span class="tc" style="--tc:{toolColor(session.current_tool)}">{toolLabel(session.current_tool).charAt(0) + toolLabel(session.current_tool).slice(1).toLowerCase()}</span>
                      <span>{session.tool_description}</span>
                    </div>
                  {/if}
                  {#if approval}
                    <div class="mapp">
                      <span class="tagp">PERMISSION</span>
                      <span class="mttl">{displayName(approval.tool_name)} wants to run a command</span>
                      <span class="sp"></span>
                      <button class="mdeny deny" onclick={() => onAnswer?.(approval.id, 'deny')}>Deny</button>
                      <button class="mallow approve" onclick={() => onAnswer?.(approval.id, 'allow')}>Allow once</button>
                    </div>
                  {/if}
                  {#if session.last_user_prompt}
                    <div class="mline"><b>›</b><span>{session.last_user_prompt}</span></div>
                  {/if}
                  {#if reply}
                    <div class="mline reply"><span>{reply}</span></div>
                  {/if}
                </div>
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
                  {#if branch}
                    <PixelText text="//" size={1.05} color={PALETTE.textDim} />
                    <svg class="ico" viewBox="0 0 16 16" width="10" height="10" style="color:{meta.color}" aria-hidden="true">
                      <circle cx="5" cy="4" r="1.6" /><circle cx="5" cy="12" r="1.6" />
                      <circle cx="11" cy="5.5" r="1.6" /><path d="M5 5.6v4.8M11 7.1c0 2.4-6 1.6-6 3.3" />
                    </svg>
                    <span class="branch" title={branch}>
                      <PixelText text={session.git_branch} size={0.95} color={meta.color} max={22} />
                      {#if session.git_is_worktree}
                        <svg class="wtmark" viewBox="0 0 9 9" width="9" height="9" style="color:{meta.color}" aria-hidden="true">
                          <rect x="0.75" y="0.75" width="5.5" height="5.5" rx="0.4" />
                          <rect x="2.75" y="2.75" width="5.5" height="5.5" rx="0.4" />
                        </svg>
                      {/if}
                    </span>
                  {/if}
                  {#if session.subagent_count > 0}
                    <span class="pb" style="--c:{PALETTE.cyan}">
                      <PixelText text={subLabel(session.subagent_count)} size={1.05} color={PALETTE.cyan} />
                    </span>
                  {/if}
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
                      <PixelText text={'▶ ' + session.task_active} size={0.95} color={PALETTE.textMuted} max={40} />
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
                    <b style="color:{meta.color}">●</b><span>{reply}</span>
                  </div>
                {/if}
                {#if session.recap}
                  <div class="precap"><b>↻</b><span>{session.recap}</span></div>
                {/if}
              </div>
            </li>
            {/if}
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
{/if}<style>
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

  /* #202 — canvas.css:194-207 / components.js modernCard(): a single card
     with a status rail, title-case chip and system-font rows. */
  .mcard {
    position: relative;
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 10px;
    padding: 10px 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.05);
    margin: 0 2px;
    overflow: hidden;
  }
  .mcard .rail {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--sc);
  }
  .mbody {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .mrow {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--f);
  }
  .mrow .hsp {
    flex: 1;
  }
  .chip {
    font: 700 clamp(9px, calc(var(--f) - 2px), 10px) var(--ui, ui-monospace, monospace);
    color: var(--sc);
    background: color-mix(in srgb, var(--sc) 15%, transparent);
    padding: 2px 5px;
    border-radius: 4px;
  }
  .mcwd {
    font: 700 var(--f) var(--ui, ui-monospace, monospace);
  }
  .msrc,
  .mbr,
  .mt {
    color: rgba(255, 255, 255, 0.55);
    font: var(--f-sec) var(--ui, ui-monospace, monospace);
    display: inline-flex;
    gap: 3px;
    align-items: center;
  }
  .mtag {
    font: 600 var(--f-cap) var(--ui, system-ui, sans-serif);
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--subagent, #a68cf2);
    background: color-mix(in srgb, var(--subagent, #a68cf2) 15%, transparent);
  }
  .mtool {
    display: flex;
    gap: 6px;
    align-items: center;
    font: var(--f) var(--ui, ui-monospace, monospace);
    color: rgba(255, 255, 255, 0.85);
    min-width: 0;
  }
  .mtool .tc {
    font: 700 max(9px, calc(var(--f) - 1px)) var(--ui, ui-monospace, monospace);
    padding: 1px 4px;
    border-radius: 3px;
    color: var(--tc);
    background: color-mix(in srgb, var(--tc) 15%, transparent);
  }
  .mtool span:last-child,
  .mline span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mline {
    display: flex;
    gap: 6px;
    font: var(--f) var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.85);
  }
  .mline b {
    color: var(--green);
    font-family: var(--ui, ui-monospace, monospace);
  }
  .mline.reply {
    color: rgba(255, 255, 255, 0.65);
  }
  .mapp {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .mapp .sp {
    flex: 1;
  }
  .mapp .tagp {
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(255, 153, 51, 0.14);
    color: var(--tag);
    font: 700 var(--f-badge) var(--ui, ui-monospace, monospace);
  }
  .mttl {
    font: 600 var(--f) var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.92);
  }
  .mdeny,
  .mallow {
    min-height: 26px;
    border: 0;
    border-radius: 6px;
    padding: 6px 11px;
    font: 600 var(--f-btn) var(--ui, system-ui, sans-serif);
  }
  .mdeny {
    background: none;
    color: var(--deny);
    box-shadow: inset 0 0 0 1px rgba(235, 97, 97, 0.7);
  }
  .mallow {
    color: #fff;
    background: var(--allow);
    padding: 6px 14px;
  }
  @media (prefers-reduced-motion: reduce) {
    .gauge i.cur {
      animation: none;
    }
  }
</style>
