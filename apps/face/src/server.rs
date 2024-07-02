use std::path::{Path, PathBuf};

use image::{imageops::FilterType, ImageBuffer, Pixel as _, Rgb};
use tonic::{Request, Response, Status};

use crate::{
    proto::{
        detector_server::Detector, DetectReply, DetectRequest, DetectVideoReply, DetectVideoRequest,
    },
    DetectionInputs, DetectionOutputs, FaceDetector,
};

pub struct DetectServer {}

impl DetectServer {}

#[tonic::async_trait]
impl Detector for DetectServer {
    async fn detect_video(
        &self,
        _request: Request<DetectVideoRequest>,
    ) -> Result<Response<DetectVideoReply>, Status> {
        unimplemented!("MP4 is not supported");
    }

    async fn detect_face(
        &self,
        _request: Request<DetectRequest>,
    ) -> Result<Response<DetectReply>, Status> {
        let frame = open_default();
        let mut detector = FaceDetector::create_default().await.map_err(|e| {
            Status::aborted(format!(
                "failed to create detector session: {}",
                Status::aborted(e.to_string())
            ))
        })?;

        let input = frame
            .as_slice()
            .expect("failed to convert input image to slice");
        let input = DetectionInputs { input };

        let DetectionOutputs { boxes, .. } = detector
            .detect(input)
            .await
            .map_err(|e| Status::aborted(e.to_string()))?;

        let resp = DetectReply {
            total_detected: boxes.len() as _,
        };
        Ok(Response::new(resp))
    }
}

fn open_default() -> ndarray::ArrayBase<ndarray::OwnedRepr<f32>, ndarray::Dim<[usize; 4]>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join("data/still-people.png");

    open(path)
}

fn open(path: PathBuf) -> ndarray::ArrayBase<ndarray::OwnedRepr<f32>, ndarray::Dim<[usize; 4]>> {
    let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> = image::open(path)
        .unwrap()
        .resize_exact(28, 28, FilterType::Nearest)
        .to_rgb8();

    // Python:
    // # image[y, x, RGB]
    // # x==0 --> left
    // # y==0 --> top

    // See https://github.com/onnx/models/blob/master/vision/classification/imagenet_inference.ipynb
    // for pre-processing image.
    // WARNING: Note order of declaration of arguments: (_,c,j,i)
    ndarray::Array::from_shape_fn((1, 1, 28, 28), |(_, c, j, i)| {
        let pixel = image_buffer.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    })
}
