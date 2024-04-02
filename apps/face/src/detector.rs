use opencv::objdetect::CascadeClassifier;
use opencv::prelude::*;
use opencv::types::VectorOfRect;
use opencv::videoio::VideoCapture;
use opencv::{core, imgproc, objdetect, types, Result};

pub const DEFAULT_VIDEO_FILE: &'static str =
    "/Users/kino-ma/Documents/research/mless/dataset/people.mp4";

pub struct VideoDetector {
    frames: Frames,
    detector: FaceDetector,
}

impl VideoDetector {
    pub fn new(capture: VideoCapture) -> Self {
        let frames = Frames::new(capture);
        let detector = FaceDetector::new();

        Self { frames, detector }
    }
}

impl Iterator for VideoDetector {
    type Item = DetectedFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.frames.next()?;

        match self.detector.detect(frame) {
            Ok(v) => Some(v),
            _ => None,
        }
    }
}

pub struct DetectedFrame {
    pub frame: Mat,
    pub faces: VectorOfRect,
}

pub struct FaceDetector {
    classifier: CascadeClassifier,
}

impl FaceDetector {
    pub fn new() -> Self {
        let xml = core::find_file_def("haarcascades/haarcascade_frontalface_alt.xml").unwrap();
        let classifier = objdetect::CascadeClassifier::new(&xml).unwrap();

        Self { classifier }
    }

    pub fn detect(&mut self, frame: Mat) -> Result<DetectedFrame> {
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&frame, &mut gray, imgproc::COLOR_BGR2GRAY)?;

        let mut reduced = Mat::default();
        imgproc::resize(
            &gray,
            &mut reduced,
            core::Size {
                width: 0,
                height: 0,
            },
            0.25f64,
            0.25f64,
            imgproc::INTER_LINEAR,
        )?;

        let mut faces = types::VectorOfRect::new();

        self.classifier.detect_multi_scale(
            &reduced,
            &mut faces,
            1.1,
            2,
            objdetect::CASCADE_SCALE_IMAGE,
            core::Size {
                width: 30,
                height: 30,
            },
            core::Size {
                width: 0,
                height: 0,
            },
        )?;

        Ok(DetectedFrame { faces, frame })
    }
}

struct Frames {
    capture: VideoCapture,
}

impl Frames {
    pub fn new(capture: VideoCapture) -> Self {
        Self { capture }
    }
}

impl Iterator for Frames {
    type Item = Mat;

    fn next(&mut self) -> Option<Self::Item> {
        let mut frame = Mat::default();

        let read_result = self.capture.read(&mut frame);

        match read_result {
            Ok(true) => Some(frame),
            _ => return None,
        }
    }
}
