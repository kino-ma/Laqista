use criterion::{criterion_group, criterion_main, Criterion};
use face::proto::{DetectionRequest, InferReply};
use laqista_core::wasm::WasmRunner;
use wasmer::{imports, wat2wasm, Cranelift, Instance, Memory, MemoryType, Module, Store, Value};

static JPEG: &'static [u8] = include_bytes!("../../../data/pelican.jpeg");
static WASM: &'static [u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/face_wasm.wasm");

pub fn bench_wasm_module(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("Wasm instantiate");
    group.bench_with_input("wasm instantiate", &wasm_bytes, |b, i| {
        b.iter(|| instantiate(i, false))
    });
    group.bench_with_input("wasm instantiate heavy", &WASM, |b, i| {
        b.iter(|| instantiate(i, true))
    });
    group.bench_with_input("wasm instantiate runner", &wasm_bytes, |b, i| {
        b.iter(|| WasmRunner::compile(i).unwrap())
    });
    group.bench_with_input("wasm instantiate runner heavy", &WASM, |b, i| {
        b.iter(|| WasmRunner::compile(i).unwrap())
    });

    let t_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    group.bench_with_input("wasm instantiate runner heavy async", &WASM, |b, i| {
        b.iter(|| t_runtime.block_on(async { tokio_instantiate(i).await }))
    });
}

fn instantiate(wasm_bytes: &[u8], import_memory: bool) {
    let compiler = Cranelift::default();

    let mut store = Store::new(compiler);
    let module = Module::new(&store, wasm_bytes).unwrap();

    let import_object = if import_memory {
        imports! {
            "env" => {
                "memory" => Memory::new(&mut store, MemoryType::new(21, None, false)).unwrap()
            }
        }
    } else {
        imports! {}
    };
    let instance = Instance::new(&mut store, &module, &import_object).unwrap();

    let _main = instance.exports.get_function("main").unwrap();
}

async fn tokio_instantiate(wasm_bytes: &[u8]) {
    WasmRunner::compile(wasm_bytes).unwrap();
}

struct WriteInput<'a>(&'a [u8], DetectionRequest);
struct ReadInput(DetectionRequest);
struct ContinuationInput(InferReply);

pub fn bench_wasm_memory(c: &mut Criterion) {
    let wasm_bytes = wat2wasm(
        r#"
(module
  (memory $mem 1)
  (type $sum_t (func (param i32 i32) (result i32)))
  (func $sum_f (type $sum_t) (param $x i32) (param $y i32) (result i32)
    local.get $x
    local.get $y
    i32.add)
  (export "sum" (func $sum_f))
  (export "memory" (memory $mem))
  )
"#
        .as_bytes(),
    )
    .unwrap();

    let request = DetectionRequest {
        image_png: b"Hello world".to_vec(),
    };
    let input = WriteInput(&wasm_bytes, request.clone());

    let mut group = c.benchmark_group("Wasm memory");
    group.bench_with_input("wasm write memory", &input, |b, i| b.iter(|| write(i)));

    let request_heavy = DetectionRequest {
        image_png: JPEG.to_vec(),
    };
    let input_heavy = WriteInput(&wasm_bytes, request_heavy.clone());

    group.bench_with_input("wasm write memory heavy", &input_heavy, |b, i| {
        b.iter(|| write(i))
    });

    let input_read = ReadInput(request.clone());

    group.bench_with_input("wasm read memory in wasm", &input_read, |b, i| {
        b.iter(|| read_in_wasm(i))
    });

    let input_read_heavy = ReadInput(request_heavy.clone());

    group.bench_with_input("wasm read memory heavy", &input_read_heavy, |b, i| {
        b.iter(|| read_in_wasm(i))
    });

    let input_read_image = ReadInput(request_heavy.clone());

    group.bench_with_input("wasm read memory image", &input_read_image, |b, i| {
        b.iter(|| read_image_in_wasm(i))
    });

    let input_main = ReadInput(request_heavy.clone());

    group.bench_with_input("wasm main", &input_main, |b, i| b.iter(|| main_wasm(i)));

    let input_get_prob = ContinuationInput(InferReply {
        squeezenet0_flatten0_reshape0: vec![0.0; 1000],
    });

    group.bench_with_input("wasm get probability", &input_get_prob, |b, i| {
        b.iter(|| get_prob_wasm(i))
    });
}

fn write(input: &WriteInput) {
    let WriteInput(wasm_bytes, detection_request) = input;
    let mut runner = WasmRunner::compile(wasm_bytes).unwrap();

    runner.write_message(detection_request.clone()).unwrap();
}

fn read_in_wasm(input: &ReadInput) {
    let ReadInput(detection_request) = input;
    let mut runner = WasmRunner::compile(&WASM).unwrap();

    let ptr = runner.write_message(detection_request.clone()).unwrap();

    let params: &[Value; 2] = &ptr.into();

    let exec_state = runner
        .call::<String>("read_detection_request", params)
        .unwrap();

    let output = exec_state.unwrap_finished();
    assert_eq!(output, format!("{}", detection_request.image_png.len()));
}

fn read_image_in_wasm(input: &ReadInput) {
    let ReadInput(detection_request) = input;
    let mut runner = WasmRunner::compile(&WASM).unwrap();

    let ptr = runner.write_message(detection_request.clone()).unwrap();

    let params: &[Value; 2] = &ptr.into();

    let exec_state = runner.call::<String>("read_image", params).unwrap();

    exec_state.unwrap_continue();
}

fn main_wasm(input: &ReadInput) {
    let ReadInput(detection_request) = input;
    let mut runner = WasmRunner::compile(&WASM).unwrap();

    let ptr = runner.write_message(detection_request.clone()).unwrap();

    let params: &[Value; 2] = &ptr.into();

    let exec_state = runner.call::<String>("main", params).unwrap();

    exec_state.unwrap_continue();
}

fn get_prob_wasm(input: &ContinuationInput) {
    let ContinuationInput(infer_reply) = input;
    let mut runner = WasmRunner::compile(&WASM).unwrap();

    let ptr = runner.write_message(infer_reply.clone()).unwrap();

    let params: &[Value; 2] = &ptr.into();

    let exec_state = runner.call::<String>("get_probability", params).unwrap();

    exec_state.unwrap_finished();
}

criterion_group!(benches, bench_wasm_module, bench_wasm_memory);
criterion_main!(benches);
