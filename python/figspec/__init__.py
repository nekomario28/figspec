"""figspec — a figure as data, rendered by Rust.

The figure is a plain dict (JSON-able). Everything here either produces one or
consumes one:

    model = figspec.from_matplotlib(fig)      # an existing figure, converted
    # the model also has a `heatmap` mark, which matplotlib's imshow cannot become
    report = figspec.inspect(model)           # checks, without rendering
    figspec.save(model, "revenue.svg")        # rendered by the Rust engine

matplotlib is supported as an *importer*, not as a dependency of the renderer:
you can keep writing matplotlib and get a checkable, license-clean, 20 MB
artifact out of it, or write the model directly and never import matplotlib.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

__all__ = [
    "Model",
    "from_matplotlib",
    "inspect",
    "render_svg",
    "render_png",
    "storyboard_svg",
    "save",
    "load",
]

try:  # the compiled extension
    from . import _figspec as _core
except ImportError as error:  # pragma: no cover - a broken install, not a code path
    raise ImportError(
        "figspec's compiled extension is missing; reinstall the package "
        f"(import error: {error})"
    ) from None


class Model(dict):
    """A figure, as data.

    A dict subclass so the boundary is inspectable and JSON-able: no hidden
    object graph, and what is stored is what is rendered.
    """

    def to_json(self) -> str:
        return json.dumps(self, indent=2, sort_keys=True)

    @classmethod
    def from_json(cls, text: str) -> "Model":
        return cls(json.loads(text))


def load(path: str | Path) -> Model:
    return Model.from_json(Path(path).read_text(encoding="utf-8"))


def inspect(model: dict[str, Any]) -> list[dict[str, Any]]:
    """The problems in a model, as a list of `{code, severity, message, mark?}`."""
    report = json.loads(_core.inspect_json(Model(model).to_json()))
    return report["problems"]


def render_svg(model: dict[str, Any]) -> str:
    return _core.render_svg(Model(model).to_json())


def render_png(model: dict[str, Any]) -> bytes:
    return bytes(_core.render_png(Model(model).to_json()))


def storyboard_svg(model: dict[str, Any]) -> str:
    """The figure as an SVG with a numbered box on every mark a problem names.

    The numbers are the positions of `inspect`'s problems (1-based), so the report
    and the picture can be read against each other.
    """
    return _core.storyboard_svg(Model(model).to_json())


def save(model: dict[str, Any], path: str | Path) -> Path:
    """Write the figure, choosing the format from the suffix."""
    path = Path(path)
    name = path.name.lower()
    if name.endswith(".storyboard.svg"):
        # The storyboard is a rendering too. It is written for the *composite*
        # name: `.svg` alone would have matched first and written the figure.
        path.write_text(storyboard_svg(model), encoding="utf-8")
    elif path.suffix.lower() == ".svg":
        path.write_text(render_svg(model), encoding="utf-8")
    elif path.suffix.lower() == ".png":
        path.write_bytes(render_png(model))
    elif path.suffix.lower() == ".json":
        path.write_text(Model(model).to_json() + "\n", encoding="utf-8")
    else:
        raise ValueError(f"figspec writes .svg, .png or .json, not {path.suffix!r}")
    return path


# --------------------------------------------------------------------------- #
# matplotlib, as an importer
# --------------------------------------------------------------------------- #

#: matplotlib's short colour names are a subset of CSS; the long ones it accepts
#: are CSS names too, so they pass through. Anything else (a tuple, an array) is
#: converted to `#rrggbb` so the model stays JSON-able and engine-neutral.
def _color(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        return value
    try:
        from matplotlib.colors import to_hex

        return to_hex(value)
    except Exception:
        return None


def _figsize(figure: Any) -> dict[str, int]:
    """Size in pixels at the figure's own dpi, which is what the renderer draws."""
    dpi = float(getattr(figure, "dpi", 100.0) or 100.0)
    width, height = getattr(figure, "get_size_inches", lambda: (6.4, 4.8))()
    return {"width": int(round(float(width) * dpi)), "height": int(round(float(height) * dpi))}


def _title(figure: Any, axes: Any) -> str | None:
    """The title of the axes being converted, or the figure's suptitle.

    Reading the *first* axes' title was wrong for any figure with several: the
    model would carry a title from a chart it does not describe.
    """
    text = getattr(figure, "_suptitle", None)
    if text is not None and getattr(text, "get_text", lambda: "")():
        return str(text.get_text())
    label = axes.get_title()
    return str(label) if label else None


def _axis_labels(axes: Any) -> dict[str, str | None]:
    x = axes.get_xlabel() or None
    y = axes.get_ylabel() or None
    return {"x_label": str(x) if x else None, "y_label": str(y) if y else None}


