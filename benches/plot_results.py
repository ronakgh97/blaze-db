#!/usr/bin/env python3

import json
import os

import matplotlib.pyplot as plt

plt.style.use("dark_background")
plt.rcParams["font.family"] = "monospace"
plt.rcParams["font.size"] = 11
plt.rcParams["axes.facecolor"] = "#1e1e1e"
plt.rcParams["figure.facecolor"] = "#1e1e1e"
plt.rcParams["axes.edgecolor"] = "#444"
plt.rcParams["axes.labelcolor"] = "#ccc"
plt.rcParams["xtick.color"] = "#ccc"
plt.rcParams["ytick.color"] = "#ccc"
plt.rcParams["text.color"] = "#ccc"

COLORS = ["#ff5050", "#50c878", "#6495ff", "#ffc850", "#c864ff"]


def load_results():
    with open("benches/benchmark_results.json") as f:
        data = json.load(f)
        items = data
    return {b["name"]: b for b in items}


def plot_construction_speed(results):
    sizes = [2560, 5120, 10240, 20480, 40960, 81920]
    speeds = [float(results[f"construction_{s}"]["metric"].split()[0]) for s in sizes]

    fig, ax = plt.subplots(figsize=(10, 6))
    ax.plot(sizes, speeds, marker="o", color=COLORS[0], linewidth=2.5, markersize=10)
    ax.set_xlabel("Index Size (vectors)", fontsize=12)
    ax.set_ylabel("Vectors/sec", fontsize=12)
    ax.set_title(
        "Construction Speed: How Fast We Build the Index",
        fontsize=14,
        fontweight="bold",
        pad=15,
    )
    ax.grid(alpha=0.3)
    ax.set_xscale("log")
    ax.set_xticks(sizes)
    ax.set_xticklabels([f"{s:,}" for s in sizes])

    for s, sp in zip(sizes, speeds):
        ax.annotate(
            f"{sp:.0f}",
            (s, sp),
            textcoords="offset points",
            xytext=(0, 12),
            ha="center",
            fontsize=10,
            fontweight="bold",
        )

    plt.tight_layout()
    plt.savefig("benches/plots/construction_speed.png", dpi=150)
    print("Saved: benches/plots/construction_speed.png")
    plt.close()


def plot_construction_ef(results):
    efs = 64, 128, 256, 512, 768, 1024
    speeds = [
        float(results[f"construction_ef_{ef}"]["metric"].split()[0]) for ef in efs
    ]

    fig, ax = plt.subplots(figsize=(8, 6))
    ax.plot(efs, speeds, marker="s", color=COLORS[1], linewidth=2.5, markersize=10)
    ax.set_xlabel("ef_construction", fontsize=12)
    ax.set_ylabel("Vectors/sec", fontsize=12)
    ax.set_title(
        "Effect of ef_construction on Build Speed",
        fontsize=13,
        fontweight="bold",
        pad=15,
    )
    ax.grid(alpha=0.3)
    ax.set_xscale("log")
    ax.set_xticks(efs)
    ax.set_xticklabels(efs)

    for ef, sp in zip(efs, speeds):
        ax.annotate(
            f"{sp:.0f}",
            (ef, sp),
            textcoords="offset points",
            xytext=(0, 12),
            ha="center",
            fontsize=10,
            fontweight="bold",
        )

    plt.tight_layout()
    plt.savefig("benches/plots/construction_ef.png", dpi=150)
    print("Saved: benches/plots/construction_ef.png")
    plt.close()


def plot_qps_vs_k(results):
    ks = [12, 24, 48, 96, 192, 384]
    qps = [float(results[f"search_qps_at_k_{k}"]["metric"].split()[0]) for k in ks]

    fig, ax = plt.subplots(figsize=(10, 6))
    ax.plot(ks, qps, marker="o", color=COLORS[2], linewidth=2.5, markersize=10)
    ax.fill_between(ks, qps, alpha=0.15, color=COLORS[2])

    ax.set_xlabel("K (number of results to return)", fontsize=12)
    ax.set_ylabel("Queries Per Second (QPS)", fontsize=12)
    ax.set_title(
        "Search Throughput: Higher K = Slower Queries",
        fontsize=14,
        fontweight="bold",
        pad=15,
    )
    ax.grid(alpha=0.3)
    ax.set_xscale("log")
    ax.set_xticks(ks)
    ax.set_xticklabels(ks)

    for k, q in zip(ks, qps):
        offset = 15 if k <= 48 else -30
        ax.annotate(
            f"{q:,}",
            (k, q),
            textcoords="offset points",
            xytext=(0, offset),
            ha="center",
            fontsize=10,
            fontweight="bold",
        )

    plt.tight_layout()
    plt.savefig("benches/plots/qps_vs_k.png", dpi=150)
    print("Saved: benches/plots/qps_vs_k.png")
    plt.close()


