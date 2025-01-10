use std::collections::HashMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use face::proto::native_detector_client::NativeDetectorClient;
use face::proto::DetectionRequest;
use futures::lock::Mutex;
use laqista_core::client::retry;
use laqista_core::{AppRpc, AppService};
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use laqista::proto::scheduler_client::SchedulerClient;
use laqista::proto::{DeployRequest, Deployment, LookupRequest};
use laqista::*;

static JPEG: &'static [u8] = include_bytes!("../data/pelican.jpeg");

pub fn bench_native(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, detector_client, _) = runtime.block_on(async { setup_clients(addr).await });

    let arc_client = Arc::new(Mutex::new(client));
    let arc_app_client = Arc::new(Mutex::new(detector_client));

    let mut group = c.benchmark_group("Face native");
    group.sampling_mode(criterion::SamplingMode::Flat);

    group.bench_with_input(
        BenchmarkId::new("face native scheduled full image", "<client>"),
        &(arc_client.clone(), arc_app_client.clone()),
        |b, (client, _)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                run_native_scheduled(&mut client, JPEG).await
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("face native direct full image", "<client>"),
        &arc_app_client,
        |b, app_client| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut app_client = app_client.lock().await;
                run_native_direct(&mut app_client, JPEG).await
            })
        },
    );
}

async fn run_native_scheduled(client: &mut SchedulerClient<Channel>, image: &[u8]) {
    let rpc = AppRpc::new("native", "NativeDetector", "RunDetection");
    let request = LookupRequest {
        name: "native".to_owned(),
        qos: None,
        service: rpc.to_string(),
    };

    let resp = client.clone().lookup(request).await.unwrap().into_inner();
    let addr = resp.server.unwrap().addr;

    let mut detector_client = retry(|| async {
        face::proto::native_detector_client::NativeDetectorClient::connect(addr.clone()).await
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

async fn run_native_direct(detector_client: &mut NativeDetectorClient<Channel>, image: &[u8]) {
    let request = DetectionRequest {
        image_png: image.to_vec(),
    };
    detector_client.run_detection(request).await.unwrap();
}

async fn setup_clients(
    addr: &str,
) -> (
    SchedulerClient<Channel>,
    NativeDetectorClient<Channel>,
    Deployment,
) {
    let mut client = retry(|| async {
        proto::scheduler_client::SchedulerClient::connect(addr.to_owned()).await
    })
    .await
    .expect("failed to connect to the server");

    let native_service = AppService::new("native", "NativeDetector");

    let request = DeployRequest {
        name: "native".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        rpcs: vec![native_service.rpc("RunDetection").to_string()],
        accuracies_percent: HashMap::from([(native_service.rpc("RunDetection").to_string(), 80.3)]),
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let detector_client = retry(|| async {
        face::proto::native_detector_client::NativeDetectorClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    (client, detector_client, deployment.deployment.unwrap())
}

criterion_group!(benches, bench_native);
criterion_main!(benches);