def _scale(axes: Any, which: str) -> str:
    getter = getattr(axes, f"get_{which}scale", None)
    return "log" if getter is not None and getter() == "log" else "linear"


def _points(line: Any) -> list[list[float]]:
    data = line.get_xydata()
    return [[float(x), float(y)] for x, y in data]


def _error_bars(axes: Any) -> tuple[dict[int, dict[str, Any]], set[int]]:
    """Recover error bars, and the collections that carry them.

    Returns `{id(data_line): {"y_err": [...]}}` plus the ids of the collections
    that belong to an error bar, so the unsupported scan does not report them as
    unknown artists. matplotlib stores the magnitudes inside a `LineCollection`
    of vertical segments, so they are recovered from the segments rather than
    from the argument the caller passed.
    """
    errors: dict[int, dict[str, Any]] = {}
    accounted: set[int] = set()
    for container in getattr(axes, "containers", []):
        if type(container).__name__ != "ErrorbarContainer":
            continue
        data_line = container.lines[0]
        yerr = container[2] if len(container) > 2 else None
        magnitudes = yerr[0] if isinstance(yerr, tuple) and yerr else yerr
        if magnitudes is None:
            continue
        accounted.add(id(magnitudes))
        centres = [float(y) for _, y in data_line.get_xydata()]
        symmetric: list[float] = []
        asymmetric: list[list[float]] = []
        for position, segment in enumerate(magnitudes.get_segments()):
            if len(segment) < 2 or position >= len(centres):
                continue
            lower = centres[position] - float(segment[0][1])
            upper = float(segment[1][1]) - centres[position]
            symmetric.append((lower + upper) / 2.0)
            asymmetric.append([lower, upper])
        if not symmetric:
            continue
        if all(abs(low - high) < 1e-9 for low, high in asymmetric):
            errors[id(data_line)] = {"y_err": symmetric}
        else:
            errors[id(data_line)] = {"y_err_asymmetric": asymmetric}
    return errors, accounted


def _bar_marks(axes: Any) -> list[dict[str, Any]]:
    """Bars, as one mark or as a group of series, from matplotlib's containers.

    A `BarContainer` is one `axes.bar` call, so it is one series; series in a
    group are the same call with an x offset, which makes patch *i* of every
    container the *i*-th category. That is the convention this follows, and the
    positions are checked against it rather than assumed — two separate bar
    charts on one axes look like a group to a naive reader and are refused.

    A stack (`bottom=` set) is a different chart the model cannot hold, and bars
    that do not sit on tick labels have no category names, so both are refused
    rather than drawn as something else.
    """
    containers = [
        container
        for container in getattr(axes, "containers", [])
        if type(container).__name__ == "BarContainer"
    ]
    containers = [container for container in containers if list(container.patches)]
    if not containers:
        return []

    widths = {
        float(getattr(list(container.patches)[0], "get_width", lambda: 1.0)())
        for container in containers
    }
    lengths = {len(container.patches) for container in containers}
    if len(lengths) != 1:
        raise ValueError(
            "figspec cannot represent bar groups with a different number of bars "
            "in each group"
        )
    if len(widths) != 1:
        raise ValueError("figspec cannot represent bar groups whose bars differ in width")
    width = widths.pop()
    per_group = lengths.pop()

    for container in containers:
        if any(abs(float(patch.get_y())) > 1e-9 for patch in container.patches):
            raise ValueError(
                "figspec cannot represent a stacked bar chart; it draws grouped "
                "bars. Refusing rather than converting part of the figure"
            )

    labels: list[str | None] = []
    colors: list[str | None] = []
    for container in containers:
        label = str(container.get_label() or "")
        labels.append(label if label and not label.startswith("_") else None)
        colors.append(_color(list(container.patches)[0].get_facecolor()))

    categories: list[str] = []
    values: list[list[float]] = [[] for _ in containers]
    for position in range(per_group):
        group = [list(container.patches)[position] for container in containers]
        centres = [float(patch.get_x()) + width / 2.0 for patch in group]
        if max(centres) - min(centres) > width * 1.5:
            raise ValueError(
                "figspec cannot represent bars that are not side by side as one "
                "group; refusing rather than pairing unrelated bars"
            )
        category, found = _category_at(axes, sum(centres) / len(centres), limit=width)
        if not found:
            raise ValueError(
                "figspec cannot represent bars that do not sit on tick labels; the "
                "model's bars are categorical. Refusing rather than inventing "
                "category names"
            )
        categories.append(category)
        for index, patch in enumerate(group):
            values[index].append(float(patch.get_height()))

    if len(containers) == 1:
        return [
            {
                "kind": "bar",
                "categories": categories,
                "values": values[0],
                "color": colors[0],
                "label": labels[0],
            }
        ]
    return [
        {
            "kind": "grouped_bar",
            "categories": categories,
            "series": [
                {"values": values[index], "label": labels[index], "color": colors[index]}
                for index in range(len(containers))
            ],
        }
    ]


