#!/usr/bin/env python3
"""Install the built wheel in a fresh environment and draw a figure with it.

A wheel that builds is not a wheel that installs, and an installed package that
imports is not a package that works. This does the three things a consumer
actually does, in isolation:

    uv venv <fresh>                     # no checkout on sys.path
    uv pip install <the wheel>          # not editable, not the workspace
    python -c "import figspec; ..."     # render, inspect, exit accordingly

Nothing here touches the developer's environment: the venv lives in a temporary
directory and is deleted with it.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile

MODEL = {
    "title": "Consumer check",
    "x_label": "quarter",
    "y_label": "revenue (M JPY)",
    "marks": [
        {"kind": "bar", "categories": ["Q1", "Q2"], "values": [31.5, 41.0]},
        {"kind": "line", "points": [[0, 31.5], [1, 41.0]], "label": "trend"},
    ],
}

TRUNCATED = {**MODEL, "y_min": 25}

SCRIPT = """
import json, sys, pathlib
import figspec

model = json.loads(pathlib.Path(sys.argv[1]).read_text())
pathlib.Path(sys.argv[2]).write_text(figspec.render_svg(model), encoding="utf-8")
problems = figspec.inspect(model)
print(json.dumps({"problems": [p["code"] for p in problems], "svg": True}))
"""


def _run(argv: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(argv, capture_output=True, text=True, check=False, **kwargs)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-dir", default="dist", help="where the built wheels are")
    args = parser.parse_args(argv)

    wheels = sorted(pathlib.Path(args.wheel_dir).glob("figspec-*.whl"))
    assert wheels, f"no wheel in {args.wheel_dir}; build one with `maturin build`"
    wheel = wheels[-1]
    print(f"consumer: installing {wheel.name}", flush=True)

    with tempfile.TemporaryDirectory(prefix="figspec-consumer-") as directory:
        root = pathlib.Path(directory)
        venv = root / "venv"
        assert _run(["uv", "venv", str(venv), "--python", "3.13"]).returncode == 0
        # Windows puts an interpreter in Scripts/, everything else in bin/.
        python = venv / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        install = _run(["uv", "pip", "install", "--python", str(python), str(wheel)])
        assert install.returncode == 0, install.stdout + install.stderr
        # The point of the package: no numpy, no matplotlib.
        check = _run(
            [
                str(python),
                "-c",
                "import sys, figspec;"
                "print('numpy' in sys.modules, 'matplotlib' in sys.modules, figspec.__doc__ is not None)",
            ]
        )
        assert check.returncode == 0, check.stderr
        assert check.stdout.split() == ["False", "False", "True"], check.stdout
        print("consumer: import brings neither numpy nor matplotlib", flush=True)

        model_path = root / "model.json"
        svg_path = root / "model.svg"
        model_path.write_text(json.dumps(MODEL), encoding="utf-8")
        run = subprocess.run(
            [str(python), "-c", SCRIPT, str(model_path), str(svg_path)],
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
            cwd=root,
        )
        assert run.returncode == 0, run.stdout + run.stderr
        report = json.loads(run.stdout)
        assert report["problems"] == [], report
        assert svg_path.read_text(encoding="utf-8").startswith("<svg")

        truncated_path = root / "truncated.json"
        truncated_path.write_text(json.dumps(TRUNCATED), encoding="utf-8")
        run = subprocess.run(
            [str(python), "-c", SCRIPT, str(truncated_path), str(root / "t.svg")],
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
            cwd=root,
        )
        assert json.loads(run.stdout)["problems"] == ["truncated-baseline"], run.stdout
        print("consumer: the checks run from the installed wheel", flush=True)

    print("Consumer check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
