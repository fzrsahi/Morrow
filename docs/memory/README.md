# Memory

This folder replaces the old implementation plans and progress boards. Those documents described work that is now in the code. Keeping a dated record is enough.

## How to use it

- One file per calendar date, named `YYYY-MM-DD.md`.
- Each file records what landed that day, with the issue or PR number, the files or modules involved, and any gate that is still open.
- Add new entries to the file for today. Do not rewrite older files unless a fact was wrong.
- Historical decisions that are still binding belong in `docs/adr/`. Current limits and build commands belong in `docs/build-windows-linux.md`. This folder records what happened.

## Entry format

```markdown
## <short topic> (#<issue or PR>)

- What landed: <one sentence>
- Where: <paths or modules>
- Verified: <tests, CI, host gates, or "not yet">
- Left open: <follow-up issue or gate>
```

Write entries in English. Name the actor: "we added" or "PR #123 added", not "it was added". Avoid em dashes and marketing language.
