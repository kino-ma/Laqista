mod interface;
mod memory;

// use image::{imageops::FilterType, GenericImageView, Pixel};

// extern "C" {
//     fn infer(squeezenet0_flatten0_reshape0: &[f32]) -> Vec<f32>;
// }

use face_proto::{DetectionReply, DetectionRequest, InferReply, InferRequest};
use image::{imageops::FilterType, GenericImageView, Pixel};
use interface::{exit_error, exit_finish, exit_hostcall, setup};
use memory::{read_message, Memory};
use prost::Message;

mod face_proto {
    tonic::include_proto!("face");
}
mod host_proto {
    tonic::include_proto!("host");
}

#[cfg(feature = "bench")]
pub use memory::read_detection_request;

extern "C" {}

static LABELS: &'static str = include_str!("../../../data/models/resnet-labels.txt");
fn get_labels() -> Vec<String> {
    LABELS.lines().map(|l| l.to_owned()).collect()
}

pub struct DetectionResult {
    _label: String,
    _probability: f32,
}

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn main(ptr: i32, len: i32) -> i64 {
    let mut memory = setup(ptr, len);

    match run(&mut memory) {
        Ok(req) => exit_hostcall(memory, "infer", "get_probability", req),
        Err(e) => exit_error(memory, &e, ()),
    }
}

fn run(memory: &mut Memory) -> Result<InferRequest, String> {
    let buffer = memory.get_whole();
    let request: DetectionRequest = read_message(buffer)?;

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

    let req = InferRequest {
        data: input.to_vec(),
    };

    Ok(req)
}

#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn get_probability(ptr: i32, len: i32) -> i64 {
    let mut memory = setup(ptr, len);

    match get_prob_run(&mut memory) {
        Ok(resp) => exit_finish(memory, resp),
        Err(e) => exit_error(memory, &e, ()),
    }
}

fn get_prob_run(memory: &mut Memory) -> Result<DetectionReply, String> {
    let buffer = memory.get_whole();
    let resp: InferReply =
        Message::decode(&buffer[..]).map_err(|e| format!("Failed to parse InferReply: {e}"))?;

    let probabilities = resp.squeezenet0_flatten0_reshape0;

    // return Err("2".to_owned());
    let mut probabilities = probabilities.iter().enumerate().collect::<Vec<_>>();
    // return Err("2.1".to_owned());
    probabilities
        .sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    // unreachable
    // return Err("2.2".to_owned());

    let class_labels = get_labels();

    // return Err("3".to_owned());
    let (i, prob) = probabilities[0];
    // return Err("3.0.1".to_owned());
    let label = class_labels
        .get(i)
        .ok_or(format!(
            "Insufficient length: want {i}, got {}",
            class_labels.len()
        ))?
        .clone();

    // unreachable
    // return Err("3.1".to_owned());
    Ok(DetectionReply {
        label,
        probability: *prob,
    })
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
