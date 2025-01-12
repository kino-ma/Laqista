use std::time::Duration;

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, Criterion};
use face::server::FaceServer;
use tokio::runtime;
use wasmer::{wat2wasm, IntoBytes};

static WASM: &'static [u8] =
    include_bytes!("../target/wasm32-unknown-unknown/release/face_wasm.wasm");
static ONNX: &'static [u8] = include_bytes!("../data/models/opt-squeeze.onnx");

pub fn bench_server_instantiate(c: &mut Criterion) {
    let wasm_bytes = wat2wasm(
        r#"
(module
  (type $sum_t (func (param i32 i32) (result i32)))
  (func $sum_f (type $sum_t) (param $x i32) (param $y i32) (result i32)
    local.get $x
    local.get $y
    i32.add)
  (export "sum" (func $sum_f))
  (export "main" (func $sum_f)))
"#
        .as_bytes(),
    )
    .unwrap();

    let onnx = Bytes::from_static(ONNX);
    let wasm_simple = Bytes::from(wasm_bytes.into_bytes());
    let wasm_face = Bytes::from_static(WASM);

    let mut group = c.benchmark_group("App instantiate");

    group.sampling_mode(criterion::SamplingMode::Flat);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    group.bench_with_input("simple app", &wasm_simple, |b, wasm| {
        let single_thread_rt = runtime::Builder::new_current_thread().build().unwrap();
        b.to_async(single_thread_rt)
            .iter(|| async { instantiate(onnx.clone(), wasm.clone()).await })
    });

    group.bench_with_input("face app", &wasm_face, |b, wasm| {
        // Run in single thread to avoid GPU allocation error
        let single_thread_rt = runtime::Builder::new_current_thread().build().unwrap();
        b.to_async(single_thread_rt)
            .iter(|| async { instantiate(onnx.clone(), wasm.clone()).await });
        // sleep(Duration::from_millis(1000));
    });
}

async fn instantiate(onnx: Bytes, wasm: Bytes) {
    FaceServer::create(onnx, wasm).await.unwrap();
}

criterion_group!(benches, bench_server_instantiate);
criterion_main!(benches);
