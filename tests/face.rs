use std::collections::HashMap;

use face::proto::InferRequest;
use image::{imageops::FilterType, GenericImageView, Pixel};
use laqista::proto::{self, DeployRequest, GetAppsRequest, LookupRequest};
use laqista_core::{client::retry, AppService};

static JPEG: &'static [u8] = include_bytes!("../data/sized-pelican.jpeg");
static LABELS: &'static str = include_str!("../data/models/resnet-labels.txt");

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

#[tokio::test]
async fn schedule_wasm() {
    let addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let wasm_service = AppService::new("face", "Detector");
    let onnx_service = AppService::new("face", "ObjectDetection");

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        rpcs: vec![
            wasm_service.rpc("RunDetection").to_string(),
            onnx_service.rpc("Squeeze").to_string(),
        ],
        accuracies_percent: HashMap::from([(onnx_service.rpc("Squeeze").to_string(), 80.3)]),
    };

    let _deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: onnx_service.to_string(),
    };

    let _deploy_resp = retry(|| async { client.clone().lookup(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let img = image::load_from_memory(&JPEG).expect("Failed to load image");

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    let input = array.as_slice().expect("Failed to get array slice");

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let od_client = retry(|| async {
        face::proto::object_detection_client::ObjectDetectionClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    let request = InferRequest {
        data: input.to_vec(),
    };

    let squeeze_resp = retry(|| async { od_client.clone().squeeze(request.clone()).await })
        .await
        .unwrap()
        .into_inner();
    dbg!(&squeeze_resp);

    let mut probs: Vec<_> = squeeze_resp
        .squeezenet0_flatten0_reshape0
        .iter()
        .enumerate()
        .collect();

    probs.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    // n02051845 pelican
    let expected_idx = 144usize;

    let top5 = &probs[..5];
    let is_top5 = top5.iter().any(|(i, _)| *i == expected_idx);

    let labels: Vec<_> = LABELS.lines().collect();
    let top5_labels: Vec<_> = top5.iter().map(|i| (i, &labels[i.0])).collect();

    assert!(is_top5, "top5 = {:?}", top5_labels);
}

#[tokio::test]
#[ignore]
async fn schedule_wasm_fog() {
    let cloud_addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(cloud_addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let wasm_service = AppService::new("face", "Detector");
    let onnx_service = AppService::new("face", "ObjectDetection");

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        rpcs: vec![
            wasm_service.rpc("RunDetection").to_string(),
            onnx_service.rpc("Squeeze").to_string(),
        ],
        accuracies_percent: HashMap::from([(onnx_service.rpc("Squeeze").to_string(), 80.3)]),
    };

    let _deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let fog_addr = "http://127.0.0.1:50052";

    let client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(fog_addr.to_owned()).await
    })
    .await
    .unwrap();

    let request = GetAppsRequest {
        names: vec!["face".to_owned()],
    };

    let _deployments = retry(|| async { client.clone().get_apps(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let onnx_service = AppService::new("face", "ObjectDetection");

    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: onnx_service.to_string(),
    };

    let client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(fog_addr.to_owned()).await
    })
    .await
    .unwrap();

    let _deploy_resp = retry(|| async { client.clone().lookup(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let img = image::load_from_memory(&JPEG).expect("Failed to load image");

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    let input = array.as_slice().expect("Failed to get array slice");

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let od_client = retry(|| async {
        face::proto::object_detection_client::ObjectDetectionClient::connect(fog_addr.to_owned())
            .await
    })
    .await
    .unwrap();

    let request = InferRequest {
        data: input.to_vec(),
    };

    let squeeze_resp = retry(|| async { od_client.clone().squeeze(request.clone()).await })
        .await
        .unwrap()
        .into_inner();
    dbg!(&squeeze_resp);

    let mut probs: Vec<_> = squeeze_resp
        .squeezenet0_flatten0_reshape0
        .iter()
        .enumerate()
        .collect();

    probs.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    // n02051845 pelican
    let expected_idx = 144usize;

    let top5 = &probs[..5];
    let is_top5 = top5.iter().any(|(i, _)| *i == expected_idx);

    let labels: Vec<_> = LABELS.lines().collect();
    let top5_labels: Vec<_> = top5.iter().map(|i| (i, &labels[i.0])).collect();

    assert!(is_top5, "top5 = {:?}", top5_labels);
}

#[tokio::test]
#[ignore]
async fn schedule_wasm_dew() {
    let cloud_addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(cloud_addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let wasm_service = AppService::new("face", "Detector");
    let onnx_service = AppService::new("face", "ObjectDetection");

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        rpcs: vec![
            wasm_service.rpc("RunDetection").to_string(),
            onnx_service.rpc("Squeeze").to_string(),
        ],
        accuracies_percent: HashMap::from([(onnx_service.rpc("Squeeze").to_string(), 80.3)]),
    };

    let _deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let dew_addr = "http://127.0.0.1:50053";

    let client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(dew_addr.to_owned()).await
    })
    .await
    .unwrap();

    let request = GetAppsRequest {
        names: vec!["face".to_owned()],
    };

    let _deployments = retry(|| async { client.clone().get_apps(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let onnx_service = AppService::new("face", "ObjectDetection");

    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: onnx_service.to_string(),
    };

    let client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(dew_addr.to_owned()).await
    })
    .await
    .unwrap();

    let _deploy_resp = retry(|| async { client.clone().lookup(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let img = image::load_from_memory(&JPEG).expect("Failed to load image");

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    let input = array.as_slice().expect("Failed to get array slice");

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let od_client = retry(|| async {
        face::proto::object_detection_client::ObjectDetectionClient::connect(dew_addr.to_owned())
            .await
    })
    .await
    .unwrap();

    let request = InferRequest {
        data: input.to_vec(),
    };

    let squeeze_resp = retry(|| async { od_client.clone().squeeze(request.clone()).await })
        .await
        .unwrap()
        .into_inner();
    dbg!(&squeeze_resp);

    let mut probs: Vec<_> = squeeze_resp
        .squeezenet0_flatten0_reshape0
        .iter()
        .enumerate()
        .collect();

    probs.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    // n02051845 pelican
    let expected_idx = 144usize;

    let top5 = &probs[..5];
    let is_top5 = top5.iter().any(|(i, _)| *i == expected_idx);

    let labels: Vec<_> = LABELS.lines().collect();
    let top5_labels: Vec<_> = top5.iter().map(|i| (i, &labels[i.0])).collect();

    assert!(is_top5, "top5 = {:?}", top5_labels);
}
