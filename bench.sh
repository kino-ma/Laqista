#!/bin/bash

set -euo pipefail

output="./logs/$(date '+%Y-%m-%dT%H:%M:%S%z')"

bench() {
    cargo bench write | tee "$output"
}

bench

ln -fs "$(basename $output)" ./logs/latest.log
