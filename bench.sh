#!/bin/bash

set -euo pipefail

output="./logs/$(date '+%Y-%m-%dT%H:%M:%S%z')"

bench() {
    cargo bench -- \
        -Z unstable-options \
        --format=json |
        tee "$output"
}

bench

ln -fs "$(basename $output)" ./logs/latest.log

echo
echo '=== result ==='
jq <logs/latest.log 'select(.type == "bench") | .name,.median,"± " + (.deviation|tostring)' | paste - - -
