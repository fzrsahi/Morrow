// #448 — the Remote card's pure view helpers. The shell owns the hosts, the
// tunnel and its status; this file only builds the blank form and maps a status
// onto the shared string keys macOS already uses.

import type { RemoteHost, RemoteStatus } from './types';

export const REMOTE_STATUS_EVENT = 'remote-status-changed';

export type RemoteStatuses = Record<string, RemoteStatus>;

export function blankHost(): RemoteHost {
  return {
    id: `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`,
    name: '',
    host: '',
    user: '',
    port: null,
    identity_file: '',
    auto_connect: false,
    auth_socket: '',
    cwd_filter: '',
    agents_run_as_other_users: false,
  };
}

/** The string key for the status chip. A failure keeps its message beside it. */
export function statusKey(status: RemoteStatus | undefined): string {
  switch (status?.state) {
    case 'connecting':
      return 'remote_connecting';
    case 'connected':
      return 'remote_connected';
    default:
      return 'remote_disconnected';
  }
}

/** The failure detail, when the engine reported one. */
export function statusMessage(status: RemoteStatus | undefined): string {
  return status?.state === 'failed' ? status.message : '';
}
