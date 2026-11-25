<script lang="ts">
  // #201 — the follow-up reminder interval. Mirrors Swift's
  // `FollowUpReminderController.intervalChoices` ([0,1,2,3,5] minutes, 0=off)
  // and writes the same `followUpReminderMinutes` key the macOS app reads.
  // #465 — the rest of the page writes the same Swift `SettingsKey` spellings.
  import {
    DEFAULT_FOLLOW_UP_MINUTES,
    FOLLOW_UP_CHOICES_MINUTES,
    SETTINGS_KEY_FOLLOW_UP_MINUTES,
    followUpMinutesFromSettings,
  } from '../reminders';
  import { BEHAVIOR_KEYS, type BehaviorSettings } from '../behavior';
  import { SETTINGS_KEYS, type GestureSettings } from '../settings';

  type Props = {
    minutes: number;
    t: (key: string) => string;
    gestures: GestureSettings;
    behavior: BehaviorSettings;
    onChange: (key: string, value: number) => void;
    onGestureChange: (key: string, value: boolean) => void;
    onBehaviorChange: (key: keyof typeof BEHAVIOR_KEYS, value: string | number | boolean) => void;
  };
  let { minutes, t, gestures, behavior, onChange, onGestureChange, onBehaviorChange }: Props =
    $props();

  const choices = FOLLOW_UP_CHOICES_MINUTES;
  const label = (value: number) =>
    value === 0
      ? t('follow_up_off')
      : // The shared copy carries a placeholder; fill it here so the interval
        // keeps the chosen language rather than the English word "min".
        t('follow_up_after_minutes').replace('%d', String(value));
  const current = $derived(followUpMinutesFromSettings(minutes));

  const delays = [0.1, 0.2, 0.3, 0.5, 0.8, 1] as const;
  const completionStyles = [
    { value: 'expand', label: 'Expand' },
    { value: 'glance', label: 'Glance' },
    { value: 'off', label: 'Off' },
  ] as const;
  const sessionModes = [
    { value: 'separate', label: 'Separate' },
    { value: 'merge', label: 'Merge' },
    { value: 'hide', label: 'Hide' },
  ] as const;
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
    over the island switches between the open tabs; the tab switch opens the panel.
  </p>
</div>

<h3>Island visibility</h3>
<div class="grp">
  <div>
    <span class="n">Hide in fullscreen</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.hideInFullscreen}
        aria-label="Hide in fullscreen"
        aria-pressed={behavior.hideInFullscreen}
        onclick={() => onBehaviorChange('hideInFullscreen', !behavior.hideInFullscreen)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Hide when no session</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.hideWhenNoSession}
        aria-label="Hide when no session"
        aria-pressed={behavior.hideWhenNoSession}
        onclick={() => onBehaviorChange('hideWhenNoSession', !behavior.hideWhenNoSession)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Smart suppress</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.smartSuppress}
        aria-label="Smart suppress"
        aria-pressed={behavior.smartSuppress}
        onclick={() => onBehaviorChange('smartSuppress', !behavior.smartSuppress)}
      ></button>
    </span>
  </div>
  <p class="hint">
    Fullscreen hiding needs a reader the desktop only has on Windows. Smart suppress is stored for
    macOS compatibility; the desktop has no frontmost-terminal detection, so it never suppresses a
    card on its own.
  </p>
</div>

