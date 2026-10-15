"""One matplotlib figure with scatter, error bars and grouped bars, converted.

The plotting code is ordinary matplotlib; what changes is that the figure becomes
data, is checked, and is drawn by the Rust engine.
"""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

import figspec

fig, axes = plt.subplots(1, 3, figsize=(12, 3.6))
axes[0].scatter([1, 2, 3, 4], [2.1, 3.9, 3.2, 4.8])
axes[0].set_title("scatter")
axes[0].set_xlabel("x"); axes[0].set_ylabel("y")

axes[1].errorbar([1, 2, 3], [10, 12, 11], yerr=[[0.5, 1.0, 0.4], [0.8, 0.6, 1.2]])
axes[1].set_title("asymmetric error bars")
axes[1].set_xlabel("run"); axes[1].set_ylabel("metric (ms)")

axes[2].set_xticks([0, 1], labels=["Q1", "Q2"])
axes[2].bar([0.0, 1.0], [31.0, 41.0], width=0.4, label="north")
axes[2].bar([0.4, 1.4], [29.0, 33.0], width=0.4, label="south")
axes[2].set_title("grouped"); axes[2].set_xlabel("quarter"); axes[2].set_ylabel("revenue (M JPY)")

for index, axes_item in enumerate(fig.axes):
    model = figspec.from_matplotlib(fig, axes_index=index)
    kinds = [mark["kind"] for mark in model["marks"]]
    problems = [(p["code"], p["severity"]) for p in figspec.inspect(model)]
    print(f"axes {index}: {kinds} -> {problems}")
    figspec.save(model, f"/tmp/figspec/{['scatter', 'errorbar', 'grouped'][index]}.png")
