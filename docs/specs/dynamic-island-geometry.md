# Morrow Dynamic Island Geometry & State Rules

This specification is normative when creating or modifying Dynamic Island UI in `macos/Sources/Morrow/`, `macos/Sources/MorrowCore/IslandDisplayMode.swift`, or cross-platform Dynamic Island shells (`shared/desktop/`).

## 1. Locked Vertical & Chrome Geometry (`!hasNotch`)

- **Uniform Collapsed Height:** All collapsed states (`Idle`, `Idle + Priority`, `Balanced Twin`, `Inline Lyrics`) use a fixed height of **`34 pt`**. Transitioning between collapsed states must cause `0 pt` vertical height jump.
- **Floating Y-Offset:** Default **`10 pt`** below the top screen edge (`DynamicIslandTopOffset`, clamped `0 – 40 pt`).
- **Outer Horizontal Padding:** **`13 pt`** when collapsed; **`14 pt`** when expanded.
- **Balanced Twin Center Divider:** **`1 × 14 pt`** (`white` at `18%` opacity) with **`10 pt`** gap on each side (`21 pt` total divider span).
- **Compact Asset Sizes:** Collapsed mascot **`20 pt`** (`19 pt` in Inline Lyrics); compact album artwork **`18 pt`** (`4.5 pt` corner radius).
- **Chrome Stroke:** **`1.2 pt`** (`white` at `24%` opacity) on standard pills; **`0 pt`** (transparent) on flush toast banners.

## 2. State Width & Height Budget Reference (`!hasNotch`, scale `100%`)

| State | Width (`W`) | Hard Cap | Height (`H`) | Shape & Radius | Layout |
|---|---|---|---|---|---|
| **1. Idle Resting** | `116 pt` *(scale: 96–136)* | `136 pt` | `34 pt` | `Capsule` | `[20pt Mascot] \| [IDLE]` |
| **2. Idle + Priority (Prayer / Mail)** | `140 – 176 pt` | `180 pt` | `34 pt` | `Capsule` | `[20pt Mascot] \| [Prayer/Mail]` |
| **3. Idle + Music (No Lyrics)** | `116 – 148 pt` | `160 pt` | `34 pt` | `Capsule` | `[20pt Mascot] \| [18pt Art + Spectrum]` |
| **4. Agent Active (No Music)** | `180 – 250 pt` | `260 pt` | `34 pt` | `Capsule` | `[Mascot + Thinking/Tool/Quota <= 110pt] \| [Standby <= 76pt]` |
| **5. Agent Active + Music (No Lyrics)** | `210 – 270 pt` | `280 pt` | `34 pt` | `Capsule` | `[Mascot + Thinking/Tool <= 110pt] \| [Art + Spectrum + 1 Badge <= 88pt]` |
| **6. Right-Wing Hover (Music Controls)** | `220 – 300 pt` | `320 pt` | `34 pt` | `Capsule` | `[Mascot] \| [Art + Track Marquee + Spectrum]` |
| **7. Inline Lyrics (Idle + Music + Lyrics)** | `180 – 320 pt` | `min(340 pt, screenWidth * 0.35)` | `34 pt` | `Capsule` | `[19pt Mascot] \| [Displayed Lyric] [18pt Art]` |
| **8. Toast — MagSafe Charging** | `360 pt` | `380 pt` | `48 – 56 pt` | `RoundedRectangle(18)` | Flush charging banner |
| **9. Toast — Prayer Mini Peek** | `460 pt` | `500 pt` | `140 – 156 pt` | `RoundedRectangle(18)` | Flush Sun-Arc banner |
| **10. Volume HUD Overlay** | `172 pt` | `188 pt` | `26 pt` | `Capsule` | Separate pill `4 pt` below main island |
| **11. Expanded HUD / Prompt Cards** | `680 – 720 pt` | `min(740 pt, screenWidth - 40 pt)` | Clamped to `screen.visibleFrame.height` | `RoundedRectangle(24)` | Unified continuous header (`H = 32 pt`, `14 pt` horizontal pad, no center cutout) |

## 3. State Priority & Anti-Ballooning Rules

- **Active Agent Priority over Inline Lyrics:** Any active or waiting agent session (`.processing`, `.running`, `.waitingApproval`, `.waitingQuestion`, active tool, or thinking state) **always overrides** Inline Lyrics back to Balanced Twin. Inline Lyrics renders only when all sessions are `.idle` or empty.
- **Inline Lyrics Width & Progress:** Width must be calculated from the **currently displayed lyric text only** (never a hidden track title). Do not place a bottom progress hairline inside the `34 pt` `Capsule` where curved corners pinch the track; use an optional subtle progress ring around the `18 pt` artwork instead.
- **No Notch Layout Fallback on `!hasNotch`:** Non-notch Dynamic Island states must never fall back to the 3-column hardware notch layout, including for Mail arrival (`IdleIndicatorBar`) and the `bilateralWings` lyric preset.
- **Hard Cap Enforcement:** No collapsed state may exceed its hard cap. Long titles, lyrics, tool names, or git branches must truncate or marquee inside their slot budget.
- **Single-Priority Status Badge:** Matches [notch-geometry.md](notch-geometry.md) (`pendingQuestion > followUpReminder > waitingAttention > completionGlance > quietHours`).

## 4. Shape Decision Rule (Single Source of Truth)

- `Capsule(style: .circular)`: collapsed single-row pills (`H <= 34 pt`) and Volume HUD only.
- `RoundedRectangle(cornerRadius: 18, style: .continuous)`: extended pills (`isToastOnly`, `isConnectedChinLyricActive`). Tall toast banners must never be clipped with `Capsule`.
- `RoundedRectangle(cornerRadius: 24, style: .continuous)`: all expanded surfaces (`shouldShowExpanded`: `sessionList`, `approvalCard`, `questionCard`, `completionCard`).

## 5. Has-Notch Dynamic Island (`hasNotch == true`)

- Inherits hardware notch cutout and strict left/right wing symmetry from [notch-geometry.md](notch-geometry.md) (`leftWidth == rightWidth`, `centerShift == 0`, center gap `>= notchW`).
- Uses Dynamic Island floating Y-offset (`10 pt`), chrome stroke (`1.2 pt`), and the Shape Decision Rule in Section 4 (e.g., Connected Chin Lyrics uses `notchHeight + 26 pt` with `RoundedRectangle(cornerRadius: 18, style: .continuous)`).

## 6. Calculate Before Modifying UI

- Before adding or adjusting Dynamic Island elements, calculate slot budgets against Section 2 and verify the resulting width stays within the state's hard cap.
- Reject PRs that exceed hard caps, reintroduce collapsed height jumps, clip tall toasts with `Capsule`, or allow Inline Lyrics to hide active agent states.
