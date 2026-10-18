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

## Fourth review: the input boundary, in both build profiles

The third review read the *model*; this one aimed at a class it could not see: a
declaration the model accepts, that reaches the raster backend, and makes it panic —
exit 101 with a Rust backtrace, where this project's own policy says exit 2 for a
figure it cannot draw. 59 models (29 malformed, 28 valid but extreme) were run
through every command — `inspect --json`, `storyboard`, `render -o *.svg`,
`render -o *.png` — in **both build profiles**, because a wrapped arithmetic behaves
differently in each.

### A panic where a refusal belongs

| the model declared | debug build | release build |
| --- | --- | --- |
| `"width": 4294967295, "height": 4294967295` (a `u32`, so the model accepted it) | `render -o x.png` exit 101, `attempt to multiply with overflow` at kuva `backend/raster.rs` | exit 101, `capacity overflow` |
| `{"kind":"scatter","points":[[0,0]],"size":1e308}` (also `1e9`) | `render -o x.png` exit 101, `attempt to multiply with overflow` at `raster.rs` | **exit 0, and a written PNG** — the mask size had wrapped |

The second row is the one worth keeping: in the build that ships, a marker length
that cannot be drawn was drawn anyway, silently. Both routes are the same budget —
`Canvas::new` allocates `w*h*4` bytes in `u32`; `make_circle_mask` allocates
`(2*ceil(r + 1.5) + 1)²` — so both are now refused in `f64` before the backend
multiplies, naming the declaration:

```
figspec: a PNG is 8589934592x8589934592 pixels at density 2, above the 16000000-pixel
raster limit; render an SVG, which has no pixel buffer, or declare a smaller width/height
figspec: marks[0].size 1e9 cannot be drawn: 2000000005x2000000005 pixels at density 2 is
above the 16000000-pixel raster limit; the SVG has no such buffer
figspec: marks[0].size 1e308 cannot be drawn: its coverage mask is not a finite size; the
SVG has no such buffer
```

The bound is the 16 000 000 pixels `shadergx` uses for the same kind of buffer. The
SVG route still draws both, deliberately — it allocates no pixels — and `inspect`
reports nothing about either, because a pixel budget is a property of the route and
not of the figure. `tests/raster.rs` pins both directions and both boundaries: at
density 2 a 2000x2000 canvas draws and 2001x2000 does not, and a 1997-point marker
draws while 1998 does not, from the backend's own arithmetic rather than a round
number.

### A command that answered about something else

