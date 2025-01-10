import { URL } from "https://jslib.k6.io/url/1.0.0/index.js";
import { Counter } from "k6/metrics";
import { Client, StatusOK } from "k6/net/grpc";
import { check } from "k6";

const SCHEDULER = "133.27.186.106:50051";
const SCHEDULER_URL = "http://133.27.186.106:50051";

const NAN1 = "http://133.27.171.130:50051";
const NAN2 = "http://133.27.171.130:50052";
const NAN3 = "http://133.27.171.130:50053";
const NAN4 = "http://133.27.171.58:50051";

// Just for deployment
const schedulerClient = new Client();
schedulerClient.load(["definitions"], "../../proto/laqista.proto");

const appClient = new Client();
appClient.load(["definitions"], "../../proto/face.proto");

const qosLookupRequest = {
  name: "face",
  qos: {
    latency_ms: 40,
  },
  service: "/face.Detector",
};
const noQosLookupRequest = {
  name: "face",
  qos: null,
  service: "/face.Detector",
};


const runDetectionRequest = JSON.parse(open("../data/run_detection.json"));

const schedulerCounter = new Counter("scheduler_counter");
const otherCounter = new Counter("other_counter");
const counters = {
  [SCHEDULER_URL]: new Counter("scheduler_counter"),
  [NAN1]: new Counter("nan1_counter"),
  [NAN2]: new Counter("nan2_counter"),
  [NAN3]: new Counter("nan3_counter"),
  [NAN4]: new Counter("nan4_counter"),
}

const withQosCounter = new Counter("with_qos");
const withoutQosCounter = new Counter("without_qos");

export const options = {
  scenarios: {
    with_qos: {
      exec: "withQos",

      executor: "constant-arrival-rate",
      startTime: "5s",
      duration: "20s",
      rate: 24,
      timeUnit: '1s',
      preAllocatedVUs: 24,
      maxVUs: 24,
    },

    no_qos: {
      exec: "noQos",

      executor: "constant-arrival-rate",
      duration: "30s",
      rate: 24,
      timeUnit: '1s',
      preAllocatedVUs: 24,
      maxVUs: 24,
    }
  }
  // A number specifying the number of VUs to run concurrently.
  // vus: 30,
  // A string specifying the total duration of the test run.
  // duration: "20s",

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

export function withQos() {
  run(qosLookupRequest)

}

export function noQos() {
  run(noQosLookupRequest)
}

function run(lookupRequest) {
  schedulerClient.connect(SCHEDULER, { plaintext: true });

  let lookupReply = schedulerClient.invoke(
    "laqista.Scheduler/Lookup",
    lookupRequest
  );

  if (typeof lookupReply.message.server?.addr === "undefined") {
    console.log({ lookupReply });
    fail(`Lookup failed: ${lookupReply}`);
    return
  }

  let url = new URL(lookupReply.message.server.addr);
  let address = url.host;
  appClient.connect(address, { plaintext: true });

  counters[lookupReply.message.server.addr].add(1);

  let detectionReply = appClient.invoke(
    "face.Detector/RunDetection",
    runDetectionRequest
  );

  check(detectionReply, {
    "status is OK": (r) => r && r.status === StatusOK,
    "label is correct": (r) =>
      r && detectionReply.message.label.includes("spoonbill"),
  });
}
