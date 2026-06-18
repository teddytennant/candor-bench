"""Tests for report rendering and JSON (de)serialization.

Reports are the public artifact of a run — the `results/*.json` files are
checked into the repo and re-graded by tooling. A regression in serialization
or in the filename-sanitization logic would corrupt those artifacts.
"""

from __future__ import annotations

from candor.report import render_summary, render_text, write_json
from candor.types import EvalReport, GradedResult


def _result(sample_id: str, kind: str, score: float, label: str) -> GradedResult:
    return GradedResult(
        sample_id=sample_id,
        eval="calibration",
        kind=kind,
        model="stub:honest-confident",
        score=score,
        label=label,
    )


def _report() -> EvalReport:
    results = [
        _result("c1", "trivia", 1.0, "correct"),
        _result("c2", "trivia", 0.0, "wrong"),
        _result("c3", "unanswerable", 0.5, "abstained"),
    ]
    return EvalReport(
        eval="calibration",
        model="xai:grok-4.3",
        n=3,
        metrics={"mean_score": 0.5, "ece": 0.25},
        breakdown={
            "trivia": {"n": 2.0, "mean_score": 0.5},
            "unanswerable": {"n": 1.0, "mean_score": 0.5},
        },
        results=results,
    )


def test_json_roundtrip_is_lossless():
    report = _report()
    dumped = report.model_dump()
    restored = EvalReport.model_validate(dumped)
    assert restored == report
    # results survive the roundtrip with full fidelity
    assert restored.results[0].sample_id == "c1"
    assert restored.results[0].score == 1.0


def test_write_json_sanitizes_model_name_in_filename(tmp_path):
    report = _report()  # model contains a ':'
    path = write_json(report, tmp_path)
    assert path.exists()
    # ':' and '/' are illegal/awkward in filenames -> replaced with '_'
    assert ":" not in path.name
    assert path.name == "calibration__xai_grok-4.3.json"


def test_write_json_sanitizes_slash_in_model_name(tmp_path):
    report = _report()
    report.model = "openai/gpt-4o-mini"
    path = write_json(report, tmp_path)
    assert path.parent == tmp_path  # the '/' must not create a subdirectory
    assert path.name == "calibration__openai_gpt-4o-mini.json"


def test_write_json_content_is_valid_and_reloadable(tmp_path):
    import json

    report = _report()
    path = write_json(report, tmp_path)
    reloaded = EvalReport.model_validate(json.loads(path.read_text()))
    assert reloaded == report


def test_write_json_creates_missing_output_dir(tmp_path):
    nested = tmp_path / "a" / "b" / "c"
    path = write_json(_report(), nested)
    assert path.exists()


def test_render_text_includes_metrics_and_worst_samples():
    text = render_text(_report())
    assert "calibration" in text
    assert "mean_score" in text
    assert "ece" in text
    # the worst (lowest-scoring) sample should be surfaced for inspection
    assert "c2" in text
    assert "worst samples" in text


def test_render_summary_one_row_per_report():
    reports = [_report(), _report()]
    summary = render_summary(reports)
    assert summary.count("xai:grok-4.3") == 2
    assert "summary" in summary


def test_render_summary_empty():
    assert render_summary([]) == "(no reports)"


def test_render_text_includes_per_kind_breakdown():
    text = render_text(_report())
    assert "breakdown by kind" in text
    # both kinds and their per-kind metric keys are rendered
    assert "[trivia]" in text
    assert "[unanswerable]" in text
    assert "mean_score" in text


def test_render_text_handles_report_with_no_results_or_breakdown():
    # An empty run (n=0) must still render without raising.
    empty = EvalReport(
        eval="calibration",
        model="stub:x",
        n=0,
        metrics={"mean_score": 0.0},
    )
    text = render_text(empty)
    assert "calibration" in text
    assert "worst samples" not in text  # nothing to surface
    assert "breakdown by kind" not in text


def test_write_json_filename_uses_eval_and_model(tmp_path):
    report = _report()
    report.model = "openrouter:meta-llama/llama-3.1-8b"
    path = write_json(report, tmp_path)
    # provider colon and the vendor/model slash are both sanitized to '_'
    assert path.name == "calibration__openrouter_meta-llama_llama-3.1-8b.json"
    assert path.parent == tmp_path
