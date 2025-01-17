import json
import os

# type: ignore
import matplotlib.pyplot as plt


def read_times(json_path: str, unit: str) -> list[float]:
    x = 1_000 if unit == "us" else 1_000_000

    data = None

    with open(json_path, "r") as f:
        content = f.read()
        data = json.loads(content)

    out = []

    for iters, time in zip(data["iters"], data["times"]):
        if iters == 0 or time == 0:
            continue

        out.append(time / iters / x)

    return out


this_dir = os.path.dirname(__file__)
results_dir = os.path.join(this_dir, "../benchmark-results")


def vs_native():
    vs_native_dir = os.path.join(results_dir, "mac_2025-01-10_20-17-24_0bcae86")
    infer_json = f"{vs_native_dir}/Onnx inference/onnx inference only inference/pelican/new/sample.json"
    native_json = f"{vs_native_dir}/Face native/face native direct full image/_client_/new/sample.json"
    wasm_json = f"{vs_native_dir}/Face wasm/face wasm direct full image/_client_/new/sample.json"

    native_overhead = {
        "infer": read_times(infer_json, "ms"),
        "native": read_times(native_json, "ms"),
        "wasm": read_times(wasm_json, "ms"),
    }

    fig, ax = plt.subplots()
    ax.boxplot(list(native_overhead.values()), tick_labels=native_overhead.keys())
    ax.set_ylabel("milli second / request")

    plt.savefig(f"{this_dir}/native-boxplot.pdf")


def vs_direct():
    vs_direct_dir = os.path.join(results_dir, "mac_2025-01-10_20-17-24_0bcae86")

    direct_json = f"{vs_direct_dir}/Face wasm/face wasm direct full image/_client_/new/sample.json"
    scheduled_json = f"{vs_direct_dir}/Face wasm/face wasm scheduled full image/_client_/new/sample.json"

    native_overhead = {
        "direct": read_times(direct_json, "ms"),
        "scheduled": read_times(scheduled_json, "ms"),
    }

    fig, ax = plt.subplots()
    ax.boxplot(list(native_overhead.values()), tick_labels=native_overhead.keys())
    ax.set_ylabel("milli second / request")

    plt.savefig(f"{this_dir}/schedule-boxplot.pdf")
    plt.show()


vs_native()
vs_direct()
