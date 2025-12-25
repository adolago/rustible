#!/bin/bash
# Generate benchmark summary report

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CRITERION_DIR="${PROJECT_ROOT}/target/criterion"
OUTPUT_FILE="${1:-benchmark_report.md}"

if [ ! -d "$CRITERION_DIR" ]; then
    echo "Error: No criterion results found. Run benchmarks first."
    echo "  cargo bench"
    exit 1
fi

echo "# Rustible Benchmark Report" > "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "Generated: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Find all benchmark groups
for group_dir in "$CRITERION_DIR"/*/; do
    if [ -d "$group_dir" ]; then
        group_name=$(basename "$group_dir")

        # Skip report directory
        if [ "$group_name" = "report" ]; then
            continue
        fi

        echo "## $group_name" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo "| Benchmark | Mean | Std Dev | Throughput |" >> "$OUTPUT_FILE"
        echo "|-----------|------|---------|------------|" >> "$OUTPUT_FILE"

        # Find all benchmarks in group
        for bench_dir in "$group_dir"/*/; do
            if [ -d "$bench_dir" ] && [ -f "$bench_dir/new/estimates.json" ]; then
                bench_name=$(basename "$bench_dir")

                # Extract mean time from estimates.json
                if command -v jq &> /dev/null; then
                    mean=$(jq -r '.mean.point_estimate' "$bench_dir/new/estimates.json" 2>/dev/null || echo "N/A")
                    std_dev=$(jq -r '.std_dev.point_estimate' "$bench_dir/new/estimates.json" 2>/dev/null || echo "N/A")

                    # Format time
                    if [ "$mean" != "N/A" ]; then
                        # Convert nanoseconds to appropriate unit
                        if (( $(echo "$mean > 1000000000" | bc -l) )); then
                            mean_fmt=$(printf "%.2f s" $(echo "$mean / 1000000000" | bc -l))
                        elif (( $(echo "$mean > 1000000" | bc -l) )); then
                            mean_fmt=$(printf "%.2f ms" $(echo "$mean / 1000000" | bc -l))
                        elif (( $(echo "$mean > 1000" | bc -l) )); then
                            mean_fmt=$(printf "%.2f us" $(echo "$mean / 1000" | bc -l))
                        else
                            mean_fmt=$(printf "%.2f ns" $mean)
                        fi
                    else
                        mean_fmt="N/A"
                    fi

                    if [ "$std_dev" != "N/A" ]; then
                        if (( $(echo "$std_dev > 1000000" | bc -l) )); then
                            std_fmt=$(printf "%.2f ms" $(echo "$std_dev / 1000000" | bc -l))
                        elif (( $(echo "$std_dev > 1000" | bc -l) )); then
                            std_fmt=$(printf "%.2f us" $(echo "$std_dev / 1000" | bc -l))
                        else
                            std_fmt=$(printf "%.2f ns" $std_dev)
                        fi
                    else
                        std_fmt="N/A"
                    fi

                    echo "| $bench_name | $mean_fmt | +/- $std_fmt | - |" >> "$OUTPUT_FILE"
                else
                    echo "| $bench_name | (install jq for details) | - | - |" >> "$OUTPUT_FILE"
                fi
            fi
        done

        echo "" >> "$OUTPUT_FILE"
    fi
done

echo "" >> "$OUTPUT_FILE"
echo "---" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "Full HTML report: \`target/criterion/report/index.html\`" >> "$OUTPUT_FILE"

echo "Report generated: $OUTPUT_FILE"
