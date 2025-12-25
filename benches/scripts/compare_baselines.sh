#!/bin/bash
# Compare two benchmark baselines and detect regressions

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CRITERION_DIR="${PROJECT_ROOT}/target/criterion"

BASELINE_OLD="${1:-baseline}"
BASELINE_NEW="${2:-current}"
THRESHOLD="${3:-10}"  # Regression threshold percentage

usage() {
    echo "Usage: $0 [OLD_BASELINE] [NEW_BASELINE] [THRESHOLD]"
    echo ""
    echo "Arguments:"
    echo "  OLD_BASELINE    Name of old baseline (default: baseline)"
    echo "  NEW_BASELINE    Name of new baseline (default: current)"
    echo "  THRESHOLD       Regression threshold % (default: 10)"
    echo ""
    echo "Examples:"
    echo "  $0                          # Compare baseline vs current"
    echo "  $0 v1.0 v1.1                # Compare v1.0 vs v1.1"
    echo "  $0 baseline current 5       # 5% threshold"
}

if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
    usage
    exit 0
fi

echo "Comparing baselines: $BASELINE_OLD vs $BASELINE_NEW"
echo "Regression threshold: ${THRESHOLD}%"
echo ""

# Run comparison
cd "$PROJECT_ROOT"

REGRESSIONS=0
IMPROVEMENTS=0
UNCHANGED=0

# Parse criterion output for regression detection
compare_output=$(cargo bench -- --baseline "$BASELINE_OLD" 2>&1) || true

# Look for regression indicators
while IFS= read -r line; do
    if [[ "$line" == *"Performance has regressed"* ]]; then
        ((REGRESSIONS++))
        echo "[REGRESSION] $line"
    elif [[ "$line" == *"Performance has improved"* ]]; then
        ((IMPROVEMENTS++))
        echo "[IMPROVED] $line"
    elif [[ "$line" == *"No change in performance"* ]]; then
        ((UNCHANGED++))
    fi
done <<< "$compare_output"

echo ""
echo "=========================================="
echo "Comparison Summary"
echo "=========================================="
echo "Regressions:  $REGRESSIONS"
echo "Improvements: $IMPROVEMENTS"
echo "Unchanged:    $UNCHANGED"
echo ""

if [ $REGRESSIONS -gt 0 ]; then
    echo "WARNING: Performance regressions detected!"
    echo "Review the changes and consider optimizations."
    exit 1
else
    echo "No significant regressions detected."
    exit 0
fi
