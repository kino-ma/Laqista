use opencv::{
    core::Mat,
    imgcodecs::{imread, IMREAD_COLOR},
    videoio::{self, VideoCapture},
    Result,
};
use tonic::{Request, Response, Status};

use crate::{
    proto::{
        detector_server::Detector, DetectReply, DetectRequest, DetectVideoReply, DetectVideoRequest,
    },
    DetectedFrame, FaceDetector, Mp4Detector, DEFAULT_IMAGE_FILE, DEFAULT_VIDEO_FILE,
};

pub struct DetectServer {}

impl DetectServer {}

#[tonic::async_trait]
impl Detector for DetectServer {
    async fn detect_video(
        &self,
        _request: Request<DetectVideoRequest>,
    ) -> Result<Response<DetectVideoReply>, Status> {
        let capture = open_video_default().map_err(|e| Status::aborted(e.to_string()))?;
        let detector = Mp4Detector::new(capture);

        let mut total_detected = 0;

        for detected_frame in detector {
            let DetectedFrame { faces, .. } = detected_frame;
            total_detected += faces.len();
        }

        let total_detected = total_detected as _;
        let resp = DetectVideoReply { total_detected };
        Ok(Response::new(resp))
    }

    async fn detect_face(
        &self,
        _request: Request<DetectRequest>,
    ) -> Result<Response<DetectReply>, Status> {
        let frame = open_default().map_err(|e| Status::aborted(e.to_string()))?;
        let mut detector = FaceDetector::new();

        let DetectedFrame { faces, .. } = detector
            .detect(frame)
            .map_err(|e| Status::aborted(e.to_string()))?;

        let resp = DetectReply {
            total_detected: faces.len() as _,
        };
        Ok(Response::new(resp))
    }
}

fn open_video_default() -> Result<VideoCapture> {
    open_video(DEFAULT_VIDEO_FILE)
}
fn open_video(filename: &str) -> Result<VideoCapture> {
    VideoCapture::from_file(filename, videoio::CAP_ANY)
}

fn open_default() -> Result<Mat> {
    open(DEFAULT_IMAGE_FILE)
}
fn open(filename: &str) -> Result<Mat> {
    imread(filename, IMREAD_COLOR)
}
