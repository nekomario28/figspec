# 02 — python bindings and the matplotlib importer

`pyo3`/`maturin` bindings exposing the same two operations over JSON, a thin
Python package, and `from_matplotlib(figure)` which converts lines and bars,
carries the y limits, and refuses artists the model cannot hold.

Checkpoint: the Python checks match the CLI's; a converted matplotlib figure
round-trips through inspect and render; an unsupported artist raises.
