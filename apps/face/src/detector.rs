use std::{
    borrow::Cow,
    collections::HashMap,
    error::Error,
    io::{BufRead, BufReader},
    path::Path,
};

use wonnx::{
    utils::{InputTensor, OutputTensor},
    Session, SessionError,
};

pub struct FaceDetector {
    session: Session,
}

pub struct DetectedFrame {
    pub frame: OutputTensor,
    pub faces: OutputTensor,
}

pub struct DetectionInputs<'a> {
    pub input: &'a [f32],
}

pub struct DetectionOutputs {
    pub scores: Vec<f32>,
    pub boxes: Vec<f32>,
}

impl FaceDetector {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    pub async fn create_default() -> Result<Self, SessionError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("models")
            // .join("face_detector_640.onnx");
            .join("opt-squeeze.onnx");

        // let labels_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        //     .join("models")
        //     .join("opt-squeeze");

        println!("loading model from {path:?}...");

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
        inputs.insert("data".to_owned(), tensor);

        let result = self.session.run(&inputs).await?;
        println!("result: {result:?}");
        let probabilities = result.into_iter().next().unwrap().1;
        let probabilities: Vec<f32> = probabilities.try_into().unwrap();
        let mut probabilities = probabilities.iter().enumerate().collect::<Vec<_>>();
        probabilities.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

        let class_labels = get_imagenet_labels();

        for i in 0..10 {
            println!(
                "Infered result: {} of class: {}",
                class_labels[probabilities[i].0], probabilities[i].0
            );
            println!("details: {:?}", probabilities[i]);
        }
        // let outputs = result.try_into()?;

        // Ok(outputs)
        panic!()
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

fn get_imagenet_labels() -> Vec<String> {
    // Download the ImageNet class labels, matching SqueezeNet's classes.
    let labels_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("squeeze-labels.txt");
    let file = BufReader::new(std::fs::File::open(labels_path).unwrap());

    file.lines().map(|line| line.unwrap()).collect()
}
