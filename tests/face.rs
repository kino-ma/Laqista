use face::proto::InferRequest;
use image::{imageops::FilterType, GenericImageView, Pixel};
use mless::proto::{self, DeployRequest, LookupRequest};

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

    let mut detector_client =
        face::proto::detector_client::DetectorClient::connect(addr.to_owned())
            .await
            .unwrap();

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

    let request = LookupRequest {
        deployment_id: deployment.deployment.unwrap().id,
        qos: None,
    };

    let _resp = client.clone().lookup(request).await.unwrap().into_inner();

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
    let request = InferRequest {
        data: input.to_vec(),
    };

    let resp = detector_client.infer(request).await.unwrap().into_inner();
    let mut probs: Vec<_> = resp
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
