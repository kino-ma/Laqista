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


def read_avg(json_path: str, unit: str) -> float:
    times = read_times(json_path, unit)
    return sum(times) / len(times)


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

    fig.savefig(f"{this_dir}/native-boxplot.pdf")
    plt.close()


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

    fig.savefig(f"{this_dir}/schedule-boxplot.pdf")
    plt.close()


def all_latency():
    latency_dir = os.path.join(results_dir, "mac_2025-01-10_20-17-24_0bcae86")

    infer_json = f"{latency_dir}/Onnx inference/onnx inference only inference/pelican/new/sample.json"
    native_json = f"{latency_dir}/Face native/face native direct full image/_client_/new/sample.json"
    direct_json = (
        f"{latency_dir}/Face wasm/face wasm direct full image/_client_/new/sample.json"
    )
    e2e_json = f"{latency_dir}/Face wasm/face wasm scheduled full image/_client_/new/sample.json"

    latencies = {
        "infer": read_avg(infer_json, "ms"),
        "native": read_avg(native_json, "ms"),
        "direct": read_avg(direct_json, "ms"),
        "e2e": read_avg(e2e_json, "ms"),
    }

    bar_colors = ["tab:blue", "tab:orange", "tab:green", "tab:red"]

    fig, ax = plt.subplots()

    p = ax.bar(latencies.keys(), latencies.values(), color=bar_colors)
    ax.bar_label(p, label_type="center")

    ax.set_ylabel("milli second / request")

    fig.savefig(f"{this_dir}/all-latency-boxplot.pdf")
    plt.show()
    plt.close()


def throughput():
    server_tp = 461.67
    desktop_tp = 135.30
    laptop_tp = 240.78
    cloud_tp = 484.30

    tps = {
        "server": server_tp,
        "desktop": desktop_tp,
        "laptop": laptop_tp,
        "cloud": cloud_tp,
    }

    bar_colors = ["tab:blue", "tab:orange", "tab:green", "tab:red"]

    fig, ax = plt.subplots()

    p = ax.bar(tps.keys(), tps.values(), color=bar_colors)
    ax.bar_label(p, label_type="center")

    bottom = 0
    for machine, color in zip(["server", "desktop", "laptop"], bar_colors):
        p = ax.bar("ideal", tps[machine], bottom=bottom, color=color)
        ax.bar_label(p, label_type="center")
        bottom += tps[machine]

    ax.set_ylabel("requests / second")
    fig.savefig(f"{this_dir}/throughput-bar.pdf")

    plt.close()

    schedule_tp = 4250.40
    bar_color = "tab:purple"
    fig, ax = plt.subplots()
    p = ax.bar("Edge-less API", schedule_tp, color=bar_color, width=0.8)
    fig.savefig(f"{this_dir}/edgeless-throughput-bar.pdf")

    plt.close()


vs_native()
vs_direct()
all_latency()
throughput()
