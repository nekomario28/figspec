#!/usr/bin/env python3
"""Restore named faults in disposable copies and require the tests to notice.

A test that passes proves nothing about what it would catch. Each mutation below
removes one rule — the row reversal a converted image depends on, the refusal that
keeps an undeclared axis start off a matrix, the category pitch a box is drawn on —
and the named test must fail, with an *assertion*, not because the crate stopped
compiling.

The controls run first: the same tests, unmutated, must pass. Without them a
mutation "failing" could be a broken test rather than a caught fault.

Nothing is edited in place: the sources are copied to a scratch tree (the build
cache with them, so a mutant rebuilds in seconds), mutated there, and deleted with
it. `cargo test` and `pytest` are given the same selector the fault is aimed at.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

#: Where the disposable copy lives. It is a *persistent* scratch tree: the build
#: cache is kept between runs (`CARGO_TARGET_DIR` points into it), so the first run
#: pays for the dependency build and every run after it is incremental. `/tmp` is a
#: small tmpfs on many machines, so the cache defaults to the XDG cache directory
#: rather than to `tempfile.gettempdir()`.
SCRATCH = Path(
    os.environ.get("FIGSPEC_MUTATION_SCRATCH")
    or Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "figspec-mutations"
)


@dataclass
class Mutation:
    name: str
    path: str
    before: str
    after: str
    #: The test that must fail, as arguments to `cargo test` or `pytest`.
    selector: list[str] = field(default_factory=list)
    python: bool = False


RUST = ["cargo", "test", "--quiet"]
def python_runner(scratch: Path) -> list[str]:
    """A pytest that belongs to nobody's project.

    `uv run` discovers the project it is invoked near and builds it — which both
    tried to build the scratch copy and pulled the *real* checkout's test files
    into the run. An interpreter made once, called with an absolute test path and
    `PYTHONPATH` at the copy, does neither.
    """
    venv = scratch / "venv"
    python = venv / "bin" / "python"
    if not python.exists():
        subprocess.run(
            ["uv", "venv", str(venv), "--python", "3.13"], check=True, capture_output=True
        )
        subprocess.run(
            ["uv", "pip", "install", "--python", str(python), "pytest", "matplotlib"],
            check=True,
            capture_output=True,
        )
    return [str(python), "-m", "pytest", "-q", "-p", "no:cacheprovider"]


def mutations() -> list[Mutation]:
    cases = [
        # --- the model's rules -------------------------------------------------
        Mutation(
            "heatmap-y-min-accepted",
            "src/model.rs",
            "        if self.y_min.is_some()",
            "        if false",
            ["--test", "heatmap", "a_heatmap_refuses_an_axis_setting_it_cannot_honour"],
        ),
        Mutation(
            "heatmap-cells-log-checked",
            "src/model.rs",
            "            if !matches!(mark, Mark::Heatmap { .. }) {",
            "            if true {",
            ["--test", "heatmap", "a_heatmap_refuses_an_axis_setting_it_cannot_honour"],
        ),
        Mutation(
            "empty-mark-not-refused",
            "src/model.rs",
            "            if mark.is_empty() {",
            "            if false {",
            ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
        ),
        Mutation(
            "short-line-not-refused",
            "src/model.rs",
            "                if points.len() == 1 {",
            "                if false {",
            ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
        ),
        Mutation(
            "ragged-heatmap-accepted",
            "src/model.rs",
            "                        if cells.len() != width {",
            "                        if false {",
            ["--test", "heatmap", "a_ragged_matrix_is_an_error"],
        ),
        Mutation(
            "ambiguous-labels-accepted",
            "src/model.rs",
            "            if declared.iter().any(|labels| *labels != *first) {",
            "            if false {",
            [
                "--test",
                "heatmap",
                "two_names_for_one_axis_are_refused_rather_than_picked_between",
            ],
        ),
        Mutation(
            "log-check-uses-both-axes",
            "src/model.rs",
            "                    (Scale::Log, Scale::Linear) => mark.x_values(),",
            "                    (Scale::Log, Scale::Linear) => mark.values(),",
            ["--test", "model_and_render", "a_log_axis_does_not_read_the_other_axes_values"],
        ),
        Mutation(
            "grouped-bar-emptiness-ignored",
            "src/model.rs",
            "                    || series.iter().all(|one| one.values.is_empty())",
            "                    || false",
            ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
        ),
        Mutation(
            "empty-string-title-accepted",
            "src/inspect.rs",
            "    if figure.title.as_deref().is_none_or(str::is_empty) {",
            "    if figure.title.is_none() {",
            ["--test", "model_and_render", "an_empty_string_is_not_a_title_or_a_label"],
        ),
        Mutation(
            "degenerate-canvas-accepted",
            "src/model.rs",
            "            if size == Some(0) {",
            "            if false {",
            ["--test", "model_and_render", "a_canvas_with_no_extent_is_refused_at_the_reader"],
        ),
        Mutation(
            "column-labels-unchecked",
            "src/model.rs",
            "                        _ => col_labels.as_ref(),",
            "                        _ => None,",
            ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
        ),
        Mutation(
            "truncated-baseline-message-stale",
            "src/inspect.rs",
            "the y axis starts at",
            "bars start at",
            ["--test", "model_and_render", "the_truncated_baseline_message_names_the_axis_not_the_bars"],
        ),
        Mutation(
            "barh-waved-through",
            "python/figspec/__init__.py",
            "    if (_spread(tall) < 1e-9 and _spread(thick) > 1e-9) or (",
            "    if False and (",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        Mutation(
            "error-bar-caps-become-marks",
            "python/figspec/__init__.py",
            "        for cap in tuple(container.lines[1]) if len(container.lines) > 1 else ():",
            "        for cap in ():",
            ["tests_python/test_figspec.py", "-k", "error_bar_caps"],
            python=True,
        ),
        Mutation(
            "x-errors-read-as-y-errors",
            "python/figspec/__init__.py",
            "        if has_xerr:",
            "        if False:",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        Mutation(
            "bar-widths-compared-exactly",
            "python/figspec/__init__.py",
            "    if _spread(thick) > 1e-9:",
            "    if len(set(thick)) != 1:",
            ["tests_python/test_figspec.py", "-k", "float_offsets"],
            python=True,
        ),
        Mutation(
            "tick-tolerance-is-the-bar-width",
            "python/figspec/__init__.py",
            "            limit=max(1e-9, min(abs(width) / 2.0, 0.25)),",
            "            limit=abs(width),",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        Mutation(
            "scatter-sizes-ignored",
            "python/figspec/__init__.py",
            "            if sizes and (_spread(sizes) > 1e-9 or abs(sizes[0] - 36.0) > 1e-9):",
            "            if False:",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        Mutation(
            "inverted-x-axis-accepted",
            "python/figspec/__init__.py",
            "        if axes.xaxis_inverted():",
            "        if False:",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        Mutation(
            "twin-axes-dropped",
            "python/figspec/__init__.py",
            "            if axes.get_shared_x_axes().joined(axes, other) and list(",
            "            if False and list(",
            ["tests_python/test_figspec.py", "-k", "right_reason"],
            python=True,
        ),
        # --- where a mark lands ------------------------------------------------
        Mutation(
            "bar-category-pitch-shifted",
            "src/place.rs",
            "    index as f64 + 1.0",
            "    index as f64",
            ["--test", "storyboard", "a_bar_maps_onto_the_centre_of_the_rectangle_the_engine_draws"],
        ),
        Mutation(
            "heatmap-extent-collapsed",
            "src/place.rs",
            "                let (y_low, y_high) = self.category_span(values.len())?;",
            "                let (y_low, y_high) = self.category_span(1)?;",
            ["--test", "storyboard", "a_heatmap_box_covers_every_cell_the_engine_draws"],
        ),
        Mutation(
            "log-mapping-linearised",
            "src/place.rs",
            "            (value.ln() - low.ln()) / (high.ln() - low.ln())",
            "            (value - low) / (high - low)",
            ["--test", "storyboard", "a_log_axis_maps_as_the_engine_does"],
        ),
        # --- the documents -----------------------------------------------------
        Mutation(
            "skip-instead-of-asserting",
            "tests_python/test_figspec.py",
            "    assert [p[\"code\"] for p in errors] == [\"mismatched-lengths\"], problems",
            "    import pytest; pytest.skip(\"dodging the assertion\")",
            ["tests_python/test_figspec.py", "-k", "ragged_matrix"],
            python=True,
        ),
        Mutation(
            "readme-figure-stale",
            "examples/storyboard_demo.json",
            '"y_min": 25,',
            '"y_min": 24,',
            ["tests_python/test_figspec.py", "-k", "readme_figure"],
            python=True,
        ),
        Mutation(
            "readme-link-dangling",
            "README.md",
            "[`docs/CONTRACT.md`](docs/CONTRACT.md)",
            "[`docs/CONTRACT.md`](docs/PLAN.md)",
            ["--test", "docs", "every_link_in_the_entry_documents_resolves"],
        ),
        # --- the storyboard ----------------------------------------------------
        Mutation(
            "storyboard-numbering-off-by-one",
            "src/storyboard.rs",
            "        let number = index + 1;",
            "        let number = index;",
            ["--test", "storyboard", "the_numbers_are_the_report_positions"],
        ),
        Mutation(
            "storyboard-unlocated-problem-boxed",
            "src/storyboard.rs",
            "        let Some(mark) = problem.mark else {",
            "        let Some(mark) = Some(0) else {",
            ["--test", "storyboard", "a_finding_about_no_mark_is_reported_and_not_boxed"],
        ),
    ]
    # --- the matplotlib importer (Python) -------------------------------------
    cases.extend(
        [
            Mutation(
                "imshow-rows-not-reversed",
                "python/figspec/__init__.py",
                "        if upper:\n            values.reverse()",
                "        if upper:\n            pass",
                ["tests_python/test_figspec.py", "-k", "the_same_colours"],
                python=True,
            ),
            Mutation(
                "imshow-extent-ignored",
                "python/figspec/__init__.py",
                "        if (\n            abs(left - expected[0]) > 1e-9",
                "        if False and (\n            abs(left - expected[0]) > 1e-9",
                ["tests_python/test_figspec.py", "-k", "what_the_converter_refuses"],
                python=True,
            ),
            Mutation(
                "imshow-unknown-ramp-accepted",
                "python/figspec/__init__.py",
                "        if name not in MODEL_RAMPS:",
                "        if False:",
                ["tests_python/test_figspec.py", "-k", "what_the_converter_refuses"],
                python=True,
            ),
            Mutation(
                "imshow-missing-cell-accepted",
                "python/figspec/__init__.py",
                "                if cell is None or not isinstance(cell, (int, float)) or cell != cell:",
                "                if False:",
                ["tests_python/test_figspec.py", "-k", "what_the_converter_refuses"],
                python=True,
            ),
            # The raster budget and the command line: seven rules, each of which was
            # a panic, a silent mis-write or a silently ignored argument before.
            Mutation(
                "raster-canvas-budget-unchecked",
                "src/render.rs",
                "    if width * height > MAX_RASTER_PIXELS {",
                "    if false {",
                ["--test", "raster", "a_canvas_the_raster_cannot_hold_is_refused_by_name"],
            ),
            Mutation(
                "raster-marker-budget-unchecked",
                "src/render.rs",
                "        if !side.is_finite() || side * side > MAX_RASTER_PIXELS {",
                "        if false {",
                ["--test", "raster", "a_marker_the_raster_cannot_hold_is_refused_by_name"],
            ),
            Mutation(
                "raster-mask-side-simplified",
                "src/render.rs",
                "    2.0 * (radius + 1.5).ceil() + 1.0",
                "    2.0 * radius + 1.0",
                ["--test", "raster", "the_marker_budget_is_the_mask_the_backend_allocates"],
            ),
            Mutation(
                "second-model-ignored",
                "src/bin/figspec.rs",
                "                if flags.model.is_some() {",
                "                if false {",
                ["--test", "cli", "a_second_model_file_is_refused_rather_than_ignored"],
            ),
            Mutation(
                "unknown-option-ignored",
                "src/bin/figspec.rs",
                '                return Err(format!("unknown option {other:?}; try `figspec --help`"));',
                "                index += 1;",
                ["--test", "cli", "an_unknown_option_is_refused_rather_than_ignored"],
            ),
            Mutation(
                "storyboard-extension-unchecked",
                "src/bin/figspec.rs",
                '    if !matches!(extension.as_str(), "" | "svg") {',
                "    if false {",
                [
                    "--test",
                    "cli",
                    "a_storyboard_refuses_a_path_that_would_lie_about_what_it_holds",
                ],
            ),
            Mutation(
                "negative-marker-size-accepted",
                "src/model.rs",
                "                if *size < 0.0 {",
                "                if false {",
                ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
            ),
        ]
    )
    # Drawn-output boundaries: geometry, XML text, report locations and plates.
    cases.extend([
        Mutation(
            "plot-area-unchecked", "src/render.rs",
            "    check_plot_area(&ComputedLayout::from_layout(&layout))?;", "",
            ["--test", "model_and_render", "a_canvas_with_no_plot_area_is_refused_by_name"],
        ),
        Mutation(
            "xml-characters-unchecked", "src/model.rs",
            "    let bad = text.chars().find(|character| !xml_allows(*character))?;",
            "    let bad = text.chars().find(|_| false)?;",
            ["--test", "refusals", "everything_that_cannot_be_drawn_is_reported_as_an_error"],
        ),
        Mutation(
            "axis-label-text-unchecked", "src/model.rs",
            '            ("y_label", &self.y_label),', "",
            ["--test", "model_and_render", "a_character_xml_does_not_allow_is_refused"],
        ),
        Mutation(
            "series-label-text-unchecked", "src/model.rs",
            '                    fields.push((format!("series[{index}].label"), label));', "",
            ["--test", "model_and_render", "a_character_xml_does_not_allow_is_refused"],
        ),
        Mutation(
            "overlapping-bars-unchecked", "src/inspect.rs",
            "        for earlier in &bars[..position] {", "        for earlier in &bars[..0] {",
            ["--test", "model_and_render", "two_bar_marks_are_drawn_over_each_other"],
        ),
        Mutation(
            "overlapping-bars-consecutive-only", "src/inspect.rs",
            "        for earlier in &bars[..position] {",
            "        for earlier in &bars[position.saturating_sub(1)..position] {",
            ["--test", "model_and_render", "every_later_bar_mark_is_reported_against_every_earlier_one"],
        ),
        Mutation(
            "plate-y-not-clamped", "src/storyboard.rs",
            "        let y = (boxed.rect.top - PLATE_HEIGHT).clamp(0.0, (area.height - PLATE_HEIGHT).max(0.0));",
            "        let y = boxed.rect.top - PLATE_HEIGHT;",
            ["--test", "storyboard", "a_number_plate_stays_inside_the_picture"],
        ),
        Mutation(
            "plate-x-not-clamped", "src/storyboard.rs",
            "        let x = boxed.rect.left.clamp(0.0, (area.width - plate).max(0.0));",
            "        let x = boxed.rect.left;",
            ["--test", "storyboard", "a_plate_is_kept_inside_the_canvas_on_the_right_too"],
        ),
        Mutation(
            "unboxable-called-unlocated", "src/storyboard.rs",
            "            unboxable.push(number);", "            unlocated.push(number);",
            ["--test", "storyboard", "a_finding_about_a_mark_the_figure_has_not_got_is_not_about_no_mark"],
        ),
        Mutation(
            "warning-box-painted-error", "src/storyboard.rs",
            "        } else {\n            OTHER_COLOR\n        };",
            "        } else {\n            ERROR_COLOR\n        };",
            ["--test", "storyboard", "a_warning_about_a_mark_is_still_boxed"],
        ),
        Mutation(
            "invisible-bar-box-invented", "src/place.rs",
            "        if matches!(mark, Mark::Bar { .. } | Mark::GroupedBar { .. })",
            "        if false && matches!(mark, Mark::Bar { .. } | Mark::GroupedBar { .. })",
            ["--test", "drawn_boundary", "bars_with_no_visible_height_have_no_box"],
        ),
        Mutation(
            "oversize-number-plate-accepted", "src/storyboard.rs",
            "        if plate > area.width || PLATE_HEIGHT > area.height {", "        if false {",
            ["--test", "drawn_boundary", "a_number_plate_wider_than_the_canvas_is_refused"],
        ),
        Mutation(
            "mark-text-reported-twice", "src/model.rs",
            "            let values = mark.values();",
            "            refusals.extend(mark.text_fields().into_iter().filter_map(|(field, text)| unwritable(&field, text, Some(index))));\n            let values = mark.values();",
            ["--test", "drawn_boundary", "an_unwritable_mark_field_is_reported_once"],
        ),
    ])
    # Independent review: all emitted strings and actual filled intersections.
    cases.extend([
        Mutation(
            "mark-color-xml-unchecked", "src/model.rs",
            '[("label", label), ("color", color)]',
            '[("label", label), ("color", None)]',
            ["--test", "drawn_boundary", "colors_obey_the_same_xml_character_contract_as_names"],
        ),
        Mutation(
            "series-color-xml-unchecked", "src/model.rs",
            '                    fields.push((format!("series[{index}].color"), color));', "",
            ["--test", "drawn_boundary", "colors_obey_the_same_xml_character_contract_as_names"],
        ),
        Mutation(
            "inverted-y-range-accepted", "src/render.rs",
            "        if !finite || upper <= lower {", "        if !finite {",
            ["--test", "drawn_boundary", "an_inverted_computed_axis_is_refused_before_boxing"],
        ),
        Mutation(
            "nonoverlapping-bars-reported", "src/inspect.rs",
            "            if !bars_overlap(figure, *earlier, *later) {",
            "            if false {",
            ["--test", "drawn_boundary", "only_filled_bar_intersections_are_overlaps"],
        ),
        Mutation(
            "grouped-overlaps-unchecked", "src/inspect.rs",
            "        .filter(|(_, mark)| mark.categories().is_some())",
            "        .filter(|(_, mark)| matches!(mark, Mark::Bar { .. }))",
            ["--test", "drawn_boundary", "grouped_and_mixed_bars_are_checked_in_their_own_subslots"],
        ),
        Mutation(
            "bar-subslots-confused", "src/inspect.rs",
            "bar_value(b, category, j)", "bar_value(b, category, i)",
            ["--test", "drawn_boundary", "grouped_and_mixed_bars_are_checked_in_their_own_subslots"],
        ),
    ])
    cases.append(Mutation(
        "bar-overlap-ignores-axis-minimum", "src/inspect.rs",
        "    let minimum = figure.y_min.unwrap_or(f64::NEG_INFINITY);",
        "    let minimum = f64::NEG_INFINITY;",
        ["--test", "drawn_boundary", "only_filled_bar_intersections_are_overlaps"],
    ))
    cases.append(Mutation(
        "nonfinite-computed-range-accepted", "src/render.rs",
        "        if !finite || upper <= lower {", "        if upper <= lower {",
        ["--test", "drawn_boundary", "a_nonfinite_computed_span_is_refused"],
    ))
    cases.append(Mutation(
        "agent-code-list-incomplete", "llms.txt",
        "inapplicable-axis-setting (error), ", "",
        ["--test", "docs", "the_agent_code_list_matches_the_codebook"],
    ))
    cases.extend([
        Mutation(
            "unsupported-colors-accepted", "src/model.rs",
            '    let supported = valid_hex && !nonfinite_rgb && !matches!(Color::from(text), Color::Css(_));',
            '    let supported = true;',
            ["--test", "render_text", "unsupported_colors_are_reported_and_refused_without_panicking"],
        ),
        Mutation(
            "nonascii-hex-reaches-parser", "src/model.rs",
            'valid_hex && !nonfinite_rgb',
            'true && !nonfinite_rgb',
            ["--test", "render_text", "unsupported_colors_are_reported_and_refused_without_panicking"],
        ),
        Mutation(
            "nonfinite-rgb-components-accepted", "src/model.rs",
            'valid_hex && !nonfinite_rgb',
            'valid_hex && true',
            ["--test", "render_text", "unsupported_colors_are_reported_and_refused_without_panicking"],
        ),
        Mutation(
            "simple-mark-color-unchecked", "src/model.rs",
            '(field == "color" || field.ends_with(".color"))',
            '(field.ends_with(".color"))',
            ["--test", "render_text", "unsupported_colors_are_reported_and_refused_without_panicking"],
        ),
        Mutation(
            "grouped-series-color-unchecked", "src/model.rs",
            '(field == "color" || field.ends_with(".color"))',
            '(field == "color")',
            ["--test", "render_text", "unsupported_colors_are_reported_and_refused_without_panicking"],
        ),
        Mutation(
            "declared-text-renamed-as-id", "src/render.rs",
            '    kuva::backend::svg::SvgBackend.render_scene(&scene)',
            '    kuva::backend::svg::SvgBackend.render_scene(&scene).replace("kuva-clip-999", "kuva-clip-0")',
            ["--test", "render_text", "engine_id_normalization_never_rewrites_declared_text"],
        ),
        Mutation(
            "scene-clip-id-not-normalized", "src/render.rs",
            '        *id = normalized;', '',
            ["--test", "render_text", "clip_declarations_and_references_stay_consistent_across_renders"],
        ),
        Mutation(
            "heatmap-row-labels-dropped", "src/render.rs",
            '                layout = layout.with_y_categories(rows.clone());', '',
            ["--test", "render_text", "heatmap_labels_are_independent_axis_declarations"],
        ),
        Mutation(
            "heatmap-column-labels-dropped", "src/render.rs",
            '                layout = layout.with_x_categories(cols.clone());', '',
            ["--test", "render_text", "heatmap_labels_are_independent_axis_declarations"],
        ),
    ])
    cases.extend([
        Mutation(
            "signed-hex-components-accepted", "src/model.rs",
            "hex.bytes().all(|byte| byte.is_ascii_hexdigit())", "true",
            ["--test", "render_text", "malformed_hex_is_not_a_rgb_triplet"],
        ),
        Mutation(
            "rgb-whitespace-overrefused", "src/model.rs",
            "valid_hex && !nonfinite_rgb && !matches!(Color::from(text), Color::Css(_))",
            "text.is_ascii() && valid_hex && !nonfinite_rgb && !matches!(Color::from(text), Color::Css(_))",
            ["--test", "render_text", "shared_colors_still_draw"],
        ),
    ])
    return cases


def prepare(scratch: Path) -> Path:
    """A disposable copy of the repository, without the build cache.

    Everything is copied except what a mutated build must not touch or cannot use.
    An allowlist was the first attempt and it drifted twice — `examples/` and then
    `conftest.py` were each missing, and a missing file looks exactly like a
    surviving mutant.
    """
    tree = scratch / "tree"
    if tree.exists():
        shutil.rmtree(tree)
    shutil.copytree(
        ROOT,
        tree,
        ignore=shutil.ignore_patterns(
            ".git",
            ".venv",
            "target",
            "dist",
            "__pycache__",
            "*.pyc",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
        ),
        # `copy` (not `copy2`): cargo decides freshness by *mtime*, and a copy that
        # preserves the original's timestamp can look older than the cached artifact
        # built from a different revision. Measured: with `copy2` a stale test binary
        # was served and a control failed.
        copy_function=shutil.copy,
    )
    return tree


def run(tree: Path, mutation: Mutation) -> tuple[int, str]:
    selector = list(mutation.selector)
    if mutation.python:
        # Absolute, so pytest cannot wander back to the real checkout and grade it.
        selector[0] = str(tree / selector[0])
        command = python_runner(tree.parent) + selector
    else:
        command = RUST + selector
    env = os.environ.copy()
    if not mutation.python:
        # The scratch cache is kept between runs: the first one builds the
        # dependencies, the rest rebuild only the mutated crate.
        env["CARGO_TARGET_DIR"] = str(tree.parent / "target")
    if mutation.python:
        # The editable install points at the real checkout, so the scratch copy has
        # to come first on the path or the mutant would never be imported.
        env["PYTHONPATH"] = str(tree / "python")
        # The harness runs where matplotlib is installed, so a test that skips is a
        # disabled test rather than a missing capability (conftest.py enforces it).
        env["FIGSPEC_REQUIRE_ALL_CAPABILITIES"] = "1"
    result = subprocess.run(
        command, cwd=tree, capture_output=True, text=True, timeout=1800, env=env
    )
    return result.returncode, (result.stdout or "") + (result.stderr or "")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keep", action="store_true", help="leave the scratch tree behind")
    parser.add_argument("--only", help="run one mutation by name")
    args = parser.parse_args(argv)

    cases = [case for case in mutations() if not args.only or case.name == args.only]
    if not cases:
        print(f"no mutation named {args.only!r}", file=sys.stderr)
        return 2

    SCRATCH.mkdir(parents=True, exist_ok=True)
    tree = prepare(SCRATCH)
    print(f"scratch tree: {tree}", flush=True)

    # The Python mutants are only meaningful if the copy's package is the one that
    # gets imported: the editable install points at the real checkout, so this is
    # checked once, loudly, rather than assumed.
    if any(case.python for case in cases):
        built = sorted((tree / "python" / "figspec").glob("_figspec*.so")) + sorted(
            (tree / "python" / "figspec").glob("_figspec*.pyd")
        )
        if not built:
            print(
                "FAIL: the compiled extension is not in the scratch copy, so the "
                "Python tests cannot import it. Build it first: "
                "`maturin develop --release`",
                file=sys.stderr,
            )
            return 1
        runner = python_runner(SCRATCH)
        env = {**os.environ, "PYTHONPATH": str(tree / "python")}
        probe = subprocess.run(
            [*runner[:1], "-c", "import figspec; print(figspec.__file__)"],
            cwd=tree,
            capture_output=True,
            text=True,
            env=env,
        )
        where = probe.stdout.strip()
        if not where.startswith(str(tree)):
            print(
                f"FAIL: `import figspec` resolves to {where!r}, not the scratch tree; "
                "the Python mutants would grade the real checkout",
                file=sys.stderr,
            )
            print((probe.stderr or probe.stdout)[-800:], file=sys.stderr)
            return 1
        print(f"python mutants import {where}", flush=True)

    # Controls: the same tests, unmutated, must pass — otherwise a "caught" fault
    # could be a broken test.
    for case in cases:
        code, output = run(tree, case)
        if code != 0:
            print(f"FAIL: the control for {case.name} does not pass", file=sys.stderr)
            print(output[-2000:], file=sys.stderr)
            return 1
    print(f"controls pass ({len(cases)} mutations)", flush=True)

    survivors: list[str] = []
    for case in cases:
        path = tree / case.path
        original = path.read_text(encoding="utf-8")
        if original.count(case.before) != 1:
            print(f"FAIL: {case.name}: ambiguous/stale mutation target", file=sys.stderr)
            return 1
        path.write_text(original.replace(case.before, case.after), encoding="utf-8")
        try:
            code, output = run(tree, case)
        finally:
            path.write_text(original, encoding="utf-8")
        lowered = output.lower()
        if case.python:
            # pytest fails only on test outcomes, so a failure is a caught fault —
            # but its wording varies ("assertion", "DID NOT RAISE", "Failed").
            caught = code != 0 and "failed" in lowered
        else:
            # A Rust mutant that does not compile is not a caught fault.
            caught = code != 0 and ("assertion" in lowered or "panicked" in lowered)
        if code != 0 and not caught:
            print(f"     {case.name}: failed for the wrong reason (not an assertion)")
            print("\n".join(line for line in output.splitlines() if "error" in line.lower())[:600])
        if caught:
            print(f"killed: {case.name}", flush=True)
        else:
            survivors.append(case.name)
            print(f"SURVIVED: {case.name}", flush=True)

    if survivors:
        print(f"\n{len(survivors)} mutation(s) survived: {', '.join(survivors)}", file=sys.stderr)
        return 1
    print(f"\nAll {len(cases)} mutations were caught.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
