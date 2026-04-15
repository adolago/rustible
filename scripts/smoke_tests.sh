#!/bin/bash
#
# Rustible smoke test runner
#
# Keeps the default CI baseline honest with a fast, explicit pass over the
# user-facing commands called out in release checklists.

set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: cargo is not installed or not on PATH" >&2
  exit 1
fi

echo "Running Rustible CLI smoke tests..."
cargo test --test cli_smoke_tests -- --test-threads=1
