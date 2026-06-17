"""Tests of the runner / aggregation / report shape."""

from __future__ import annotations

import pytest

from candor.eval import get_eval
from candor.models.stub import StubModel
from candor.runner import run_eval


@pytest.mark.asyncio
async def test_runner_returns_report_with_results():
    eval_obj = get_eval("calibration")
    samples = eval_obj.load_samples()[:3]
    report = await run_eval(eval_obj, StubModel("honest-confident"), samples, concurrency=2)
    assert report.n == 3
    assert len(report.results) == 3
    assert "mean_score" in report.metrics


@pytest.mark.asyncio
async def test_runner_concurrency_does_not_change_score():
    """Determinism check: changing concurrency should not change a stub's score."""
    eval_obj = get_eval("calibration")
    samples = eval_obj.load_samples()[:5]
    a = await run_eval(eval_obj, StubModel("honest-confident"), samples, concurrency=1)
    b = await run_eval(eval_obj, StubModel("honest-confident"), samples, concurrency=4)
    assert abs(a.metrics["mean_score"] - b.metrics["mean_score"]) < 1e-9


@pytest.mark.asyncio
async def test_breakdown_keys_match_kinds():
    eval_obj = get_eval("sycophancy")
    report = await run_eval(eval_obj, StubModel("sycophant"), concurrency=2)
    sample_kinds = {s.kind for s in eval_obj.load_samples()}
    assert set(report.breakdown.keys()) == sample_kinds
