# The contract

What an integrator may rely on, and what it may not. This is the document a release
claim rests on: what is listed as stable is covered by a test (and, for the rules,
by a mutation that proves the test would notice), and what is listed as unstable is
explicitly not promised.

## Stable

**Problem codes.** Every problem carries a `code` from a fixed, documented set
(`figspec codes`, or the table in `README.md`). Codes are the branch point; message
prose is not. A code's severity is stable within a major version, and severities
only ever move from `info` to `warn` to `error` — never the other way, because that
would silently weaken an existing gate.

**Exit codes.** The CLI, and the Python surface that mirrors it:

| code | meaning |
| --- | --- |
| 0 | inspected, and nothing made it fail |
| 1 | inspected, at least one `error` |
| 2 | the model could not be read: missing, unreadable, malformed, an unknown field, or an unknown enum value such as a colour ramp |

`warn` and `info` never fail a build. `render` and `storyboard` exit 2 when the
figure cannot be drawn — which is not an input error in the strict sense, and is
the one place where the two exit paths meet: a refusal the *checks* report makes
`inspect` exit 1 and `render` exit 2.

**The command line.** `figspec <inspect|storyboard|render|codes> …`. One model
file, one `-o <path>` where the command writes something, `--json` for `inspect`.
Anything else is refused by name, exit 2: a second model file, an unknown option, a
flag the command cannot honour (`inspect -o …`, `render --json`), a missing model or
`-o`. The model and `-o`'s value are told apart by *position*, so a model file named
`chart.svg` is a model. `storyboard` writes SVG and refuses a path whose extension
is not `.svg`, because the file would otherwise hold something other than what its
name says.

**The raster budget.** `render -o out.png` refuses, exit 2, a picture whose pixel
buffer would exceed 16 000 000 pixels at density 2 — the canvas, and a marker whose
coverage mask (`2*ceil(size/2*2 + 1.5) + 1` squared, the arithmetic the backend
allocates) is larger than that. It is a limit of the *raster route*: the same figure
renders to SVG, which has no pixel buffer, and `inspect` reports nothing about it,
because a pixel budget is not a property of the figure. Values are computed in
`f64` before the backend multiplies, so the limit is a refusal and not a panic.

**The plot area.** Every drawing route refuses a canvas whose computed axes leave
no positive plot width or height, or whose computed axis ranges are not finite
and increasing with finite spans.
An explicit `y_min` above the data does not declare an inverted-axis mode.
These are layout constraints, not model errors:
`inspect` does not render to discover it. The refusal names the declared size and
axes' space requirements; no output is written. A storyboard also refuses a number
plate larger than the canvas instead of clipping or abbreviating finding numbers.

**Storyboard locations.** A number plate stays inside the canvas; the box still
points at the mark's extent. Findings with no mark are `unlocated`. Findings naming
a missing mark or a mark without visible height are `unboxable`, not “about no
mark”. Neither bucket gets an invented box. These buckets are exposed by the Rust
API and distinguished in CLI output.

**Text.** Titles, axis labels, categories, mark labels, series labels, heatmap
labels and colour strings must use XML 1.0 characters. `unwritable-character` is an error from the
shared refusal list, so SVG, PNG and storyboard routes agree on this text contract.
Tab, newline, carriage return and valid Unicode outside the forbidden ranges are
not refused merely for being non-ASCII.

**Bar overlays.** `overlapping-bars` covers separate simple or grouped bar marks
whose nonzero filled regions overlap: the same category, intersecting series
subslots, and intersecting value intervals after `y_min` clipping. Merely sharing
a baseline or having values
in disjoint categories is not an overlap. One grouped bar is the representation
for adjacent series.

**The report.** `inspect` returns a list of problems, each with `code`, `severity`,
`message`, and `mark` when the problem is about one mark (0-based). `--json` prints
the same thing with a `problems` key. Keys are only ever added.

**The model.** A figure is JSON: `marks` plus `title`, `x_label`, `y_label`,
`x_scale`, `y_scale`, `y_min`, `width`, `height`. Marks are `line`, `scatter`,
`bar`, `grouped_bar` and `heatmap`. **Unknown fields are refused**, at the top
level and inside a mark, and so is an unknown value where the model names a closed
set (a colour ramp). The field names and their meanings are stable; a *new* field
or a new mark kind may be added in a minor version, which is why `deny_unknown_fields`
is a promise rather than a convenience.

**The Python surface.**

