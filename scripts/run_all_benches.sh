#!/bin/bash

# Runs all benchmarks for all pallets, for each of the runtimes specified below
# Should be run on a reference machine to gain accurate benchmarks
# current reference machine: digital ocean droplet

runtimes=(
  selendra
  cardamom
)

for runtime in "${runtimes[@]}"; do
  "$(dirname "$0")/run_benches_for_runtime.sh" "$runtime"
done
