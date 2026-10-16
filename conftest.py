"""A skipped test is not a passing test.

The importer tests need matplotlib, and they say so with `importorskip`. That is
correct where matplotlib is absent — but this project's gate installs it, and on a
machine that has it a skip means a test was switched off rather than a capability
missing. `FIGSPEC_REQUIRE_ALL_CAPABILITIES=1` turns that into a failure; the local
gate and the CI test lane set it, and a bare `cargo test` or a checkout without
matplotlib keeps the documented behaviour.
"""

from __future__ import annotations

import os

_SKIPPED: list[str] = []


def pytest_runtest_logreport(report) -> None:  # noqa: ANN001 - pytest's type
    if not report.skipped:
        return
    reason = report.longrepr
    if isinstance(reason, tuple) and len(reason) == 3:
        reason = reason[2]
    _SKIPPED.append(f"{report.nodeid}: {reason}")


def pytest_sessionfinish(session, exitstatus) -> None:  # noqa: ANN001 - pytest's type
    if not os.environ.get("FIGSPEC_REQUIRE_ALL_CAPABILITIES") or not _SKIPPED:
        return
    print(
        f"\nFailed: {len(_SKIPPED)} test(s) skipped although every capability is "
        "present — a test must not be disabled by skipping it:"
    )
    for entry in _SKIPPED:
        print(f"  {entry}")
    session.exitstatus = 1
