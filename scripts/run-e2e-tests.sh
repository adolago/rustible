#!/bin/bash
#
# End-to-End Module Test Runner
#
# This script runs the comprehensive E2E module tests for Rustible.
# It supports multiple execution modes: local, Docker, and SSH VMs.
#
# Usage:
#   ./scripts/run-e2e-tests.sh [OPTIONS]
#
# Options:
#   --local           Run local tests only (default)
#   --ssh             Run SSH tests against VMs
#   --all             Run all available tests
#   --check           Run check mode tests
#   --idempotency     Run idempotency tests
#   --performance     Run performance tests
#   --verbose         Enable verbose output
#   --help            Show this help message
#
# Examples:
#   ./scripts/run-e2e-tests.sh --local
#   ./scripts/run-e2e-tests.sh --ssh
#   ./scripts/run-e2e-tests.sh --all --verbose
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default settings
RUN_LOCAL=1
RUN_SSH=0
RUN_ALL=0
VERBOSE=0
TEST_FILTER=""

# Function to print colored output
print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ $1${NC}"
}

# Function to show help
show_help() {
    cat << EOF
End-to-End Module Test Runner

Usage:
  ./scripts/run-e2e-tests.sh [OPTIONS]

Options:
  --local           Run local tests only (default)
  --ssh             Run SSH tests against VMs
  --all             Run all available tests
  --check           Run check mode tests only
  --idempotency     Run idempotency tests only
  --performance     Run performance tests only
  --individual      Run individual module tests only
  --verbose         Enable verbose output
  --help            Show this help message

Environment Variables:
  RUSTIBLE_TEST_SSH_ENABLED       Enable SSH tests (1/true)
  RUSTIBLE_TEST_SSH_USER          SSH username (default: testuser)
  RUSTIBLE_TEST_SSH_HOSTS         Comma-separated list of SSH hosts
  RUSTIBLE_TEST_SSH_KEY           Path to SSH private key
  RUSTIBLE_TEST_INVENTORY         Path to inventory file
  RUSTIBLE_TEST_VERBOSE           Verbosity level (0-3)

Examples:
  # Run local tests
  ./scripts/run-e2e-tests.sh --local

  # Run SSH tests
  export RUSTIBLE_TEST_SSH_ENABLED=1
  export RUSTIBLE_TEST_SSH_HOSTS="192.168.178.141,192.168.178.142"
  ./scripts/run-e2e-tests.sh --ssh

  # Run all tests with verbose output
  ./scripts/run-e2e-tests.sh --all --verbose

  # Run only idempotency tests
  ./scripts/run-e2e-tests.sh --idempotency

  # Using test infrastructure
  cd tests/infrastructure && ./provision.sh deploy
  export RUSTIBLE_TEST_INVENTORY=tests/infrastructure/test_inventory.yml
  ./scripts/run-e2e-tests.sh --ssh
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --local)
            RUN_LOCAL=1
            RUN_SSH=0
            shift
            ;;
        --ssh)
            RUN_LOCAL=0
            RUN_SSH=1
            export RUSTIBLE_TEST_SSH_ENABLED=1
            shift
            ;;
        --all)
            RUN_ALL=1
            shift
            ;;
        --check)
            TEST_FILTER="test_e2e_modules_check_mode"
            shift
            ;;
        --idempotency)
            TEST_FILTER="test_e2e_modules_idempotency"
            shift
            ;;
        --performance)
            TEST_FILTER="test_e2e_modules_performance"
            shift
            ;;
        --individual)
            TEST_FILTER="test_e2e_individual_modules"
            shift
            ;;
        --verbose)
            VERBOSE=1
            export RUSTIBLE_TEST_VERBOSE=1
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "tests" ]; then
    print_error "This script must be run from the root of the Rustible repository"
    exit 1
fi

# Verify playbook exists
PLAYBOOK_PATH="tests/fixtures/integration/playbooks/modules_e2e.yml"
if [ ! -f "$PLAYBOOK_PATH" ]; then
    print_error "E2E playbook not found at $PLAYBOOK_PATH"
    exit 1
fi

print_header "Rustible E2E Module Tests"

# Show configuration
print_info "Configuration:"
echo "  Local tests: $([ $RUN_LOCAL -eq 1 ] && echo 'enabled' || echo 'disabled')"
echo "  SSH tests: $([ $RUN_SSH -eq 1 ] && echo 'enabled' || echo 'disabled')"
echo "  Verbose: $([ $VERBOSE -eq 1 ] && echo 'yes' || echo 'no')"
if [ -n "$TEST_FILTER" ]; then
    echo "  Filter: $TEST_FILTER"
fi
echo

# Check SSH configuration if SSH tests are enabled
if [ $RUN_SSH -eq 1 ]; then
    print_info "SSH Configuration:"
    echo "  User: ${RUSTIBLE_TEST_SSH_USER:-not set}"
    echo "  Hosts: ${RUSTIBLE_TEST_SSH_HOSTS:-not set}"
    echo "  Key: ${RUSTIBLE_TEST_SSH_KEY:-default}"
    echo "  Inventory: ${RUSTIBLE_TEST_INVENTORY:-not set}"
    echo

    if [ -z "$RUSTIBLE_TEST_SSH_HOSTS" ] && [ -z "$RUSTIBLE_TEST_INVENTORY" ]; then
        print_error "SSH tests require either RUSTIBLE_TEST_SSH_HOSTS or RUSTIBLE_TEST_INVENTORY to be set"
        exit 1
    fi
fi

# Build the test command
TEST_CMD="cargo test --test modules_e2e_tests"

if [ $VERBOSE -eq 1 ]; then
    TEST_CMD="$TEST_CMD -- --nocapture"
fi

# Add test filter if specified
if [ -n "$TEST_FILTER" ]; then
    TEST_CMD="$TEST_CMD $TEST_FILTER"
elif [ $RUN_LOCAL -eq 1 ] && [ $RUN_SSH -eq 0 ]; then
    # Run only local tests
    TEST_CMD="$TEST_CMD -- --skip ssh"
elif [ $RUN_SSH -eq 1 ] && [ $RUN_LOCAL -eq 0 ]; then
    # Run only SSH tests
    TEST_CMD="$TEST_CMD test_e2e_modules_ssh"
fi

# Run the tests
print_header "Running Tests"
echo "Command: $TEST_CMD"
echo

if eval "$TEST_CMD"; then
    echo
    print_success "All E2E module tests passed!"
    exit 0
else
    echo
    print_error "Some E2E module tests failed"
    exit 1
fi
