use std::{
    collections::HashMap,
    error::Error,
    io::{BufRead, BufReader},
    path::Path,
};

use face::open_default;
use mless_core::{session::Session, tensor::Inputs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let model_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("opt-squeeze.onnx");

    println!("opening frame");
    let frame = open_default();

    println!("creating detector");
    let mut session = Session::from_path(model_path).await?;

    println!("creating input");
    let input = frame
        .as_slice()
        .expect("failed to convert input image to slice")
        .into();
    let inputs: Inputs = HashMap::from([("data".to_owned(), input)]);

    println!("detecting");
    let outputs = session.detect(&inputs).await?;

    println!("result: {outputs:?}");
    let probabilities = outputs.into_iter().next().unwrap().1;
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

    Ok(())
}

fn get_imagenet_labels() -> Vec<String> {
    // Download the ImageNet class labels, matching SqueezeNet's classes.
    let labels_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("squeeze-labels.txt");
    let file = BufReader::new(std::fs::File::open(labels_path).unwrap());

    file.lines().map(|line| line.unwrap()).collect()
}