| what was run | what it did |
| --- | --- |
| `storyboard m.json -o out.png` | exit 0, and **SVG contents** in a file named `.png` |
| `inspect a.json b.json` | inspected `a.json`; `b.json` was never mentioned |
| `inspect m.json --oops` (a typo'd `--json`) | the human report, exit 0 |
| `render chart.svg -o out.svg` | "render needs a model file", about a file that was there |
| `{"size": -5}` | clean, and drawn with a size the renderer chose |

The fourth is why the others existed: the model and `-o`'s value were told apart by
their *extensions* (`find(|arg| !arg.ends_with(".svg"))`), and the same hack swallowed
a second positional argument and an unknown flag. One parser now — one model, one
`-o`, `--json` for `inspect`, everything else refused by name with exit 2 — and
`storyboard` refuses a non-`.svg` extension the way `render` refuses anything but
`.svg`/`.png`. A negative marker size is a length that is not a length, so it is
refused like a negative error magnitude: `negative-size`, a new error code, reported
by `inspect` and refused by the renderer through the one list they share.

### Attacked and found sound

The other 54 probed models: ragged matrices, empty marks, one-point lines, a heatmap
with `y_min` and a log axis, mismatched label counts, `NaN`/infinite coordinates, a
`u32::MAX` canvas on the SVG route, coordinates at 1e308, an empty figure, an unknown
mark kind, unknown fields, a directory in place of a model, an unwritable output
path. Each was reported or refused, exit 0/1/2, in both build profiles, with no panic
in either.

### Recorded, not changed

`render -o -` writes a file named `-`; stdout is not an output of these commands, and
a second way to get the bytes was not added. `inspect` calls a `u32::MAX` canvas
clean — the raster budget is the route's, and the SVG route honours the declaration.

## Fifth review: the drawn pixels

This review read storyboard SVG back as XML and checked its boxes and number
plates against the canvas and drawn primitives. It also exercised the Rust API
with caller-supplied reports, not only the reports `inspect` creates.

### Found and fixed

- A log bar with `y_min` put its box at the plot's top, 10 px down, but its 16 px
  number plate at **y = -6**. Plates now stay inside the canvas on both axes.
  A plate wider than its canvas is refused: 20 findings on one mark made a 358 px
  plate on a 100 px canvas; moving its x to zero did not make it fit.
- A 1x1 canvas produced a plot rectangle `(71.65, 10, -70.65, -60)` and exited 0.
  The shared geometry route now refuses non-positive plot area for SVG, PNG,
  storyboard and placement. The refusal's suggested canvas is exercised by tests.
- A title `a` + U+0001 + `b` made SVG that an XML parser rejected while `inspect`
  reported no text error. `unwritable-character` now covers every text field via
  the shared model refusals, including mark and series colour strings; valid XML
  characters are still accepted. A regression
  test also prevents reporting the same mark field twice.
- A caller-supplied report naming mark 7 of a one-mark figure was described as
  “about no mark”. `unboxable` now distinguishes it from genuinely `unlocated`
  findings. No rectangle is invented for a missing mark.
- Two separate bar marks occupied the same slots: a 480 px red bar was covered by
  a taller green bar at the same x. `overlapping-bars` reports each later/earlier
  pair with intersecting filled regions and recommends one grouped bar. Rendering
  is still allowed to show it. The check includes separate grouped marks and
  mixed simple/grouped marks, but not opposite signs or disjoint subslots.
- Zero-valued bars drew rectangles of height 0 at y=10 while their storyboard box
  appeared at y=456 with height 4. Bars with no visible height now have no box;
  their findings remain in `unboxable`, including bars wholly below `y_min` on a
  valid increasing axis. A reversed computed y range is refused, not interpreted
  as a declaration to invert the axis.

### Independent review

A separate read-only review reproduced four additional cases: U+0001 in a colour
attribute still broke XML; an inverted computed range made a visible negative bar
look unboxable; opposite-sign bars were falsely called overlapping; and separate
grouped marks overplotted without a finding. Each reproduction became a failing
test before its fix, with matching mutations. The overlap check now sweeps each
category's two series partitions rather than comparing only series indices.

A follow-up review proposed an overflowing computed range. It was reproduced:
finite values from -1e308 to 1e308 produced an SVG path containing `NaN`, exit 0.
Drawing now requires finite, increasing computed ranges with finite spans on both
axes. Tests distinguish these route failures from non-finite model values.
The overlap test also excludes bars wholly clipped away by `y_min`.

### Test gap closed

A warning naming a mark is reachable through `storyboard::of(figure, report)`, even
though the inspector's current mark-level findings are errors. Its non-error box
colour is now tested and mutation-checked, rather than removed as unreachable.

### Limits

The SVG probes check geometry, document parsing and report-number agreement, not
font rasterization or pixel-identical output across platforms. Zero-height bars
remain valid model data; only an invented visual location is removed. Ordinary
bar/line overlays are not reported as `overlapping-bars`.

## Sixth review: strings at the backend boundary

The previous XML-character audit did not exercise the colour parser's byte
slices, internal-id renaming, or independently optional heatmap axis names.

### Found and fixed

- `#éx` and `#aébcd` passed inspection, then panicked in the engine's colour
  parser: a byte slice ended inside a UTF-8 character. All four colour-bearing
  mark kinds reproduced this. `unsupported-color` now refuses such input before
  the parser is called, via the model's shared refusal list.
- Valid CSS is not necessarily a shared SVG/PNG colour. A bar declared with
  `rebeccapurple` or `rgba(1,2,3,0.5)` exited 0 but its PNG fill was absent:
  an interior pixel was white (255,255,255), versus (255,0,0) for the red control.
  Unsupported CSS and non-finite `rgb` components are now refused, not silently
  omitted or cast. Known colours and explicit no-paint declarations still draw.
- A title containing `kuva-clip-999` was rewritten to an internal clip id.
  Renumbering now changes typed scene clip ids and their engine-generated
  definitions, never the completed document's arbitrary strings. Tests preserve
  titles, axis labels, categories and legends, while repeated renders still have
  matching deterministic clip definitions and references.
- Declaring only heatmap row names, or only column names, silently dropped those
  names. The layout now handles each axis independently; it is the single owner
  of axis labels, with no redundant heatmap label copy for unused tooltips.

### Independent review and refinement

A separate read-only review caught an overbroad first fix: requiring every colour
string to be ASCII rejected `rgb(12,\u00a034,56)`, although the engine trims that
Unicode whitespace and can draw the finite RGB components in both formats.
The acceptance regression failed before the restriction was narrowed to hex
digits. Self-review also reproduced `#+1+2+3` rendering as `#010203`: Rust's integer
parser accepts leading plus signs, but those are not CSS hex digits. Hex inputs
now require exactly three or six ASCII hex digits before the backend parser runs.

The new Rust and Python regressions were run against the old behavior and failed
before fixes. These checks do not claim arbitrary CSS support or cross-platform
font/pixel equality.
