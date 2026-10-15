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

## The model, extended

`line` and `scatter` carry `y_err` (symmetric) or `y_err_asymmetric`; `grouped_bar`
carries one entry per series; `bar` stays the single-series shape. They are
separate mark kinds rather than optional fields, because one series and several
are different shapes and a field that changed the meaning would be two shapes
pretending to be one.

Three defects were found by the tests written for them, which is why the tests
came first:

- an error array one element short was **reported by the checks and rendered
  anyway** — the renderer now refuses from the same list the checks report;
- the axis stopped at the data maximum, so an error bar reaching 12.6 was clipped
  by a range ending at 12.0 — and the first fix did nothing, because the engine
  caps the padded range against the raw one, so both had to be extended;
- `from_matplotlib` took the *first* axes' title for any axes, so a converted
  chart could carry another chart's title.

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

## A possible integration, recorded rather than assumed

The sibling project `gxkit` checks artifacts, and its `chartgx` tool currently
executes a matplotlib script and reads the resulting `Figure` — so checking a
chart means running the chart's code. With a figure as data, the same check reads
a model instead: `inspect` needs neither matplotlib nor the artifact's execution,
and a converted matplotlib figure and a hand-written model are checkable by one
implementation. That is a reason this project exists; it is not a commitment
about when `chartgx` changes, and nothing here depends on it.
