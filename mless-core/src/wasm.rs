use std::error::Error;

use prost::Message;
use wasmer::{imports, Cranelift, Instance, Memory, MemoryType, Module, Store, Value};

pub struct WasmRunner {
    pub store: Store,
    pub module: Module,
    pub memory: Memory,
    pub instance: Instance,

    pub ptr: WasmPointer,
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
            ptr: WasmPointer::new(0, 0),
        })
    }

    pub fn call(&mut self, name: &str, params: &[Value]) -> Result<Box<[Value]>, Box<dyn Error>> {
        let func = self.instance.exports.get_function(name)?;

        let values = func.call(&mut self.store, params)?;
        Ok(values)
    }

    pub fn write_message<M: Message>(&mut self, msg: M) -> Result<(i32, i32), Box<dyn Error>> {
        let mut buf: Vec<u8> = Vec::new();
        msg.encode(&mut buf)?;

        let out = self.write_bytes(&buf)?;

        Ok(out)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(i32, i32), wasmer::MemoryAccessError> {
        let start = self.ptr.start;
        let len = bytes.len();

        let view = self.memory.view(&mut self.store);
        view.write(start as _, bytes)?;

        self.ptr.consume(len as i32);
        Ok(self.ptr.into())
    }
}

#[derive(Clone, Copy)]
pub struct WasmPointer {
    pub start: i32,
    pub len: i32,
}

impl WasmPointer {
    pub fn new(start: i32, len: i32) -> Self {
        Self { start, len }
    }

    pub fn consume<L: Into<i32>>(&mut self, consumed: L) -> i32 {
        self.len += consumed.into();
        self.len
    }
}

impl Into<(i32, i32)> for WasmPointer {
    fn into(self) -> (i32, i32) {
        (self.start, self.len)
    }
}
