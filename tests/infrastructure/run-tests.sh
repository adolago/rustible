#!/bin/bash
# Rustible Heavy-Duty Test Runner
# Orchestrates test execution across the VM test cluster

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SSH_CONFIG="$SCRIPT_DIR/ssh_config"
INVENTORY="$SCRIPT_DIR/test_inventory.yml"

# Test environment
export RUSTIBLE_TEST_SSH_ENABLED=1
export RUSTIBLE_TEST_PARALLEL_ENABLED=1
export RUSTIBLE_TEST_DOCKER_ENABLED=1
export RUSTIBLE_TEST_CHAOS_ENABLED=1
export RUSTIBLE_TEST_SSH_USER="${RUSTIBLE_TEST_SSH_USER:-testuser}"
export RUSTIBLE_TEST_SSH_KEY="${RUSTIBLE_TEST_SSH_KEY:-$HOME/.ssh/id_ed25519}"
export RUSTIBLE_TEST_INVENTORY="$INVENTORY"

# SSH target hosts
export RUSTIBLE_TEST_SSH_HOSTS="192.168.178.141,192.168.178.142,192.168.178.143,192.168.178.144,192.168.178.145"
export RUSTIBLE_TEST_SCALE_HOSTS="192.168.178.151,192.168.178.152,192.168.178.153,192.168.178.154,192.168.178.155,192.168.178.156,192.168.178.157,192.168.178.158,192.168.178.159,192.168.178.160"
export RUSTIBLE_TEST_DOCKER_HOST="tcp://192.168.178.210:2375"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_section() { echo -e "\n${CYAN}════════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}════════════════════════════════════════════════════════════════${NC}\n"; }

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check Rust toolchain
    if ! command -v cargo &>/dev/null; then
        log_error "Cargo not found. Install Rust toolchain first."
        exit 1
    fi

    # Check SSH key
    if [[ ! -f "$RUSTIBLE_TEST_SSH_KEY" ]]; then
        log_error "SSH key not found: $RUSTIBLE_TEST_SSH_KEY"
        exit 1
    fi

    # Check SSH config
    if [[ ! -f "$SSH_CONFIG" ]]; then
        log_warn "SSH config not found. Run provision.sh first."
    fi

    # Check connectivity to at least one host
    local first_host=$(echo $RUSTIBLE_TEST_SSH_HOSTS | cut -d, -f1)
    if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "$RUSTIBLE_TEST_SSH_USER@$first_host" "echo ok" &>/dev/null; then
        log_warn "Cannot reach first test host $first_host. Tests may fail."
    else
        log_success "SSH connectivity verified"
    fi

    log_success "Prerequisites check passed"
}

# Run specific test suite
run_test_suite() {
    local suite=$1
    local extra_args="${2:-}"

    log_section "Running: $suite"

    cd "$PROJECT_ROOT"

    case $suite in
        real-ssh|ssh)
            cargo test --test real_ssh_tests $extra_args -- --test-threads=2
            ;;
        parallel|stress)
            cargo test --test parallel_stress_tests $extra_args -- --test-threads=1
            ;;
        docker)
            cargo test --test real_docker_tests --features docker $extra_args -- --test-threads=1
            ;;
        chaos)
            cargo test --test chaos_tests $extra_args -- --test-threads=1
            ;;
        integration)
            cargo test --test integration_tests $extra_args
            ;;
        unit)
            cargo test --lib $extra_args
            ;;
        all-unit)
            cargo test $extra_args
            ;;
        quick)
            # Quick smoke test - just basic connectivity
            cargo test --test real_ssh_tests test_ssh_connect $extra_args -- --test-threads=1
            cargo test --test real_ssh_tests test_ssh_command $extra_args -- --test-threads=1
            ;;
        *)
            log_error "Unknown test suite: $suite"
            echo "Available suites: real-ssh, parallel, docker, chaos, integration, unit, all-unit, quick"
            return 1
            ;;
    esac
}

