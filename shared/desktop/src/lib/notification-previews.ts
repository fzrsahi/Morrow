// #465 — the 19 Dynamic Island preview buttons in SettingsView.swift:2123-2341
// (the issue's "14" counted an earlier revision). Ids and titles are shared
// with the shell's `notifications.rs` registry.

export type NotificationPreview = { id: string; title: string };

export const NOTIFICATION_PREVIEWS: readonly NotificationPreview[] = [
  { id: 'magsafe', title: 'Test MagSafe Charging' },
  { id: 'prayer-maghrib', title: 'Test Prayer (Maghrib)' },
  { id: 'prayer-subuh', title: 'Test Subuh' },
  { id: 'prayer-dzuhur', title: 'Test Dzuhur' },
  { id: 'prayer-ashar', title: 'Test Ashar' },
  { id: 'prayer-isya', title: 'Test Isya' },
  { id: 'usbc', title: 'Test USB-C Charging' },
  { id: 'on-hold', title: 'Test Charging On Hold' },
  { id: 'fully-charged', title: 'Test Fully Charged' },
  { id: 'low-battery', title: 'Test Low Battery' },
  { id: 'volume-hud', title: 'Test Volume HUD' },
  { id: 'track-expand', title: 'Test Track Auto-Expand' },
  { id: 'mail-arrival', title: 'Test Mail Arrival' },
  { id: 'prayer-wing-idle', title: 'Test Prayer Wing (Idle)' },
  { id: 'prayer-wing-thinking', title: 'Test Prayer Wing (Thinking)' },
  { id: 'inline-lyrics', title: 'Test Inline Lyrics' },
  { id: 'approval-modal', title: 'Test Approval Modal' },
  { id: 'question-modal', title: 'Test Question Modal' },
  { id: 'quick-prompt', title: 'Test Quick Prompt' },
];
