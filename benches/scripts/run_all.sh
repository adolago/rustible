#!/bin/bash
# Rustible Benchmark Runner
# Runs all benchmark suites and generates reports

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RESULTS_DIR="${PROJECT_ROOT}/target/benchmark-results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
COMPARE_BASELINE=""
SAVE_BASELINE=""
BENCHMARKS=""
SAMPLE_SIZE=""
QUIET=false

usage() {
    echo "Usage: $0 [OPTIONS] [BENCHMARKS...]"
    echo ""
    echo "Options:"
    echo "  --compare BASELINE    Compare against saved baseline"
    echo "  --save BASELINE       Save results as baseline"
    echo "  --sample-size N       Set sample size (default: criterion default)"
    echo "  --quick               Run with reduced sample size (20)"
    echo "  --quiet               Suppress detailed output"
    echo "  -h, --help            Show this help"
    echo ""
    echo "Available benchmarks:"
    echo "  performance           Core performance benchmarks"
    echo "  callback              Callback system benchmarks"
    echo "  sprint2               Sprint 2 feature benchmarks"
    echo "  russh                 SSH connection benchmarks"
    echo "  all                   Run all benchmarks (default)"
    echo ""
    echo "Examples:"
    echo "  $0                           # Run all benchmarks"
    echo "  $0 performance callback      # Run specific benchmarks"
    echo "  $0 --save baseline           # Save as baseline"
    echo "  $0 --compare baseline        # Compare to baseline"
    echo "  $0 --quick                   # Quick run with fewer samples"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --compare)
            COMPARE_BASELINE="$2"
            shift 2
            ;;
        --save)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        --sample-size)
            SAMPLE_SIZE="$2"
            shift 2
            ;;
        --quick)
            SAMPLE_SIZE="20"
            shift
            ;;
        --quiet)
            QUIET=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
        *)
            BENCHMARKS="$BENCHMARKS $1"
            shift
            ;;
    esac
done

# Default to all benchmarks
if [ -z "$BENCHMARKS" ]; then
    BENCHMARKS="all"
fi

# Create results directory
mkdir -p "$RESULTS_DIR"

log() {
    if [ "$QUIET" = false ]; then
        echo -e "$1"
    fi
}

log_header() {
    log ""
    log "${BLUE}========================================${NC}"
    log "${BLUE}$1${NC}"
    log "${BLUE}========================================${NC}"
    log ""
}

run_benchmark() {
    local name="$1"
    local bench_name="$2"
    local extra_args=""

    log_header "Running $name benchmarks"

    if [ -n "$COMPARE_BASELINE" ]; then
        extra_args="$extra_args --baseline $COMPARE_BASELINE"
    fi

    if [ -n "$SAVE_BASELINE" ]; then
        extra_args="$extra_args --save-baseline $SAVE_BASELINE"
    fi

    if [ -n "$SAMPLE_SIZE" ]; then
        extra_args="$extra_args --sample-size $SAMPLE_SIZE"
    fi

    cd "$PROJECT_ROOT"

    if cargo bench --bench "$bench_name" -- $extra_args 2>&1 | tee "$RESULTS_DIR/${bench_name}_${TIMESTAMP}.log"; then
        log "${GREEN}[PASS]${NC} $name benchmarks completed"
        return 0
    else
        log "${RED}[FAIL]${NC} $name benchmarks failed"
        return 1
    fi
}

# Track results
PASSED=0
FAILED=0
SKIPPED=0

run_if_requested() {
    local key="$1"
    local name="$2"
    local bench_name="$3"

    if [[ "$BENCHMARKS" == *"all"* ]] || [[ "$BENCHMARKS" == *"$key"* ]]; then
        if run_benchmark "$name" "$bench_name"; then
            ((PASSED++))
        else
            ((FAILED++))
        fi
    else
        log "${YELLOW}[SKIP]${NC} $name benchmarks"
        ((SKIPPED++))
    fi
}

log_header "Rustible Benchmark Suite"
log "Timestamp: $TIMESTAMP"
log "Project: $PROJECT_ROOT"
log "Results: $RESULTS_DIR"
log ""

# Run requested benchmarks
run_if_requested "performance" "Performance" "performance_benchmark"
run_if_requested "callback" "Callback" "callback_benchmark"
run_if_requested "sprint2" "Sprint 2 Features" "sprint2_feature_benchmark"
run_if_requested "russh" "SSH/Russh" "russh_benchmark"

# Summary
log_header "Benchmark Summary"
log "Passed:  ${GREEN}$PASSED${NC}"
log "Failed:  ${RED}$FAILED${NC}"
log "Skipped: ${YELLOW}$SKIPPED${NC}"
log ""

if [ $FAILED -gt 0 ]; then
    log "${RED}Some benchmarks failed!${NC}"
    exit 1
fi

log "${GREEN}All requested benchmarks completed successfully!${NC}"
log ""
log "View HTML reports:"
log "  xdg-open $PROJECT_ROOT/target/criterion/report/index.html"
log ""
log "Results saved to:"
log "  $RESULTS_DIR"

exit 0
