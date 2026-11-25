<script lang="ts">
  // #448 — the Remote settings card. Hosts, tunnel decisions and status all
  // come from the shell; this page edits a draft host and draws the status the
  // shell publishes over the `remote-status-changed` event.
  import { blankHost, statusKey, statusMessage, type RemoteStatuses } from '../remote';
  import type { RemoteHost } from '../types';

  type Props = {
    hosts: RemoteHost[];
    statuses: RemoteStatuses;
    busy: boolean;
    t: (key: string) => string;
    onSaveHost: (host: RemoteHost) => Promise<boolean>;
    onRemoveHost: (id: string) => void;
    onConnect: (id: string) => void;
    onDisconnect: (id: string) => void;
  };
  let {
    hosts,
    statuses,
    busy,
    t,
    onSaveHost,
    onRemoveHost,
    onConnect,
    onDisconnect,
  }: Props = $props();

  let draft = $state<RemoteHost>(blankHost());

  function text(key: keyof RemoteHost) {
    return (event: Event) => {
      const value = (event.currentTarget as HTMLInputElement).value;
      (draft as unknown as Record<string, unknown>)[key] = value;
    };
  }

  function flag(key: 'auto_connect' | 'agents_run_as_other_users') {
    return (event: Event) => {
      draft[key] = (event.currentTarget as HTMLInputElement).checked;
    };
  }

  function port(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    draft.port = Number.isFinite(value) && value > 0 ? Math.trunc(value) : null;
  }

  async function save() {
    if (!draft.name.trim() || !draft.host.trim()) return;
    // Clear only on success: a rejected host must keep the user's entry so
    // the error shown by SettingsApp can be acted on.
    if (await onSaveHost({ ...draft })) draft = blankHost();
  }

  function edit(host: RemoteHost) {
    draft = { ...host };
  }

  function target(host: RemoteHost): string {
    const user = host.user.trim();
    const port = host.port === null ? '' : `:${host.port}`;
    return `${user ? `${user}@` : ''}${host.host}${port}`;
  }
</script>

<h2>{t('remote_hosts')}</h2>

<div class="grp">
  {#if hosts.length === 0}
    <div><span class="s">{t('remote_hosts_empty')}</span></div>
  {:else}
    {#each hosts as host (host.id)}
      <div>
        <span class="n">{host.name || host.host}</span>
        <span class="s">
          {target(host)} · {t(statusKey(statuses[host.id]))}
          {#if statusMessage(statuses[host.id])}
            · {statusMessage(statuses[host.id])}
          {/if}
        </span>
        {#if statuses[host.id]?.state === 'connected'}
          <button class="b" disabled={busy} onclick={() => onDisconnect(host.id)}>
            {t('remote_disconnect')}
          </button>
        {:else}
          <button
            class="b"
            disabled={busy || statuses[host.id]?.state === 'connecting'}
            onclick={() => onConnect(host.id)}
          >
            {t('remote_connect')}
          </button>
        {/if}
        <button class="b" disabled={busy} onclick={() => edit(host)}>{t('remote_name')}</button>
        <button class="b" disabled={busy} onclick={() => onRemoveHost(host.id)}>
          {t('remote_remove')}
        </button>
      </div>
    {/each}
  {/if}
</div>

<div class="grp">
  <div>
    <span class="n">{t('remote_name')}</span>
    <span class="s">
      <input value={draft.name} oninput={text('name')} aria-label={t('remote_name')} />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_host')}</span>
    <span class="s">
      <input value={draft.host} oninput={text('host')} aria-label={t('remote_host')} />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_user')}</span>
    <span class="s">
      <input value={draft.user} oninput={text('user')} aria-label={t('remote_user')} />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_port')}</span>
    <span class="s">
      <input
        type="number"
        min="1"
        max="65535"
        value={draft.port ?? ''}
        oninput={port}
        aria-label={t('remote_port')}
      />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_identity')}</span>
    <span class="s">
      <input
        value={draft.identity_file}
        oninput={text('identity_file')}
        aria-label={t('remote_identity')}
      />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_auth_socket')}</span>
    <span class="s">
      <input
        value={draft.auth_socket}
        placeholder={t('remote_auth_socket_placeholder')}
        oninput={text('auth_socket')}
        aria-label={t('remote_auth_socket')}
      />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_cwd_filter')}</span>
    <span class="s">
      <input
        value={draft.cwd_filter}
        placeholder={t('remote_cwd_filter_placeholder')}
        oninput={text('cwd_filter')}
        aria-label={t('remote_cwd_filter')}
      />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_auto_connect')}</span>
    <span class="s">
      <input
        type="checkbox"
        checked={draft.auto_connect}
        onchange={flag('auto_connect')}
        aria-label={t('remote_auto_connect')}
      />
    </span>
  </div>
  <div>
    <span class="n">{t('remote_other_users')}</span>
    <span class="s">
      <input
        type="checkbox"
        checked={draft.agents_run_as_other_users}
        onchange={flag('agents_run_as_other_users')}
        aria-label={t('remote_other_users')}
      />
    </span>
  </div>
  <div>
    <span class="n"></span>
    <span class="s">
      <button class="b" disabled={busy || !draft.name.trim() || !draft.host.trim()} onclick={save}>
        {t('remote_add_button')}
      </button>
    </span>
  </div>
</div>

<p class="hint">{t('remote_hint')}</p>

<style>
  input:not([type='checkbox']) {
    max-width: 100%;
    width: 260px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  .hint {
    color: rgba(255, 255, 255, 0.5);
    font-size: 11.5px;
    line-height: 1.5;
    margin: 0;
  }
</style>
