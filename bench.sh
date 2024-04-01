#!/bin/bash

set -euo pipefail

lookup() {
    id=$1

    grpcurl \
        -plaintext \
        -import-path ./proto \
        -proto mless.proto \
        -d "{\"deployment_id\":\"$id\"}" \
        '127.0.0.1:50051' \
        mless.Scheduler/Lookup | jq --raw-output .server.addr | sed -E 's_http://(.*)_\1_'
}

call() {
    address=$1

    grpcurl \
        -plaintext \
        -import-path ./proto \
        -proto app.proto \
        -d "{}" \
        "$address" \
        app.Greeter/SayHello
}

deploy() {
    grpcurl \
        -plaintext \
        -import-path ./proto \
        -proto mless.proto \
        -d '{"source":"https://github.com/kino-ma/MLess","authoritative":true}' \
        '127.0.0.1:50051' \
        mless.Scheduler/Deploy | jq --raw-output .deployment.id
}

bench() {
    id=$1

    for i in {1..1000}; do
        addr=$(lookup "$id")
        call "$addr" >/dev/null
        echo -n .
    done >&2
    echo >&2
}

bench_baseline() {
    addr=$1

    for i in {1..1000}; do
        call "$addr" >/dev/null
        echo -n .
    done >&2
    echo >&2
}

id=$(deploy)

addr=$(lookup "$id")
echo addr $addr

for i in {1..10}; do
    echo 'bench (our method)'
    time bench "$id"
    echo 'bench (baseline)'
    time bench_baseline "$addr"
done
