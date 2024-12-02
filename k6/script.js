import { URL } from "https://jslib.k6.io/url/1.0.0/index.js";
import { Client, StatusOK } from "k6/net/grpc";
import { check } from "k6";

// Just for deployment
const schedulerClient = new Client();
schedulerClient.load(["definitions"], "../../proto/laqista.proto");

const appClient = new Client();
appClient.load(["definitions"], "../../proto/face.proto");

const deployRequest = {
  name: "face",
  source:
    "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz",
  rpcs: ["/face.Detector/RunDetection", "/face.ObjectDetection/Squeeze"],
  accuracies_percent: { "/face.ObjectDetection/Squeeze": 80.3 },
};

const lookupRequest = {
  deployment_id: "0975def6-2786-4bc5-9b39-8e9441e6b96f",
  qos: {},
  service: "/face.Detector",
};

const runDetectionRequest = JSON.parse(open("../data/run_detection.json"));

export const options = {
  // A number specifying the number of VUs to run concurrently.
  vus: 30,
  // A string specifying the total duration of the test run.
  duration: "30s",

  // The following section contains configuration options for execution of this
  // test script in Grafana Cloud.
  //
  // See https://grafana.com/docs/grafana-cloud/k6/get-started/run-cloud-tests-from-the-cli/
  // to learn about authoring and running k6 test scripts in Grafana k6 Cloud.
  //
  // cloud: {
  //   // The ID of the project to which the test is assigned in the k6 Cloud UI.
  //   // By default tests are executed in default project.
  //   projectID: "",
  //   // The name of the test in the k6 Cloud UI.
  //   // Test runs with the same name will be grouped.
  //   name: "script.js"
  // },

  // Uncomment this section to enable the use of Browser API in your tests.
  //
  // See https://grafana.com/docs/k6/latest/using-k6-browser/running-browser-tests/ to learn more
  // about using Browser API in your test scripts.
  //
  // scenarios: {
  //   // The scenario name appears in the result summary, tags, and so on.
  //   // You can give the scenario any name, as long as each name in the script is unique.
  //   ui: {
  //     // Executor is a mandatory parameter for browser-based tests.
  //     // Shared iterations in this case tells k6 to reuse VUs to execute iterations.
  //     //
  //     // See https://grafana.com/docs/k6/latest/using-k6/scenarios/executors/ for other executor types.
  //     executor: 'shared-iterations',
  //     options: {
  //       browser: {
  //         // This is a mandatory parameter that instructs k6 to launch and
  //         // connect to a chromium-based browser, and use it to run UI-based
  //         // tests.
  //         type: 'chromium',
  //       },
  //     },
  //   },
  // }
};

// The function that defines VU logic.
//
// See https://grafana.com/docs/k6/latest/examples/get-started-with-k6/ to learn more
// about authoring k6 scripts.
//
export default function () {
  // console.log("start");
  schedulerClient.connect("127.0.0.1:50051", { plaintext: true });
  // console.log("connected");

  let lookupReply = schedulerClient.invoke(
    "laqista.Scheduler/Lookup",
    lookupRequest
  );
  // console.log(lookupReply);
  let url = new URL(lookupReply.message.server.addr);
  let address = url.host;
  appClient.connect(address, { plaintext: true });
  // console.log("connected 2");

  let detectionReply = appClient.invoke(
    "face.Detector/RunDetection",
    runDetectionRequest
  );
  // console.log("invoked");

  check(detectionReply, {
    "status is OK": (r) => r && r.status === StatusOK,
    "label is correct": (r) =>
      r && detectionReply.message.label.includes("spoonbill"),
  });
  // console.log(detectionReply);
  // console.log("checked");
}
