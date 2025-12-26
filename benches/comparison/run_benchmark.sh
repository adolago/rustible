#!/bin/bash
# Rustible vs Ansible Benchmark Runner
# Runs identical playbooks with both tools and compares performance

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INVENTORY="$SCRIPT_DIR/inventory.yml"
RESULTS_DIR="$SCRIPT_DIR/results"
RUNS="${RUNS:-5}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_header() { echo -e "\n${CYAN}════════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}════════════════════════════════════════════════════════════════${NC}\n"; }

# Create results directory
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="$RESULTS_DIR/benchmark_${TIMESTAMP}.csv"
SUMMARY_FILE="$RESULTS_DIR/summary_${TIMESTAMP}.txt"

# Initialize CSV
echo "tool,playbook,run,duration_ms,hosts,tasks" > "$RESULTS_FILE"

# Build rustible if needed
log_header "Building Rustible (release mode)"
cd "$PROJECT_ROOT"
cargo build --release 2>&1 | tail -5
RUSTIBLE="$PROJECT_ROOT/target/release/rustible"

if [[ ! -f "$RUSTIBLE" ]]; then
    log_info "Rustible binary not found, using cargo run"
    RUSTIBLE="cargo run --release --"
fi

# Playbooks to benchmark
PLAYBOOKS=(
    "bench_01_simple.yml"
    "bench_02_file_ops.yml"
    "bench_03_multi_task.yml"
    "bench_04_comprehensive.yml"
    "bench_05_many_hosts.yml"
    "bench_06_many_tasks.yml"
    "bench_07_templates.yml"
    "bench_08_loops.yml"
    "bench_09_handlers.yml"
    "bench_10_conditionals.yml"
)

# Function to run Ansible benchmark
run_ansible() {
    local playbook=$1
    local run_num=$2

    local start_ms=$(date +%s%3N)
    ansible-playbook -i "$INVENTORY" "$SCRIPT_DIR/$playbook" > /dev/null 2>&1
    local end_ms=$(date +%s%3N)
    local duration=$((end_ms - start_ms))

    # Get task count
    local tasks=$(grep -c "name:" "$SCRIPT_DIR/$playbook" || echo 0)

    echo "ansible,$playbook,$run_num,$duration,5,$tasks" >> "$RESULTS_FILE"
    echo "$duration"
}

# Function to run Rustible benchmark
run_rustible() {
    local playbook=$1
    local run_num=$2

    local start_ms=$(date +%s%3N)
    $RUSTIBLE run "$SCRIPT_DIR/$playbook" -i "$INVENTORY" > /dev/null 2>&1 || true
    local end_ms=$(date +%s%3N)
    local duration=$((end_ms - start_ms))

    # Get task count
    local tasks=$(grep -c "name:" "$SCRIPT_DIR/$playbook" || echo 0)

    echo "rustible,$playbook,$run_num,$duration,5,$tasks" >> "$RESULTS_FILE"
    echo "$duration"
}

# Warm-up run
log_header "Warm-up Run"
log_info "Running warm-up to establish SSH connections..."
ansible-playbook -i "$INVENTORY" "$SCRIPT_DIR/bench_01_simple.yml" > /dev/null 2>&1 || true
$RUSTIBLE run "$SCRIPT_DIR/bench_01_simple.yml" -i "$INVENTORY" > /dev/null 2>&1 || true
log_success "Warm-up complete"

# Run benchmarks
log_header "Running Benchmarks ($RUNS runs each)"

declare -A ansible_times
declare -A rustible_times

for playbook in "${PLAYBOOKS[@]}"; do
    log_info "Benchmarking: $playbook"

    ansible_total=0
    rustible_total=0

    for run in $(seq 1 $RUNS); do
        echo -n "  Run $run/$RUNS: "

        # Run Ansible
        ansible_time=$(run_ansible "$playbook" "$run")
        echo -n "Ansible: ${ansible_time}ms, "
        ansible_total=$((ansible_total + ansible_time))

        # Run Rustible
        rustible_time=$(run_rustible "$playbook" "$run")
        echo "Rustible: ${rustible_time}ms"
        rustible_total=$((rustible_total + rustible_time))

        # Small pause between runs
        sleep 1
    done

    ansible_avg=$((ansible_total / RUNS))
    rustible_avg=$((rustible_total / RUNS))
    ansible_times[$playbook]=$ansible_avg
    rustible_times[$playbook]=$rustible_avg

    echo ""
done

# Generate summary
log_header "Benchmark Results Summary"

{
    echo "Rustible vs Ansible Benchmark Results"
    echo "======================================"
    echo "Date: $(date)"
    echo "Hosts: 5 (LXC containers on Proxmox)"
    echo "Runs per playbook: $RUNS"
    echo ""
    echo "Average Execution Times (ms):"
    echo "-----------------------------"
    printf "%-30s %12s %12s %10s\n" "Playbook" "Ansible" "Rustible" "Speedup"
    printf "%-30s %12s %12s %10s\n" "--------" "-------" "--------" "-------"

    total_ansible=0
    total_rustible=0

    for playbook in "${PLAYBOOKS[@]}"; do
        ansible_avg=${ansible_times[$playbook]}
        rustible_avg=${rustible_times[$playbook]}

        if [[ $rustible_avg -gt 0 ]]; then
            speedup=$(echo "scale=2; $ansible_avg / $rustible_avg" | bc)
        else
            speedup="N/A"
        fi

        printf "%-30s %12d %12d %10sx\n" "$playbook" "$ansible_avg" "$rustible_avg" "$speedup"

        total_ansible=$((total_ansible + ansible_avg))
        total_rustible=$((total_rustible + rustible_avg))
    done

    echo ""
    if [[ $total_rustible -gt 0 ]]; then
        overall_speedup=$(echo "scale=2; $total_ansible / $total_rustible" | bc)
    else
        overall_speedup="N/A"
    fi
    printf "%-30s %12d %12d %10sx\n" "TOTAL" "$total_ansible" "$total_rustible" "$overall_speedup"

    echo ""
    echo "Raw data saved to: $RESULTS_FILE"
} | tee "$SUMMARY_FILE"

log_success "Benchmark complete!"
log_info "Results saved to: $RESULTS_DIR"
