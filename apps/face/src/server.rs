use opencv::{
    videoio::{self, VideoCapture},
    Result,
};
use tonic::{Request, Response, Status};

use crate::{
    proto::{detector_server::Detector, DetectReply, DetectRequest},
    DetectedFrame, VideoDetector, DEFAULT_VIDEO_FILE,
};

pub struct DetectServer {}

impl DetectServer {}

#[tonic::async_trait]
impl Detector for DetectServer {
    async fn detect_face(
        &self,
        _request: Request<DetectRequest>,
    ) -> Result<Response<DetectReply>, Status> {
        let capture = open_default().map_err(|e| Status::aborted(e.to_string()))?;
        let detector = VideoDetector::new(capture);

        let mut total_detected = 0;

        for detected_frame in detector {
            let DetectedFrame { faces, .. } = detected_frame;
            total_detected += faces.len();
        }

        let total_detected = total_detected as _;
        let resp = DetectReply { total_detected };
        Ok(Response::new(resp))
    }
}

fn open_default() -> Result<VideoCapture> {
    open(DEFAULT_VIDEO_FILE)
}
fn open(filename: &str) -> Result<VideoCapture> {
    VideoCapture::from_file(filename, videoio::CAP_ANY)
}
