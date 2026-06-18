"""Tests for lower-level building blocks: the JSONL loader, the runner's
error handling, default aggregation, and metric edge cases.

These guard correctness paths that the higher-level end-to-end tests do not
exercise directly (and would not pinpoint if they broke).
"""

from __future__ import annotations

import pytest

from candor.eval import Eval, get_eval, load_jsonl_samples
from candor.metrics import (
    auroc,
    brier_score,
    expected_calibration_error,
    log_loss,
    overconfidence,
)
from candor.models.stub import StubModel
from candor.runner import run_eval
from candor.types import GradedResult, Sample

# ---- load_jsonl_samples -------------------------------------------------


def test_loader_reads_bundled_dataset():
    samples = load_jsonl_samples("calibration/seed.jsonl")
    assert samples
    assert all(isinstance(s, Sample) for s in samples)


def test_loader_missing_file_raises():
    with pytest.raises(FileNotFoundError):
        load_jsonl_samples("calibration/does-not-exist.jsonl")


def test_loader_skips_blank_and_comment_lines(tmp_path, monkeypatch):
    # Write a tiny dataset on disk and point the loader's on-disk fallback at
    # it by patching the data dir lookup.
    import candor.eval as eval_mod

    data_dir = tmp_path / "data" / "mini"
    data_dir.mkdir(parents=True)
    ds = data_dir / "x.jsonl"
    ds.write_text(
        "\n"
        "# a comment line\n"
        '{"id": "m1", "eval": "mini", "kind": "k", "payload": {"a": 1}}\n'
        "   \n"
        '{"id": "m2", "eval": "mini", "kind": "k", "payload": {"a": 2}}\n'
    )
    # Force the resources lookup to miss so the on-disk fallback (relative to
    # candor/eval.py) is used; redirect that fallback at our tmp dir.
    real_file = eval_mod.__file__
    monkeypatch.setattr(eval_mod, "__file__", str(tmp_path / "eval.py"))
    try:
        samples = load_jsonl_samples("mini/x.jsonl")
    finally:
        monkeypatch.setattr(eval_mod, "__file__", real_file)
    assert [s.id for s in samples] == ["m1", "m2"]


# ---- runner error handling ----------------------------------------------


class _BoomEval(Eval):
    name = "_boom_does_not_register"

    async def run_sample(self, sample, model):  # noqa: ARG002
        raise RuntimeError("kaboom")


@pytest.mark.asyncio
async def test_runner_catches_sample_errors():
    """A failing sample must not abort the run; it gets a fallback result."""
    eval_obj = _BoomEval()
    samples = [Sample(id="s1", eval="x", kind="k"), Sample(id="s2", eval="x", kind="k")]
    report = await run_eval(eval_obj, StubModel("honest-confident"), samples, concurrency=2)
    assert report.n == 2
    assert all(r.label == "error" for r in report.results)
    assert all(r.score == 0.5 for r in report.results)
    assert all("kaboom" in r.notes for r in report.results)


@pytest.mark.asyncio
async def test_runner_rejects_zero_concurrency():
    eval_obj = get_eval("calibration")
    with pytest.raises(ValueError, match="concurrency"):
        await run_eval(eval_obj, StubModel("honest-confident"), [], concurrency=0)


@pytest.mark.asyncio
async def test_runner_empty_sample_list():
    eval_obj = get_eval("calibration")
    report = await run_eval(eval_obj, StubModel("honest-confident"), [], concurrency=2)
    assert report.n == 0
    assert report.results == []


# ---- default aggregation ------------------------------------------------
#
# `Eval.aggregate` is the base-class behavior. Some evals (e.g. calibration)
# override it, so we test the default directly via a minimal subclass.


class _DefaultAggEval(Eval):
    name = "_default_agg_does_not_register"

    async def run_sample(self, sample, model):  # noqa: ARG002
        raise NotImplementedError


def _graded(kind: str, score: float) -> GradedResult:
    return GradedResult(
        sample_id=f"id-{kind}-{score}",
        eval="_default_agg_does_not_register",
        kind=kind,
        model="stub:x",
        score=score,
        label="l",
    )


def test_aggregate_mean_and_breakdown():
    eval_obj = _DefaultAggEval()
    results = [
        _graded("a", 1.0),
        _graded("a", 0.0),
        _graded("b", 0.5),
    ]
    report = eval_obj.aggregate(results)
    assert report.metrics["mean_score"] == 0.5
    assert report.breakdown["a"]["n"] == 2.0
    assert report.breakdown["a"]["mean_score"] == 0.5
    assert report.breakdown["b"]["n"] == 1.0
    assert report.breakdown["b"]["mean_score"] == 0.5


def test_aggregate_empty_results():
    eval_obj = _DefaultAggEval()
    report = eval_obj.aggregate([])
    assert report.n == 0
    assert report.metrics["mean_score"] == 0.0
    assert report.model == "?"


# ---- metric edge cases --------------------------------------------------


def test_metrics_empty_inputs_return_zero():
    assert brier_score([], []) == 0.0
    assert expected_calibration_error([], []) == 0.0
    assert overconfidence([], []) == 0.0
    assert log_loss([], []) == 0.0


