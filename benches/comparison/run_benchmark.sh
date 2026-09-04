#!/bin/bash
# Compatibility entrypoint for raw Ansible/Rustible invocation timings.
# Equal effects are not verified; use disposable benchmark targets.
set -euo pipefail
umask 077
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SCRIPT_DIR/run_benchmark.py" "$@"
