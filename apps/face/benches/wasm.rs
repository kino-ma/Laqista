use criterion::{criterion_group, criterion_main, Criterion};
use wasmer::{imports, wat2wasm, Cranelift, Instance, Module, Store, Value};

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

criterion_group!(benches, bench_wasm_module);
criterion_main!(benches);