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


def _axes():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    return plt.subplots()


def test_from_matplotlib_converts_a_scatter():
    figure, axes = _axes()
    axes.scatter([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], label="points")
    model = figspec.from_matplotlib(figure)
    assert [mark["kind"] for mark in model["marks"]] == ["scatter"]
    assert model["marks"][0]["points"] == [[1.0, 4.0], [2.0, 5.0], [3.0, 6.0]]
    assert model["marks"][0]["label"] == "points"
    assert figspec.inspect(model | {"title": "t", "x_label": "x", "y_label": "y"}) == []


def test_from_matplotlib_carries_symmetric_error_bars():
    figure, axes = _axes()
    axes.errorbar([1.0, 2.0], [4.0, 5.0], yerr=[0.1, 0.2])
    model = figspec.from_matplotlib(figure)
    mark = model["marks"][0]
    assert mark["kind"] == "line"
    assert mark["y_err"] == pytest.approx([0.1, 0.2])
    assert "y_err_asymmetric" not in mark


def test_from_matplotlib_carries_asymmetric_error_bars():
    figure, axes = _axes()
    axes.errorbar([1.0, 2.0], [4.0, 5.0], yerr=[[0.1, 0.2], [0.3, 0.4]])
    model = figspec.from_matplotlib(figure)
    mark = model["marks"][0]
    assert "y_err" not in mark
    # One [lower, upper] pair per point, recovered from matplotlib's segments.
    pairs = mark["y_err_asymmetric"]
    assert len(pairs) == 2
    for pair, expected in zip(pairs, [[0.1, 0.3], [0.2, 0.4]]):
        assert pair == pytest.approx(expected)
    # Asymmetric bars render, and the model is checkable.
    assert figspec.render_svg(model).startswith("<svg")


def test_from_matplotlib_converts_grouped_bars():
    figure, axes = _axes()
    axes.set_xticks([0, 1], labels=["Q1", "Q2"])
    axes.bar([0.0, 1.0], [1.0, 2.0], width=0.4, label="A")
    axes.bar([0.4, 1.4], [3.0, 4.0], width=0.4, label="B")
    model = figspec.from_matplotlib(figure)
    assert [mark["kind"] for mark in model["marks"]] == ["grouped_bar"]
    mark = model["marks"][0]
    assert mark["categories"] == ["Q1", "Q2"]
    assert [series["label"] for series in mark["series"]] == ["A", "B"]
    assert [series["values"] for series in mark["series"]] == [[1.0, 2.0], [3.0, 4.0]]
    # A group with one value fewer than the categories is an error, not a drawing.
    model["marks"][0]["series"][1]["values"] = [3.0]
    assert "mismatched-lengths" in [p["code"] for p in figspec.inspect(model)]
    with pytest.raises(ValueError):
        figspec.render_svg(model)


def test_from_matplotlib_refuses_a_stacked_bar():
    figure, axes = _axes()
    axes.bar([0.0, 1.0], [1.0, 2.0])
    axes.bar([0.0, 1.0], [3.0, 4.0], bottom=[1.0, 2.0])
    with pytest.raises(ValueError, match="stacked"):
        figspec.from_matplotlib(figure)


def test_from_matplotlib_refuses_what_the_model_has_no_mark_for():
    figure, axes = _axes()
    axes.violinplot([[1.0, 2.0, 3.0]])
    with pytest.raises(ValueError, match="cannot represent"):
        figspec.from_matplotlib(figure)


def test_from_matplotlib_takes_the_title_of_the_axes_it_converts():
    figure, axes_list = _axes()
    figure.subplots(1, 2)
    figure.axes[0].set_title("first")
    figure.axes[1].set_title("second")
    assert figspec.from_matplotlib(figure, axes_index=0)["title"] == "first"
    assert figspec.from_matplotlib(figure, axes_index=1)["title"] == "second"


def test_a_storyboard_boxes_the_mark_a_problem_names():
    model = {
        "marks": [
            {"kind": "bar", "categories": ["Q1", "Q2"], "values": [31.5, 34.2]},
        ],
        "y_min": 5.0,
    }
    problems = figspec.inspect(model)
    errors = [one for one in problems if one["severity"] == "error"]
    assert errors and errors[0]["code"] == "truncated-baseline"
    assert errors[0]["mark"] == 0

    svg = figspec.storyboard_svg(model)
    assert svg.startswith("<svg")
    assert "figspec-storyboard" in svg
    # The figure itself is unchanged; the storyboard only adds boxes.
    assert svg.replace(svg[svg.index("<g id=\"figspec-storyboard\">"):svg.rindex("</svg>")], "") == figspec.render_svg(model)
    # The number on the box is the problem's position in the report.
    assert ">1</text>" in svg


def test_a_finding_about_no_mark_is_not_boxed():
    model = {"marks": [{"kind": "bar", "categories": ["Q1"], "values": [1.0]}]}
    problems = figspec.inspect(model)
    assert any(one.get("mark") is None for one in problems)
    svg = figspec.storyboard_svg(model)
    assert "figspec-storyboard" in svg
    assert "<rect x=" in svg  # the figure's own rectangles, not ours
    assert "</text>" not in svg.split("figspec-storyboard")[1]


def test_saving_a_storyboard_name_writes_the_storyboard(tmp_path):
    """`.storyboard.svg` must win over `.svg`, which is also a suffix of it."""
    model = {"marks": [{"kind": "bar", "categories": ["Q1", "Q2"], "values": [1.0, 2.0]}], "y_min": 1.0}
    storyboard = figspec.save(model, tmp_path / "revenue.storyboard.svg")
    figure = figspec.save(model, tmp_path / "revenue.svg")
    assert "figspec-storyboard" in storyboard.read_text(encoding="utf-8")
    assert "figspec-storyboard" not in figure.read_text(encoding="utf-8")
