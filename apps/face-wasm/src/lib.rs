// use image::{imageops::FilterType, GenericImageView, Pixel};

// extern "C" {
//     fn infer(squeezenet0_flatten0_reshape0: &[f32]) -> Vec<f32>;
// }

use core::slice;

use face_proto::DetectionRequest;
use host_proto::{Continuation, HostCall};
use image::{imageops::FilterType, GenericImageView, Pixel};
use prost::Message;

mod face_proto {
    tonic::include_proto!("face");
}
mod host_proto {
    tonic::include_proto!("host");
}

extern "C" {}

pub struct DetectionResult {
    _label: String,
    _probability: f32,
}

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

// pub fn main(image_png: &[u8]) -> DetectionResult {
#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn main(ptr: i32, len: i32) -> i64 {
    let buffer: &[u8] = unsafe { slice::from_raw_parts(ptr as _, len as _) };

    let msg_start = len + 1;

    let request: DetectionRequest = match Message::decode(buffer) {
        Ok(req) => req,
        Err(e) => {
            let msg = format!("ERR: Failed to decode request: {e}");
            write_str(len as isize + 1, &msg);
            let ret = join(msg_start, msg.len() as _);
            return ret;
        }
    };

    let img = image::load_from_memory(&request.image_png);

    if let Err(e) = &img {
        let msg = format!("ERR: Failed to load image: {e}");
        write_str(len as isize + 1, &msg);
        let ret = join(msg_start, msg.len() as _);
        return ret;
    }
    let img = img.unwrap();

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    let _input = match array.as_slice() {
        Some(slic) => slic,
        None => {
            let msg = format!("ERR: Failed to get array slice");
            write_str(len as isize + 1, &msg);
            let ret = join(msg_start, msg.len() as _);
            return ret;
        }
    };

    let cont = Continuation {
        name: "Next!".to_owned(),
    };
    let call = HostCall {
        name: "INVOKING!!".to_owned(),
        cont: Some(cont),
    };
    let buffer = call.encode_to_vec();

    write_bytes(msg_start as _, &buffer)

    // let outputs = unsafe { infer(input.as_ptr() as _, input.len() as _) };
    // let (ptr, len) = split(outputs);
    // let data: &[f32] = unsafe { slice::from_raw_parts(ptr as _, len as _) };

    // let probabilities: Vec<f32> = outputs.try_into().unwrap();
    // let mut probabilities = probabilities.iter().enumerate().collect::<Vec<_>>();
    // probabilities.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    // let class_labels = get_imagenet_labels();

    // let mut msg = String::new();

    // for i in 0..10 {
    //     msg.push_str(&format!(
    //         "Infered result: {} of class: {}",
    //         class_labels[probabilities[i].0], probabilities[i].0
    //     ));
    //     msg.push_str(&format!("details: {:?}", probabilities[i]));
    // }

    // write_str(len as isize + 1, &msg);
    // let ret = join(msg_start, msg.len() as _);
    // return ret;
}

fn write_str(offset: isize, data: &str) -> i64 {
    write_bytes(offset, data.as_bytes())
}

fn write_bytes(offset: isize, data: &[u8]) -> i64 {
    #[cfg(target_family = "wasm")]
    grow_to(offset - 1, data.len() as _);

    let ptr: *mut u8 = offset as _;
    let len = data.len();
    unsafe {
        std::ptr::copy(data.as_ptr(), ptr, len);
    }

    join(offset as _, len as _)
}

fn join(upper: i32, lower: i32) -> i64 {
    (upper as i64) << 32 | lower as i64
}
fn split(joined: i64) -> (i32, i32) {
    let upper = (joined >> 32) as i32;
    let lower = (joined & 0xffff_ffff) as i32;

    (upper, lower)
}

#[cfg(target_family = "wasm")]
const PAGE_SIZE: isize = 65536;

#[cfg(target_family = "wasm")]
fn grow_to(tail_idx: isize, data_len: isize) -> isize {
    use core::arch;
    let current_size = arch::wasm32::memory_size(0) as isize;
    let cap = current_size * PAGE_SIZE;
    assert!(tail_idx <= cap);

    let start = tail_idx + 1;
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
