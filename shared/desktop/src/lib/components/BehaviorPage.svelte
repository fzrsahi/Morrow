<script lang="ts">
  // #201 — the follow-up reminder interval. Mirrors Swift's
  // `FollowUpReminderController.intervalChoices` ([0,1,2,3,5] minutes, 0=off)
  // and writes the same `followUpReminderMinutes` key the macOS app reads.
  import {
    DEFAULT_FOLLOW_UP_MINUTES,
    FOLLOW_UP_CHOICES_MINUTES,
    SETTINGS_KEY_FOLLOW_UP_MINUTES,
    followUpMinutesFromSettings,
  } from '../reminders';
  import { SETTINGS_KEYS, type GestureSettings } from '../settings';

  type Props = {
    minutes: number;
    gestures: GestureSettings;
    onChange: (key: string, value: number) => void;
    onGestureChange: (key: string, value: boolean) => void;
  };
  let { minutes, gestures, onChange, onGestureChange }: Props = $props();

  const choices = FOLLOW_UP_CHOICES_MINUTES;
  const label = (value: number) => (value === 0 ? 'Off' : `${value} min`);
  const current = $derived(followUpMinutesFromSettings(minutes));
</script>

<h2>Behavior</h2>
<div class="grp">
  <div>
    <span class="n">Follow-up reminder</span>
    <span class="s">
      <span class="seg2">
        {#each choices as value}
          <button
            class:on={current === value}
            aria-pressed={current === value}
            onclick={() => onChange(SETTINGS_KEY_FOLLOW_UP_MINUTES, value)}
          >
            {label(value)}
          </button>
        {/each}
      </span>
    </span>
  </div>
  <p class="hint">
    One notification per waiting approval or question after this interval. “Off” disables
    follow-ups; the current choice persists in the settings file.
  </p>
</div>
<div class="grp">
  <div>
    <span class="n">Scroll on the island for volume</span>
    <span class="s">
      <button
        class="sw"
        class:off={!gestures.scrollVolume}
        aria-label="Scroll on the island for volume"
        aria-pressed={gestures.scrollVolume}
        onclick={() => onGestureChange(SETTINGS_KEYS.scrollVolume, !gestures.scrollVolume)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Swipe sideways to switch tabs</span>
    <span class="s">
      <button
        class="sw"
        class:off={!gestures.swipeTabs}
        aria-label="Swipe sideways to switch tabs"
        aria-pressed={gestures.swipeTabs}
        onclick={() => onGestureChange(SETTINGS_KEYS.swipeTabs, !gestures.swipeTabs)}
      ></button>
    </span>
  </div>
  <p class="hint">
    Mouse wheel or trackpad over the collapsed island changes system volume. Swipe left or right
    over the island switches between Agents and Music; the tab switch opens the panel.
  </p>
</div>

<style>
  .hint {
    margin: 6px 0 0;
    font-size: 11px;
    opacity: 0.6;
  }
</style>
