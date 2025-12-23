#!/bin/bash
# SSH Library Comparison Benchmark Runner
# Convenient script to run benchmarks with various configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
HOST="${SSH_BENCH_HOST:-192.168.178.141}"
PORT="${SSH_BENCH_PORT:-22}"
USER="${SSH_BENCH_USER:-testuser}"
KEY="${SSH_BENCH_KEY:-~/.ssh/id_ed25519}"

print_header() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_info() {
    echo -e "${GREEN}ℹ${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

check_ssh_connectivity() {
    print_info "Checking SSH connectivity to ${USER}@${HOST}:${PORT}..."

    if ssh -i "${KEY/#\~/$HOME}" -p "$PORT" -o ConnectTimeout=5 -o StrictHostKeyChecking=no \
        "${USER}@${HOST}" "echo 'SSH connection successful'" >/dev/null 2>&1; then
        print_info "SSH connection test: ${GREEN}SUCCESS${NC}"
        return 0
    else
        print_error "SSH connection test: ${RED}FAILED${NC}"
        print_warning "Cannot connect to ${USER}@${HOST}:${PORT}"
        print_warning "Please check your SSH configuration"
        return 1
    fi
}

build_benchmark() {
    print_info "Building benchmark in release mode..."
    cargo build --release
    print_info "Build complete"
}

show_usage() {
    cat << EOF
SSH Library Comparison Benchmark Runner

Usage: $0 [OPTIONS] [BENCHMARK_TYPE]

BENCHMARK_TYPE:
    quick       Quick benchmark (fewer iterations, skip file transfer)
    standard    Standard benchmark (default: 100 iterations)
    thorough    Thorough benchmark (500 iterations, larger files)
    connection  Only connection benchmarks
    command     Only command execution benchmarks
    file        Only file transfer benchmarks
    parallel    Only parallel execution benchmarks

OPTIONS:
    -h, --host HOST         SSH host (default: $HOST)
    -p, --port PORT         SSH port (default: $PORT)
    -u, --user USER         SSH user (default: $USER)
    -k, --key KEY           SSH key file (default: $KEY)
    -v, --verbose           Verbose output
    --help                  Show this help message

ENVIRONMENT VARIABLES:
    SSH_BENCH_HOST          Default SSH host
    SSH_BENCH_PORT          Default SSH port
    SSH_BENCH_USER          Default SSH user
    SSH_BENCH_KEY           Default SSH key path

EXAMPLES:
    # Quick benchmark
    $0 quick

    # Standard benchmark with custom host
    $0 -h 192.168.1.100 standard

    # Thorough benchmark with verbose output
    $0 -v thorough

    # Only file transfer tests
    $0 file

EOF
}

# Parse command line arguments
BENCHMARK_TYPE="standard"
EXTRA_ARGS=""
VERBOSE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--host)
            HOST="$2"
            shift 2
            ;;
        -p|--port)
            PORT="$2"
            shift 2
            ;;
        -u|--user)
            USER="$2"
            shift 2
            ;;
        -k|--key)
            KEY="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE="--verbose"
            shift
            ;;
        --help)
            show_usage
            exit 0
            ;;
        quick|standard|thorough|connection|command|file|parallel)
            BENCHMARK_TYPE="$1"
            shift
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Configure based on benchmark type
case "$BENCHMARK_TYPE" in
    quick)
        print_header "Running QUICK Benchmark"
        EXTRA_ARGS="--iterations 20 --skip-file-transfer"
        ;;
    standard)
        print_header "Running STANDARD Benchmark"
        EXTRA_ARGS="--iterations 100"
        ;;
    thorough)
        print_header "Running THOROUGH Benchmark"
        EXTRA_ARGS="--iterations 500 --file-size-kb 1024"
        ;;
    connection)
        print_header "Running CONNECTION Benchmarks Only"
        print_warning "Note: This will run all benchmarks but focus on connection metrics"
        EXTRA_ARGS="--iterations 100 --skip-file-transfer"
        ;;
    command)
        print_header "Running COMMAND Execution Benchmarks Only"
        print_warning "Note: This will run all benchmarks but focus on command metrics"
        EXTRA_ARGS="--iterations 200 --skip-file-transfer"
        ;;
    file)
        print_header "Running FILE Transfer Benchmarks Only"
        EXTRA_ARGS="--iterations 50 --file-size-kb 500"
        ;;
    parallel)
        print_header "Running PARALLEL Execution Benchmarks Only"
        print_warning "Note: This will run all benchmarks but focus on parallel metrics"
        EXTRA_ARGS="--iterations 50"
        ;;
    *)
        print_error "Unknown benchmark type: $BENCHMARK_TYPE"
        show_usage
        exit 1
        ;;
esac

echo ""
print_info "Configuration:"
echo "  Host:       ${HOST}:${PORT}"
echo "  User:       ${USER}"
echo "  Key:        ${KEY}"
echo "  Type:       ${BENCHMARK_TYPE}"
echo ""

# Check SSH connectivity first
if ! check_ssh_connectivity; then
    print_error "Aborting due to SSH connectivity issues"
    exit 1
fi

echo ""

# Build
if ! build_benchmark; then
    print_error "Build failed"
    exit 1
fi

echo ""

# Run benchmark
print_header "Running Benchmarks"
echo ""

BENCHMARK_CMD="./target/release/ssh_bench --host $HOST --port $PORT --user $USER --key $KEY $EXTRA_ARGS $VERBOSE"

print_info "Executing: $BENCHMARK_CMD"
echo ""

# Create results directory if it doesn't exist
mkdir -p results

# Generate timestamp for results file
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="results/benchmark_${TIMESTAMP}.txt"

# Run the benchmark and tee output to both console and file
if eval "$BENCHMARK_CMD" 2>&1 | tee "$RESULTS_FILE"; then
    echo ""
    print_header "Benchmark Complete"
    print_info "Results saved to: ${RESULTS_FILE}"

    # Show quick summary if available
    if grep -q "BENCHMARK RESULTS" "$RESULTS_FILE"; then
        echo ""
        print_info "Quick Summary:"
        grep -A 20 "BENCHMARK RESULTS" "$RESULTS_FILE" | head -25
    fi
else
    print_error "Benchmark failed"
    exit 1
fi
