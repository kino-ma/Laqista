#!/usr/bin/env bash

set -xeuo pipefail

datetime="$(date '+%Y-%m-%d_%H-%M-%S')"
hash="$(git show HEAD --pretty=format:%h --no-patch)"

(
cd "$(git rev-parse --show-toplevel)"
for f in k6/*.html; do
    title="$(basename ${f%.html})"
    mv "$f" "data/benchmark-results/k6_${title}_${datetime}_${hash}.html"
done
)
