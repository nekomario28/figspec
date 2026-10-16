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
            "if self.y_min.is_some() || matches!(self.y_scale, Scale::Log) {",
            "if false {",
            ["--test", "heatmap", "a_heatmap_refuses_an_axis_setting_it_cannot_honour"],
        ),
        Mutation(
            "heatmap-cells-log-checked",
            "src/model.rs",
            "if logged && !matches!(mark, Mark::Heatmap { .. }) {",
            "if logged {",
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
        ]
    )
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
