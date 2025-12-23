#!/bin/bash
# Run Parallel Execution E2E Tests
#
# This script helps run the parallel execution end-to-end tests with proper configuration.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
SSH_USER="${RUSTIBLE_E2E_SSH_USER:-testuser}"
SSH_KEY="${RUSTIBLE_E2E_SSH_KEY:-$HOME/.ssh/id_ed25519}"
HOSTS="${RUSTIBLE_E2E_HOSTS:-}"

# Usage information
usage() {
    cat << EOF
Usage: $0 [OPTIONS] [TEST_NAME]

Run parallel execution E2E tests for Rustible.

OPTIONS:
    -h, --help              Show this help message
    -u, --user USER         SSH user for test hosts (default: testuser)
    -k, --key KEY_PATH      Path to SSH private key (default: ~/.ssh/id_ed25519)
    -H, --hosts HOSTS       Comma-separated list of host IPs
    -l, --localhost-only    Run only localhost tests (no SSH hosts)
    -v, --verbose           Verbose output
    --list                  List available tests

TEST_NAME:
    If provided, runs only the specified test. Otherwise runs all tests.
    Examples:
      - test_parallel_execution_on_localhost
      - test_parallel_execution_multiple_hosts
      - test_linear_vs_free_strategy_performance
      - test_connection_reuse_in_parallel_execution
      - test_fork_limiting_with_many_hosts
      - test_parallel_performance_improvement

ENVIRONMENT VARIABLES:
    RUSTIBLE_E2E_SSH_USER       SSH username
    RUSTIBLE_E2E_SSH_KEY        Path to SSH private key
    RUSTIBLE_E2E_HOSTS          Comma-separated list of host IPs

EXAMPLES:
    # Run localhost-only tests
    $0 --localhost-only

    # Run all tests with SSH hosts
    $0 --hosts "192.168.1.10,192.168.1.11,192.168.1.12"

    # Run specific test
    $0 test_parallel_execution_on_localhost

    # Run with custom SSH configuration
    $0 --user ubuntu --key ~/.ssh/aws-key.pem --hosts "ec2-1.amazonaws.com,ec2-2.amazonaws.com"

EOF
}

# List available tests
list_tests() {
    echo -e "${GREEN}Available E2E Tests:${NC}"
    echo "  - test_parallel_execution_on_localhost"
    echo "  - test_parallel_execution_multiple_hosts"
    echo "  - test_linear_vs_free_strategy_performance"
    echo "  - test_connection_reuse_in_parallel_execution"
    echo "  - test_fork_limiting_with_many_hosts"
    echo "  - test_parallel_performance_improvement"
}

# Parse arguments
LOCALHOST_ONLY=false
VERBOSE=""
TEST_NAME=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -u|--user)
            SSH_USER="$2"
            shift 2
            ;;
        -k|--key)
            SSH_KEY="$2"
            shift 2
            ;;
        -H|--hosts)
            HOSTS="$2"
            shift 2
            ;;
        -l|--localhost-only)
            LOCALHOST_ONLY=true
            shift
            ;;
        -v|--verbose)
            VERBOSE="--show-output"
            shift
            ;;
        --list)
            list_tests
            exit 0
            ;;
        *)
            TEST_NAME="$1"
            shift
            ;;
    esac
done

# Print configuration
echo -e "${GREEN}=== Rustible Parallel E2E Tests ===${NC}"
echo ""
echo "Configuration:"
echo "  SSH User: $SSH_USER"
echo "  SSH Key:  $SSH_KEY"

if [ "$LOCALHOST_ONLY" = true ]; then
    echo "  Mode:     Localhost only"
    TEST_NAME="${TEST_NAME:-test_parallel_execution_on_localhost}"
else
    echo "  Hosts:    ${HOSTS:-<none - will skip multi-host tests>}"
fi

echo ""

# Set environment variables
export RUSTIBLE_E2E_SSH_USER="$SSH_USER"
export RUSTIBLE_E2E_SSH_KEY="$SSH_KEY"

if [ "$LOCALHOST_ONLY" = true ]; then
    # Don't enable E2E for localhost-only tests
    unset RUSTIBLE_E2E_ENABLED
    unset RUSTIBLE_E2E_HOSTS
else
    export RUSTIBLE_E2E_ENABLED=1
    if [ -n "$HOSTS" ]; then
        export RUSTIBLE_E2E_HOSTS="$HOSTS"
    fi
fi

# Build test command
TEST_CMD="cargo test --test parallel_e2e_tests"

if [ -n "$TEST_NAME" ]; then
    TEST_CMD="$TEST_CMD $TEST_NAME"
fi

TEST_CMD="$TEST_CMD -- --nocapture --test-threads=1 $VERBOSE"

# Run tests
echo -e "${GREEN}Running tests...${NC}"
echo "Command: $TEST_CMD"
echo ""

if eval "$TEST_CMD"; then
    echo ""
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
