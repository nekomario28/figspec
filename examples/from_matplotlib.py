"""Take an existing matplotlib script's figure and make it a figspec model.

Nothing about the plotting code changes: the figure is converted, checked, and
re-rendered by the Rust engine.
"""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

import figspec

figure, axes = plt.subplots(figsize=(7, 4.4))
axes.bar(["Q1", "Q2", "Q3", "Q4"], [31.5, 34.2, 29.8, 41.0])
axes.set_ylim(25, None)          # the classic trimmed bar chart
axes.set_title("Quarterly revenue")
axes.set_xlabel("quarter")
axes.set_ylabel("revenue (M JPY)")

model = figspec.from_matplotlib(figure)
print("model:", model["marks"][0]["kind"], model["marks"][0]["values"], "y_min:", model["y_min"])
print("inspect:", [(p["code"], p["severity"]) for p in figspec.inspect(model)])
print("svg bytes:", len(figspec.render_svg(model)))
