use std::collections::HashMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use futures::lock::Mutex;
use laqista::proto::{DeployRequest, Deployment, LookupRequest};
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use laqista::proto::scheduler_client::SchedulerClient;
use laqista::*;

use hello::proto::greeter_client::GreeterClient;
use hello::proto::HelloRequest;

pub fn bench_greeter(c: &mut Criterion) {
    let addr = "http://127.0.0.1:50051";

    let runtime = Runtime::new().unwrap();
    let (client, app_client, deployment) = runtime.block_on(async { setup_clients(addr).await });

    let deployment_id = deployment.id;

    let arc_client = Arc::new(Mutex::new(client));
    let arc_app_client = Arc::new(Mutex::new(app_client));

    let mut group = c.benchmark_group("Greeter");

    group.bench_with_input(
        BenchmarkId::new("scheduled", "<client>"),
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
        BenchmarkId::new("direct", "<client>"),
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
) -> (SchedulerClient<Channel>, GreeterClient<Channel>, Deployment) {
    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let app_client = hello::proto::greeter_client::GreeterClient::connect(addr.to_owned())
        .await
        .unwrap();

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        accuracies_percent: HashMap::from([("Infer".to_owned(), 80.3)]),
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    (client, app_client, deployment.deployment.unwrap())
}

async fn run_scheduled(
    client: &mut SchedulerClient<Channel>,
    app_client: &mut GreeterClient<Channel>,
    deployment_id: &str,
) {
    let request = LookupRequest {
        deployment_id: deployment_id.to_owned(),
        qos: None,
        name: "SayHello".to_owned(),
    };

    let _resp = client.clone().lookup(request).await.unwrap().into_inner();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = HelloRequest {
        name: "Laqista benchamrk".to_owned(),
    };
    app_client.say_hello(request).await.unwrap();
}

async fn run_direct(app_client: &mut GreeterClient<Channel>) {
    let request = HelloRequest {
        name: "Laqista benchamrk".to_owned(),
    };
    app_client.say_hello(request).await.unwrap();
}

criterion_group!(benches, bench_greeter);
criterion_main!(benches);
