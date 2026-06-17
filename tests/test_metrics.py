import math

from candor.metrics import (
    auroc,
    brier_score,
    expected_calibration_error,
    log_loss,
    overconfidence,
)


def test_brier_perfect():
    assert brier_score([0.0, 1.0, 1.0], [0, 1, 1]) == 0.0


def test_brier_uniform():
    # Predicting 0.5 for everything: each squared error is 0.25.
    assert abs(brier_score([0.5, 0.5, 0.5, 0.5], [0, 0, 1, 1]) - 0.25) < 1e-9


def test_brier_worst():
    assert brier_score([1.0, 0.0], [0, 1]) == 1.0


def test_ece_perfect():
    assert expected_calibration_error([0.0, 1.0, 0.0, 1.0], [0, 1, 0, 1]) == 0.0


def test_ece_uniform_overconfidence():
    # Always-confident-wrong predictions => ECE = |1 - 0| = 1.
    assert abs(expected_calibration_error([0.99, 0.99], [0, 0]) - 0.99) < 1e-9


def test_overconfidence_sign():
    # Says 0.9 but actual is 0.5 => overconfident, mean diff +0.4
    assert abs(overconfidence([0.9, 0.9], [1, 0]) - 0.4) < 1e-9


def test_auroc_perfect():
    assert auroc([0.9, 0.8, 0.2, 0.1], [1, 1, 0, 0]) == 1.0


def test_auroc_worst():
    assert auroc([0.1, 0.2, 0.8, 0.9], [1, 1, 0, 0]) == 0.0


def test_auroc_random_ish():
    # Tied scores => 0.5
    assert auroc([0.5, 0.5, 0.5, 0.5], [1, 0, 1, 0]) == 0.5


def test_auroc_degenerate():
    # All-positive or all-negative -> 0.5 sentinel
    assert auroc([0.9, 0.8], [1, 1]) == 0.5
    assert auroc([0.1, 0.2], [0, 0]) == 0.5


def test_log_loss_finite_at_extremes():
    # Should clip; not raise
    val = log_loss([0.0, 1.0], [1, 0])
    assert math.isfinite(val)
    assert val > 10  # heavily penalized


def test_log_loss_perfect_is_small():
    val = log_loss([0.99, 0.01], [1, 0])
    assert val < 0.05
