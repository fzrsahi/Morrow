export interface FeatureShot {
  id: string;
  label: string;
  title: string;
  summary: string;
  detail: string;
  pixel: { src: string; width: number; height: number; alt: string };
  modern: { src: string; width: number; height: number; alt: string };
}

export const FEATURE_SHOTS: FeatureShot[] = [
  {
    id: 'agents',
    label: 'Agents',
    title: 'Every coding session in one glance',
    summary:
      'Track Claude Code, Codex, Gemini CLI, Cursor, and 30+ hook-enabled tools live in the notch—status, active tool call, branch, and token usage without switching windows.',
    detail: 'Live session list · subagent counts · 5h & daily Claude usage',
    pixel: {
      src: 'shots/app/agents-pixel.webp',
      width: 1960,
      height: 874,
      alt: 'Morrow Pixel theme showing four live coding agent sessions (Cursor, Claude, Codex, Gemini) with 8-bit badges and token usage footer',
    },
    modern: {
      src: 'shots/app/agents-modern.webp',
      width: 1960,
      height: 1008,
      alt: 'Morrow Modern theme showing four live coding agent sessions (Cursor, Claude, Codex, Gemini) with smooth pill badges and token usage footer',
    },
  },
  {
    id: 'approval',
    label: 'Approvals',
    title: 'Approve or deny tool calls inline',
    summary:
      'Permission prompts land right inside the notch. Review the exact command, allow once, always allow for the session, or deny—while the tool keeps its real exit result.',
    detail: 'PermissionRequest hook · global shortcuts · zero focus theft',
    pixel: {
      src: 'shots/app/approval-pixel.webp',
      width: 1880,
      height: 482,
      alt: 'Morrow Pixel theme inline permission card asking to run npm run test with Allow once and Deny buttons',
    },
    modern: {
      src: 'shots/app/approval-modern.webp',
      width: 1880,
      height: 484,
      alt: 'Morrow Modern theme inline permission card asking to run npm run test with rounded Allow once and Deny buttons',
    },
  },
  {
    id: 'question',
    label: 'Questions',
    title: 'Answer agent prompts without breaking flow',
    summary:
      'Multi-choice agent questions surface directly beneath the camera cutout so you can pick an architecture choice and send the agent straight back to work.',
    detail: 'AskUserQuestion card · keyboard-navigable options · skip or dismiss',
    pixel: {
      src: 'shots/app/question-pixel.webp',
      width: 1880,
      height: 720,
      alt: 'Morrow Pixel theme question card asking which state library to use with numbered options',
    },
    modern: {
      src: 'shots/app/question-modern.webp',
      width: 1880,
      height: 720,
      alt: 'Morrow Modern theme question card asking which state library to use with clean rounded option rows',
    },
  },
  {
    id: 'workspace',
    label: 'Workspace',
    title: 'Interactive Git spine and live diff',
    summary:
      'Inspect recent commits, ahead/behind counts, modified files, and AI-assisted commit prompts from an async Git engine that never stalls the main UI thread.',
    detail: 'Commit spine · staged & modified status · quick commit & push',
    pixel: {
      src: 'shots/app/workspace-pixel.webp',
      width: 1880,
      height: 434,
      alt: 'Morrow Pixel theme workspace tab showing branch feat/notch-lyrics, commit hashes, and commit input',
    },
    modern: {
      src: 'shots/app/workspace-modern.webp',
      width: 1880,
      height: 654,
      alt: 'Morrow Modern theme workspace tab showing connected git commit spine, author timestamps, and commit input',
    },
  },
  {
    id: 'music',
    label: 'Music HUD',
    title: 'Album art, spectrum visualizer, and synced lyrics',
    summary:
      'Control Spotify or Apple Music from the third notch tab with live progress, beat equalizer bars, and line-by-line synced lyrics.',
    detail: 'Spotify & Apple Music · browser media detection · live visualizer',
    pixel: {
      src: 'shots/app/music-pixel.webp',
      width: 1880,
      height: 448,
      alt: 'Morrow Pixel theme Music HUD showing Midnight City by M83, segmented progress bar, and glowing cyan synced lyrics',
    },
    modern: {
      src: 'shots/app/music-modern.webp',
      width: 1880,
      height: 448,
      alt: 'Morrow Modern theme Music HUD showing Midnight City by M83, circular play control, and soft glowing synced lyrics',
    },
  },
  {
    id: 'lyrics',
    label: 'Floating Lyrics',
    title: 'Sub-notch capsule that floats beneath the camera',
    summary:
      'Keep synced lyrics hovering 1mm below the physical notch while your workspace stays uncluttered—with 13 presets from 8-Bit Retro to Frosted Glass and Braun Studio.',
    detail: '13 layout presets · spring width fit · zero marquee jitter',
    pixel: {
      src: 'shots/app/lyrics-pixel.webp',
      width: 1520,
      height: 268,
      alt: 'Morrow Pixel theme sub-notch floating lyric bar in 8-bit frame below the MacBook notch',
    },
    modern: {
      src: 'shots/app/lyrics-modern.webp',
      width: 1520,
      height: 262,
      alt: 'Morrow Modern theme sub-notch frosted glass floating lyric pill below the MacBook notch',
    },
  },
  {
    id: 'hoops',
    label: 'Notch Hoops',
    title: 'Built-in arcade break right under the notch',
    summary:
      'Waiting on a long test suite? Flick shots into the rim across 1PT, 2PT, and 3PT positions without opening a browser tab.',
    detail: 'Physics arc · high-score tracking · 1PT / 2PT / 3PT / Random',
    pixel: {
      src: 'shots/app/hoops-pixel.webp',
      width: 1880,
      height: 716,
      alt: 'Morrow Pixel theme Notch Hoops basketball mini-game inside the notch panel',
    },
    modern: {
      src: 'shots/app/hoops-modern.webp',
      width: 1880,
      height: 716,
      alt: 'Morrow Modern theme Notch Hoops basketball mini-game inside the notch panel',
    },
  },
  {
    id: 'prayer',
    label: 'Prayer Times',
    title: 'Quiet daily prayer timeline at a glance',
    summary:
      'Localized calculation methods (Kemenag, MWL, ISNA, Umm al-Qura) with Hijri date, countdown to the next prayer, and sky scene matching day or night.',
    detail: 'City & auto location · day/night sky · compact wing countdown',
    pixel: {
      src: 'shots/app/prayer-pixel.webp',
      width: 1880,
      height: 460,
      alt: 'Morrow Pixel theme Prayer tab showing night pixel mosque silhouette, Hijri date, and five daily prayer times',
    },
    modern: {
      src: 'shots/app/prayer-modern.webp',
      width: 1880,
      height: 460,
      alt: 'Morrow Modern theme Prayer tab showing night sky gradient, Hijri date, and smooth five-prayer timeline',
    },
  },
];

