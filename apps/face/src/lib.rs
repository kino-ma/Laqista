pub mod proto;
pub mod server;

use std::path::{Path, PathBuf};

use image::{imageops::FilterType, ImageBuffer, Pixel, Rgb};

pub fn open_default() -> ndarray::ArrayBase<ndarray::OwnedRepr<f32>, ndarray::Dim<[usize; 4]>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join("data/pelican.jpeg");

    open(path)
}

pub fn open(
    path: PathBuf,
) -> ndarray::ArrayBase<ndarray::OwnedRepr<f32>, ndarray::Dim<[usize; 4]>> {
    // let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> = image::open(path)
    //     .unwrap()
    //     .resize_exact(28, 28, FilterType::Nearest)
    //     .to_rgb8();
    let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> = image::open(path)
        .unwrap()
        .resize_to_fill(224, 224, FilterType::Nearest)
        .to_rgb8();

    // Python:
    // # image[y, x, RGB]
    // # x==0 --> left
    // # y==0 --> top

    // See https://github.com/onnx/models/blob/master/vision/classification/imagenet_inference.ipynb
    // for pre-processing image.
    // WARNING: Note order of declaration of arguments: (_,c,j,i)
    ndarray::Array::from_shape_fn((1, 3, 224, 224), |(_, c, j, i)| {
        let pixel = image_buffer.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    })
}

pub fn model_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("opt-squeeze.onnx")
}
