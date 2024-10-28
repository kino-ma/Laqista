use std::error::Error;

use prost::Message;
use wasmer::{imports, Cranelift, Instance, Memory, MemoryType, Module, Store, Value};

use crate::proto::host::{HostCall, InvokeResult, MemorySlice};

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
            ptr: (0, 0).into(),
        })
    }

    pub fn call<M: Message + Default>(
        &mut self,
        name: &str,
        params: &[Value],
    ) -> Result<ExecState<M>, Box<dyn Error>> {
        let func = self.instance.exports.get_function(name)?;

        let values = func.call(&mut self.store, params)?;
        let ptr = values[0].unwrap_i64().into();
        let result: InvokeResult = self.read_message(ptr)?;
        let result = result.result.ok_or("Failed to read invoke result")?;

        {
            use crate::proto::host::invoke_result::Result as R;
            match result {
                R::Finished(m) => {
                    let ptr = m.ptr.ok_or("Failed to read finished result")?;
                    let out = self.read_message(ptr.into())?;
                    Ok(ExecState::Finished(out))
                }
                R::HostCall(call) => Ok(ExecState::Continue(call)),
                R::Error(e) => Err(format!("Error in WASM function: {}", e.message))?,
            }
        }
    }

    pub fn write_message<M: Message>(&mut self, msg: M) -> Result<WasmPointer, Box<dyn Error>> {
        let buf = msg.encode_to_vec();

        let out = self.write_bytes(&buf)?;

        Ok(out)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<WasmPointer, wasmer::MemoryAccessError> {
        let start = self.ptr.next();
        let len = bytes.len();

        let view = self.memory.view(&mut self.store);
        view.write(start as _, bytes)?;

        self.ptr.consume(len as i32);

        Ok(WasmPointer {
            start,
            len: len as _,
        })
    }

    pub fn read_message<M: Message + Default>(
        &mut self,
        ptr: WasmPointer,
    ) -> Result<M, Box<dyn Error>> {
        let buffer = self.read_bytes(ptr)?;

        let msg = Message::decode(&buffer[..])?;
        Ok(msg)
    }

    pub fn read_bytes(&mut self, ptr: WasmPointer) -> Result<Vec<u8>, wasmer::MemoryAccessError> {
        let mut buffer = vec![0; ptr.len as usize];

        let view = self.memory.view(&mut self.store);
        view.read(ptr.start as _, &mut buffer)?;
        self.ptr.join(ptr);

        Ok(buffer)
    }
}

#[derive(Debug)]
pub enum ExecState<M> {
    Finished(M),
    Continue(HostCall),
}

impl<M: Message + Default> ExecState<M> {
    pub fn unwrap_finished(self) -> M {
        if let Self::Finished(m) = self {
            m
        } else {
            panic!(
                "`ExecState::unwrap_finished()` on non-Finished value: {:?}",
                self
            )
        }
    }
}
impl ExecState<()> {
    pub fn unwrap_continue(self) -> HostCall {
        if let Self::Continue(c) = self {
            c
        } else {
            panic!(
                "`ExecState::unwrap_continue()` on non-Continue value: {:?}",
                self
            )
        }
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

    pub fn next(&self) -> i32 {
        self.last() + 1
    }

    pub fn last(&self) -> i32 {
        self.start + self.len - 1
    }

    pub fn join(&mut self, other: Self) {
        if self.last() >= other.last() {
            return;
        }

        self.len = other.last() - self.last();
    }
}

impl Into<(i32, i32)> for WasmPointer {
    fn into(self) -> (i32, i32) {
        (self.start, self.len)
    }
}

impl From<(i32, i32)> for WasmPointer {
    fn from((start, len): (i32, i32)) -> Self {
        Self::new(start, len)
    }
}

impl From<i64> for WasmPointer {
    fn from(value: i64) -> Self {
        let start = value >> 32;
        let len = value & 0xffff_ffff;
        Self::new(start as _, len as _)
    }
}

impl From<MemorySlice> for WasmPointer {
    fn from(value: MemorySlice) -> Self {
        Self::new(value.start as _, value.len as _)
    }
}

impl Into<[Value; 2]> for WasmPointer {
    fn into(self) -> [Value; 2] {
        [Value::I32(self.start), Value::I32(self.len)]
    }
}
