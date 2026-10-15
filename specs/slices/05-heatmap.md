# 05 — A matrix, and one list of what cannot be drawn

## The mark

`{"kind": "heatmap", "values": [[row], ...], "row_labels", "col_labels",
"color_map"}` — a matrix where the value is the colour. `color_map` is a closed
set (serde enum), so an unknown name is a **load** failure (exit 2) rather than a
silent fall back to the default ramp.

Measured against kuva 0.6.0, as the other marks were:

- a 2×3 matrix computes `x_range (0.5, 3.5)` and `y_range (0.5, 2.5)`, i.e. both
  axes use the same `i + 1` category pitch as bars, and its cells are drawn as one
  `RectBatch` spanning the plot rectangle;
- **row 0 is drawn at the bottom** (a y axis grows upward). Verified by eye as
  well as by geometry: a matrix whose first row holds the maximum renders that row
  at the bottom, with the bright cell where the values say.
- `row_labels`/`col_labels` are axis tick labels, so they are placed on the
  `Layout` (from the mark, so a name is written in exactly one place).

Its axes are categorical, so neither `truncated-baseline` nor a log-scale check
applies: the colour is the value, and colour is not a length — asserted in
`tests/heatmap.rs`.

The box is the category span on both axes — the plot rectangle — and
`tests/storyboard.rs` requires it to contain all six cells the engine drew.

## The defect this slice found first

`inspect` reported **no problems (exit 0)** for three figures the renderer refused
(exit 2):

| declaration | `inspect` before | `render` before |
| --- | --- | --- |
| `{"kind": "scatter", "points": []}` | clean | `mark 0: a scatter needs points` |
| `{"kind": "line", "points": [[1, 2]]}` | clean | `mark 0: a line needs at least two points` |
| a grouped bar with `series: []` | clean | `mark 0: a grouped bar needs a series` |

That contradicts the README's claim that the checks and the renderer work from
**the same list**. It is the same class as gxkit's reaudit finding R03 (a
recognised declaration falling back to "clean").

Fixed by making the claim structural rather than aspirational:

- `Mark::is_empty()` and the row/label checks live in `refusals()`, with two new
  codes — `empty-mark` and `short-line` — and a third for a genuine ambiguity,
  `ambiguous-labels` (two marks declaring different names for one axis: picking
  one would caption the other's data).
- `plots_of` no longer contains any refusal of its own. Its per-mark checks were
  deleted, because the refusals gate at its top now covers every one of them.

`tests/refusals.rs` checks it in **both** directions: everything that cannot be
drawn is an `error` *and* a render failure, and every mark kind in a valid figure
has no refusals and renders.

## `imshow`, converted

`from_matplotlib` now converts an `imshow` whose cells are a matrix of values, and
refuses the rest with the reason. Every rule came from measuring matplotlib:

| measured | what the converter does |
| --- | --- |
| a default image spans `(-0.5, cols - 0.5, rows - 0.5, -0.5)` with `origin="upper"` — row 0 is at the **top** | reverses the rows, because the model draws row 0 at the bottom |
| a default image's ticks are numeric fractions (`-0.5, 0.0, 0.5, ...`) | carries no names — those numbers are not cell names |
| `set_xticks([0, 1], labels=[...])` puts ticks exactly on the cells | carries those as `col_labels`/`row_labels`, reversed for `origin="upper"` |
| `get_ylim()` for an image is the **cell grid** in inverted order (`(2.5, -0.5)`) | drops it: carrying it set `y_min`, which stretched the cells and pushed the last row past the canvas |
| the figure's inches are a matplotlib notion and the engine sizes a matrix from its cells | drops `width`/`height` for a matrix figure |
| an explicit `extent`, a colour image, a masked cell, a ramp the model does not name, a non-`Normalize` norm, a partial tick naming, a log y axis | refused, each naming what it saw |

The conversion is checked cell by cell against matplotlib's own ramp output, so
the ramp, the values and the row order are verified together
(`tests_python/test_figspec.py`), and the two pictures were compared by eye.

### The defect that measurement found

Writing the conversion exposed that `y_min` and a log scale are **not inert** for a
heatmap: measured, `y_min` stretches its cells (3 rows of a 548 px canvas: 148.5 px
each at `y_min` 0.5, 178.2 at 1.0, 445.5 at 2.5 — the last row beyond the canvas)
and a log scale changes them (106.3 px). So a figure that declares either for a
heatmap is now refused as `inapplicable-axis-setting` rather than drawn as a
picture that follows an instruction it cannot honour. The earlier note in this file
that they were "not a finding" was an assumption; the render says otherwise.
