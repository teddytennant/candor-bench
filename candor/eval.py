"""Base class and registry for evals."""

from __future__ import annotations

import json
import statistics
from abc import ABC, abstractmethod
from importlib import resources
from pathlib import Path
from typing import ClassVar

from .model import Model
from .types import EvalReport, GradedResult, Sample


class Eval(ABC):
    name: ClassVar[str]
    version: ClassVar[str] = "0.1"
    dataset: ClassVar[str]  # relative path under candor/data, e.g. "sycophancy/seed.jsonl"
    KINDS: ClassVar[set[str]] = set()  # valid kind values for this eval

    @abstractmethod
    async def run_sample(self, sample: Sample, model: Model) -> GradedResult: ...

    def aggregate(self, results: list[GradedResult]) -> EvalReport:
        """Default aggregation: mean score plus per-kind means."""
        n = len(results)
        mean = sum(r.score for r in results) / n if n else 0.0
        breakdown: dict[str, dict[str, float]] = {}
        kinds = sorted({r.kind for r in results})
        for k in kinds:
            sub = [r for r in results if r.kind == k]
            breakdown[k] = {
                "n": float(len(sub)),
                "mean_score": sum(r.score for r in sub) / len(sub) if sub else 0.0,
            }

        # Latency aggregation across all ModelCalls
        all_latencies: list[float] = []
        for r in results:
            for c in r.calls:
                all_latencies.append(c.latency_ms)

        metrics: dict[str, float] = {"mean_score": mean}
        if all_latencies:
            metrics["mean_latency_ms"] = statistics.mean(all_latencies)
            if len(all_latencies) >= 20:
                metrics["p95_latency_ms"] = statistics.quantiles(all_latencies, n=20)[18]
            else:
                metrics["p95_latency_ms"] = max(all_latencies)
        else:
            metrics["mean_latency_ms"] = 0.0
            metrics["p95_latency_ms"] = 0.0

        return EvalReport(
            eval=self.name,
            model=results[0].model if results else "?",
            n=n,
            metrics=metrics,
            breakdown=breakdown,
            results=results,
        )

    def load_samples(self) -> list[Sample]:
        samples = load_jsonl_samples(self.dataset)
        if self.KINDS:
            for s in samples:
                if s.kind not in self.KINDS:
                    raise ValueError(
                        f"{self.name}: invalid kind {s.kind!r} for sample {s.id}; "
                        f"valid: {sorted(self.KINDS)}"
                    )
        return samples


_REGISTRY: dict[str, type[Eval]] = {}


def register(cls: type[Eval]) -> type[Eval]:
    if not getattr(cls, "name", None):
        raise ValueError(f"Eval {cls.__name__} missing class-level `name`")
    if cls.name in _REGISTRY:
        raise ValueError(f"Duplicate eval name: {cls.name}")
    _REGISTRY[cls.name] = cls
    return cls


def get_eval(name: str) -> Eval:
    if name not in _REGISTRY:
        raise KeyError(f"Unknown eval: {name}. Registered: {sorted(_REGISTRY)}")
    return _REGISTRY[name]()


def list_evals() -> list[str]:
    return sorted(_REGISTRY)


def load_jsonl_samples(rel_path: str) -> list[Sample]:
    """Load a JSONL dataset bundled inside the candor package.

    Falls back to an on-disk lookup so that running from the repo root works
    even before the package is installed.
    """
    text: str | None = None
    try:
        text = resources.files("candor").joinpath("data", rel_path).read_text(encoding="utf-8")
    except (FileNotFoundError, ModuleNotFoundError):
        on_disk = Path(__file__).resolve().parent / "data" / rel_path
        if on_disk.exists():
            text = on_disk.read_text(encoding="utf-8")
    if text is None:
        raise FileNotFoundError(f"Dataset not found: {rel_path}")
    out: list[Sample] = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        out.append(Sample.model_validate(json.loads(line)))
    return out
