use std::error::Error;

use face::{open_default, DetectionInputs, DetectionOutputs, FaceDetector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let frame = open_default();
    let mut detector = FaceDetector::create_default().await?;

    let input = frame
        .as_slice()
        .expect("failed to convert input image to slice");
    let input = DetectionInputs { input };

    let DetectionOutputs { boxes, .. } = detector.detect(input).await?;
    let len = boxes.len();

    println!("Detected {len} boxes");

    Ok(())
}