def _marks(axes: Any) -> list[dict[str, Any]]:
    """The marks of one axes, in the order matplotlib draws them.

    Only what the model can represent is converted. A mark kind it cannot hold
    (an image, a contour set, a 3-D projection, a stacked bar) is **refused**, not
    skipped: a converted figure that silently omits half the chart is worse than
    no conversion, because it looks complete.
    """
    errors, accounted = _error_bars(axes)
    marks: list[dict[str, Any]] = []
    unsupported: list[str] = []

    for line in axes.get_lines():
        mark: dict[str, Any] = {
            "kind": "line",
            "points": _points(line),
            "color": _color(line.get_color()),
            "label": line.get_label() if not str(line.get_label()).startswith("_") else None,
            **errors.get(id(line), {}),
        }
        marks.append(mark)

    for collection in getattr(axes, "collections", []):
        if id(collection) in accounted:
            continue
        name = type(collection).__name__
        if name == "PathCollection":
            # A scatter: the offsets are the points, the colour is one value.
            marks.append(
                {
                    "kind": "scatter",
                    "points": [
                        [float(x), float(y)] for x, y in collection.get_offsets().tolist()
                    ],
                    "color": _color(collection.get_facecolor()),
                    "label": (
                        str(collection.get_label())
                        if not str(collection.get_label()).startswith("_")
                        else None
                    ),
                }
            )
            continue
        unsupported.append(name)

    marks.extend(_bar_marks(axes))

    for patch in axes.patches:
        # A rectangle that is not part of a bar container is an annotation with a
        # size, which this model has no mark for.
        name = type(patch).__name__
        if name not in {"Rectangle", "FancyBboxPatch"}:
            unsupported.append(name)

    # `images` is the one that bites: `imshow` puts its picture there, not in
    # `patches` or `collections`, so a converter that only looked at those would
    # report a figure with no marks as complete.
    for attribute in ("images", "tables", "texts"):
        for artist in getattr(axes, attribute, []):
            unsupported.append(type(artist).__name__)

    if unsupported:
        raise ValueError(
            "figspec cannot represent "
            + ", ".join(sorted(set(unsupported)))
            + "; it converts lines, scatter, bars and grouped bars, with error "
            "bars, and the *model* also has a heatmap mark — write that one "
            "directly. Refusing rather than converting part of the figure"
        )
    return marks


def _category_at(axes: Any, position: float, *, limit: float = 0.51) -> tuple[str, bool]:
    """The tick label at ``position``, and whether there was one.

    The second element is what lets a caller refuse rather than invent a name: a
    bar that sits between ticks has no category, and calling it "0.2" would be a
    label the figure never had.
    """
    ticks = [float(value) for value in axes.get_xticks()]
    labels = [str(text.get_text()) for text in axes.get_xticklabels()]
    if not ticks:
        return f"{position:g}", False
    nearest = min(range(len(ticks)), key=lambda index: abs(ticks[index] - position))
    if abs(ticks[nearest] - position) <= limit and nearest < len(labels) and labels[nearest]:
        return labels[nearest], True
    return f"{position:g}", False


def from_matplotlib(figure: Any, *, axes_index: int = 0) -> Model:
    """Convert a matplotlib `Figure` into a model.

    The figure must have a single axes of the kinds the model can hold; anything
    else raises rather than converting part of it. `axes_index` picks which axes
    when a figure has several and the caller knows which one matters.

    matplotlib is not a dependency of this package: import it yourself, or never.
    """
    if not hasattr(figure, "axes"):
        raise TypeError("from_matplotlib needs a matplotlib Figure")
    axes_list = list(figure.axes)
    if not axes_list:
        raise ValueError("the figure has no axes")
    if axes_index >= len(axes_list):
        raise ValueError(f"the figure has {len(axes_list)} axes, not {axes_index + 1}")
    axes = axes_list[axes_index]

    model: Model = Model(
        {
            "marks": _marks(axes),
            "title": _title(figure, axes),
            **_axis_labels(axes),
            "x_scale": _scale(axes, "x"),
            "y_scale": _scale(axes, "y"),
            **_figsize(figure),
        }
    )
    # matplotlib's own y limits are carried over, because that is where a
    # truncated baseline comes from and the checks must see it.
    low, _high = axes.get_ylim()
    if _scale(axes, "y") == "linear" and low > 0.0:
        model["y_min"] = float(low)
    return model
