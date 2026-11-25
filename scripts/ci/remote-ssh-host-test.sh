#!/usr/bin/env bash
# Optional real-host check for issue #233. Not part of CI: an sshd host is
# expensive and belongs to a developer's machine or a throwaway container.
#
# Use an existing host:
#   MORROW_TEST_SSH_HOST=devbox MORROW_TEST_SSH_USER=test \
#     MORROW_TEST_SSH_KEY=~/.ssh/id_ed25519 scripts/ci/remote-ssh-host-test.sh
#
# Start a throwaway sshd container instead:
#   MORROW_TEST_SSH_CONTAINER=1 scripts/ci/remote-ssh-host-test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

CONTAINER=""
KEY_DIR=""
cleanup() {
    if [ -n "$CONTAINER" ]; then docker rm -f "$CONTAINER" >/dev/null 2>&1 || true; fi
    if [ -n "$KEY_DIR" ]; then rm -rf "$KEY_DIR"; fi
}
trap cleanup EXIT

if [ "${MORROW_TEST_SSH_CONTAINER:-0}" = "1" ]; then
    command -v docker >/dev/null || { echo "docker is required for the container mode" >&2; exit 1; }
    KEY_DIR="$(mktemp -d)"
    ssh-keygen -q -t ed25519 -N '' -f "$KEY_DIR/id_ed25519"
    CONTAINER="morrow-remote-233-$$"
    docker run -d --name "$CONTAINER" -p 127.0.0.1:2222:22 \
        -e USER_NAME=test -e PUBLIC_KEY="$(cat "$KEY_DIR/id_ed25519.pub")" \
        lscr.io/linuxserver/openssh-server:latest >/dev/null
    export MORROW_TEST_SSH_HOST=127.0.0.1
    export MORROW_TEST_SSH_PORT=2222
    export MORROW_TEST_SSH_USER=test
    export MORROW_TEST_SSH_KEY="$KEY_DIR/id_ed25519"
    for _ in $(seq 1 30); do
        if nc -z 127.0.0.1 2222 2>/dev/null; then break; fi
        sleep 1
    done
fi

: "${MORROW_TEST_SSH_HOST:?set MORROW_TEST_SSH_HOST or MORROW_TEST_SSH_CONTAINER=1}"

cargo test -p morrow-platform --test remote_ssh_host -- --ignored --nocapture
