use std::collections::HashMap;

use face::proto::DetectionRequest;
use laqista::proto::{self, DeployRequest, LookupRequest};
use laqista_core::{client::retry, AppService};

static JPEG: &'static [u8] = include_bytes!("../data/sized-pelican.jpeg");

#[tokio::test]
async fn schedule_native() {
    let addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let wasm_service = AppService::new("face", "NativeDetector");

    let deploy_request = DeployRequest {
        name: "native".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
        rpcs: vec![wasm_service.rpc("RunDetection").to_string()],
        accuracies_percent: HashMap::from([(wasm_service.rpc("RunDetection").to_string(), 80.3)]),
    };

    let _deployment = client
        .deploy(deploy_request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let request = LookupRequest {
        name: "native".to_owned(),
        qos: None,
        service: wasm_service.to_string(),
    };

    let _lookup_resp = retry(|| async { client.clone().lookup(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let app_client = retry(|| async {
        face::proto::native_detector_client::NativeDetectorClient::connect(addr).await
    })
    .await
    .unwrap();

    let request = DetectionRequest {
        image_png: JPEG.to_owned(),
    };

    let detection_resp =
        retry(|| async { app_client.clone().run_detection(request.clone()).await })
            .await
            .unwrap()
            .into_inner();

    let contained = ["spoonbill", "pelican"]
        .iter()
        .any(|l| detection_resp.label.contains(l));
    assert!(contained, "label = {:?}", detection_resp.label);
}