# Run all infrastructure tests
run_all() {
    local failed=0

    log_section "Running All Heavy-Duty Tests"

    # Quick connectivity check
    run_test_suite quick || ((failed++))

    # Real SSH tests
    run_test_suite real-ssh || ((failed++))

    # Parallel stress tests
    run_test_suite parallel || ((failed++))

    # Docker tests (if docker host available)
    if curl -s --connect-timeout 2 "http://$(echo $RUSTIBLE_TEST_DOCKER_HOST | sed 's/tcp://')/version" &>/dev/null; then
        run_test_suite docker || ((failed++))
    else
        log_warn "Docker host not available, skipping docker tests"
    fi

    # Chaos tests
    run_test_suite chaos || ((failed++))

    log_section "Test Summary"
    if [[ $failed -eq 0 ]]; then
        log_success "All test suites passed!"
    else
        log_error "$failed test suite(s) failed"
        return 1
    fi
}

# Generate test report
generate_report() {
    local report_file="$SCRIPT_DIR/test_report_$(date +%Y%m%d_%H%M%S).md"

    log_info "Generating test report: $report_file"

    cat > "$report_file" << EOF
# Rustible Heavy-Duty Test Report

**Date:** $(date)
**Host:** $(hostname)
**Rust Version:** $(rustc --version)

## Environment

- SSH User: $RUSTIBLE_TEST_SSH_USER
- SSH Hosts: $RUSTIBLE_TEST_SSH_HOSTS
- Scale Hosts: $RUSTIBLE_TEST_SCALE_HOSTS
- Docker Host: $RUSTIBLE_TEST_DOCKER_HOST

## Test Results

EOF

    # Run tests with output capture
    cd "$PROJECT_ROOT"

    echo "### Unit Tests" >> "$report_file"
    echo '```' >> "$report_file"
    cargo test --lib 2>&1 | tail -20 >> "$report_file" || true
    echo '```' >> "$report_file"

    echo "### SSH Integration Tests" >> "$report_file"
    echo '```' >> "$report_file"
    cargo test --test real_ssh_tests 2>&1 | tail -30 >> "$report_file" || true
    echo '```' >> "$report_file"

    echo "### Parallel Stress Tests" >> "$report_file"
    echo '```' >> "$report_file"
    cargo test --test parallel_stress_tests -- --test-threads=1 2>&1 | tail -30 >> "$report_file" || true
    echo '```' >> "$report_file"

    echo "### Chaos Tests" >> "$report_file"
    echo '```' >> "$report_file"
    cargo test --test chaos_tests -- --test-threads=1 2>&1 | tail -30 >> "$report_file" || true
    echo '```' >> "$report_file"

    log_success "Report generated: $report_file"
}

# Benchmark mode
run_benchmark() {
    log_section "Running Performance Benchmarks"

    cd "$PROJECT_ROOT"

    # Check if criterion is available
    if ! cargo bench --help &>/dev/null; then
        log_warn "Benchmarks not available. Install criterion first."
        return 1
    fi

    cargo bench
}

# Watch mode - run tests on file changes
watch_mode() {
    log_info "Starting watch mode (requires cargo-watch)"

    if ! command -v cargo-watch &>/dev/null; then
        log_error "cargo-watch not found. Install with: cargo install cargo-watch"
        exit 1
    fi

    cd "$PROJECT_ROOT"
    cargo watch -x "test --test real_ssh_tests" -x "test --test parallel_stress_tests -- --test-threads=1"
}

