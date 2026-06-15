#!/usr/bin/env bash
# `opp test` runs this from the project root, so the implementation is right here.
set -euo pipefail

actual="$(bash hello.sh)"
expected="Hello, OPPF!"

if [[ "$actual" == "$expected" ]]; then
    echo "ok: hello.sh prints the expected greeting"
    exit 0
else
    echo "FAIL: expected '$expected', got '$actual'" >&2
    exit 1
fi
