# Review policy: substance over paperwork (normative)

A review exists to catch code that is wrong, not to grade the PR description.

## Blocking findings

Only these justify `CHANGES_REQUESTED`:

- behaviour that contradicts the issue's acceptance criteria;
- an actual or likely bug: wrong branch, missing guard, stale state, data loss, race, wrong path or offset, leaked resource;
- a security or privacy regression: secrets in argv or logs, unredacted payloads, permission bypass;
- a regression for existing callers, persisted data, or OS-specific behaviour;
- a PR-body claim the code does not support, such as "closes AC 3" with no producer.

A blocking finding must be writable as: "with input X the code does Y instead of Z."

## Non-blocking notes

Approve with a note when the code is correct and the remaining problem is:

- missing `verify-local.sh` output, Before/After screenshots, benchmark tables, or test logs;
- branch-name prefix, commit type or title formatting, stale `Refs`, wording in the body;
- size above the 250-line ideal, speculative smells, style preferences.

Record each note once, briefly. Do not withhold approval for paperwork.

## Actionable comments

Every comment on an issue or PR states:

- the exact location, as `file:line` or commit SHA;
- the wrong behaviour and its impact, with who is affected and when;
- the concrete fix, or the decision needed from a maintainer;
- whether it is a HARD blocker or a note.

Re-read the latest commit before re-reviewing. Do not paste a finding the current head already fixes; acknowledge what was resolved. Do not repeat the same note across sibling PRs. Put it in one issue or PR and link it from the others.
