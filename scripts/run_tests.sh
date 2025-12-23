#!/bin/bash
#
# Rustible Test Runner Script
#
# This script runs all test categories in the proper order with appropriate
# failure handling and optional coverage generation.
#
# Usage:
#   ./scripts/run_tests.sh              # Run all tests
#   ./scripts/run_tests.sh --quick      # Run quick tests only
#   ./scripts/run_tests.sh --coverage   # Run with coverage (requires grcov/llvm-cov)
#   ./scripts/run_tests.sh --bench      # Include benchmarks
#   ./scripts/run_tests.sh --verbose    # Verbose output
#
# Environment Variables:
#   RUSTIBLE_TEST_SSH_HOST    - Enable SSH tests against this host
#   RUSTIBLE_TEST_DOCKER      - Enable Docker tests (set to "1")
#   RUSTIBLE_TEST_VERBOSE     - Enable verbose output (set to "1")
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default options
RUN_QUICK=false
RUN_COVERAGE=false
RUN_BENCH=false
VERBOSE=false
FAILED_TESTS=0
PASSED_TESTS=0

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick|-q)
            RUN_QUICK=true
            shift
            ;;
        --coverage|-c)
            RUN_COVERAGE=true
            shift
            ;;
        --bench|-b)
            RUN_BENCH=true
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Rustible Test Runner"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --quick, -q      Run quick tests only (skip slow tests)"
            echo "  --coverage, -c   Generate code coverage report"
            echo "  --bench, -b      Include benchmark tests"
            echo "  --verbose, -v    Show verbose output"
            echo "  --help, -h       Show this help message"
            echo ""
            echo "Environment Variables:"
            echo "  RUSTIBLE_TEST_SSH_HOST    Enable SSH tests against this host"
            echo "  RUSTIBLE_TEST_DOCKER      Enable Docker tests (set to '1')"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Check for verbose environment variable
if [[ "$RUSTIBLE_TEST_VERBOSE" == "1" ]]; then
    VERBOSE=true
fi

# Helper functions
print_header() {
    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
}

print_step() {
    echo -e "${YELLOW}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
    ((PASSED_TESTS++))
}

print_failure() {
    echo -e "${RED}✗ $1${NC}"
    ((FAILED_TESTS++))
}

print_skip() {
    echo -e "${YELLOW}⊘ $1 (skipped)${NC}"
}

run_test_category() {
    local name="$1"
    local command="$2"
    local required="${3:-false}"

    print_step "Running: $name"

    if $VERBOSE; then
        if eval "$command"; then
            print_success "$name passed"
            return 0
        else
            print_failure "$name failed"
            if [[ "$required" == "true" ]]; then
                return 1
            fi
            return 0
        fi
    else
        if eval "$command" > /dev/null 2>&1; then
            print_success "$name passed"
            return 0
        else
            print_failure "$name failed"
            # Re-run to show output on failure
            echo "  Re-running with output:"
            eval "$command" || true
            if [[ "$required" == "true" ]]; then
                return 1
            fi
            return 0
        fi
    fi
}

# Change to project root
cd "$PROJECT_ROOT"

print_header "Rustible Test Suite"

echo ""
echo "Project: $PROJECT_ROOT"
echo "Quick mode: $RUN_QUICK"
echo "Coverage: $RUN_COVERAGE"
echo "Benchmarks: $RUN_BENCH"
echo "Verbose: $VERBOSE"
echo ""

# Check for required tools
print_step "Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo is not installed${NC}"
    exit 1
fi

if ! command -v rustc &> /dev/null; then
    echo -e "${RED}Error: rustc is not installed${NC}"
    exit 1
fi

echo "  Rust version: $(rustc --version)"
echo "  Cargo version: $(cargo --version)"

# Setup coverage if requested
if $RUN_COVERAGE; then
    print_step "Setting up code coverage..."

    if command -v cargo-llvm-cov &> /dev/null; then
        COVERAGE_TOOL="llvm-cov"
        echo "  Using cargo-llvm-cov"
    elif command -v grcov &> /dev/null; then
        COVERAGE_TOOL="grcov"
        export CARGO_INCREMENTAL=0
        export RUSTFLAGS="-Cinstrument-coverage"
        export LLVM_PROFILE_FILE="$PROJECT_ROOT/target/coverage/%p-%m.profraw"
        echo "  Using grcov"
    else
        echo -e "${YELLOW}  Warning: No coverage tool found. Install cargo-llvm-cov or grcov.${NC}"
        echo "  Continuing without coverage..."
        RUN_COVERAGE=false
    fi
fi

# Phase 1: Formatting and Linting
print_header "Phase 1: Code Quality Checks"

if ! $RUN_QUICK; then
    run_test_category "Format check (cargo fmt)" \
        "cargo fmt --all -- --check"

    run_test_category "Clippy lints" \
        "cargo clippy --all-targets --all-features -- -D warnings"
fi

# Phase 2: Compilation Check
print_header "Phase 2: Compilation"

run_test_category "Build (debug)" \
    "cargo build --all-features" \
    true

if ! $RUN_QUICK; then
    run_test_category "Build (release)" \
        "cargo build --release --all-features"
fi

# Phase 3: Unit Tests
print_header "Phase 3: Unit Tests"

TEST_ARGS=""
if $VERBOSE; then
    TEST_ARGS="$TEST_ARGS --nocapture"
fi

