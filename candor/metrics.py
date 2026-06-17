"""Calibration metrics. No numpy dependency."""

from __future__ import annotations

import math
from collections.abc import Sequence


def brier_score(probs: Sequence[float], outcomes: Sequence[int]) -> float:
    """Mean squared error between predicted probability and outcome (0/1)."""
    if len(probs) != len(outcomes):
        raise ValueError("probs and outcomes must have the same length")
    if not probs:
        return 0.0
    return sum((p - y) ** 2 for p, y in zip(probs, outcomes, strict=True)) / len(probs)


def expected_calibration_error(
    probs: Sequence[float],
    outcomes: Sequence[int],
    n_bins: int = 10,
) -> float:
    """ECE with equal-width bins on [0, 1].

    Each bin contributes |bin_size / N| * |bin_mean_prob - bin_mean_outcome|.
    """
    if len(probs) != len(outcomes):
        raise ValueError("probs and outcomes must have the same length")
    n = len(probs)
    if n == 0:
        return 0.0
    bins: list[tuple[list[float], list[int]]] = [([], []) for _ in range(n_bins)]
    for p, y in zip(probs, outcomes, strict=True):
        idx = min(n_bins - 1, max(0, int(p * n_bins)))
        bins[idx][0].append(p)
        bins[idx][1].append(y)
    ece = 0.0
    for ps, ys in bins:
        if not ps:
            continue
        avg_p = sum(ps) / len(ps)
        avg_y = sum(ys) / len(ys)
        ece += (len(ps) / n) * abs(avg_p - avg_y)
    return ece


def overconfidence(probs: Sequence[float], outcomes: Sequence[int]) -> float:
    """Mean of (prob - outcome). Positive => systematic overconfidence."""
    if len(probs) != len(outcomes):
        raise ValueError("probs and outcomes must have the same length")
    if not probs:
        return 0.0
    return sum(p - y for p, y in zip(probs, outcomes, strict=True)) / len(probs)


def auroc(scores: Sequence[float], outcomes: Sequence[int]) -> float:
    """Area under ROC curve via Mann-Whitney U.

    Useful for ranking: do higher self-confidences correspond to correct answers?
    Returns 0.5 if degenerate.
    """
    if len(scores) != len(outcomes):
        raise ValueError("scores and outcomes must have the same length")
    pos = [s for s, y in zip(scores, outcomes, strict=True) if y == 1]
    neg = [s for s, y in zip(scores, outcomes, strict=True) if y == 0]
    if not pos or not neg:
        return 0.5
    wins = 0.0
    for a in pos:
        for b in neg:
            if a > b:
                wins += 1
            elif a == b:
                wins += 0.5
    return wins / (len(pos) * len(neg))


def log_loss(probs: Sequence[float], outcomes: Sequence[int], eps: float = 1e-9) -> float:
    """Mean negative log-likelihood. Lower is better."""
    if len(probs) != len(outcomes):
        raise ValueError("probs and outcomes must have the same length")
    if not probs:
        return 0.0
    total = 0.0
    for p, y in zip(probs, outcomes, strict=True):
        p = min(1 - eps, max(eps, p))
        total += -(y * math.log(p) + (1 - y) * math.log(1 - p))
    return total / len(probs)
