// #197 — the decisions behind "hide on fullscreen, silence on lock".
//
// The shell owns the window and the probes; this module only answers what the
// frontend should ask it to do. Both functions are pure so the polling rules
// can be tested without a desktop, a clock, or Tauri.

export type AmbientState = {
  /** `fullscreen` command. Always false on Linux. */
  fullscreen: boolean;
  /** `screen_locked` command; null means "unknown", treated as unlocked. */
  locked: boolean;
  /** `quiet_hours` command; QUNS_QUIET_TIME on Windows, false elsewhere. */
  quiet: boolean;
};

export type AmbientSupport = {
  /** Capability "Fullscreen detection" is Supported (Windows only). */
  fullscreenDetection: boolean;
  /** Capability "Screen lock" is Supported. */
  screenLock: boolean;
};

export type AmbientView = {
  /** Hide the island window while a fullscreen app is in front. */
  hidden: boolean;
  /** Show the quiet-hours moon in the collapsed bar. */
  quiet: boolean;
  /** Locked: no sound, no animation, no content probes. */
  suspended: boolean;
};

/**
 * Every content probe (volume HUD reads, music deck reads and transcript
 * tails) stops while the screen is locked; only the lock watch stays. The
 * island window itself stays drawn — see `hidden` for the fullscreen case.
 */
export function probesActive(view: AmbientView): boolean {
  return !view.suspended;
}

export type PollPlan = {
  fullscreen: boolean;
  quietHours: boolean;
  screenLocked: boolean;
  /** False means stop the polling loop: nothing left to ask the shell. */
  active: boolean;
};

export function ambientView(state: AmbientState, support: AmbientSupport): AmbientView {
  return {
    hidden: support.fullscreenDetection && state.fullscreen,
    quiet: state.quiet,
    suspended: state.locked,
  };
}

export function pollPlan(view: AmbientView, support: AmbientSupport): PollPlan {
  // The lock watch is the one probe that must keep running while locked: it is
  // how the island learns it may wake again. Everything else — fullscreen,
  // quiet hours, the volume meter and transcript tails — is a content probe
  // and stops on lock. On a machine with no fullscreen detection the
  // Windows notification-state probes (fullscreen, quiet hours) are never
  // scheduled at all, and with no lock watch either the loop is not needed.
  const watchState = support.fullscreenDetection && !view.suspended;
  return {
    fullscreen: watchState,
    quietHours: watchState,
    screenLocked: support.screenLock,
    active: watchState || support.screenLock,
  };
}
