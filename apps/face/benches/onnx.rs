use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use face::proto::{InferReply, InferRequest};
use image::{imageops::FilterType, GenericImageView, Pixel};
use laqista_core::{session::Session, tensor::AsInputs};
use tokio::{runtime::Runtime, sync::Mutex};

static JPEG: &'static [u8] = include_bytes!("../../../data/pelican.jpeg");
static ONNX: &'static [u8] = include_bytes!("../../../data/models/opt-squeeze.onnx");

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

pub fn bench_onnx(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let session = runtime.block_on(async { Session::from_bytes(ONNX).await.unwrap() });

    let req = setup_input();
    let data = req.as_inputs();

    let mut group = c.benchmark_group("Onnx inference");

    group.bench_with_input(
        BenchmarkId::new("onnx inference", "pelican"),
        &Arc::new(Mutex::new(session)),
        |b, session| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut session = session.lock().await;
                let resp = session.detect(&data).await.unwrap();
                let output = InferReply::try_from(resp).unwrap();
                let mut v = output
                    .squeezenet0_flatten0_reshape0
                    .into_iter()
                    .zip(0..)
                    .collect::<Vec<_>>();
                v.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

                let expected_idx = 144;
                let top5 = &v[..5];
                let is_top5 = top5.iter().any(|(_, i)| *i == expected_idx);

                assert!(is_top5);
            })
        },
    );
}

fn setup_input() -> InferRequest {
    let img = image::load_from_memory(&JPEG).expect("Failed to load image");

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    InferRequest {
        data: array.into_raw_vec(),
    }
}

criterion_group!(benches, bench_onnx);
criterion_main!(benches);
