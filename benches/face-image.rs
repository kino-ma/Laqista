use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use face::proto::detector_client::DetectorClient;
use face::proto::DetectRequest;
use futures::lock::Mutex;
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use mless::proto::scheduler_client::SchedulerClient;
use mless::proto::{DeployRequest, Deployment, LookupRequest};
use mless::*;

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

    let detector_client = face::proto::detector_client::DetectorClient::connect(addr.to_owned())
        .await
        .unwrap();

    let request = DeployRequest {
        source: "https://github.com/kino-ma/MLess/apps/face".to_owned(),
        authoritative: true,
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    (client, detector_client, deployment.deployment.unwrap())
}

async fn run_scheduled(
    client: &mut SchedulerClient<Channel>,
    detector_client: &mut DetectorClient<Channel>,
    deployment_id: &str,
) {
    let request = LookupRequest {
        deployment_id: deployment_id.to_owned(),
        qos: None,
    };

    let _resp = client.clone().lookup(request).await.unwrap().into_inner();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = DetectRequest {
        image_file: "".to_owned(),
    };
    detector_client.detect_face(request).await.unwrap();
}

async fn run_direct(detector_client: &mut DetectorClient<Channel>) {
    let request = DetectRequest {
        image_file: "".to_owned(),
    };
    detector_client.detect_face(request).await.unwrap();
}

criterion_group!(benches, bench_face_image);
criterion_main!(benches);
