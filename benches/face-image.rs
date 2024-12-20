use std::collections::HashMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use face::proto::detector_client::DetectorClient;
use face::proto::object_detection_client::ObjectDetectionClient;
use face::proto::{DetectionRequest, InferRequest};
use futures::lock::Mutex;
use image::imageops::FilterType;
use image::{GenericImageView, Pixel};
use laqista_core::client::retry;
use laqista_core::{AppRpc, AppService};
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use laqista::proto::scheduler_client::SchedulerClient;
use laqista::proto::{DeployRequest, Deployment, LookupRequest};
use laqista::*;

static JPEG: &'static [u8] = include_bytes!("../data/pelican.jpeg");

const IMAGE_WIDTH: usize = 224;
const IMAGE_HEIGHT: usize = 224;

pub fn bench_face_image(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, od_client, _, _) = runtime.block_on(async { setup_clients(addr).await });

    let arc_client = Arc::new(Mutex::new(client));
    let arc_od_client = Arc::new(Mutex::new(od_client));

    let mut group = c.benchmark_group("Face image");

    group.bench_with_input(
        BenchmarkId::new("face image scheduled", "<client>"),
        &(arc_client.clone(), vec![]),
        |b, (client, data)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                run_scheduled(&mut client, data.clone()).await
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("face image direct", "<client>"),
        &(arc_od_client.clone(), vec![]),
        |b, (od_client, data)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut od_client = od_client.lock().await;
                run_direct(&mut od_client, data.clone()).await
            })
        },
    );

    let data = setup_image();

    group.bench_with_input(
        BenchmarkId::new("face full image scheduled", "<client>"),
        &(arc_client, data.clone()),
        |b, (client, data)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                run_scheduled(&mut client, data.clone()).await
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("face full image direct", "<client>"),
        &(arc_od_client, data.clone()),
        |b, (od_client, data)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut od_client = od_client.lock().await;
                run_direct(&mut od_client, data.clone()).await
            })
        },
    );
}

async fn setup_clients(
    addr: &str,
) -> (
    SchedulerClient<Channel>,
    ObjectDetectionClient<Channel>,
    DetectorClient<Channel>,
    Deployment,
) {
    let mut client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(addr.to_owned()).await
    })
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
        accuracies_percent: HashMap::from([
            (onnx_service.rpc("Squeeze").to_string(), 80.3),
            (wasm_service.rpc("RunDetection").to_string(), 80.3),
        ]),
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let od_client = retry(|| async {
        face::proto::object_detection_client::ObjectDetectionClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    let detector_client = retry(|| async {
        face::proto::detector_client::DetectorClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    (
        client,
        od_client,
        detector_client,
        deployment.deployment.unwrap(),
    )
}

fn setup_image() -> Vec<f32> {
    let img = image::load_from_memory(&JPEG).expect("Failed to load image");

    let img = img.resize_to_fill(IMAGE_WIDTH as _, IMAGE_HEIGHT as _, FilterType::Nearest);

    let array = ndarray::Array::from_shape_fn((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT), |(_, c, j, i)| {
        let pixel = img.get_pixel(i as u32, j as u32);
        let channels = pixel.channels();

        // range [0, 255] -> range [0, 1]
        (channels[c] as f32) / 255.0
    });

    array.into_raw_vec()
}

async fn run_scheduled(client: &mut SchedulerClient<Channel>, data: Vec<f32>) {
    let rpc = AppRpc::new("face", "ObjectDetection", "Squeeze");
    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: rpc.to_string(),
    };

    let resp = client.clone().lookup(request).await.unwrap().into_inner();
    let addr = resp.server.unwrap().addr;

    let mut detector_client = retry(|| async {
        face::proto::object_detection_client::ObjectDetectionClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = InferRequest { data };
    detector_client.squeeze(request).await.unwrap();
}

async fn run_direct(detector_client: &mut ObjectDetectionClient<Channel>, data: Vec<f32>) {
    let request = InferRequest { data };
    detector_client.squeeze(request).await.unwrap();
}

pub fn bench_wasm(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, _, detector_client, _) = runtime.block_on(async { setup_clients(addr).await });

    let arc_client = Arc::new(Mutex::new(client));
    let arc_app_client = Arc::new(Mutex::new(detector_client));

    let mut group = c.benchmark_group("Face wasm");

    // group.bench_with_input(
    //     BenchmarkId::new("face wasm scheduled", "<client>"),
    //     &(arc_client.clone(), arc_app_client.clone()),
    //     |b, (client, app_client)| {
    //         b.to_async(Runtime::new().unwrap()).iter(|| async {
    //             let mut client = client.lock().await;
    //             let mut app_client = app_client.lock().await;
    //             run_wasm_scheduled(&mut client, &mut app_client, &deployment_id, &[2, 40]).await
    //         })
    //     },
    // );

    // group.bench_with_input(
    //     BenchmarkId::new("face wasm direct", "<client>"),
    //     &arc_app_client,
    //     |b, app_client| {
    //         b.to_async(Runtime::new().unwrap()).iter(|| async {
    //             let mut app_client = app_client.lock().await;
    //             run_wasm_direct(&mut app_client, &[2, 40]).await
    //         })
    //     },
    // );

    group.bench_with_input(
        BenchmarkId::new("face wasm scheduled full image", "<client>"),
        &(arc_client.clone(), arc_app_client.clone()),
        |b, (client, _)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                run_wasm_scheduled(&mut client, JPEG).await
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("face wasm direct full image", "<client>"),
        &arc_app_client,
        |b, app_client| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut app_client = app_client.lock().await;
                run_wasm_direct(&mut app_client, JPEG).await
            })
        },
    );
}

async fn run_wasm_scheduled(client: &mut SchedulerClient<Channel>, image: &[u8]) {
    let rpc = AppRpc::new("face", "Detector", "RunDetection");
    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: rpc.to_string(),
    };

    let resp = client.clone().lookup(request).await.unwrap().into_inner();
    let addr = resp.server.unwrap().addr;

    let mut detector_client = retry(|| async {
        face::proto::detector_client::DetectorClient::connect(addr.clone()).await
    })
    .await
    .unwrap();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = DetectionRequest {
        image_png: image.to_vec(),
    };
    detector_client.run_detection(request).await.unwrap();
}

async fn run_wasm_direct(detector_client: &mut DetectorClient<Channel>, image: &[u8]) {
    let request = DetectionRequest {
        image_png: image.to_vec(),
    };
    detector_client.run_detection(request).await.unwrap();
}

pub fn bench_scheduler(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, _, _, _) = runtime.block_on(async { setup_clients(addr).await });

    let arc_client = Arc::new(Mutex::new(client));

    let mut group = c.benchmark_group("Face image");

    group.bench_with_input(
        BenchmarkId::new("scheduler lookup", "<client>"),
        &arc_client,
        |b, client| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                run_lookup(&mut client).await
            })
        },
    );
}

async fn run_lookup(client: &mut SchedulerClient<Channel>) {
    let rpc = AppRpc::new("face", "Detector", "RunDetection");

    let request = LookupRequest {
        name: "face".to_owned(),
        qos: None,
        service: rpc.to_string(),
    };

    client.clone().lookup(request).await.unwrap().into_inner();
}

criterion_group!(benches, bench_face_image, bench_wasm, bench_scheduler);
criterion_main!(benches);
