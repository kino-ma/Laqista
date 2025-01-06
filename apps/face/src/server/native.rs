use std::{error::Error, sync::Arc};

use image::{imageops::FilterType, GenericImageView, Pixel};
use laqista_core::{session::Session, tensor::Inputs};
use tonic::{Request, Response, Status};
use wonnx::utils::{InputTensor, OutputTensor};

use crate::proto::{native_detector_server::NativeDetector, DetectionReply, DetectionRequest};

static LABELS: &'static str = include_str!("../../../../data/models/resnet-labels.txt");
const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

static ONNX: &'static [u8] = include_bytes!("../../../../data/models/opt-squeeze.onnx");

pub struct NativeFaceServer {
    session: Arc<Session>,
    labels: Vec<String>,
}

impl NativeFaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        let session = Arc::new(Session::from_bytes(ONNX).await?);
        let labels = LABELS.lines().map(|l| l.to_owned()).collect();

        Ok(Self { session, labels })
    }
}

#[tonic::async_trait]
impl NativeDetector for NativeFaceServer {
    async fn run_detection(
        &self,
        request: Request<DetectionRequest>,
    ) -> Result<Response<DetectionReply>, Status> {
        let img = image::load_from_memory(&request.into_inner().image_png)
            .map_err(|e| Status::aborted(format!("ERR: Failed to load image: {e}")))?;

        let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

        let array =
            ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
                let pixel = img.get_pixel(i as u32, j as u32);
                let channels = pixel.channels();

                // range [0, 255] -> range [0, 1]
                (channels[c] as f32) / 255.0
            });

        let tensor = InputTensor::F32(array.as_slice().unwrap().to_vec().into());
        let inputs = Inputs::from([("data".to_owned(), tensor)]);

        let output = self
            .session
            .detect(&inputs)
            .await
            .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;

        let OutputTensor::F32(probabilities_vec) =
            output.get("squeezenet0_flatten0_reshape0").unwrap()
        else {
            return Err(Status::aborted("not f32"));
        };
        let mut probabilities: Vec<_> = probabilities_vec.into_iter().enumerate().collect();
        probabilities
            .sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (idx, probability) = probabilities[0].clone();

        let label = self.labels.get(idx).unwrap().to_owned();

        let reply = DetectionReply {
            label,
            probability: probability.to_owned(),
        };
        Ok(Response::new(reply))
    }
}
