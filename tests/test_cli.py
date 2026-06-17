"""Tests for the `candor` CLI entrypoint.

All runs use the in-process `stub:` provider, so the CLI is exercised
end-to-end (arg parsing -> runner -> report -> JSON) without the network.
"""

from __future__ import annotations

import json

import pytest

from candor.cli import main
from candor.types import EvalReport


def test_list_command(capsys):
    rc = main(["list"])
    assert rc == 0
    out = capsys.readouterr().out
    assert "Registered evals" in out
    for name in ("sycophancy", "pressure", "calibration", "factuality"):
        assert name in out


def test_run_command_writes_report_json(tmp_path, capsys):
    rc = main(
        [
            "run",
            "--model",
            "stub:honest-confident",
            "--eval",
            "calibration",
            "--output",
            str(tmp_path),
            "--limit",
            "3",
        ]
    )
    assert rc == 0
    written = list(tmp_path.glob("*.json"))
    assert len(written) == 1
    report = EvalReport.model_validate(json.loads(written[0].read_text()))
    assert report.eval == "calibration"
    assert report.model == "stub:honest-confident"
    assert report.n == 3


def test_run_limit_caps_sample_count(tmp_path):
    main(
        [
            "run",
            "--model",
            "stub:refuser",
            "--eval",
            "factuality",
            "--output",
            str(tmp_path),
            "--limit",
            "2",
        ]
    )
    report = EvalReport.model_validate(
        json.loads(next(tmp_path.glob("*.json")).read_text())
    )
    assert report.n == 2


def test_run_multiple_evals(tmp_path):
    rc = main(
        [
            "run",
            "--model",
            "stub:honest-confident",
            "--eval",
            "calibration",
            "--eval",
            "factuality",
            "--output",
            str(tmp_path),
            "--limit",
            "2",
        ]
    )
    assert rc == 0
    assert len(list(tmp_path.glob("*.json"))) == 2


def test_score_command_rerenders_existing_report(tmp_path, capsys):
    # First produce a report, then re-render it via `score`.
    main(
        [
            "run",
            "--model",
            "stub:honest-confident",
            "--eval",
            "calibration",
            "--output",
            str(tmp_path),
            "--limit",
            "2",
        ]
    )
    capsys.readouterr()  # drop run output
    report_path = next(tmp_path.glob("*.json"))
    rc = main(["score", str(report_path)])
    assert rc == 0
    out = capsys.readouterr().out
    assert "calibration" in out
    assert "mean_score" in out


def test_run_requires_model_arg():
    with pytest.raises(SystemExit):
        main(["run", "--eval", "calibration"])


def test_no_subcommand_exits():
    with pytest.raises(SystemExit):
        main([])
