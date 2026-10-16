# figspec, for agents

Write a figure as **data**, check it, then render it. Nothing is executed to be
inspected, so the check runs before the picture exists.

## The loop

1. Write or convert a model (JSON): `marks`, plus `title`, `x_label`, `y_label`,
   `x_scale`/`y_scale` (`linear`/`log`), `y_min`, `width`/`height`.
2. `figspec inspect model.json` — exit 1 on any `error`.
3. Fix, inspect again.
4. `figspec render model.json -o chart.svg` (or `.png`).
5. `figspec storyboard model.json -o chart.storyboard.svg` — the figure with the
   marks the findings name boxed and numbered; the numbers are the positions in
   step 2's report, so the two can be read against each other.

In Python: `figspec.inspect(model)`, `figspec.render_svg(model)`,
`figspec.storyboard_svg(model)`, `figspec.save(model, "chart.svg")`,
`figspec.from_matplotlib(figure)`.

## Reading the report

- `error` — `truncated-baseline` (bars start above zero), `mismatched-lengths`,
  `non-finite-value`, `non-positive-on-log-scale`, `empty-figure`, `empty-mark`,
  `short-line`, `ambiguous-labels`.
- `warn` — `no-title`.
- `info` — `no-axis-label`.

Common fixes: `truncated-baseline` → drop `y_min`, or use a line instead of bars;
`empty-mark` → give the mark data; `short-line` → a line needs two points;
`mismatched-lengths` → one value per category; `non-positive-on-log-scale` → a log
axis needs positive values; `empty-figure` → add a mark.

## Before calling a change done

`bash scripts/check.sh` — formatting, clippy with `-D warnings`, the Rust tests,
the mutation catalogue, the Python bindings and tests, the wheel, and the consumer
check. The mutation catalogue is the part that proves the *tests* would notice:
each entry removes one rule and requires an assertion to fail.

## Two rules this project holds itself to

- **Refuse rather than approximate.** An unknown field, a mark that cannot be
  drawn, a matplotlib figure with an unsupported artist: an input failure naming
  what was wrong, never a chart that quietly differs from the data.
- **The model is the artifact.** Checks read the model, so they never need to
  render and never execute the artifact's code.
