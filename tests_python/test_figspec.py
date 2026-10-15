"""The Python surface, including the matplotlib importer."""

from __future__ import annotations

import json

import pytest

import figspec


def honest_model() -> dict:
    return {
        "title": "Quarterly revenue",
        "x_label": "quarter",
        "y_label": "revenue (M JPY)",
        "marks": [{"kind": "bar", "categories": ["Q1", "Q2"], "values": [31.5, 41.0]}],
    }


def test_a_clean_model_has_nothing_to_report():
    assert figspec.inspect(honest_model()) == []


def test_a_truncated_baseline_is_an_error_naming_the_mark():
    model = honest_model() | {"y_min": 25}
    problems = figspec.inspect(model)
    assert [p["code"] for p in problems] == ["truncated-baseline"]
    assert problems[0]["severity"] == "error"
    assert problems[0]["mark"] == 0


def test_an_unknown_field_is_refused_rather_than_ignored():
    with pytest.raises(ValueError):
        figspec.inspect(honest_model() | {"titlee": "typo"})


def test_the_same_model_renders_to_the_same_bytes():
    model = honest_model()
    assert figspec.render_svg(model) == figspec.render_svg(model)
    assert figspec.render_png(model) == figspec.render_png(model)


def test_the_model_is_what_is_rendered(tmp_path):
    model = honest_model()
    path = figspec.save(model, tmp_path / "revenue.svg")
    assert path.read_text(encoding="utf-8").startswith("<svg")
    assert figspec.load(path.with_suffix(".json")) if False else True  # json path below
    figspec.save(model, tmp_path / "revenue.json")
    assert figspec.load(tmp_path / "revenue.json") == model


def test_from_matplotlib_converts_a_real_figure():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots()
    axes.bar(["Q1", "Q2"], [31.5, 41.0])
    model = figspec.from_matplotlib(figure)
    assert [mark["kind"] for mark in model["marks"]] == ["bar"]
    assert model["marks"][0]["values"] == [31.5, 41.0]
    assert model["marks"][0]["categories"] == ["Q1", "Q2"]
    # The converted figure renders and is checkable.
    assert figspec.inspect(model | {"title": "Revenue", "x_label": "q", "y_label": "M JPY"}) == []
    assert figspec.render_svg(model).startswith("<svg")


def test_from_matplotlib_carries_a_truncated_axis_into_the_checks():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots()
    axes.bar(["Q1", "Q2"], [31.5, 41.0])
    axes.set_ylim(25, None)  # the classic trimmed bar chart
    model = figspec.from_matplotlib(figure)
    assert model["y_min"] == 25.0
    # A bare figure also has no title or axis labels, so the finding of interest
    # is asserted among them, not as the only one.
    problems = figspec.inspect(model)
    assert "truncated-baseline" in [p["code"] for p in problems]
    assert [p["severity"] for p in problems if p["code"] == "truncated-baseline"] == ["error"]


def test_from_matplotlib_refuses_what_it_cannot_represent():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots()
    axes.imshow([[1.0, 0.0], [0.0, 1.0]])
    with pytest.raises(ValueError, match="cannot represent"):
        figspec.from_matplotlib(figure)
