# Safety policy (normative)

Read this before running any command that changes the filesystem, git state, or a system service. The short list in `AGENTS.md` is the reminder; this file is the rule.

## Prohibited actions

Never execute destructive commands or attempt privilege escalation:

- Recursive forced deletion: `rm -rf /`, `rm -rf ~`, `rm -rf *`, `rm -rf $HOME`.
- Direct disk modification or formatting: `mkfs`, `dd of=/dev/...`, `diskutil eraseDisk`, `fdisk`.
- Privilege escalation: `sudo`, `su -`, `doas`.
- Destructive git actions rewriting remote history: `git push --force`, `git push -f`, `git reset --hard`.
- Writing directly to `.git/` internal objects or credentials files.

`git -c core.hooksPath=/dev/null commit` for a local WIP commit is allowed only when the repository hook fails for a reason outside the change. Document it in the PR. It is not a way to skip tests.

## Safe operations

- Building and testing: `swift build`, `swift test`, `cargo test`, `./scripts/ci/verify-local.sh`.
- Inspection and navigation: `git status`, `git diff`, `git log`, `ls`, `grep`, `find`.
- Reading and editing source in `macos/Sources/`, `macos/Tests/`, `shared/crates/`, `shared/desktop/src-tauri/src/`, and project configuration files, except the restricted zones in the ownership doc.
