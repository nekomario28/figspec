# figspec

**A figure as data.** Declare the chart as a plain JSON model, render it with a
Rust engine, and inspect *the same model* — with stable codes, an exit code, and
no matplotlib in the renderer.

```sh
figspec inspect revenue.json     # problems, no rendering, no execution
figspec render  revenue.json -o revenue.svg
```

```python
import figspec

model = figspec.from_matplotlib(figure)   # or write the dict yourself
figspec.inspect(model)                    # [{'code': 'truncated-baseline', ...}]
figspec.save(model, "revenue.svg")
```

```json
{
  "title": "Quarterly revenue",
  "x_label": "quarter",
  "y_label": "revenue (M JPY)",
  "marks": [
    {"kind": "bar", "categories": ["Q1", "Q2", "Q3", "Q4"],
     "values": [31.5, 34.2, 29.8, 41.0]}
  ]
}
```

## Why the model is data

A chart is a claim about numbers. If the claim lives in an opaque object graph,
the only way to check it is to render it and look — so the check is a picture,
and a picture is not a gate.

Here the claim is a value you can diff, hash, store, generate and check *before*
anything is drawn:

- **Inspect without executing.** `inspect` reads the model. Nothing is imported,
  nothing is rendered, and no code from the artifact runs.
- **A renderer is a pure function.** The same model renders to the same bytes
  (the engine's clip ids are renumbered so they cannot leak a process counter), so
  two renders are comparable and a rendered artifact is content-addressable.
- **The same checks in both languages.** Python and the CLI go through one Rust
  implementation, so they cannot drift.

## The checks

Few on purpose. Each one says what is wrong, where, and what to do instead.

| code | severity | what it means |
| --- | --- | --- |
| `truncated-baseline` | error | bars start above zero, so their lengths no longer mean their values |
| `mismatched-lengths` | error | a bar mark's categories and values disagree in number |
| `non-finite-value` | error | a coordinate is NaN or infinite and cannot be placed |
| `non-positive-on-log-scale` | error | a log axis is asked to place zero or a negative value |
| `empty-figure` | error | the figure declares no marks |
| `no-title` | warn | the figure has no title, so a reader cannot tell what it shows |
| `no-axis-label` | info | an axis has no label |

Exit policy, the same as the CLI's: **0** nothing to report, **1** at least one
`error`, **2** the input could not be read. `warn` and `info` never fail a build.

An unknown field is refused rather than ignored — a mistyped `y_min` that is
silently dropped renders a different chart from the one that was written.

## What draws it

[`kuva`](https://github.com/Psy-Fer/kuva) — a mature MIT-licensed Rust plotting
library with 68 plot types and SVG/PNG/PDF backends. figspec is the **model, the
checks, and the Python surface**; it does not reimplement a renderer, and it is
not a fork of kuva. Reuse over reimplementation is the point.

## matplotlib, as an importer

matplotlib is supported as an *input*, never as a dependency of the renderer:

```python
model = figspec.from_matplotlib(figure)
```

That converts lines and bars, carries the y limits across (which is where a
truncated baseline comes from), and **refuses** a figure it cannot represent —
an image, a contour set, a 3-D projection — instead of converting part of it.
A converted figure that silently omits half the chart is worse than no
conversion, because it looks complete.

`pip install figspec` needs no matplotlib; `pip install figspec[matplotlib]` adds
it for the importer. A later release moves matplotlib into the default install so
the importer works out of the box — under one condition, recorded in
[`specs/slices/03-release.md`](specs/slices/03-release.md): importing figspec must
stay numpy-free, so the renderer and the checks keep working without it.

## What this is not

- **Not a matplotlib-compatible API.** You do not port your plotting code to
  figspec; you describe a figure, or you convert one, and then check it.
- **Not a chart engine.** The drawing is kuva's, including its layout and its
  text handling — see its documentation for what the output looks like.
- **Not a visualisation library for exploration.** It is aimed at figures that
  are *generated* — by a script, a pipeline, an agent — and must be checked
  before they are shipped.

## Install

```sh
pip install figspec          # from a wheel, no matplotlib, no numpy
```

Until the first release, from a checkout:

```sh
pip install maturin && maturin develop --release
cargo test
```

MIT licensed. The renderer is MIT-licensed too (kuva), and its dependency tree is
the one that ships an image.
