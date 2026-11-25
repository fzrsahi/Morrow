<script lang="ts">
  // #465 — the Buddy (ESP32) settings slice: the Swift keys and a frame
  // preview of the ported wire protocol. Pairing and the BLE reader need
  // hardware and an OS permission, so they stay out of scope (#240).
  import { BUDDY_KEYS, type BuddySettings } from '../buddy-settings';

  type Props = {
    settings: BuddySettings;
    t: (key: string) => string;
    onChange: (key: string, value: string | number | boolean) => void;
    onPreview: (
      source: string,
      status: string,
      brightness: number,
      orientation: string,
    ) => Promise<string[]>;
  };
  let { settings, t, onChange, onPreview }: Props = $props();

  let status = $state('idle');
  let frames = $state<string[]>([]);
  let error = $state('');
  const sources = ['claude', 'codex', 'gemini', 'cursor', 'kimi', 'hermes'];

  async function preview() {
    error = '';
    try {
      frames = await onPreview(
        sources.includes(settings.selectedIdentifier)
          ? settings.selectedIdentifier
          : 'claude',
        status,
        settings.brightness,
        settings.orientation,
      );
    } catch (cause) {
      frames = [];
      error = String(cause);
    }
  }
</script>

<h2>Buddy / ESP32</h2>
<p class="note">
  The wire protocol is ported and previewable below. A paired screen needs the BLE reader and OS
  permission, which this shell does not have; those surfaces are out of scope (#240).
</p>
<div class="grp">
  <div>
    <span class="n">Bridge enabled</span>
    <span class="s">Send agent state to a paired Buddy screen</span>
    <button
      class="sw"
      class:off={!settings.bridgeEnabled}
      role="switch"
      aria-checked={settings.bridgeEnabled}
      aria-label="Bridge enabled"
      onclick={() => onChange(BUDDY_KEYS.bridgeEnabled, !settings.bridgeEnabled)}
    ></button>
  </div>
  <div>
    <span class="n">Heartbeat</span>
    <span class="s">
      <input
        type="number"
        min="1"
        max="60"
        step="1"
        value={settings.heartbeatSeconds}
        aria-label="Heartbeat seconds"
        onchange={(event) => onChange(BUDDY_KEYS.heartbeatSeconds, Number(event.currentTarget.value))}
      /> seconds
    </span>
  </div>
  <div>
    <span class="n">Brightness</span>
    <span class="s">
      <input
        type="range"
        min="10"
        max="100"
        step="1"
        value={settings.brightness}
        aria-label="Brightness"
        oninput={(event) => onChange(BUDDY_KEYS.brightness, Number(event.currentTarget.value))}
      />
      {settings.brightness}%
    </span>
  </div>
  <div>
    <span class="n">Orientation</span>
    <span class="s">
      <select
        value={settings.orientation}
        aria-label="Orientation"
        onchange={(event) => onChange(BUDDY_KEYS.orientation, event.currentTarget.value)}
      >
        <option value="up">Up</option>
        <option value="down">Down</option>
      </select>
    </span>
  </div>
  <div>
    <span class="n">Selected Buddy</span>
    <span class="s">
      <input
        value={settings.selectedName}
        placeholder="Not paired"
        aria-label="Selected Buddy name"
        onchange={(event) => onChange(BUDDY_KEYS.selectedName, event.currentTarget.value)}
      />
    </span>
  </div>
</div>
<h3>Frame preview</h3>
<div class="grp">
  <div>
    <span class="n">Source</span>
    <span class="s">
      <select value={settings.selectedIdentifier} aria-label="Preview source" onchange={(event) => onChange(BUDDY_KEYS.selectedIdentifier, event.currentTarget.value)}>
        {#each sources as source (source)}<option value={source}>{source}</option>{/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Status</span>
    <span class="s">
      <select bind:value={status} aria-label="Preview status">
        <option value="idle">Idle</option>
        <option value="processing">Processing</option>
        <option value="running">Running</option>
        <option value="waitingApproval">Waiting approval</option>
        <option value="waitingQuestion">Waiting question</option>
      </select>
    </span>
    <button class="b" onclick={preview}>Preview frames</button>
  </div>
  {#each frames as frame, index (index)}
    <div><span class="n">Frame {index + 1}</span><span class="s"><code>{frame}</code></span></div>
  {:else}
    <div><span class="s">Choose a source and press Preview frames.</span></div>
  {/each}
  {#if error}<div><span class="s err">{error}</span></div>{/if}
</div>

<style>
  .note {
    margin: 0 0 12px;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    line-height: 1.5;
  }
  h3 {
    margin: 14px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .grp input[type='number'] {
    width: 58px;
  }
  .grp :is(input:not([type='range']), select) {
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  .grp input[type='range'] {
    width: 130px;
    vertical-align: middle;
    margin-right: 6px;
  }
  code {
    font: 11px ui-monospace, monospace;
  }
  .err {
    color: #ff9c9c;
  }
</style>
