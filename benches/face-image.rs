use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use face::proto::detector_client::DetectorClient;
use face::proto::{DetectionRequest, InferRequest};
use futures::lock::Mutex;
use mless_core::client::retry;
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use mless::proto::scheduler_client::SchedulerClient;
use mless::proto::{DeployRequest, Deployment, LookupRequest};
use mless::*;

static JPEG: &'static [u8] = include_bytes!("../data/pelican.jpeg");

pub fn bench_face_image(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, detector_client, deployment) =
        runtime.block_on(async { setup_clients(addr).await });

    let deployment_id = deployment.id;

    let arc_client = Arc::new(Mutex::new(client));
    let arc_app_client = Arc::new(Mutex::new(detector_client));

    let mut group = c.benchmark_group("Face image");

    group.bench_with_input(
        BenchmarkId::new("face image scheduled", "<client>"),
        &(arc_client, arc_app_client.clone()),
        |b, (client, app_client)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                let mut app_client = app_client.lock().await;
                run_scheduled(&mut client, &mut app_client, &deployment_id).await
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("face image direct", "<client>"),
        &arc_app_client,
        |b, app_client| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut app_client = app_client.lock().await;
                run_direct(&mut app_client).await
            })
        },
    );
}

async fn setup_clients(
    addr: &str,
) -> (
    SchedulerClient<Channel>,
    DetectorClient<Channel>,
    Deployment,
) {
    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/MLess/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let detector_client = retry(|| async {
        face::proto::detector_client::DetectorClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    (client, detector_client, deployment.deployment.unwrap())
}

async fn run_scheduled(
    client: &mut SchedulerClient<Channel>,
    _detector_client: &mut DetectorClient<Channel>,
    deployment_id: &str,
) {
    let request = LookupRequest {
        deployment_id: deployment_id.to_owned(),
        qos: None,
    };

    let resp = client.clone().lookup(request).await.unwrap().into_inner();
    let addr = resp.server.unwrap().addr;

    let mut detector_client = retry(|| async {
        face::proto::detector_client::DetectorClient::connect(addr.to_owned()).await
    })
    .await
    .unwrap();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = InferRequest { data: vec![] };
    detector_client.infer(request).await.unwrap();
}

async fn run_direct(detector_client: &mut DetectorClient<Channel>) {
    let request = InferRequest { data: vec![] };
    detector_client.infer(request).await.unwrap();
}

pub fn bench_wasm(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, detector_client, deployment) =
        runtime.block_on(async { setup_clients(addr).await });

    let deployment_id = deployment.id;

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
        |b, (client, app_client)| {
            b.to_async(Runtime::new().unwrap()).iter(|| async {
                let mut client = client.lock().await;
                let mut app_client = app_client.lock().await;
                run_wasm_scheduled(&mut client, &mut app_client, &deployment_id, JPEG).await
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

async fn run_wasm_scheduled(
    client: &mut SchedulerClient<Channel>,
    _detector_client: &mut DetectorClient<Channel>,
    deployment_id: &str,
    image: &[u8],
) {
    let request = LookupRequest {
        deployment_id: deployment_id.to_owned(),
        qos: None,
    };

    let resp = client.clone().lookup(request).await.unwrap().into_inner();
    let addr = resp.server.unwrap().addr;

    let mut detector_client = face::proto::detector_client::DetectorClient::connect(addr)
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
criterion_group!(benches, bench_face_image, bench_wasm);
criterion_main!(benches);
