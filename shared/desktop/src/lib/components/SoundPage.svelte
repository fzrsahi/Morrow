<script lang="ts">
  // #241/#465 — the Sound page: one play button per cue, plus the master
  // switch, volume, per-event switches, quiet hours and the away mute that the
  // shell's `attention::sound_preferences` and `SoundHold` now obey.
  import { cueLabel } from '../sound';
  import { SOUND_KEYS, SOUND_EVENTS, minutesToTime, timeToMinutes, type SoundControls } from '../sound-controls';
  import type { SoundCue } from '../types';

  type Props = {
    cues: SoundCue[];
    controls: SoundControls;
    supported: boolean;
    busy: boolean;
    t: (key: string) => string;
    onPreview: (cue: string) => void;
    onChange: (key: string, value: string | number | boolean) => void;
  };
  let { cues, controls, supported, busy, t, onPreview, onChange }: Props = $props();
</script>

<h2>{t('sound')}</h2>
<div class="grp">
  <div>
    <span class="n">Master</span>
    <span class="s">Turn every event sound on or off</span>
    <button
      class="sw"
      class:off={!controls.enabled}
      role="switch"
      aria-checked={controls.enabled}
      aria-label="Master sound"
      onclick={() => onChange(SOUND_KEYS.enabled, !controls.enabled)}
    ></button>
  </div>
  <div>
    <span class="n">{t('volume')}</span>
    <span class="s">
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={controls.volume}
        aria-label={t('volume')}
        oninput={(event) => onChange(SOUND_KEYS.volume, Number(event.currentTarget.value))}
      />
      {controls.volume}%
    </span>
  </div>
</div>
<div class="grp">
  {#each SOUND_EVENTS as event (event.field)}
    <div>
      <span class="n">{event.label}</span>
      <span class="s">Event sound</span>
      <button
        class="sw"
        class:off={!controls[event.field]}
        role="switch"
        aria-checked={controls[event.field]}
        aria-label={event.label}
        onclick={() => onChange(SOUND_KEYS[event.field], !controls[event.field])}
      ></button>
    </div>
  {/each}
</div>
<div class="grp">
  <div>
    <span class="n">{t('quiet_hours')}</span>
    <span class="s">{t('quiet_hours_desc')}</span>
    <button
      class="sw"
      class:off={!controls.quietHoursEnabled}
      role="switch"
      aria-checked={controls.quietHoursEnabled}
      aria-label={t('quiet_hours')}
      onclick={() => onChange(SOUND_KEYS.quietHoursEnabled, !controls.quietHoursEnabled)}
    ></button>
  </div>
  <div>
    <span class="n">{t('quiet_hours_start')}</span>
    <span class="s"><input
      type="time"
      value={minutesToTime(controls.quietHoursStart)}
      aria-label={t('quiet_hours_start')}
      onchange={(event) => {
        const minutes = timeToMinutes(event.currentTarget.value);
        if (minutes !== null) onChange(SOUND_KEYS.quietHoursStart, minutes);
      }}
    /></span>
  </div>
  <div>
    <span class="n">{t('quiet_hours_end')}</span>
    <span class="s"><input
      type="time"
      value={minutesToTime(controls.quietHoursEnd)}
      aria-label={t('quiet_hours_end')}
      onchange={(event) => {
        const minutes = timeToMinutes(event.currentTarget.value);
        if (minutes !== null) onChange(SOUND_KEYS.quietHoursEnd, minutes);
      }}
    /></span>
  </div>
  <div>
    <span class="n">{t('auto_mute_when_away')}</span>
    <span class="s">{t('auto_mute_when_away_desc')}</span>
    <button
      class="sw"
      class:off={!controls.autoMuteWhenAway}
      role="switch"
      aria-checked={controls.autoMuteWhenAway}
      aria-label={t('auto_mute_when_away')}
      onclick={() => onChange(SOUND_KEYS.autoMuteWhenAway, !controls.autoMuteWhenAway)}
    ></button>
  </div>
</div>
<h3>Sound previews</h3>
<div class="grp">
  {#each cues as cue (cue.name)}
    <div>
      <span class="n">{cueLabel(cue.name)}</span>
      <span class="s">{cue.file}</span>
      <button class="b" disabled={busy || !supported} onclick={() => onPreview(cue.name)}>
        Play
      </button>
    </div>
  {:else}
    <div><span class="s">Checking…</span></div>
  {/each}
</div>
{#if !supported}
  <p class="note">
    No sound player is available on this machine, so these cues stay silent. The Capability table
    says the same under <b>Sound effects</b>.
  </p>
{/if}

<style>
  .note {
    margin: 0;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    line-height: 1.5;
  }
  h3 {
    margin: 14px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  input[type='range'] {
    width: 140px;
    vertical-align: middle;
    margin-right: 6px;
  }
  input[type='time'] {
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 3px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
</style>