if $RUN_COVERAGE && [[ "$COVERAGE_TOOL" == "llvm-cov" ]]; then
    run_test_category "Library tests (with coverage)" \
        "cargo llvm-cov --lib --all-features" \
        true
else
    run_test_category "Library tests" \
        "cargo test --lib --all-features -- $TEST_ARGS" \
        true
fi

# Phase 4: Integration Tests
print_header "Phase 4: Integration Tests"

# Core integration tests
run_test_category "Executor tests" \
    "cargo test --test executor_tests -- $TEST_ARGS" \
    true

run_test_category "Connection tests" \
    "cargo test --test connection_tests -- $TEST_ARGS" \
    true

run_test_category "Module tests" \
    "cargo test --test module_tests -- $TEST_ARGS" \
    true

# Optional integration tests
if ! $RUN_QUICK; then
    run_test_category "Inventory tests" \
        "cargo test --test inventory_tests -- $TEST_ARGS"

    run_test_category "Template tests" \
        "cargo test --test template_tests -- $TEST_ARGS"

    run_test_category "Parser tests" \
        "cargo test --test parser_tests -- $TEST_ARGS"

    run_test_category "Handler tests" \
        "cargo test --test handler_tests -- $TEST_ARGS"

    run_test_category "Role tests" \
        "cargo test --test role_tests -- $TEST_ARGS"

    run_test_category "Strategy tests" \
        "cargo test --test strategy_tests -- $TEST_ARGS"

    run_test_category "Facts tests" \
        "cargo test --test facts_tests -- $TEST_ARGS"

    run_test_category "Config tests" \
        "cargo test --test config_tests -- $TEST_ARGS"

    run_test_category "Vault tests" \
        "cargo test --test vault_tests -- $TEST_ARGS"

    run_test_category "Error tests" \
        "cargo test --test error_tests -- $TEST_ARGS"

    run_test_category "CLI tests" \
        "cargo test --test cli_tests -- $TEST_ARGS"

    run_test_category "Ansible compatibility tests" \
        "cargo test --test ansible_compat_tests -- $TEST_ARGS"

    run_test_category "Full integration tests" \
        "cargo test --test integration_tests -- $TEST_ARGS"
fi

# Phase 5: Doc Tests
print_header "Phase 5: Documentation Tests"

if ! $RUN_QUICK; then
    run_test_category "Documentation tests" \
        "cargo test --doc --all-features"
fi

# Phase 6: Optional Tests (SSH, Docker)
if [[ -n "$RUSTIBLE_TEST_SSH_HOST" ]] || [[ "$RUSTIBLE_TEST_DOCKER" == "1" ]]; then
    print_header "Phase 6: Optional Integration Tests"

    if [[ -n "$RUSTIBLE_TEST_SSH_HOST" ]]; then
        run_test_category "SSH connection tests" \
            "cargo test --test connection_tests ssh -- --ignored $TEST_ARGS"
    else
        print_skip "SSH tests (set RUSTIBLE_TEST_SSH_HOST to enable)"
    fi

    if [[ "$RUSTIBLE_TEST_DOCKER" == "1" ]]; then
        run_test_category "Docker connection tests" \
            "cargo test --test connection_tests docker --features docker -- --ignored $TEST_ARGS"
    else
        print_skip "Docker tests (set RUSTIBLE_TEST_DOCKER=1 to enable)"
    fi
else
    print_header "Phase 6: Optional Integration Tests"
    print_skip "SSH tests (set RUSTIBLE_TEST_SSH_HOST to enable)"
    print_skip "Docker tests (set RUSTIBLE_TEST_DOCKER=1 to enable)"
fi

# Phase 7: Benchmarks
if $RUN_BENCH; then
    print_header "Phase 7: Benchmarks"

    run_test_category "Execution benchmarks" \
        "cargo bench --bench execution_benchmark"

    run_test_category "Performance benchmarks" \
        "cargo bench --bench performance_benchmark"
fi

# Phase 8: Coverage Report
if $RUN_COVERAGE && [[ "$COVERAGE_TOOL" == "llvm-cov" ]]; then
    print_header "Phase 8: Coverage Report"

    print_step "Generating coverage report..."
    cargo llvm-cov report --html --output-dir "$PROJECT_ROOT/target/coverage-report"
    echo "  Coverage report: $PROJECT_ROOT/target/coverage-report/html/index.html"

    # Show summary
    cargo llvm-cov report

elif $RUN_COVERAGE && [[ "$COVERAGE_TOOL" == "grcov" ]]; then
    print_header "Phase 8: Coverage Report"

    print_step "Generating coverage report with grcov..."

    grcov . \
        --binary-path ./target/debug/ \
        -s . \
        -t html \
        --branch \
        --ignore-not-existing \
        --ignore "/*" \
        --ignore "target/*" \
        -o "$PROJECT_ROOT/target/coverage-report"

    echo "  Coverage report: $PROJECT_ROOT/target/coverage-report/index.html"
fi

# Summary
print_header "Test Summary"

TOTAL_TESTS=$((PASSED_TESTS + FAILED_TESTS))

echo ""
echo -e "  ${GREEN}Passed:${NC} $PASSED_TESTS"
echo -e "  ${RED}Failed:${NC} $FAILED_TESTS"
echo -e "  Total:  $TOTAL_TESTS"
echo ""

if [[ $FAILED_TESTS -gt 0 ]]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
