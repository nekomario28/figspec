# What has been attacked, and what held

A record of independent reviews of this project's *claims* — its codes' names, its
importer's reading, its agreement with the sibling route — written down so the next
review can differ from this one deliberately instead of repeating it. Each entry
names what was probed, what was found, and what the probe could not reach.

The first two reviews of the sibling project `gxkit` (whose `chartgx` reads the same
model files) are recorded in its `docs/SEMANTICS.md`; this file covers figspec.

## Third review: the checks' names, the matplotlib importer, and the two routes

An independent reviewer (`opencode` with `deepseek-v4.1-flash-free`) was given
figspec plus the consumer route and asked to falsify: each code's name in both
directions, `from_matplotlib` against matplotlib's own objects, and the two routes
against each other on a corpus of models. **Eighteen findings.** Every one was
reproduced here before it was fixed, and the fixes carry tests and mutations (32
mutants, up from 18).

### A verdict about a figure that did not hold

| the model said | the tool said |
| --- | --- |
| a line whose *x* is 0 with `y_scale: "log"` (the log axis is y) | `non-positive-on-log-scale`, and `render` refused it |
| two heatmaps naming the **columns** differently | clean; the second set of names was drawn over the first's cells |
| `{"categories": [], "series": [{"values": []}]}` | clean; the other route called it empty |
| `"title": ""` | a title |
| `{"width": 0}` | clean, and a 0-pixel-wide SVG was written |

The first is the per-axis mistake: `values()` concatenated x and y, so either
axis' numbers could be reported against the other's scale. The third was
`GroupedBar`'s emptiness testing only `series.is_empty()` while
`mismatched-lengths` agreed with it (`0 == 0`). The last was `u32` accepting zero;
the model reader refuses it now, which is also how the other route refuses it.

### A reading that invented or dropped material

- `capsize` adds two cap `Line2D`s to `axes.get_lines()`; each became a full line
  mark (a two-point error bar converted into three marks). `container.lines[1]`
  holds them as *elements*, not as itself — the first attempt at this fix accounted
  for the tuple and changed nothing.
- `errorbar(..., xerr=...)` was read as a y-errorbar of length zero: the magnitudes
  came from `container[2][0]`, which for an x-errorbar is the horizontal
  collection. `has_xerr`/`has_yerr` decide now, and either is refused with its name
  (a figure with both used to be refused because the *y* collection looked
  unsupported under a wrong name).
- `errorbar(..., fmt="none")` has no data line, and `None.get_xydata()` was an
  `AttributeError` rather than a conversion.
- `barh` was diagnosed as a stacked bar chart, because `abs(get_y()) > 0` is what a
  horizontal bar's *category position* looks like.
- Two `bar` calls at the same x were merged into a `grouped_bar` — their positions
  are identical, so the "not side by side" guard never tripped and an overlapping
  chart was drawn side by side.
- Bars half a step off their tick were still named by that tick: the tolerance was
  the bar width (`0.8`), not a fraction of a step.
- A scatter's `s=[…]` and `c=[…], cmap=…` vanished (the model draws one size and one
  colour), and twin axes' second series was dropped silently — the importer's own
  principle ("silently omitting half the chart is worse than no conversion") did not
  hold for them.
- An inverted y axis produced a false `truncated-baseline` at the *top* of the
  chart, because `get_ylim()[0]` is the top limit when inverted.
- The canonical `x ± width/2` grouped bar was **refused** as "bars whose width
  differs": `0.4` and `0.4000000000000001` are not the same float. Measured
  dimensions are compared with a tolerance now.

### The two routes, on fifty models

The error/no-error *verdict* agreed except where the two defects above made it
differ (`grouped_bar` emptiness, `width: 0`) — and where the model readers differ
on `NaN`/`Infinity`, which figspec refuses at parse (Serde) and the Python reader
accepts.

**Deliberate, not defects:** the two routes use their own codebooks
(`no-axis-label`/info against `no-xlabel`/warn), and `chartgx` emits additional
codes (`empty-chart`, `mismatched-lengths` on an empty array) without changing the
verdict. Each tool's codes are stable for its own consumers.

### Attacked and found sound

`empty-figure`; `short-line` in both directions; `two-error-shapes`; `negative-error`
(including `0.0` and `-0.0`); `mismatched-lengths` across every pair it compares;
`truncated-baseline` for `y_min` of 0 and below; `inapplicable-axis-setting` for a
matrix's y axis; `ambiguous-labels` for the row axis.

Importer conversions verified faithful: a line; a scatter; a bar (single, `width=0.4`,
negative, `bottom=[0,0]`); symmetric and asymmetric `errorbar`; an `imshow` matrix in
both origins, with 2 and 3 rows, with and without tick labels. Refusals verified
correct: a colour image, `contour`, `violinplot`, a stacked bar, an explicit
`extent`, a masked cell, an unnamed ramp, a log y image, a non-`Normalize` norm,
partial tick labels.

### Not reachable through the documented surfaces

`non-finite-value` cannot be exercised: the model reader refuses `NaN`/`Infinity`
at parse, so no readable model holds one (`chartgx`, whose reader is Python's
`json.loads`, accepts the same file — recorded above as a route difference).
`docgx`-style claims with no observable surface were not attacked.
