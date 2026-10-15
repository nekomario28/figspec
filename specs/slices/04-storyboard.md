# 04 — The storyboard

## The question this slice opened with

A declared figure had a report but no picture: `chartgx` refuses to box anything,
because a declaration has no artists, and figspec had no way to say where a mark
*is*. The working assumption was that kuva would have to expose per-mark
positions — an upstream feature — or figspec would have to derive the layout a
second time.

## Measured: kuva already publishes the layout, and no fork is needed

Read from `kuva` 0.6.0 (the version `Cargo.toml` pins):

- `render::layout::ComputedLayout::from_layout(&Layout) -> ComputedLayout` is
  **public**, and its fields are public: `width`, `height`, `margin_left/right/
  top/bottom`, `x_range`, `y_range`, `log_x`, `log_y`.
- `render::render::render_multiple(plots, layout) -> Scene` is **public**, and
  `backend::svg::SvgBackend.render_scene(&scene)` is the documented way to render
  it. kuva's own doc comment for `render_to_svg` says: *"For fine-grained control
  … use `render::render::render_multiple` and `backend::svg::SvgBackend`
  directly."*
- `Scene.axis_meta` exists but is populated **only** when the scene is
  interactive, so it is not the seam for a static figure. `ComputedLayout` is.

So the mapping is a read of kuva's numbers, not a second derivation:

| mark | how the box is placed |
| --- | --- |
| line, scatter | bbox of the points (including error-bar extent) mapped through `x_range`/`y_range` |
| bar, grouped bar | the category slot: centre at data `x = i + 1`, half-width 0.5, y from the baseline to the value |

### Evidence for each claim

Measured with `Layout::auto_from_plots` and the primitives kuva actually drew
(now permanent tests in `tests/storyboard.rs`):

| case | engine's numbers | the primitive kuva drew | figspec's mapping |
| --- | --- | --- | --- |
| line `(0,0)–(10,100)` | `x_range (0, 10.5)`, margins L 61.296875 R 14 T 10 B 51, 675.296875×511 | `PolyLine` = `(61.296875, 460.0) → (632.7254464285714, 31.428571428571445)` | identical to 1e-9 |
| scatter `(1,1) (2,4)` | `x_range (0.95, 2.05)`, `y_range (0.85, 4.15)` | `CircleBatch cx=[88.56960227272725, 634.0241477272727]` | identical to 1e-9 |
| bars `5,10,15` | `x_range (0.5, 3.5)` | `Rect x=84.02/284.02/484.02 w=160`, one centre per tick at 164.02/364.02/564.02 | centre `category_center(i)` maps onto `left + width/2`, exactly |
| log-y scatter `(1,1) (2,100)` | `y_range (1, 100)`, `log_y true` | `CircleBatch cy=[460.0, 10.0]` | identical to 1e-9 |
| bars with `y_min = 5` | `y_range (4, 16)` (padded by the engine) | bars drawn from data `y = 0` — i.e. `y+h = 610`, below the canvas | the box clamps to the plot rectangle and reaches the axis |

The bar result also explains why kuva's own doc comment says categorical labels
sit on "a uniform pitch (`i + 1.0`)": two bars give `x_range (0.5, 2.5)`, three
give `(0.5, 3.5)`, and a bar mixed with a numeric line keeps `i + 1` in the
numeric space (verified: rect centre maps to data x = 1.0000 with a line present).
That last case is why the rule is `i + 1` and **not** "spread `n` categories over
the range" — the latter would place the mixed chart's bars at 0.667/2.0/3.333.

## What was built

- `src/place.rs` — `PlotArea` (read from `ComputedLayout`), `category_center`,
  `MarkBox`, and `boxes(figure)`. A mark with nothing drawn returns `None`: it has
  no location, and a plausible-looking box would be a fabricated pointer.
- `src/storyboard.rs` — `of(figure, report) -> Storyboard`. The report is an
  argument, so a storyboard is a pure function of a figure and a report and a
  report from anywhere can be drawn over the figure it is about.
- `render::geometry` — one function producing the plots and layout, used by
  `to_svg`, `to_png` **and** the storyboard, so a box cannot be computed against a
  different layout than the picture was drawn with.
- CLI `figspec storyboard model.json -o out.svg`, Python `storyboard_svg(model)`
  and `save(model, "x.storyboard.svg")`.

## Two rules the storyboard obeys

- **The box is the mark's real rectangle** — required by test against kuva's own
  primitives, per mark kind, including a log axis and a truncated baseline.
- **A finding about no mark is not boxed.** `no-title` and `no-axis-label` are
  reported and listed as unboxed rather than pointed at some plausible place.
  Two findings on one mark share one box with both numbers.

## A defect the slice found on the way

`truncated-baseline` reported **once** for a figure however many categorical marks
it had, so a figure with two bar marks on a trimmed axis reported one of them and
a fix to the other would have been invisible. It is now one finding per
categorical mark, each naming its own mark — the same class of defect as gxkit's
identity work, found because a storyboard has to point at exactly one place.

## Gate

`cargo clippy --all-targets --all-features -- -D warnings` was added to
`scripts/check.sh` and CI, and shown to fail (exit 101) on a deliberate unused
binding before being left in place.
