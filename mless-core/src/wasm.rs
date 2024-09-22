use std::error::Error;

use wasmer::{imports, Cranelift, Instance, Memory, MemoryType, Module, Store};

pub struct WasmRunner {
    pub store: Store,
    pub module: Module,
    pub memory: Memory,
    pub instance: Instance,
}

impl WasmRunner {
    pub fn compile(wasm: &[u8]) -> Result<Self, Box<dyn Error>> {
        let compiler = Cranelift::default();

        let mut store = Store::new(compiler);
        let module = Module::new(&store, wasm)?;

        let memory = Memory::new(&mut store, MemoryType::new(21, None, false))?;

        let import_object = imports! {
            "env" => {
                "memory" => memory.clone(),
            }
        };

        let instance = Instance::new(&mut store, &module, &import_object)?;

        Ok(Self {
            store,
            module,
            memory,
            instance,
        })
    }
}