def plot_latency_vs_k(results):
    ks = [12, 24, 48, 96, 192, 384]
    p50 = [results[f"search_latency_p50_k_{k}"]["time_ms"] for k in ks]
    p95 = [results[f"search_latency_p95_k_{k}"]["time_ms"] for k in ks]
    p99 = [results[f"search_latency_p99_k_{k}"]["time_ms"] for k in ks]

    fig, ax = plt.subplots(figsize=(10, 6))

    ax.plot(
        ks,
        p50,
        marker="o",
        color=COLORS[0],
        linewidth=2.5,
        markersize=8,
        label="p50 (median)",
    )
    ax.plot(
        ks, p95, marker="s", color=COLORS[1], linewidth=2.5, markersize=8, label="p95"
    )
    ax.plot(
        ks, p99, marker="^", color=COLORS[2], linewidth=2.5, markersize=8, label="p99"
    )
    ax.fill_between(ks, p50, p99, alpha=0.1, color=COLORS[0])

    ax.set_xlabel("K (number of results to return)", fontsize=12)
    ax.set_ylabel("Latency (milliseconds)", fontsize=12)
    ax.set_title(
        "Query Latency: p50 vs p95 vs p99", fontsize=14, fontweight="bold", pad=15
    )
    ax.legend(loc="upper left", framealpha=0.9)
    ax.grid(alpha=0.3)
    ax.set_xscale("log")
    ax.set_xticks(ks)
    ax.set_xticklabels(ks)

    for k, v in zip(ks, p50):
        ax.annotate(
            f"{v:.1f}ms",
            (k, v),
            textcoords="offset points",
            xytext=(8, 8),
            ha="left",
            fontsize=9,
            color=COLORS[0],
        )
    for k, v in zip(ks, p95):
        ax.annotate(
            f"{v:.1f}ms",
            (k, v),
            textcoords="offset points",
            xytext=(12, 8),
            ha="left",
            fontsize=9,
            color=COLORS[1],
        )
    for k, v in zip(ks, p99):
        ax.annotate(
            f"{v:.1f}ms",
            (k, v),
            textcoords="offset points",
            xytext=(12, 10),
            ha="left",
            fontsize=9,
            color=COLORS[2],
        )

    plt.tight_layout()
    plt.savefig("benches/plots/latency_vs_k.png", dpi=150)
    print("Saved: benches/plots/latency_vs_k.png")
    plt.close()


def plot_recall_vs_ef(results):
    efs = [32, 64, 128, 256, 512, 768, 1024]
    recalls = [
        float(results[f"recall_at_64_ef_{ef}"]["metric"].replace("%", "")) for ef in efs
    ]

    fig, ax = plt.subplots(figsize=(10, 6))

    ax.axhline(
        y=90, color="gray", linestyle="--", alpha=0.6, linewidth=1.5, label="90% target"
    )
    ax.plot(efs, recalls, marker="o", color=COLORS[3], linewidth=2.5, markersize=10)
    ax.fill_between(efs, recalls, alpha=0.15, color=COLORS[3])

    ax.set_xlabel("ef_search (search-time exploration width)", fontsize=12)
    ax.set_ylabel("Recall % (accuracy vs brute-force)", fontsize=12)
    ax.set_title(
        "Recall: Higher ef_search = Better Accuracy",
        fontsize=14,
        fontweight="bold",
        pad=15,
    )
    ax.grid(alpha=0.3)
    ax.set_xscale("log")
    ax.set_ylim(70, 100)
    ax.set_xticks(efs)
    ax.set_xticklabels(efs)
    ax.legend(loc="lower right")

    for ef, rec in zip(efs, recalls):
        ax.annotate(
            f"{rec:.1f}%",
            (ef, rec),
            textcoords="offset points",
            xytext=(0, 12),
            ha="center",
            fontsize=9,
            fontweight="bold",
        )

    plt.tight_layout()
    plt.savefig("benches/plots/recall_vs_ef.png", dpi=150)
    print("Saved: benches/plots/recall_vs_ef.png")
    plt.close()


def main():
    os.makedirs("benches/plots", exist_ok=True)
    results = load_results()

    print("Generating plots...")
    plot_construction_speed(results)
    plot_construction_ef(results)
    plot_qps_vs_k(results)
    plot_latency_vs_k(results)
    plot_recall_vs_ef(results)
    print("\nAll plots saved to benches/plots/")


if __name__ == "__main__":
    main()
