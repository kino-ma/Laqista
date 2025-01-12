#!/usr/bin/env bash

set -euo pipefail

if git diff --compact-summary | grep -Ev 'files? changed'; then
    echo 'file changes are left. refusing archive' >&2
    exit 1
fi

datetime="$(date '+%Y-%m-%d_%H-%M-%S')"
hash="$(git show HEAD --pretty=format:%h --no-patch)"

this_dir="$(dirname $0)"
out_file="data/benchmark-results/curl_${datetime}_${hash}.csv"

(
echo 'time_total,time_namelookup,time_connect,time_appconnect,time_pretransfer,time_redirect,time_starttransfer'

for _i in {1..5}; do
    curl -w "@$this_dir/curl-format.txt" -o /dev/null -s "$@" | tee 
    sleep 1
done
) | tee "$out_file"

echo
echo 'wrote output to:'
echo "$out_file"