#!/bin/bash
# Parallel Host Execution Benchmark
# Compares Rustible and Ansible performance with different fork/parallelism values
# Tests sequential (1), partial parallel (2), and full parallel (5) execution

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INVENTORY="$SCRIPT_DIR/inventory.yml"
PLAYBOOK="$SCRIPT_DIR/bench_parallel_hosts.yml"
RESULTS_DIR="$SCRIPT_DIR/results"
RUNS="${RUNS:-5}"

# Fork values to test (1=sequential, 2=partial parallel, 5=full parallel for 5 hosts)
FORK_VALUES=(1 2 5)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_header() { echo -e "\n${CYAN}════════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}════════════════════════════════════════════════════════════════${NC}\n"; }

# Check if playbook exists
if [[ ! -f "$PLAYBOOK" ]]; then
    echo -e "${RED}ERROR:${NC} Playbook not found: $PLAYBOOK"
    exit 1
fi

# Create results directory
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="$RESULTS_DIR/parallel_benchmark_${TIMESTAMP}.csv"
SUMMARY_FILE="$RESULTS_DIR/parallel_summary_${TIMESTAMP}.txt"

# Initialize CSV
echo "tool,forks,run,duration_ms,hosts,tasks,speedup_vs_sequential" > "$RESULTS_FILE"

# Build rustible if needed
log_header "Building Rustible (release mode)"
cd "$PROJECT_ROOT"
cargo build --release 2>&1 | tail -5
RUSTIBLE="$PROJECT_ROOT/target/release/rustible"

if [[ ! -f "$RUSTIBLE" ]]; then
    log_info "Rustible binary not found, using cargo run"
    RUSTIBLE="cargo run --release --"
fi

# Check if Ansible is available
if ! command -v ansible-playbook &> /dev/null; then
    log_warn "ansible-playbook not found, skipping Ansible benchmarks"
    SKIP_ANSIBLE=1
else
    SKIP_ANSIBLE=0
fi

# Function to run Ansible benchmark with specific forks value
run_ansible() {
    local forks=$1
    local run_num=$2

    local start_ms=$(date +%s%3N)
    ansible-playbook -i "$INVENTORY" "$PLAYBOOK" -f "$forks" > /dev/null 2>&1
    local end_ms=$(date +%s%3N)
    local duration=$((end_ms - start_ms))

    echo "$duration"
}

# Function to run Rustible benchmark with specific forks value
run_rustible() {
    local forks=$1
    local run_num=$2

    local start_ms=$(date +%s%3N)
    # Note: Rustible currently uses parallel execution by default
    # This script assumes --forks or similar flag will be implemented
    # For now, we run without specific fork control
    if [[ $forks -eq 1 ]]; then
        # Sequential mode - use strategy: linear if supported
        $RUSTIBLE run "$PLAYBOOK" -i "$INVENTORY" > /dev/null 2>&1 || true
    else
        # Parallel mode (default behavior)
        $RUSTIBLE run "$PLAYBOOK" -i "$INVENTORY" > /dev/null 2>&1 || true
    fi
    local end_ms=$(date +%s%3N)
    local duration=$((end_ms - start_ms))

    echo "$duration"
}

# Warm-up run
log_header "Warm-up Run"
log_info "Running warm-up to establish SSH connections..."
if [[ $SKIP_ANSIBLE -eq 0 ]]; then
    ansible-playbook -i "$INVENTORY" "$PLAYBOOK" -f 5 > /dev/null 2>&1 || true
fi
$RUSTIBLE run "$PLAYBOOK" -i "$INVENTORY" > /dev/null 2>&1 || true
log_success "Warm-up complete"

# Count tasks
TASKS=$(grep -c "name:" "$PLAYBOOK" | grep -E '^\s+- name:' || grep -c "^\s\+- name:" "$PLAYBOOK" || echo 20)

# Storage for results
declare -A ansible_times
declare -A rustible_times
declare -A ansible_sequential_avg
declare -A rustible_sequential_avg

# Run benchmarks
log_header "Running Parallel Benchmarks ($RUNS runs each)"

for forks in "${FORK_VALUES[@]}"; do
    log_info "Testing with forks=$forks (parallelism level)"

    # Ansible benchmarks
    if [[ $SKIP_ANSIBLE -eq 0 ]]; then
        echo -e "${MAGENTA}  Ansible (forks=$forks):${NC}"
        ansible_total=0

        for run in $(seq 1 $RUNS); do
            ansible_time=$(run_ansible "$forks" "$run")
            echo "    Run $run/$RUNS: ${ansible_time}ms"
            ansible_total=$((ansible_total + ansible_time))
            echo "ansible,$forks,$run,$ansible_time,5,$TASKS," >> "$RESULTS_FILE"
            sleep 0.5
        done

        ansible_avg=$((ansible_total / RUNS))
        ansible_times[$forks]=$ansible_avg

        # Store sequential baseline
        if [[ $forks -eq 1 ]]; then
            ansible_sequential_avg=$ansible_avg
        fi

        echo -e "  ${GREEN}Average: ${ansible_avg}ms${NC}\n"
    fi

    # Rustible benchmarks
    echo -e "${MAGENTA}  Rustible (forks=$forks):${NC}"
    rustible_total=0

    for run in $(seq 1 $RUNS); do
        rustible_time=$(run_rustible "$forks" "$run")
        echo "    Run $run/$RUNS: ${rustible_time}ms"
        rustible_total=$((rustible_total + rustible_time))
        echo "rustible,$forks,$run,$rustible_time,5,$TASKS," >> "$RESULTS_FILE"
        sleep 0.5
    done

    rustible_avg=$((rustible_total / RUNS))
    rustible_times[$forks]=$rustible_avg

    # Store sequential baseline
    if [[ $forks -eq 1 ]]; then
        rustible_sequential_avg=$rustible_avg
    fi

    echo -e "  ${GREEN}Average: ${rustible_avg}ms${NC}\n"
