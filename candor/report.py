"""Report rendering: human-readable text + machine-readable JSON."""

from __future__ import annotations

import json
from pathlib import Path

from .types import EvalReport


def render_text(report: EvalReport) -> str:
    lines: list[str] = []
    lines.append(f"== {report.eval}  model={report.model}  n={report.n} ==")
    for k, v in sorted(report.metrics.items()):
        lines.append(f"  {k:>18s}: {v:.4f}")
    if report.breakdown:
        lines.append("  breakdown by kind:")
        # column-aligned per-kind breakdown
        keys = sorted({k for d in report.breakdown.values() for k in d})
        col = max(len(k) for k in keys) if keys else 4
        for kind in sorted(report.breakdown):
            lines.append(f"    [{kind}]")
            for k in keys:
                v = report.breakdown[kind].get(k)
                if v is None:
                    continue
                lines.append(f"      {k:>{col}s}: {v:.4f}")
    # Top-3 worst-scoring samples — useful for inspection.
    if report.results:
        worst = sorted(report.results, key=lambda r: r.score)[:3]
        lines.append("  worst samples:")
        for r in worst:
            lines.append(f"    - {r.sample_id} [{r.kind}] -> {r.label} ({r.score:.2f})")
    return "\n".join(lines)


def write_json(report: EvalReport, out_dir: Path) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    safe_model = report.model.replace("/", "_").replace(":", "_")
    path = out_dir / f"{report.eval}__{safe_model}.json"
    path.write_text(json.dumps(report.model_dump(), indent=2), encoding="utf-8")
    return path


def render_summary(reports: list[EvalReport]) -> str:
    if not reports:
        return "(no reports)"
    rows = []
    for r in reports:
        rows.append(
            {
                "eval": r.eval,
                "model": r.model,
                "n": r.n,
                "mean_score": r.metrics.get("mean_score", float("nan")),
            }
        )
    width_eval = max(len(row["eval"]) for row in rows)
    width_model = max(len(row["model"]) for row in rows)
    out = ["", "== summary =="]
    out.append(f"{'eval'.ljust(width_eval)}  {'model'.ljust(width_model)}    n  mean_score")
    for row in rows:
        out.append(
            f"{row['eval'].ljust(width_eval)}  {row['model'].ljust(width_model)}  {row['n']:>3d}  {row['mean_score']:.4f}"
        )
    return "\n".join(out)
