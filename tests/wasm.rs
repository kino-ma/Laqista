use face::proto::DetectionRequest;
use mless::proto::{self, DeployRequest, LookupRequest};

static JPEG: &'static [u8] = include_bytes!("../data/pelican.jpeg");

#[tokio::test]
async fn schedule_wasm() {
    let addr = "http://127.0.0.1:50051";

    let mut client = proto::scheduler_client::SchedulerClient::connect(addr.to_owned())
        .await
        .expect("failed to connect to the server");

    let mut detector_client =
        face::proto::detector_client::DetectorClient::connect(addr.to_owned())
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

    let request = LookupRequest {
        deployment_id: deployment.deployment.unwrap().id,
        qos: None,
    };

    let _resp = client.clone().lookup(request).await.unwrap().into_inner();

    // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
    //     .await
    //     .unwrap();
    let request = DetectionRequest {
        image_png: JPEG.to_vec(),
    };

    let resp = detector_client
        .run_detection(request)
        .await
        .unwrap()
        .into_inner();

    let correct_labels = ["pelican", "spoonbill", "mollymawk", "oyster catcher"];
    let is_correct = correct_labels.iter().any(|l| resp.label.contains(l));

    assert!(is_correct, "{:?}", resp);
}
