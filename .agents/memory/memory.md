# Memory index

This folder replaces the old implementation plans and progress boards. Those documents described work already in code. A dated record of what and why is enough.

| File | Contents |
|---|---|
| [2026-11-16.md](2026-11-16.md) | Fork and rebrand (#47), port specification (#33, PR #49), monorepo restructure (#86, PR #171). |
| [2026-11-21.md](2026-11-21.md) | Rust workspace and Tauri shell (#87 to #90), Option E rejected (#85, PR #117), endpoint contract (#84, PR #115), packaging decisions (#70). |
| [2026-11-23.md](2026-11-23.md) | Discovery stack (#232 to #287), installer batches (#288 to #312), prayer, quota and push ports, capability truth (#235) and sound outlet (#241). |
| [2026-11-24.md](2026-11-24.md) | Host gates and build docs (#400, #405 to #408), main build repairs (#397, #398, #404), stack merge, docs cleanup, mascot animation gate (#410), Dynamic Island geometry spec (#417). |

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
