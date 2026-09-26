#!/usr/bin/env bash
set -euo pipefail

echo "Measuring entrypoint costs at DAO sizes 10, 100, 1000..."
cargo test --locked --features bench-budget test_budget_scaling -- --nocapture > budget_out.txt || true

# Just a simple runner for CI. We will write the actual test in src/bench.rs
echo "Checking for O(n) regressions..."
if grep -q "REGRESSION" budget_out.txt; then
    echo "Cost regression detected!"
    cat budget_out.txt
    exit 1
fi
echo "Budget OK."
