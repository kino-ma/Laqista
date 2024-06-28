use std::{borrow::Cow, collections::HashMap, error::Error, path::Path};

use wonnx::{
    utils::{InputTensor, OutputTensor},
    Session, SessionError,
};

pub const DEFAULT_IMAGE_FILE: &'static str =
    "/Users/kino-ma/Documents/research/mless/dataset/still-people.png";
pub const DEFAULT_VIDEO_FILE: &'static str =
    "/Users/kino-ma/Documents/research/mless/dataset/people.mp4";

pub struct FaceDetector {
    session: Session,
}

pub struct DetectedFrame {
    pub frame: OutputTensor,
    pub faces: OutputTensor,
}

pub struct DetectionInputs<'a> {
    input: &'a [f32],
}

pub struct DetectionOutputs {
    scores: Vec<f32>,
    boxes: Vec<f32>,
}

impl FaceDetector {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    pub async fn create_default() -> Result<Self, SessionError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("models")
            .join("face_detector_640.onnx");

        let session = Session::from_path(path).await?;

        Ok(Self::new(session))
    }

    pub async fn detect<'a>(
        &mut self,
        input: DetectionInputs<'a>,
    ) -> Result<DetectionOutputs, Box<dyn Error>> {
        let cow = Cow::Borrowed(input.input);
        let tensor = InputTensor::F32(cow);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_owned(), tensor);

        let result = self.session.run(&inputs).await?;
        let outputs = result.try_into()?;

        Ok(outputs)
    }
}

impl TryFrom<HashMap<String, OutputTensor>> for DetectionOutputs {
    type Error = String;
    fn try_from(mut result: HashMap<String, OutputTensor>) -> Result<Self, Self::Error> {
        let scores = result
            .remove("scores")
            .ok_or("scores not found".to_owned())?;
        let boxes = result.remove("boxes").ok_or("boxes not found".to_owned())?;

        use OutputTensor::F32;
        match (scores, boxes) {
            (F32(s), F32(b)) => Ok(Self {
                scores: s,
                boxes: b,
            }),
            _ => Err("invalid type of scores or boxes".to_owned()),
        }
    }
}
