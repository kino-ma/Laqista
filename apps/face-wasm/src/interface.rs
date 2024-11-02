use prost::Message;

use crate::host_proto::{self, invoke_result::Result};
use crate::host_proto::{Continuation, InvokeResult, MemorySlice};
use crate::memory::Memory;

pub fn setup(ptr: i32, len: i32) -> Memory {
    Memory::with_used_len(ptr as *const u8, len)
}

fn exit(mut memory: Memory, result: Result) -> i64 {
    let invoke_result = InvokeResult {
        result: Some(result),
    };
    let buffer = invoke_result.encode_to_vec();
    let out_slice = memory.write_bytes(&buffer);

    slice_to_i64(out_slice)
}

pub fn exit_finish<M: Message + Default>(mut memory: Memory, message: M) -> i64 {
    let message_bytes = message.encode_to_vec();
    let slic = memory.write_bytes(&message_bytes);

    let result = Result::Finished(host_proto::Finished {
        ptr: wrap_memslice(slic),
    });

    exit(memory, result)
}

pub fn exit_hostcall<M: Message + Default>(
    mut memory: Memory,
    name: &str,
    cont: &str,
    parameters: M,
) -> i64 {
    let params_bytes = parameters.encode_to_vec();
    let slic = memory.write_bytes(&params_bytes);

    let result = Result::HostCall(host_proto::HostCall {
        name: name.to_owned(),
        cont: Some(Continuation {
            name: cont.to_owned(),
        }),
        parameters: wrap_memslice(slic),
    });

    exit(memory, result)
}

pub fn exit_error<M: Message + Default>(mut memory: Memory, message: &str, details: M) -> i64 {
    let details_bytes = details.encode_to_vec();
    let slic = memory.write_bytes(&details_bytes);

    let result = Result::Error(host_proto::Error {
        message: message.to_owned(),
        details: wrap_memslice(slic),
    });

    exit(memory, result)
}

pub fn slice_to_i64(s: &[u8]) -> i64 {
    let ptr = (s.as_ptr() as i64) << 32;
    let len = s.len() as i64;

    ptr | len
}

pub fn wrap_memslice(slic: &[u8]) -> Option<MemorySlice> {
    Some(MemorySlice {
        start: slic.as_ptr() as _,
        len: slic.len() as _,
    })
}