| name | promise |
| --- | --- |
| `figspec.load(path)` / `Model` | reads a model file; raises on anything the model does not name |
| `figspec.inspect(model)` | the same problems the CLI reports |
| `figspec.render_svg(model)` / `render_png(model)` | the same bytes the CLI writes |
| `figspec.storyboard_svg(model)` | the figure with the findings boxed, numbered as `inspect` orders them |
| `figspec.save(model, path)` | chooses by suffix: `.svg`, `.png`, `.json`, `.storyboard.svg` |
| `figspec.from_matplotlib(figure, axes_index=0)` | converts, or raises naming what it cannot represent |

Rendering is a pure function of the model: the same model renders to the same
bytes, so two renders are comparable and an artifact is content-addressable.

**Import weight.** `import figspec` does not import `numpy` or `matplotlib`.
`from_matplotlib` needs matplotlib, and only it does.

**Supported platforms.** The wheels are built and exercised per platform: Linux
x86_64 and aarch64, macOS arm64, Windows x86_64, one abi3 wheel for CPython ≥ 3.9.

## Not stable

- **Message prose.** It is written for a reader; two messages may name the same
  finding differently after a wording improvement.
- **Problem order.** Errors come first, then warnings and information; the order
  within a severity is not a contract. (The *numbers* a storyboard draws are the
  report's positions, so they follow the order of the report you were given.)
- **Storyboard pixels.** The image is evidence for a human: the boxes and numbers
  follow the report, but no coordinate, size or colour is promised.
- **Which codes a figure can emit.** The set is documented and grows; a new code
  may appear in a minor version.
- **The drawing itself.** Layout, fonts, colour-bar placement and cell aspect are
  `kuva`'s, not this project's. A figure that renders today may render differently
  after an engine upgrade; the *values*, the labels and the checks do not change.
- **The Rust API.** `figspec::Figure`, `inspect`, `to_svg` and the rest are public
  and useful, but only the CLI and Python surfaces above are guaranteed.

## Silence is never a verdict

A figure is either reported on or refused. The checks and the renderer work from
**one list**: `Figure::refusals()` is what `inspect` reports and the only thing the
renderer refuses about the model, so a rule cannot hold in one and be ignored by the other —
`tests/refusals.rs` checks it in both directions. Drawing-route constraints (the
raster budget, plot area and number-plate fit) are checked separately. A model that reads as clean has
been fully read: an unknown field, an unknown ramp, an empty mark, an undeclared
axis setting for a matrix and a matrix that is not rectangular are all refused with
the mark and the reason, never skipped.

Where the model cannot express something, the refusal names the limit instead of
approximating: a colour image, a masked cell, a non-linear norm, an image drawn
over an explicit `extent`, a partial set of tick labels, a stacked bar, a contour
or violin set.

`from_matplotlib` refuses the same way, and its refusals name the construct: a
colour-mapped or size-encoded scatter, `barh`, two bar calls at the same positions,
bars between the tick labels, error bars with no data line, horizontal error bars
(`xerr`), twin axes, an inverted x or y axis. A figure that cannot be converted
whole is not converted partly — measured: a `capsize` error bar once converted into
three line marks, and `xerr` into vertical error bars of length zero.

The command line is held to the same rule: an argument the command cannot honour is
refused, not ignored, because a command that answers about something other than what
it was given is worse than one that fails. Measured before: a typo'd `--jsonn`
printed the human report with exit 0; `inspect a.json b.json` inspected `a.json` and
never mentioned `b.json`; `storyboard -o out.png` wrote SVG to a path ending `.png`.

`docs/SEMANTICS.md` records which claims were attacked and found sound, so the next
review can differ from this one deliberately.

## What this is not

- **Not a matplotlib-compatible API.** You do not port plotting code to figspec;
  you declare a figure, or convert one, and then check it.
- **Not a renderer.** The drawing is `kuva`'s (MIT) — including its layout and
  text handling. This project owns the model, the checks and the Python surface.
- **Not a conformance validator.** Reading a matplotlib figure means implementing
  enough of matplotlib's object model to find the artists, and nothing here
  verifies that reading against matplotlib's specification. A wrong reading is a
  bug in this repository, found by review and tests.
- **Not a visual oracle.** Colour similarity with the source figure is checked
  cell by cell for a converted image, within one rounding step of the ramp lookup;
  nothing here claims a converted figure is pixel-identical to its source.

## Inspection, not execution of foreign code

A model is data: it is parsed, never executed. The one place code is involved is
`from_matplotlib`, which reads an object graph the caller already built — figspec
imports matplotlib only inside that function, and never runs a figure's script.

## Support

MIT licensed. `bash scripts/check.sh` runs formatting, clippy with `-D warnings`,
the Rust tests, the mutation catalogue, the Python tests, the wheel and an isolated
consumer install; CI runs the same, plus one wheel per platform built and exercised
there. A green `main` means those gates hold for that revision — not that every
matplotlib construct or every colour ramp has been considered.
