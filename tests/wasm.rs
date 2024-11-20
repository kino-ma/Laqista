use face::proto::DetectionRequest;
use laqista::proto::{self, DeployRequest, LookupRequest};
use laqista_core::client::retry;

static JPEG: &'static [u8] = include_bytes!("../data/pelican.jpeg");

#[tokio::test]
async fn schedule_wasm() {
    let addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let request = DeployRequest {
        name: "face".to_owned(),
        source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
            .to_owned(),
    };

    let deployment = client
        .deploy(request)
        .await
        .expect("failed to deploy")
        .into_inner();

    let request = LookupRequest {
        deployment_id: deployment.deployment.unwrap().id.to_owned(),
        qos: None,
    };

    let resp = retry(|| async { client.clone().lookup(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let target_addr = resp.server.unwrap().addr;
    let detector_client = retry(|| async {
        face::proto::detector_client::DetectorClient::connect(target_addr.clone()).await
    })
    .await
    .unwrap();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = DetectionRequest {
        image_png: JPEG.to_vec(),
    };

    let resp = retry(|| async { detector_client.clone().run_detection(request.clone()).await })
        .await
        .unwrap()
        .into_inner();

    let correct_labels = ["pelican", "spoonbill", "mollymawk", "oyster catcher"];
    let is_correct = correct_labels.iter().any(|l| resp.label.contains(l));

    assert!(is_correct, "{:?}", resp);
}
