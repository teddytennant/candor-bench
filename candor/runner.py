"""Parallel sample runner."""

from __future__ import annotations

import asyncio
import logging
import traceback
from collections.abc import Iterable

from .eval import Eval
from .model import Model
from .types import EvalReport, GradedResult, Sample

logger = logging.getLogger(__name__)


async def run_eval(
    eval_obj: Eval,
    model: Model,
    samples: Iterable[Sample] | None = None,
    *,
    concurrency: int = 4,
    progress: bool = False,
) -> EvalReport:
    if concurrency < 1:
        raise ValueError(f"concurrency must be >= 1, got {concurrency}")
    sample_list = list(samples) if samples is not None else eval_obj.load_samples()
    sem = asyncio.Semaphore(concurrency)
    total = len(sample_list)
    done = 0
    lock = asyncio.Lock()

    async def go(s: Sample) -> GradedResult:
        nonlocal done
        async with sem:
            try:
                res = await eval_obj.run_sample(s, model)
            except Exception as exc:  # noqa: BLE001
                # Distinguish model errors from eval/grading errors
                is_model_err = isinstance(exc, (RuntimeError, TimeoutError, ConnectionError))
                err_type = "model_error" if is_model_err else "eval_error"
                logger.error(
                    f"{err_type} on {eval_obj.name}/{s.id}: {exc}\n{traceback.format_exc()}"
                )
                res = GradedResult(
                    sample_id=s.id,
                    eval=eval_obj.name,
                    kind=s.kind,
                    model=model.name,
                    score=0.5,
                    label="error",
                    notes=f"{type(exc).__name__}: {str(exc)[:200]}",
                    calls=[],
                )
        if progress:
            async with lock:
                done += 1
                print(f"  [{done}/{total}] {eval_obj.name} {s.id} -> {res.label} ({res.score:.2f})")
        return res

    results = await asyncio.gather(*(go(s) for s in sample_list))
    return eval_obj.aggregate(list(results))
