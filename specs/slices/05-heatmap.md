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

## Not in this slice

`from_matplotlib` still refuses `imshow`: converting it needs the extent and
origin conventions and the axis semantics checked against matplotlib's own
rendering. The refusal now names the heatmap mark, so the reader knows the model
can express it even though the converter does not.
