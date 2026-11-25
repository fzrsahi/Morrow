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
    t: (key: string) => string;
    gestures: GestureSettings;
    onChange: (key: string, value: number) => void;
    onGestureChange: (key: string, value: boolean) => void;
  };
  let { minutes, t, gestures, onChange, onGestureChange }: Props = $props();

  const choices = FOLLOW_UP_CHOICES_MINUTES;
  const label = (value: number) =>
    value === 0
      ? t('follow_up_off')
      : // The shared copy carries a placeholder; fill it here so the interval
        // keeps the chosen language rather than the English word "min".
        t('follow_up_after_minutes').replace('%d', String(value));
  const current = $derived(followUpMinutesFromSettings(minutes));
</script>

<h2>{t('behavior')}</h2>
<div class="grp">
  <div>
    <span class="n">{t('follow_up_reminders')}</span>
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
  <!-- The macOS key describes quiet hours and a retry count this shell does
       not implement, so the desktop copy stays here until there is a key that
       says what this page actually does. -->
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
