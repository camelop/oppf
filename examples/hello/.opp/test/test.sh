#!/usr/bin/env bash
# Verify the implementation at $OPP_PROJECT_ROOT satisfies the design.
set -euo pipefail

root="${OPP_PROJECT_ROOT:?OPP_PROJECT_ROOT must be set by 'opp test'}"

actual="$(bash "$root/hello.sh")"
expected="Hello, OPPF!"

if [[ "$actual" == "$expected" ]]; then
    echo "ok: hello.sh prints the expected greeting"
    exit 0
else
    echo "FAIL: expected '$expected', got '$actual'" >&2
    exit 1
fi
