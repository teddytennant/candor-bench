#!/usr/bin/env python3
"""Re-grade saved EvalReport JSONs against the current grading code.

Usage:
    python scripts/regrade.py results/*.json --output results-regraded/

This rebuilds an EvalReport by replaying each sample's recorded model
responses through the eval's `run_sample` method (via a playback model that
returns the canned response for each request). No network calls happen.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path

# Allow running from repo root without install.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from candor import evals  # noqa: F401  importing registers all evals
from candor.eval import get_eval
from candor.runner import run_eval
from candor.types import EvalReport, Message


class PlaybackModel:
    """Returns canned responses keyed by the exact (role, content) message
    tuple. Raises if asked something it doesn't know."""

    def __init__(self, name: str):
        self.name = name
        self._table: dict[tuple[tuple[str, str], ...], str] = {}

    def add(self, messages: list[dict], response: str) -> None:
        key = tuple((m["role"], m["content"]) for m in messages)
        self._table[key] = response

    async def chat(
        self,
        messages: list[Message],
        *,
        temperature: float = 0.0,
        max_tokens: int = 1024,
    ) -> str:
        key = tuple((m.role, m.content) for m in messages)
        if key not in self._table:
            head = key[-1][1][:80] if key else "()"
            raise KeyError(f"PlaybackModel: no canned response for {head!r}")
        return self._table[key]


def build_playback(report_data: dict) -> PlaybackModel:
    pb = PlaybackModel(report_data["model"])
    for r in report_data["results"]:
        for call in r.get("calls", []):
            pb.add(call["messages"], call["response"])
    return pb


async def regrade_file(path: Path, out_dir: Path) -> tuple[Path, EvalReport, EvalReport]:
    data = json.loads(path.read_text())
    eval_name = data["eval"]
    eval_obj = get_eval(eval_name)
    pb = build_playback(data)

    # Reconstruct samples from the dataset rather than the JSON, so any data
    # fixes since the original run get applied. We filter to the IDs that
    # were actually present in the original run.
    all_samples = eval_obj.load_samples()
    seen_ids = {r["sample_id"] for r in data["results"]}
    samples = [s for s in all_samples if s.id in seen_ids]
    missing = seen_ids - {s.id for s in samples}
    for mid in missing:
        print(f"  warn: sample {mid} no longer in dataset, skipping", file=sys.stderr)

    new_report = await run_eval(eval_obj, pb, samples=samples, concurrency=8)

    old_report = EvalReport.model_validate(data)
    out_path = out_dir / path.name
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(new_report.model_dump(), indent=2))
    return out_path, old_report, new_report


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("paths", nargs="+", help="EvalReport JSONs to regrade")
    p.add_argument("--output", default="results-regraded", help="output directory")
    args = p.parse_args()

    async def go():
        out_dir = Path(args.output)
        for path_str in args.paths:
            path = Path(path_str)
            if not path.exists():
                print(f"skip: {path} does not exist")
                continue
            try:
                _, old, new = await regrade_file(path, out_dir)
            except Exception as exc:
                print(f"error regrading {path}: {exc}")
                continue
            old_score = old.metrics.get("mean_score", 0.0)
            new_score = new.metrics.get("mean_score", 0.0)
            delta = new_score - old_score
            arrow = "↑" if delta > 0.001 else "↓" if delta < -0.001 else "·"
            print(
                f"{old.eval:12s} {old.model:42s}  {old_score:.4f} -> {new_score:.4f}  {arrow}{abs(delta):.4f}"
            )
            if delta != 0:
                # Show the samples whose label changed
                old_by_id = {r.sample_id: r for r in old.results}
                for new_r in new.results:
                    old_r = old_by_id.get(new_r.sample_id)
                    if old_r and (old_r.label != new_r.label or abs(old_r.score - new_r.score) > 0.001):
                        print(f"    {new_r.sample_id:10s} {old_r.label} ({old_r.score:.2f}) -> {new_r.label} ({new_r.score:.2f})")
        return 0

    return asyncio.run(go())


if __name__ == "__main__":
    sys.exit(main())