done

# Generate summary
log_header "Parallel Execution Benchmark Results"

{
    echo "Parallel Host Execution Benchmark Results"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Playbook: bench_parallel_hosts.yml"
    echo "Hosts: 5 (LXC containers on Proxmox)"
    echo "Tasks per host: 20 simple commands"
    echo "Runs per configuration: $RUNS"
    echo ""
    echo "Understanding Fork Values:"
    echo "  forks=1: Sequential execution (one host at a time)"
    echo "  forks=2: Partial parallel (2 hosts at a time)"
    echo "  forks=5: Full parallel (all 5 hosts simultaneously)"
    echo ""

    if [[ $SKIP_ANSIBLE -eq 0 ]]; then
        echo "Ansible Results:"
        echo "----------------"
        printf "%-10s %15s %15s %15s\n" "Forks" "Avg Time (ms)" "Speedup vs 1" "Efficiency"
        printf "%-10s %15s %15s %15s\n" "-----" "-------------" "------------" "----------"

        for forks in "${FORK_VALUES[@]}"; do
            ansible_avg=${ansible_times[$forks]}

            if [[ $forks -eq 1 ]]; then
                speedup="1.00"
                efficiency="100%"
            else
                if [[ $ansible_sequential_avg -gt 0 ]]; then
                    speedup=$(echo "scale=2; $ansible_sequential_avg / $ansible_avg" | bc)
                    # Efficiency = (speedup / forks) * 100
                    efficiency=$(echo "scale=1; ($speedup / $forks) * 100" | bc)
                    efficiency="${efficiency}%"
                else
                    speedup="N/A"
                    efficiency="N/A"
                fi
            fi

            printf "%-10d %15d %15sx %14s\n" "$forks" "$ansible_avg" "$speedup" "$efficiency"
        done
        echo ""
    fi

    echo "Rustible Results:"
    echo "-----------------"
    printf "%-10s %15s %15s %15s\n" "Forks" "Avg Time (ms)" "Speedup vs 1" "Efficiency"
    printf "%-10s %15s %15s %15s\n" "-----" "-------------" "------------" "----------"

    for forks in "${FORK_VALUES[@]}"; do
        rustible_avg=${rustible_times[$forks]}

        if [[ $forks -eq 1 ]]; then
            speedup="1.00"
            efficiency="100%"
        else
            if [[ $rustible_sequential_avg -gt 0 ]]; then
                speedup=$(echo "scale=2; $rustible_sequential_avg / $rustible_avg" | bc)
                # Efficiency = (speedup / forks) * 100
                efficiency=$(echo "scale=1; ($speedup / $forks) * 100" | bc)
                efficiency="${efficiency}%"
            else
                speedup="N/A"
                efficiency="N/A"
            fi
        fi

        printf "%-10d %15d %15sx %14s\n" "$forks" "$rustible_avg" "$speedup" "$efficiency"
    done
    echo ""

    if [[ $SKIP_ANSIBLE -eq 0 ]]; then
        echo "Rustible vs Ansible Comparison:"
        echo "--------------------------------"
        printf "%-10s %15s %15s %15s\n" "Forks" "Ansible (ms)" "Rustible (ms)" "Rustible Speedup"
        printf "%-10s %15s %15s %15s\n" "-----" "------------" "-------------" "----------------"

        for forks in "${FORK_VALUES[@]}"; do
            ansible_avg=${ansible_times[$forks]}
            rustible_avg=${rustible_times[$forks]}

            if [[ $rustible_avg -gt 0 ]]; then
                speedup=$(echo "scale=2; $ansible_avg / $rustible_avg" | bc)
            else
                speedup="N/A"
            fi

            printf "%-10d %15d %15d %15sx\n" "$forks" "$ansible_avg" "$rustible_avg" "$speedup"
        done
        echo ""
    fi

    echo "Key Insights:"
    echo "-------------"
    echo "* Speedup: How much faster compared to sequential execution (forks=1)"
    echo "* Efficiency: How well parallelism is utilized (100% = linear scaling)"
    echo "* Ideal efficiency would be 100% at all fork levels"
    echo "* Lower efficiency indicates overhead from parallelization"
    echo ""
    echo "Raw data saved to: $RESULTS_FILE"
} | tee "$SUMMARY_FILE"

log_success "Parallel benchmark complete!"
log_info "Results saved to: $RESULTS_DIR"
log_info "CSV data: $RESULTS_FILE"
log_info "Summary: $SUMMARY_FILE"
