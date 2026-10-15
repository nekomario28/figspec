# figspec — plan

## Goal

A figure whose model can be checked *before* it is drawn, rendered by a Rust
engine, and produced by a converter from matplotlib — so a chart is inspectable
without executing anything and without importing a 30 MB dependency.

## Decided

- **The renderer is not ours.** `kuva` (MIT, 68 plot types, SVG/PNG/PDF) draws;
  this repository owns the model, the checks, and the Python surface. Verified
  reuse, not a fork.
- **The model is the artifact.** Checks read it; rendering is a pure function of
  it. Unknown fields are refused, and a mark that cannot be drawn is an error.
- **matplotlib is an importer**, never a runtime dependency of the renderer. A
  figure the model cannot hold is refused rather than half-converted.
- **Two languages, one implementation.** Python and the CLI both call the same
  Rust functions, over JSON.

## Slices

- [x] [01 — model, checks, renderer, CLI](slices/01-core.md)
- [x] [02 — python bindings and the matplotlib importer](slices/02-python.md)
- [ ] [03 — distribution](slices/03-release.md)

## Evidence

- `cargo test`: 13 properties, including byte-identical renders and a refused
  unknown field. One test caught a false claim on the first run: the engine's
  clip ids come from a process-global counter, so two renders inside one process
  differed. The SVG is renumbered now.
- Rendered PNGs inspected by eye: an honest bar+line figure, and the same data
  with `y_min: 25` where Q1 looks ~2.5× Q3 instead of 30× — which is the finding
  `truncated-baseline` reports.
- `tests_python`: the model round-trips, checks match the CLI's, a real
  matplotlib bar chart converts (including its trimmed axis), and an `imshow`
  figure is refused.

## Not in scope

A matplotlib-compatible API, a new renderer, 3-D, or interactive output. The
engine's text and layout are kuva's, and its gaps are kuva's.
