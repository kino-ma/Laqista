// use image::{imageops::FilterType, GenericImageView, Pixel};

// extern "C" {
//     fn infer(squeezenet0_flatten0_reshape0: &[f32]) -> Vec<f32>;
// }

use core::slice;

use face_proto::{DetectionReply, DetectionRequest, InferRequest};
use host_proto::{Continuation, HostCall, MemorySlice};
use image::{imageops::FilterType, GenericImageView, Pixel};
use prost::Message;

mod face_proto {
    tonic::include_proto!("face");
}
mod host_proto {
    tonic::include_proto!("host");
}

extern "C" {}

static LABELS: &'static str = include_str!("../../../data/models/resnet-labels.txt");
fn get_labels() -> Vec<String> {
    LABELS.lines().map(|l| l.to_owned()).collect()
}

pub struct DetectionResult {
    _label: String,
    _probability: f32,
}

#[cfg(target_family = "wasm")]
const PAGE_SIZE: usize = 65536;

struct Memory {
    head: *const u8,
    last: *const u8,
}

impl Memory {
    pub fn new<P: Into<*const u8>, L: Into<*const u8>>(ptr: P, last: L) -> Self {
        let head = ptr.into();
        let last = last.into();

        Self { head, last }
    }

    pub fn with_used_len<P: Into<*const u8>, L: Into<i64>>(ptr: P, len: L) -> Self {
        let ptr: *const u8 = ptr.into();
        let len: i64 = len.into();
        let last = unsafe { ptr.add(len as _) };

        Self::new(ptr, last)
    }

    pub fn len(&self) -> usize {
        let offset = unsafe { self.last.offset_from(self.head) };
        offset as _
    }

    pub unsafe fn get_slice<L: Into<usize>, T>(&self, start: *const T, len: L) -> &[T] {
        slice::from_raw_parts(start, len.into())
    }

    pub fn get_whole<T>(&self) -> &[T] {
        unsafe { self.get_slice(self.head as _, self.len()) }
    }

    pub fn write_str(&mut self, data: &str) -> &str {
        let bytes = self.write_bytes(data.as_bytes());
        core::str::from_utf8(bytes).unwrap()
    }

    pub fn write_bytes<'a, 'b>(&'a mut self, data: &'b [u8]) -> &'a [u8] {
        #[cfg(target_family = "wasm")]
        self.grow_to(data.len());

        let start: *mut u8 = unsafe { self.last.add(1).cast_mut() };
        let len = data.len();
        unsafe {
            std::ptr::copy(data.as_ptr(), start, len);
            self.last = self.last.add(len);
            self.get_slice(start, len)
        }
    }

    #[cfg(target_family = "wasm")]
    fn grow_to(&self, data_len: usize) -> usize {
        use core::arch;
        let current_size = arch::wasm32::memory_size(0) as usize;
        let cap = current_size * PAGE_SIZE;

        let len = self.len();
        assert!(len <= cap);

        let start = len + 1;
        let available = cap - start;
        let missing = data_len - available;
        if missing > 0 {
            let to_grow = missing / PAGE_SIZE + 1;
            arch::wasm32::memory_grow(0, to_grow as _);
            to_grow
        } else {
            0
        }
    }
}

fn slice_to_i64(s: &[u8]) -> i64 {
    let ptr = (s.as_ptr() as i64) << 32;
    let len = s.len() as i64;

    ptr | len
}

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn main(ptr: i32, len: i32) -> i64 {
    let mut memory = Memory::with_used_len(ptr as *const u8, len);

    let out_ptr = match run(&mut memory) {
        Ok(ret) => ret,
        Err(e) => {
            let s = memory.write_str(&e);
            s.as_bytes()
        }
    };

    slice_to_i64(out_ptr)
}

fn run(memory: &mut Memory) -> Result<&[u8], String> {
    let buffer: &[u8] = memory.get_whole();

    let request: DetectionRequest =
        Message::decode(buffer).map_err(|e| format!("ERR: Failed to decode request: {e}"))?;

    let img = image::load_from_memory(&request.image_png)
        .map_err(|e| format!("ERR: Failed to load image: {e}"))?;

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    let input = array.as_slice().ok_or("ERR: Failed to get array slice")?;

    let cont = Continuation {
        name: "get_probability".to_owned(),
    };

    let req = InferRequest {
        data: input.to_vec(),
    };
    let req_bytes = req.encode_to_vec();
    let req_slice = memory.write_bytes(&req_bytes);

    let params = MemorySlice {
        start: req_slice.as_ptr() as _,
        len: req_bytes.len() as _,
    };
    let call = HostCall {
        name: "infer".to_owned(),
        cont: Some(cont),
        parameters: Some(params),
    };
    let buffer = call.encode_to_vec();

    let ret = memory.write_bytes(&buffer);

    Ok(ret)
}

#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn get_probability(ptr: i32, len: i32) -> i64 {
    let mut memory = Memory::with_used_len(ptr as *const u8, len);
    let data = memory.get_whole();

    let probabilities: Vec<f32> = data.try_into().unwrap();
    let mut probabilities = probabilities.iter().enumerate().collect::<Vec<_>>();
    probabilities.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    let class_labels = get_labels();

    let (i, prob) = probabilities[0];
    let label = class_labels[i].clone();

    let reply = DetectionReply {
        label,
        probability: *prob,
    };

    let out = reply.encode_to_vec();
    let slic = memory.write_bytes(&out);

    slice_to_i64(slic)
}

#[cfg(test)]
mod test {
    use image::{imageops::FilterType, GenericImageView};

    use super::*;

    static JPEG: &'static [u8] = include_bytes!("../../../data/pelican.jpeg");

    #[test]
    fn test_image_load() {
        let img = image::load_from_memory(JPEG).unwrap();

        let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

        let array = ndarray::Array::from_shape_fn(
            (1, 3, super::IMAGE_WIDTH, IMAGE_HEIGHT),
            |(_, c, j, i)| {
                let pixel = img.get_pixel(i as u32, j as u32);
                let channels = pixel.channels();

                // range [0, 255] -> range [0, 1]
                (channels[c] as f32) / 255.0
            },
        );

        let _input = array
            .as_slice()
            .expect("failed to convert array into a slice");
    }
}
