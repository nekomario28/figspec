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

A **heatmap** mark carries a matrix, optional row/column names and a closed set of
colour ramps; both axes are categorical, and its checks (ragged rows, label counts,
an empty matrix, two names for one axis) live in the same `refusals()` list the
renderer consults — see [slice 05](slices/05-heatmap.md).

Five defects were found by the tests written for them, which is why the tests
came first:

- an error array one element short was **reported by the checks and rendered
  anyway** — the renderer now refuses from the same list the checks report;
- the axis stopped at the data maximum, so an error bar reaching 12.6 was clipped
  by a range ending at 12.0 — and the first fix did nothing, because the engine
  caps the padded range against the raw one, so both had to be extended;
- `from_matplotlib` took the *first* axes' title for any axes, so a converted
  chart could carry another chart's title;
- a **storyboard** was impossible for a declared figure until kuva's computed
  layout was found to be public — measured rather than assumed, so no fork and no
  second layout derivation;
- and `inspect` called a figure **clean** (exit 0) in three places the renderer
  refused (exit 2): an empty scatter, a one-point line, a grouped bar with no
  series. The renderer now has no refusals of its own at all.

## Slices

- [x] [01 — model, checks, renderer, CLI](slices/01-core.md)
- [x] [02 — python bindings and the matplotlib importer](slices/02-python.md)
- [ ] [03 — distribution](slices/03-release.md)
- [x] [04 — the storyboard](slices/04-storyboard.md)
- [x] [05 — a matrix, and one list of what cannot be drawn](slices/05-heatmap.md)

## Mechanisms added after the slices

Three things are not slices but parts of the repository now, and each exists
because a claim needed evidence rather than an argument:

- [`docs/CONTRACT.md`](../docs/CONTRACT.md) — what an integrator may rely on, and
  explicitly what may change in a minor version. The drawing itself is on the
  second list: layout, fonts and cell aspect are `kuva`'s.
- `scripts/check_mutations.py` — 16 named faults, one per rule, each required to
  make its test fail **with an assertion**, after green controls. Writing it found a
  test that compared a log axis only at its own two ends, where a logarithmic map
  and a linear one agree.
- `tests/docs.rs` — every relative link in the entry documents must resolve.
  Written after a README learned to point at files that did not exist.

## Evidence

- `cargo test`: 46 properties, including byte-identical renders and a refused
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
