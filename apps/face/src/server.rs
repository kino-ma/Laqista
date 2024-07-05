use tonic::{Request, Response, Status};

use crate::{
    open_default,
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