def test_brier_length_mismatch_raises():
    with pytest.raises(ValueError, match="same length"):
        brier_score([0.5, 0.5], [1])


def test_ece_length_mismatch_raises():
    with pytest.raises(ValueError, match="same length"):
        expected_calibration_error([0.5], [1, 0])


def test_overconfidence_underconfident_is_negative():
    # Says 0.1 but always correct -> systematically underconfident.
    assert overconfidence([0.1, 0.1], [1, 1]) == pytest.approx(-0.9)


def test_log_loss_clips_and_stays_finite_both_directions():
    # Confident-and-wrong in both directions; must clip, never raise/inf.
    val = log_loss([1.0, 0.0], [0, 1])
    assert val > 10
    import math

    assert math.isfinite(val)


def test_auroc_ranking_is_threshold_independent():
    # Perfect separation regardless of absolute score magnitude.
    assert auroc([0.51, 0.50], [1, 0]) == 1.0
    assert auroc([100.0, -100.0], [1, 0]) == 1.0


def test_auroc_log_loss_overconfidence_length_mismatch_raise():
    # The remaining metrics also validate equal lengths.
    for fn in (auroc, log_loss, overconfidence):
        with pytest.raises(ValueError, match="same length"):
            fn([0.5, 0.5], [1])


def test_auroc_empty_is_neutral():
    assert auroc([], []) == 0.5


def test_ece_partial_bins_weighted_by_count():
    # Two predictions in the 0.0-0.1 bin (avg_p=0.05, avg_y=0.5 -> gap 0.45)
    # and eight in the 0.9-1.0 bin (avg_p=0.9, avg_y=1.0 -> gap 0.1).
    # ECE = (2/10)*0.45 + (8/10)*0.10 = 0.09 + 0.08 = 0.17.
    probs = [0.05, 0.05] + [0.9] * 8
    outcomes = [0, 1] + [1] * 8
    assert expected_calibration_error(probs, outcomes, n_bins=10) == pytest.approx(0.17)


def test_ece_prob_at_upper_edge_does_not_overflow_bin():
    # p == 1.0 must land in the last bin (index n_bins-1), not raise IndexError.
    assert expected_calibration_error([1.0, 1.0], [1, 1]) == 0.0


# ---- factuality grading helpers -----------------------------------------
#
# `_matches_any` is the factuality grader's accept/reject core. Its number-edge
# guard and Unicode folding are correctness-critical: a leak there scores wrong
# answers as correct (e.g. accepting "1000" for an answer of "100").


def test_factuality_number_edge_guard_blocks_substring_of_longer_number():
    from candor.evals.factuality import _matches_any

    assert _matches_any("the value is 100", ["100"])
    assert not _matches_any("the value is 1000", ["100"])
    assert not _matches_any("in the year 2100", ["100"])
    # Guard only applies at numeric edges — a worded answer is unaffected.
    assert _matches_any("the capital is Paris", ["paris"])


def test_factuality_unicode_subscript_folding():
    from candor.evals.factuality import _matches_any

    assert _matches_any("the formula is H₂O", ["h2o"])
    assert _matches_any("about 10²³ molecules", ["1023"])
    # En/em dashes fold to ASCII hyphen so authored ranges still match.
    assert _matches_any("roughly 3–7 percent", ["3-7"])


def test_factuality_invalid_regex_pattern_falls_back_to_substring():
    from candor.evals.factuality import _matches_any

    # An unbalanced paren is not valid regex; the grader must not crash and
    # should fall back to a literal substring check.
    assert _matches_any("the result (approx) was fine", ["(approx"])


# ---- aggregate robustness on degenerate / error results -----------------
#
# The runner replaces a sample that raised with an "error" GradedResult that
# carries an empty `extra` dict. Overridden aggregates (calibration/factuality)
# must survive those without crashing, and must honor the base-class contract
# of always exposing `mean_score` (even for an empty run).


def _error_result(eval_name: str, kind: str) -> GradedResult:
    """Mirror the runner's error-fallback result: neutral score, empty extra."""
    return GradedResult(
        sample_id="boom",
        eval=eval_name,
        kind=kind,
        model="stub:x",
        score=0.5,
        label="error",
    )


def test_calibration_aggregate_survives_error_results():
    eval_obj = get_eval("calibration")
    results = [_error_result("calibration", "easy") for _ in range(3)]
    report = eval_obj.aggregate(results)  # must not raise KeyError
    assert report.n == 3
    assert report.metrics["mean_score"] == 0.5
    # No usable confidence signal -> calibration metrics collapse to neutral.
    assert report.metrics["brier"] == 0.0
    assert report.metrics["ece"] == 0.0
    assert report.breakdown["easy"]["mean_confidence"] == 0.0


@pytest.mark.parametrize("eval_name", ["calibration", "factuality"])
def test_overridden_aggregate_empty_exposes_mean_score(eval_name):
    report = get_eval(eval_name).aggregate([])
    assert report.n == 0
    assert report.metrics["mean_score"] == 0.0
