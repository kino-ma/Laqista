#!/usr/bin/env bash

set -euxo pipefail

sudo killall mless

rm -f target/wasm32-unknown-unknown/release/face_wasm.wasm

(
    cd apps/face-wasm
    sudo -E cargo build --release --target wasm32-unknown-unknown
)

sudo -E cargo build
sudo -E cargo run server start &
sleep 1 && sudo -E cargo run server start --server http://127.0.0.1:50051 --listen 127.0.0.1:50052
