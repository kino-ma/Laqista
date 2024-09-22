// use image::{imageops::FilterType, GenericImageView, Pixel};

// extern "C" {
//     fn infer(squeezenet0_flatten0_reshape0: &[f32]) -> Vec<f32>;
// }

use core::slice;

use image::{imageops::FilterType, GenericImageView, Pixel};

extern "C" {
    // fn print(ptr: u32, len: u32);
}

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

    let img = image::load_from_memory(buffer);

    let msg_start = len as usize + 1;

    let msg = match img {
        Ok(_img) => "ok".to_owned(),
        Err(e) => {
            format!("ERR: Failed to load image: {e}")
        }
    };

    write_str(len as usize + 1, &msg);
    (msg_start as i64) << 32 | msg.len() as i64

    // let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    // let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
    //     let pixel = img.get_pixel(i as u32, j as u32);
    //     let channels = pixel.channels();

    //     // range [0, 255] -> range [0, 1]
    //     (channels[c] as f32) / 255.0
    // });

    // let input = array
    //     .as_slice()
    //     .expect("failed to convert array into a slice");

    // (input.first().unwrap() + input.last().unwrap()) as _

    // let outputs = unsafe { infer(input) };

    // let probabilities: Vec<f32> = outputs.try_into().unwrap();
    // let mut probabilities = probabilities.iter().enumerate().collect::<Vec<_>>();
    // probabilities.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    // todo!("See comment below");
    // let class_labels = get_imagenet_labels();
    //
    // for i in 0..10 {
    //     println!(
    //         "Infered result: {} of class: {}",
    //         class_labels[probabilities[i].0], probabilities[i].0
    //     );
    //     println!("details: {:?}", probabilities[i]);
    // }
}

fn write_str(offset: usize, data: &str) {
    #[cfg(target_family = "wasm")]
    grow_to(data.len(), data.len());

    let ptr: *mut u8 = offset as _;
    unsafe {
        std::ptr::copy(data.as_ptr(), ptr, data.len());
    }
}

#[cfg(target_family = "wasm")]
const PAGE_SIZE: usize = 65536;

#[cfg(target_family = "wasm")]
fn grow_to(tail_idx: usize, data_len: usize) -> usize {
    use core::arch;
    let current_size = arch::wasm32::memory_size(0);
    let cap = current_size * PAGE_SIZE;
    assert!(tail_idx <= cap);

    let start = tail_idx + 1;
    let available = cap - start;
    let missing = data_len - available;
    if missing > 0 {
        let to_grow = missing / PAGE_SIZE + 1;
        arch::wasm32::memory_grow(0, to_grow);
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

        let input = array
            .as_slice()
            .expect("failed to convert array into a slice");
    }
}
