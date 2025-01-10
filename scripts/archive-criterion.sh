#!/usr/bin/env bash

set -xeuo pipefail

if git diff --compact-summary | grep -Ev 'files? changed'; then
    echo 'file changes are left. refusing archive' >&2
    exit 1
fi

datetime="$(date '+%Y-%m-%d_%H-%M-%S')"
hash="$(git show HEAD --pretty=format:%h --no-patch)"

case "$(uname)" in
    "Darwin")
        host=mac
        ;;
    "Linux")
        host="$(hostname)"
        ;;
    *)
        host='unknown'
esac


(
cd "$(git rev-parse --show-toplevel)"
cp -R 'target/criterion' "data/benchmark-results/${host}_${datetime}_${hash}"
)


for f in k6/*.html; do
    mv "$f" "data/benchmark-results/k6_${f%.html}_${datetime}_${hash}"
done


if [[ ${1:-x} == '--clean' ]]; then
    rm -rf 'target/criterion'
    rm -rf k6/*.html
fi
