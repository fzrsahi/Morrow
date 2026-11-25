# Memory index

This folder replaces the old implementation plans and progress boards. Those documents described work already in code. A dated record of what and why is enough.

| File | Contents |
|---|---|
| [2026-11-16.md](2026-11-16.md) | Fork and rebrand (#47), port specification (#33, PR #49), monorepo restructure (#86, PR #171). |
| [2026-11-21.md](2026-11-21.md) | Rust workspace and Tauri shell (#87 to #90), Option E rejected (#85, PR #117), endpoint contract (#84, PR #115), packaging decisions (#70). |
| [2026-11-23.md](2026-11-23.md) | Discovery stack (#232 to #287), installer batches (#288 to #312), prayer, quota and push ports, capability truth (#235) and sound outlet (#241). |
| [2026-11-25.md](2026-11-25.md) | Dead `pointer_position` cleanup (#399), remote installer core (#233), multi-display localization keys restored (#454), desktop island wheel volume and swipe tabs (#462). |
| [2026-11-24.md](2026-11-24.md) | Agent docs and gates, port status map, X11 readers, #399 ports, blocking bridge answer, and desktop Remote shell wiring (#448). |
| [2026-11-24.md](2026-11-24.md) | Host gates and build docs (#400, #405 to #408), main build repairs (#397, #398, #404), stack merge, docs cleanup, mascot animation gate (#410), Dynamic Island geometry spec (#417), Notch fullscreen concave ears (#424), Dynamic Island geometry fixes (#421), Swift power-banner parity (#399). |
| [2026-11-25.md](2026-11-25.md) | Dead `pointer_position` cleanup (#399), remote installer core (#233), Dynamic Island geometry, clipping, and state reactivity (#444), Dynamic Island Pixel/Modern visual polish and notification previews (#445), and CI lane router (#369). |
| [2026-11-25.md](2026-11-25.md) | Dead `pointer_position` cleanup (#399), remote installer core (#233), Dynamic Island geometry, clipping, and state reactivity (#444), Dynamic Island Pixel/Modern visual polish and notification previews (#445), and the compact charging-pill dismiss control (#45). |
| [2026-11-25.md](2026-11-25.md) | Dead `pointer_position` cleanup (#399), remote installer core (#233), Dynamic Island geometry, clipping, and state reactivity (#444), and Dynamic Island Pixel/Modern visual polish and notification previews (#445). |
| [2026-11-25.md](2026-11-25.md) | Desktop Settings page localization (#463, Refs #462): translator threading and one new shared string for the Unavailable page. |
| [2026-11-25.md](2026-11-25.md) | Dead `pointer_position` cleanup (#399), remote installer core (#233), Dynamic Island geometry, clipping, and state reactivity (#444), Dynamic Island Pixel/Modern visual polish and notification previews (#445), and editable desktop shortcut bindings (#462). |
| [2026-11-25.md](2026-11-25.md) | RAM and idle-wakeup audit: remote pump and peer-check intervals, bounded desktop lyric cache, and the untouched SwiftUI and poll-loop candidates. |

> Format: `date | what landed | why it matters`. Update this index when you create or modify a memory file. Older entries keep their original text; if a decision changed, append `Superseded on YYYY-MM-DD` to that entry instead of rewriting it.

## Entry format

```markdown
## <short topic> (#<issue or PR>)

- Landed: <what changed, one sentence>
- Where: <paths or modules>
- Verified: <tests, CI, host gate, or "not yet">
- Left open: <follow-up issue or gate>
- Why: <the constraint or failure mode that drove the change>
```

Write entries in English. Name the actor: "PR #123 added" or "we rejected", not "it was added". Avoid em dashes and marketing language.

## When to write an entry

Write one whenever you implement code, not only when a PR merges. Read this index before starting work that touches an area listed here. Historical decisions that still bind the project belong in `docs/adr/`. Current build commands and limits belong in `docs/build-windows-linux.md`. This folder records what happened and why.
