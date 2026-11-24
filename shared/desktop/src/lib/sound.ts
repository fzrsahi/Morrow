// #241 — the Sound page's labels. The cue names come from the shell (they are
// `AttentionPolicy`'s names, shared with macOS), so this map only turns them
// into something a person reads; a cue the map has not met yet still renders,
// as its own name, rather than as a blank row.

const LABELS: Record<string, string> = {
  SessionStart: 'Session start',
  TaskRoundComplete: 'Round complete',
  Stop: 'Turn finished',
  StopFailure: 'Turn failed',
  PermissionRequest: 'Approval needed',
  UserPromptSubmit: 'Prompt submitted',
  AppLaunch: 'App launch',
};

export function cueLabel(name: string): string {
  return LABELS[name] ?? name;
}
