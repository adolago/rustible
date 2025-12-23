#!/bin/bash
# Connection Pooling Benchmark

INVENTORY="benches/comparison/inventory.yml"
PLAYBOOK="benches/comparison/bench_connection_pool.yml"

echo "=== Connection Pooling Benchmark ==="
echo "Playbook: 10 tasks x 5 hosts = 50 commands"
echo ""

# Warmup
echo "Warmup..."
./target/release/rustible run $PLAYBOOK -i $INVENTORY >/dev/null 2>&1
ansible-playbook -i $INVENTORY $PLAYBOOK >/dev/null 2>&1

echo ""
echo "=== RUSTIBLE (3 runs) ==="
for i in 1 2 3; do
    start=$(date +%s.%N)
    ./target/release/rustible run $PLAYBOOK -i $INVENTORY >/dev/null 2>&1
    end=$(date +%s.%N)
    elapsed=$(echo "$end - $start" | bc)
    echo "Run $i: ${elapsed}s"
done

echo ""
echo "=== ANSIBLE (3 runs) ==="
for i in 1 2 3; do
    start=$(date +%s.%N)
    ansible-playbook -i $INVENTORY $PLAYBOOK >/dev/null 2>&1
    end=$(date +%s.%N)
    elapsed=$(echo "$end - $start" | bc)
    echo "Run $i: ${elapsed}s"
done
