"""figspec — a figure as data, rendered by Rust.

The figure is a plain dict (JSON-able). Everything here either produces one or
consumes one:

    model = figspec.from_matplotlib(fig)      # an existing figure, converted
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

__all__ = ["Model", "from_matplotlib", "inspect", "render_svg", "render_png", "save", "load"]

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


def save(model: dict[str, Any], path: str | Path) -> Path:
    """Write the figure, choosing the format from the suffix."""
    path = Path(path)
    if path.suffix.lower() == ".svg":
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


def _title(figure: Any) -> str | None:
    text = getattr(figure, "_suptitle", None)
    if text is not None and getattr(text, "get_text", lambda: "")():
        return str(text.get_text())
    for axes in getattr(figure, "axes", []):
        label = axes.get_title()
        if label:
            return str(label)
    return None


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


def _marks(axes: Any) -> list[dict[str, Any]]:
    """The marks of one axes, in the order matplotlib draws them.

    Only what the model can represent is converted. A mark kind it cannot hold
    (an image, a contour set, a 3-D projection) is **refused**, not skipped: a
    converted figure that silently omits half the chart is worse than no
    conversion, because it looks complete.
    """
    marks: list[dict[str, Any]] = []
    unsupported: list[str] = []

    for line in axes.get_lines():
        marks.append(
            {
                "kind": "line",
                "points": _points(line),
                "color": _color(line.get_color()),
                "label": line.get_label() if not str(line.get_label()).startswith("_") else None,
            }
        )

    for patch in axes.patches:
        name = type(patch).__name__
        if name not in {"Rectangle", "FancyBboxPatch"}:
            unsupported.append(name)
            continue
        height = float(getattr(patch, "get_height", lambda: 0.0)())
        width = float(getattr(patch, "get_width", lambda: 0.0)())
        # A bar is a rectangle on a categorical axis; matplotlib gives it an x
        # position and a width, and the category labels live on the ticks.
        if width <= 0.0:
            unsupported.append(name)
            continue
        category = _category_at(axes, float(patch.get_x()) + width / 2.0)
        marks.append({"kind": "bar", "categories": [category], "values": [height], "color": _color(patch.get_facecolor())})

    # Everything else an axes can hold is outside the model. `images` is the one
    # that bites: `imshow` puts its picture there, not in `patches` or
    # `collections`, so a converter that only looked at those would report a
    # figure with no marks as complete. `containers` is deliberately not scanned:
    # a `BarContainer` groups the rectangles that `patches` already carries, so
    # flagging it would refuse every bar chart.
    for attribute in ("collections", "images", "tables", "texts"):
        for artist in getattr(axes, attribute, []):
            unsupported.append(type(artist).__name__)

    if unsupported:
        raise ValueError(
            "figspec cannot represent "
            + ", ".join(sorted(set(unsupported)))
            + "; it converts lines and bars. Refusing rather than converting part of the figure"
        )
    if len([mark for mark in marks if mark["kind"] == "bar"]) > 1:
        # One bar mark carries the whole series, so merge the rectangles that share
        # a colour and are adjacent on the tick axis.
        marks = _merge_bars(marks)
    return marks


def _category_at(axes: Any, position: float) -> str:
    ticks = [float(value) for value in axes.get_xticks()]
    labels = [str(text.get_text()) for text in axes.get_xticklabels()]
    if not ticks:
        return f"{position:g}"
    nearest = min(range(len(ticks)), key=lambda index: abs(ticks[index] - position))
    if abs(ticks[nearest] - position) < 0.51 and nearest < len(labels) and labels[nearest]:
        return labels[nearest]
    return f"{position:g}"


def _merge_bars(marks: list[dict[str, Any]]) -> list[dict[str, Any]]:
    bars = [mark for mark in marks if mark["kind"] == "bar"]
    first = bars[0]
    merged = {
        "kind": "bar",
        "categories": [category for mark in bars for category in mark["categories"]],
        "values": [value for mark in bars for value in mark["values"]],
        "color": first.get("color"),
        "label": None,
    }
    out = [mark for mark in marks if mark["kind"] != "bar"]
    out.append(merged)
    return out


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
            "title": _title(figure),
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