<h3>Panel timing</h3>
<div class="grp">
  <div>
    <span class="n">Hover expand delay</span>
    <span class="s">
      <span class="seg2">
        {#each delays as delay}
          <button
            class:on={behavior.hoverExpandDelay === delay}
            aria-pressed={behavior.hoverExpandDelay === delay}
            onclick={() => onBehaviorChange('hoverExpandDelay', delay)}>{delay}s</button
          >
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">Collapse on mouse leave</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.collapseOnMouseLeave}
        aria-label="Collapse on mouse leave"
        aria-pressed={behavior.collapseOnMouseLeave}
        onclick={() => onBehaviorChange('collapseOnMouseLeave', !behavior.collapseOnMouseLeave)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Collapse after session jump</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.autoCollapseAfterSessionJump}
        aria-label="Collapse after session jump"
        aria-pressed={behavior.autoCollapseAfterSessionJump}
        onclick={() =>
          onBehaviorChange('autoCollapseAfterSessionJump', !behavior.autoCollapseAfterSessionJump)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Completion notification</span>
    <span class="s">
      <span class="seg2">
        {#each completionStyles as style}
          <button
            class:on={behavior.completionNotificationStyle === style.value}
            aria-pressed={behavior.completionNotificationStyle === style.value}
            onclick={() => onBehaviorChange('completionNotificationStyle', style.value)}
            >{style.label}</button
          >
        {/each}
      </span>
    </span>
  </div>
</div>

<h3>Approvals and sub-sessions</h3>
<div class="grp">
  <div>
    <span class="n">Expand on permission</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.autoExpandOnPermission}
        aria-label="Expand on permission"
        aria-pressed={behavior.autoExpandOnPermission}
        onclick={() => onBehaviorChange('autoExpandOnPermission', !behavior.autoExpandOnPermission)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Expand on question</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.autoExpandOnQuestion}
        aria-label="Expand on question"
        aria-pressed={behavior.autoExpandOnQuestion}
        onclick={() => onBehaviorChange('autoExpandOnQuestion', !behavior.autoExpandOnQuestion)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Sub-session mode</span>
    <span class="s">
      <span class="seg2">
        {#each sessionModes as mode}
          <button
            class:on={behavior.pluginSessionMode === mode.value}
            aria-pressed={behavior.pluginSessionMode === mode.value}
            onclick={() => onBehaviorChange('pluginSessionMode', mode.value)}>{mode.label}</button
          >
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">Auto-approve tools</span>
    <span class="s">
      <input
        class="field"
        aria-label="Auto-approve tools"
        placeholder="Read, Edit"
        value={behavior.autoApproveTools}
        onchange={(event) => onBehaviorChange('autoApproveTools', event.currentTarget.value)}
      />
    </span>
  </div>
  <div>
    <span class="n">Auto-approve sources</span>
    <span class="s">
      <input
        class="field"
        aria-label="Auto-approve sources"
        placeholder="claude, codex"
        value={behavior.autoApproveSources}
        onchange={(event) => onBehaviorChange('autoApproveSources', event.currentTarget.value)}
      />
    </span>
  </div>
  <p class="hint">
    The desktop keeps approvals in the panel; these two lists are written for macOS compatibility
    and do not bypass a permission card here. Sub-session mode is likewise a macOS routing key: the
    desktop feed lists every session it finds.
  </p>
</div>

<h3>Session lifecycle</h3>
<div class="grp">
  <div>
    <span class="n">Session timeout (minutes)</span>
    <span class="s">
      <input
        class="field num"
        type="number"
        min="1"
        max="1440"
        aria-label="Session timeout in minutes"
        value={behavior.sessionTimeout}
        onchange={(event) => onBehaviorChange('sessionTimeout', Number(event.currentTarget.value))}
      />
    </span>
  </div>
  <div>
    <span class="n">Rotation interval (seconds)</span>
    <span class="s">
      <input
        class="field num"
        type="number"
        min="1"
        max="3600"
        aria-label="Rotation interval in seconds"
        value={behavior.rotationInterval}
        onchange={(event) => onBehaviorChange('rotationInterval', Number(event.currentTarget.value))}
      />
    </span>
  </div>
  <div>
    <span class="n">Maximum tool history</span>
    <span class="s">
      <input
        class="field num"
        type="number"
        min="1"
        max="1000"
        aria-label="Maximum tool history"
        value={behavior.maxToolHistory}
        onchange={(event) => onBehaviorChange('maxToolHistory', Number(event.currentTarget.value))}
      />
    </span>
  </div>
  <p class="hint">
    Stored with the same keys Swift reads. The desktop pruner uses process identity rather than the
    timeout, and the card history is owned by the feed, so these three are compatibility values
    until the shell consumes them.
  </p>
</div>

<h3>Config directories and hook filter</h3>
<div class="grp">
  <div>
    <span class="n">Claude config directory</span>
    <span class="s">
      <input
        class="field"
        aria-label="Claude config directory"
        placeholder="Auto-detect"
        value={behavior.claudeConfigDir}
        onchange={(event) => onBehaviorChange('claudeConfigDir', event.currentTarget.value)}
      />
    </span>
  </div>
  <div>
    <span class="n">Extra config directories</span>
    <span class="s">
      <input
        class="field"
        aria-label="Extra config directories"
        placeholder="JSON list"
        value={behavior.extraConfigDirs}
        onchange={(event) => onBehaviorChange('extraConfigDirs', event.currentTarget.value)}
      />
    </span>
  </div>
  <div>
    <span class="n">Excluded cwd substrings</span>
    <span class="s">
      <input
        class="field"
        aria-label="Excluded cwd substrings"
        placeholder="/tmp, node_modules"
        value={behavior.excludedHookCwdSubstrings}
        onchange={(event) =>
          onBehaviorChange('excludedHookCwdSubstrings', event.currentTarget.value)}
      />
    </span>
  </div>
  <p class="hint">
    Hooks already come from the directories discovery found. These keys are written for macOS
    compatibility; the desktop does not yet read them when it scans config roots or accepts hooks.
  </p>
</div>

<h3>Webhook forwarding</h3>
<div class="grp">
  <div>
    <span class="n">Enable webhook</span>
    <span class="s">
      <button
        class="sw"
        class:off={!behavior.webhookEnabled}
        aria-label="Enable webhook"
        aria-pressed={behavior.webhookEnabled}
        onclick={() => onBehaviorChange('webhookEnabled', !behavior.webhookEnabled)}
      ></button>
    </span>
  </div>
  <div>
    <span class="n">Webhook URL</span>
    <span class="s">
      <input
        class="field"
        aria-label="Webhook URL"
        placeholder="http://localhost:9000/hook"
        value={behavior.webhookURL}
        onchange={(event) => onBehaviorChange('webhookURL', event.currentTarget.value)}
      />
    </span>
  </div>
  <div>
    <span class="n">Event filter</span>
    <span class="s">
      <input
        class="field"
        aria-label="Webhook event filter"
        placeholder="All events"
        value={behavior.webhookEventFilter}
        onchange={(event) => onBehaviorChange('webhookEventFilter', event.currentTarget.value)}
      />
    </span>
  </div>
  <p class="hint">
    These are the macOS forwarding keys. The desktop shell sends hook events over its local socket
    only; it does not POST to an external URL, so the switches are stored without starting a
    network sender.
  </p>
</div>

<style>
  h3 {
    margin: 18px 0 8px;
    font-size: 12px;
    opacity: 0.7;
  }
  .hint {
    margin: 6px 0 0;
    font-size: 11px;
    opacity: 0.6;
  }
  .field {
    width: 100%;
    max-width: 320px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  .field.num {
    max-width: 90px;
  }
</style>