# Show status
show_status() {
    log_section "Test Infrastructure Status"

    echo "SSH Hosts:"
    for host in $(echo $RUSTIBLE_TEST_SSH_HOSTS | tr ',' ' '); do
        if ssh -o ConnectTimeout=2 -o BatchMode=yes "$RUSTIBLE_TEST_SSH_USER@$host" "echo ok" &>/dev/null; then
            echo -e "  ${GREEN}✓${NC} $host"
        else
            echo -e "  ${RED}✗${NC} $host"
        fi
    done

    echo ""
    echo "Scale Hosts:"
    for host in $(echo $RUSTIBLE_TEST_SCALE_HOSTS | tr ',' ' '); do
        if ssh -o ConnectTimeout=2 -o BatchMode=yes "$RUSTIBLE_TEST_SSH_USER@$host" "echo ok" &>/dev/null; then
            echo -e "  ${GREEN}✓${NC} $host"
        else
            echo -e "  ${RED}✗${NC} $host"
        fi
    done

    echo ""
    echo "Docker Host:"
    if curl -s --connect-timeout 2 "http://$(echo $RUSTIBLE_TEST_DOCKER_HOST | sed 's/tcp://')/version" &>/dev/null; then
        echo -e "  ${GREEN}✓${NC} $RUSTIBLE_TEST_DOCKER_HOST"
    else
        echo -e "  ${RED}✗${NC} $RUSTIBLE_TEST_DOCKER_HOST"
    fi
}

# Print help
print_help() {
    cat << EOF
Rustible Heavy-Duty Test Runner

Usage: $0 <command> [options]

Commands:
  all           Run all test suites
  ssh           Run real SSH integration tests
  parallel      Run parallel execution stress tests
  docker        Run Docker connection tests
  chaos         Run chaos engineering tests
  integration   Run integration tests
  unit          Run unit tests only
  quick         Run quick smoke tests
  benchmark     Run performance benchmarks
  report        Generate test report
  status        Show infrastructure status
  watch         Watch mode (rerun on changes)
  help          Show this help

Options:
  --release     Run tests in release mode
  --verbose     Enable verbose output
  --nocapture   Show test output (-- --nocapture)

Environment Variables:
  RUSTIBLE_TEST_SSH_USER      SSH username (default: testuser)
  RUSTIBLE_TEST_SSH_KEY       Path to SSH private key
  RUSTIBLE_TEST_SSH_HOSTS     Comma-separated list of SSH hosts
  RUSTIBLE_TEST_SCALE_HOSTS   Comma-separated list of scale test hosts
  RUSTIBLE_TEST_DOCKER_HOST   Docker daemon URL

Examples:
  $0 all                      # Run all tests
  $0 ssh --verbose            # Run SSH tests with verbose output
  $0 parallel --release       # Run stress tests in release mode
  $0 status                   # Check infrastructure connectivity
  $0 report                   # Generate comprehensive report

Before running tests, ensure the test infrastructure is deployed:
  ./provision.sh deploy
EOF
}

# Main entry point
main() {
    local cmd="${1:-help}"
    shift || true

    # Parse extra args
    local extra_args=""
    for arg in "$@"; do
        case $arg in
            --release)
                extra_args="$extra_args --release"
                ;;
            --verbose|-v)
                extra_args="$extra_args -- --nocapture"
                ;;
            --nocapture)
                extra_args="$extra_args -- --nocapture"
                ;;
            *)
                extra_args="$extra_args $arg"
                ;;
        esac
    done

    case $cmd in
        all)
            check_prerequisites
            run_all
            ;;
        ssh|real-ssh)
            check_prerequisites
            run_test_suite real-ssh "$extra_args"
            ;;
        parallel|stress)
            check_prerequisites
            run_test_suite parallel "$extra_args"
            ;;
        docker)
            check_prerequisites
            run_test_suite docker "$extra_args"
            ;;
        chaos)
            check_prerequisites
            run_test_suite chaos "$extra_args"
            ;;
        integration)
            run_test_suite integration "$extra_args"
            ;;
        unit)
            run_test_suite unit "$extra_args"
            ;;
        all-unit)
            run_test_suite all-unit "$extra_args"
            ;;
        quick)
            check_prerequisites
            run_test_suite quick "$extra_args"
            ;;
        benchmark|bench)
            run_benchmark
            ;;
        report)
            check_prerequisites
            generate_report
            ;;
        status)
            show_status
            ;;
        watch)
            watch_mode
            ;;
        help|--help|-h)
            print_help
            ;;
        *)
            log_error "Unknown command: $cmd"
            print_help
            exit 1
            ;;
    esac
}

main "$@"
