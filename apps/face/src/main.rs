use std::error::Error;

use face::{DetectedFrame, Mp4Detector};
use opencv::{
    core, highgui, imgproc,
    videoio::{self, VideoCapture, VideoCaptureTraitConst, CAP_ANY},
};

fn main() -> Result<(), Box<dyn Error>> {
    // let maybe_filename = args().nth(1);
    // let filename = maybe_filename.as_deref().unwrap_or(DEFAULT_VIDEO_FILE);

    // let capture = VideoCapture::from_file(filename, videoio::CAP_ANY)?;
    let capture = VideoCapture::new(0, CAP_ANY).unwrap();

    let window = "video capture";
    highgui::named_window_def(window)?;
    let opened = videoio::VideoCapture::is_opened(&capture)?;
    if !opened {
        panic!("Unable to open default camera!");
    }

    let detector = Mp4Detector::new(capture);

    for detected_frame in detector {
        println!("loop");
        let DetectedFrame { faces, mut frame } = detected_frame;

        println!("faces: {}", faces.len());

        for face in faces.iter() {
            println!("face {face:?}");
            let scaled_face =
                core::Rect::new(face.x * 4, face.y * 4, face.width * 4, face.height * 4);
            println!("sealed");

            imgproc::rectangle_def(&mut frame, scaled_face, (0, 255, 0).into())?;
            println!("rected");
        }
        println!("inner end");

        println!("faces {faces:?}");
        highgui::imshow(window, &frame)?;
        println!("showed");
        if highgui::wait_key(10)? > 0 {
            break;
        }
        println!("next");
    }

    Ok(())
}