export interface MascotItem {
  name: string;
  file: string;
}

export const MASCOTS: MascotItem[] = [
  { name: 'Claude Code', file: 'shots/mascots/claude.gif' },
  { name: 'Codex', file: 'shots/mascots/codex.gif' },
  { name: 'Gemini CLI', file: 'shots/mascots/gemini.gif' },
  { name: 'Cursor', file: 'shots/mascots/cursor.gif' },
  { name: 'OpenCode', file: 'shots/mascots/opencode.gif' },
  { name: 'Trae', file: 'shots/mascots/trae.gif' },
  { name: 'Qoder', file: 'shots/mascots/qoder.gif' },
  { name: 'Kimi CLI', file: 'shots/mascots/kimi.gif' },
  { name: 'Cline', file: 'shots/mascots/cline.gif' },
  { name: 'Kiro CLI', file: 'shots/mascots/kiro.gif' },
];

export const MORE_CLIS: string[] = [
  'Google Antigravity',
  'Copilot CLI',
  'Grok CLI',
  'Factory Droid',
  'CodeBuddy',
  'Qwen Code',
  'Pi / Oh My Pi',
  'ZCode',
  'Hermes',
  'DeepSeek Harness',
  'OpenClaw',
  'Custom Hooks',
];

export interface PlatformRow {
  os: string;
  stack: string;
  status: string;
  tone: 'ready' | 'experimental';
  notes: string;
  actionLabel: string;
  actionHref: string;
}

export const PLATFORMS: PlatformRow[] = [
  {
    os: 'macOS 14+',
    stack: 'SwiftUI + Rust core (Morrow.app)',
    status: 'Primary · Ready',
    tone: 'ready',
    notes: 'Full notch & Dynamic Island modes, Pixel & Modern skins, Sparkle auto-updates, Apple Music & Spotify sync, iPhone/Watch companion.',
    actionLabel: 'Install with Homebrew',
    actionHref: '#install',
  },
  {
    os: 'Linux (X11)',
    stack: 'Tauri v2 + Svelte 5 + Rust',
    status: 'Experimental · Source',
    tone: 'experimental',
    notes: 'Shared hook contract using xdotool, xprop, and playerctl. Port parity is active in repo; host verification gates are still open.',
    actionLabel: 'Build guide',
    actionHref: 'https://github.com/fzrsahi/Morrow/blob/main/docs/build-windows-linux.md',
  },
  {
    os: 'Windows',
    stack: 'Tauri v2 + WinRT + Rust',
    status: 'Experimental · Source',
    tone: 'experimental',
    notes: 'Shared hook contract with WinRT media and named pipe IPC. Remote ssh -R pipe forwarding (#451) and host gates remain in progress.',
    actionLabel: 'Build guide',
    actionHref: 'https://github.com/fzrsahi/Morrow/blob/main/docs/build-windows-linux.md',
  },
];

export const SPLIT_PRESETS = ['music', 'lyrics', 'approval', 'question', 'hoops', 'prayer'] as const;
