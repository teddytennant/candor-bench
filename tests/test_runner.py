"""Tests of the runner / aggregation / report shape."""

from __future__ import annotations

import pytest

from candor.eval import Eval, get_eval
from candor.models.stub import StubModel
from candor.runner import run_eval
from candor.types import GradedResult, Message, Sample


class _DummyEval(Eval):
    """A minimal eval with the *default* aggregate, used to test runner
    behaviour in isolation from any eval-specific aggregation quirks."""

    name = "dummy-runner-test"
    dataset = ""  # unused: tests always pass explicit samples

    async def run_sample(self, sample: Sample, model) -> GradedResult:
        text = await model.chat([Message(role="user", content="hi")])
        return GradedResult(
            sample_id=sample.id,
            eval=self.name,
            kind=sample.kind,
            model=model.name,
            score=1.0 if text else 0.0,
            label="ok",
        )


def _dummy_samples(n: int) -> list[Sample]:
    return [Sample(id=f"d{i}", eval="dummy-runner-test", kind="k") for i in range(n)]


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


class _BoomModel:
    """A model that always raises, to exercise the runner's error path."""

    def __init__(self, exc: Exception):
        self.name = "stub:boom"
        self._exc = exc

    async def chat(self, messages, *, temperature: float = 0.0, max_tokens: int = 1024) -> str:
        raise self._exc


@pytest.mark.asyncio
async def test_runner_isolates_model_errors_into_results():
    """A model exception must not abort the whole run: every sample still gets a
    GradedResult, flagged with the 'error' label and a neutral 0.5 score, and the
    transcript is empty (calls=[]) because the call never completed."""
    report = await run_eval(
        _DummyEval(), _BoomModel(RuntimeError("api down")), _dummy_samples(3), concurrency=2
    )
    assert report.n == 3
    assert len(report.results) == 3
    for r in report.results:
        assert r.label == "error"
        assert r.score == 0.5
        assert r.calls == []
        # RuntimeError is classified as a model error and the message is captured
        assert "RuntimeError" in r.notes


@pytest.mark.asyncio
async def test_runner_classifies_non_model_errors_as_eval_error():
    """A non-network exception (e.g. a bug in run_sample) is still isolated, but
    its message — not its classification — is what surfaces in the notes."""

    class _BadEval(_DummyEval):
        async def run_sample(self, sample, model):
            raise ValueError("grading bug")

    report = await run_eval(_BadEval(), StubModel("honest-confident"), _dummy_samples(2))
    assert {r.label for r in report.results} == {"error"}
    assert all("ValueError" in r.notes for r in report.results)


@pytest.mark.asyncio
async def test_runner_rejects_bad_concurrency():
    with pytest.raises(ValueError, match="concurrency must be >= 1"):
        await run_eval(_DummyEval(), StubModel("honest-confident"), _dummy_samples(1), concurrency=0)


@pytest.mark.asyncio
async def test_runner_progress_does_not_change_results(capsys):
    """progress=True only adds stdout; the aggregated report is unchanged."""
    report = await run_eval(
        _DummyEval(), StubModel("honest-confident"), _dummy_samples(3), concurrency=2, progress=True
    )
    out = capsys.readouterr().out
    assert "[3/3]" in out  # final progress line printed
    assert report.n == 3


@pytest.mark.asyncio
async def test_runner_handles_empty_sample_list():
    report = await run_eval(_DummyEval(), StubModel("honest-confident"), [], concurrency=2)
    assert report.n == 0
    assert report.results == []
    assert report.metrics["mean_score"] == 0.0
