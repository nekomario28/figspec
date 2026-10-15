# figspec

**A figure as data.** Declare the chart as a plain JSON model, render it with a
Rust engine, and inspect *the same model* — with stable codes, an exit code, and
no matplotlib in the renderer.

```sh
figspec inspect    revenue.json            # problems, no rendering, no execution
figspec render     revenue.json -o revenue.svg
figspec storyboard revenue.json -o revenue.storyboard.svg   # with the problems boxed
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
     "values": [31.5, 34.2, 29.8, 41.0]},
    {"kind": "line", "points": [[0, 31.5], [3, 41.0]], "label": "trend",
     "y_err": [0.4, 0.5, 0.3, 0.6]},
    {"kind": "scatter", "points": [[0, 30.0], [1, 33.0]]},
    {"kind": "grouped_bar", "categories": ["Q1", "Q2"],
     "series": [{"label": "north", "values": [31.0, 41.0]},
                {"label": "south", "values": [29.0, 33.0]}]},
    {"kind": "heatmap",
     "values": [[31.5, 34.2], [22.0, 25.5]],
     "row_labels": ["north", "south"], "col_labels": ["Q1", "Q2"],
     "color_map": "viridis"}
  ]
}
```

`y_err` is one magnitude per point, drawn both ways; `y_err_asymmetric` is
`[lower, upper]` per point when the sides differ. Declaring both on one mark is
refused, because one drawing cannot mean two things.

A `heatmap` is a matrix: the value is the colour, both axes are categorical, and
`color_map` is a closed set (`viridis` — the default — `turbo`, `inferno`,
`magma`, `plasma`, `cividis`, `warm`, `cool`, `cubehelix`), so a typo cannot
quietly become the default ramp. Its **row 0 is drawn at the bottom**, which is
what a y axis means; `row_labels` is read in the same order as `values`. Its rows
*are* its y axis, so `y_min` and a log scale **cannot apply to it** — and they are
not inert either: measured, `y_min` stretches the cells (3 rows of a 548 px canvas:
148.5 px each at `y_min` 0.5, 178.2 at 1.0, 445.5 at 2.5, the last row ending up
past the canvas) and a log scale changes them (106.3 px). Declaring either is
`inapplicable-axis-setting`, so the picture cannot follow an instruction it cannot
honour.

## The storyboard

`inspect` says what is wrong and which mark it is about. A reader still has to
find that mark, so `storyboard` draws the figure and boxes the marks the findings
name — numbered with the report's own positions:

```
revenue.json: 2 marks
inspect: 4 problems (1 error)
  [1] error m[0]     truncated-baseline: bars start at y=5, not 0, ...
  [2] warn  -        no-title: the figure has no title, ...
storyboard: 1 box(es) in revenue.storyboard.svg
  [1] mark 0
  finding(s) 2,3,4 are about no mark and are not boxed
```

The box is the mark's **real rectangle**, computed from the layout the engine
computed — kuva publishes the plot rectangle and the ranges it drew against, and
`tests/storyboard.rs` requires every box to land on the primitive kuva drew, for
every mark kind, including log axes and a truncated baseline. So the picture
cannot disagree with the report.

A finding that is about no particular mark (a missing title) is reported and
**not** boxed: there is nowhere honest to point.

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
| `mismatched-lengths` | error | a collection disagrees in number with what it annotates: a group's series, or an error array and its points |
| `non-finite-value` | error | a coordinate is NaN or infinite and cannot be placed |
| `non-positive-on-log-scale` | error | a log axis is asked to place zero or a negative value |
| `two-error-shapes` | error | a mark declares symmetric and asymmetric error bars at once |
| `negative-error` | error | an error magnitude is negative, which is not a distance |
| `empty-figure` | error | the figure declares no marks |
| `empty-mark` | error | a mark declares nothing to draw, so it would render as nothing |
| `short-line` | error | a line has fewer than two points, so there is no line to draw |
| `ambiguous-labels` | error | two marks declare different names for the same axis, so one would caption the other |
| `inapplicable-axis-setting` | error | a heatmap's rows are its y axis, so `y_min` and a log scale cannot apply to it |
| `no-title` | warn | the figure has no title, so a reader cannot tell what it shows |
| `no-axis-label` | info | an axis has no label |

Two kinds of finding, and the difference is deliberate:

- **Refusals** — shapes that cannot be drawn as written. The checks report these
  and the renderer refuses them *from the same list*, so a rule cannot hold in one
  and be ignored by the other. The renderer has no refusals of its own: it consults
  `refusals()` and nothing else, which `tests/refusals.rs` checks in both
  directions. (Measured twice while writing it: a short error array was reported by
  `inspect` and rendered anyway, silently without that point's bar; and an empty
  scatter, a one-point line and a grouped bar with no series were reported as a
  **clean figure** (exit 0) and refused by the renderer (exit 2).)
- **Judgement** — a figure that draws fine while being misleading, such as bars
  that start above zero. Rendering it is how a reader sees the problem, so it is
  an error, not a refusal.

Exit policy, the same as the CLI's: **0** nothing to report, **1** at least one
`error`, **2** the input could not be read. `warn` and `info` never fail a build.

`truncated-baseline` is reported **once per categorical mark**: two bar marks on a
trimmed axis are two misleading marks, and a fix to one of them is not the other.

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

That converts **lines, scatter, single and grouped bars, symmetric or asymmetric
error bars, and an `imshow` whose cells are a matrix of values**, carries the y
limits across (which is where a truncated baseline comes from), and **refuses**
what it cannot represent: a colour image, a contour or violin set, a stacked bar,
bars that sit between tick labels (the model's bars are categorical, and inventing
category names would be a label the figure never had), a group whose series
disagree in length, an image drawn over an explicit `extent` or in a colour ramp
the model does not name, and an image with a masked cell. A converted figure that
silently omits half the chart is worse than no conversion, because it looks
complete.

For an `imshow` the picture is checked cell by cell: every converted cell's colour
is compared with matplotlib's own ramp output for the value it holds, so the ramp,
the values and the row order are verified together. An `origin="upper"` image is
reversed, because the model draws row 0 at the bottom. The source's y limits and
canvas size are dropped, because an image's y limits are its cell grid and the
engine sizes a matrix from its cells.

Grouped bars follow matplotlib's own convention — one `axes.bar` call per series,
the same x array with an offset — and the positions are *checked* against it, so
two unrelated bar charts on one axes are refused rather than paired.

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
