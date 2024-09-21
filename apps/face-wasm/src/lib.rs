// use image::{imageops::FilterType, GenericImageView, Pixel};

// extern "C" {
//     fn infer(squeezenet0_flatten0_reshape0: &[f32]) -> Vec<f32>;
// }

use image::imageops::FilterType;

pub struct DetectionResult {
    label: String,
    probability: f32,
}

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

// pub fn main(image_png: &[u8]) -> DetectionResult {
#[no_mangle]
pub extern "C" fn main(num: i32) -> i32 {
    // let buffer = image::load_from_memory(&[1])
    //     .expect("failed to load PNG file")
    //     .resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    // let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
    //     let pixel = buffer.get_pixel(i as u32, j as u32);
    //     let channels = pixel.channels();

    //     // range [0, 255] -> range [0, 1]
    //     (channels[c] as f32) / 255.0
    // });

    // let input = array
    //     .as_slice()
    //     .expect("failed to convert array into a slice");

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

    num + 42
}
