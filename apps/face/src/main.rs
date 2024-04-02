use std::env::args;
use std::thread;
use std::time::Duration;

use opencv::objdetect::CascadeClassifier;
use opencv::prelude::*;
use opencv::videoio::VideoCapture;
use opencv::{core, highgui, imgproc, objdetect, types, videoio, Result};

const DEFAULT_VIDEO_FILE: &'static str =
    "/Users/kino-ma/Documents/research/mless/dataset/people.mp4";

fn main() -> Result<()> {
    run()
}

pub fn run() -> Result<()> {
    let maybe_filename = args().nth(1);
    let filename = maybe_filename.as_deref().unwrap_or(DEFAULT_VIDEO_FILE);

    let mut video = VideoCapture::from_file(filename, videoio::CAP_ANY)?;

    let window = "video capture";
    highgui::named_window_def(window)?;
    let opened = videoio::VideoCapture::is_opened(&video)?;
    if !opened {
        panic!("Unable to open default camera!");
    }

    let mut detector = Detector::new();

    loop {
        let mut frame = Mat::default();

        if !video.read(&mut frame)? {
            break;
        }

        let detected_frame = detector.detect(frame)?;

        highgui::imshow(window, &detected_frame)?;
        if highgui::wait_key(10)? > 0 {
            break;
        }
    }

    Ok(())
}

struct Detector {
    classifier: CascadeClassifier,
}

impl Detector {
    pub fn new() -> Self {
        let xml = core::find_file_def("haarcascades/haarcascade_frontalface_alt.xml").unwrap();
        let classifier = objdetect::CascadeClassifier::new(&xml).unwrap();

        Self { classifier }
    }

    pub fn detect(&mut self, mut frame: Mat) -> Result<Mat> {
        if frame.size()?.width == 0 {
            thread::sleep(Duration::from_secs(50));
        }

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

        println!("faces: {}", faces.len());

        for face in faces {
            println!("face {face:?}");
            let scaled_face =
                core::Rect::new(face.x * 4, face.y * 4, face.width * 4, face.height * 4);

            imgproc::rectangle_def(&mut frame, scaled_face, (0, 255, 0).into())?;
        }

        Ok(frame)
    }
}
