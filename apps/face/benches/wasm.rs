use criterion::{criterion_group, criterion_main, Criterion};
use face::proto::DetectionRequest;
use mless_core::wasm::WasmRunner;
use wasmer::{imports, wat2wasm, Cranelift, Instance, Module, Store, Value};

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
  (export "sum" (func $sum_f)))
"#
        .as_bytes(),
    )
    .unwrap();

    let mut group = c.benchmark_group("Wasm instantiate");
    group.bench_with_input("wasm instantiate", &wasm_bytes, |b, i| {
        b.iter(|| instantiate(i))
    });
}

fn instantiate(wasm_bytes: &[u8]) {
    let compiler = Cranelift::default();

    let mut store = Store::new(compiler);
    let module = Module::new(&store, wasm_bytes).unwrap();

    let import_object = imports! {};
    let instance = Instance::new(&mut store, &module, &import_object).unwrap();

    let main = instance.exports.get_function("sum").unwrap();

    let params = &[Value::I32(1), Value::I32(41)];

    let out = main.call(&mut store, params).unwrap();
    assert_eq!(out[0].i32().unwrap(), 42);
}

struct WriteInput<'a>(&'a [u8], DetectionRequest);
struct ReadInput(DetectionRequest);

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

    let input_read_heavy = ReadInput(request_heavy);

    group.bench_with_input("wasm write memory heavy", &input_read_heavy, |b, i| {
        b.iter(|| read_in_wasm(i))
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
    assert_eq!(output, format!("{}", ptr.len));
}

criterion_group!(benches, bench_wasm_module, bench_wasm_memory);
criterion_main!(benches);
