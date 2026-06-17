#!/usr/bin/env python3
"""Render the candor-bench headline results as a clean, tweet-ready SVG.

Reads the 8 canonical result files -- 4 evals x 2 xAI models -- from
``results/`` and draws two panels:

  * a grouped bar chart of the headline mean honesty score per (eval, model),
    drawn on a zoomed 0.5-1.0 y-axis so the gaps between the scores are
    actually distinguishable; and
  * a "where the models diverge" panel showing the two per-kind slices that
    separate the models the most (pressure/deception_request and
    sycophancy/escalating_pressure).

The renderer is dependency-free: it emits SVG text directly, so it needs
nothing beyond the standard library to run.

Usage:
    python scripts/plot_results.py
    python scripts/plot_results.py --results-dir results --out results/figure.svg
    python scripts/plot_results.py --help

Every canonical file must be present and expose ``metrics.mean_score``; a
missing or malformed file is a hard error rather than a silent gap, because the
figure ships as a launch artifact and a partial chart would be misleading.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from xml.sax.saxutils import escape

# --- canonical inputs -----------------------------------------------------
# Eval groups, in chart order.
EVALS = ["sycophancy", "pressure", "calibration", "factuality"]

# Models, in chart/legend order, with short display labels.
MODELS = [
    ("xai:grok-4.3", "grok-4.3"),
    ("xai-responses:grok-4.20-multi-agent-0309", "grok-4.20-multi-agent"),
]

# Result path for each (eval, model), relative to the results dir. Each model
# lives in its own subdirectory with a provider-tagged filename:
# results/<subdir>/<eval>__<tag>.json.
MODEL_SUBDIR = {
    "xai:grok-4.3": "grok-4.3",
    "xai-responses:grok-4.20-multi-agent-0309": "grok-4.20-multi-agent-0309",
}
MODEL_TAG = {
    "xai:grok-4.3": "xai_grok-4.3",
    "xai-responses:grok-4.20-multi-agent-0309": "xai-responses_grok-4.20-multi-agent-0309",
}

# Per-kind slices for the divergence panel: (eval, kind, human label).
DIVERGENCE_SLICES = [
    ("pressure", "deception_request", "pressure / deception_request"),
    ("sycophancy", "escalating_pressure", "sycophancy / escalating_pressure"),
]

# --- palette --------------------------------------------------------------
# One colour per model. Colour-blind-friendly (Okabe-Ito-ish blue/orange pair).
MODEL_COLORS = {
    "xai:grok-4.3": "#4C72B0",  # blue
    "xai-responses:grok-4.20-multi-agent-0309": "#DD8452",  # orange
}

BG = "#ffffff"
INK = "#1a1a1a"
SUBTLE = "#666666"
FAINT = "#999999"
GRID = "#e6e6e6"
AXIS = "#9a9a9a"
PANEL_RULE = "#d8d8d8"

FONT = "Helvetica,Arial,sans-serif"


def load_cell(results_dir: Path, eval_name: str, model: str) -> dict:
    """Load one canonical result file and pull out the bits the chart needs."""
    path = results_dir / MODEL_SUBDIR[model] / f"{eval_name}__{MODEL_TAG[model]}.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover - operational guard
        raise SystemExit(f"error: missing canonical result file: {path}") from exc
    except (OSError, json.JSONDecodeError) as exc:  # pragma: no cover
        raise SystemExit(f"error: cannot read {path}: {exc}") from exc

    if data.get("eval") != eval_name or data.get("model") != model:
        raise SystemExit(
            f"error: {path.name} has eval/model "
            f"{data.get('eval')!r}/{data.get('model')!r}, expected "
            f"{eval_name!r}/{model!r}"
        )

    metrics = data.get("metrics")
    if not isinstance(metrics, dict) or not isinstance(metrics.get("mean_score"), (int, float)):
        raise SystemExit(f"error: {path.name} has no numeric metrics.mean_score")

    breakdown = data.get("breakdown") or {}
    return {
        "eval": eval_name,
        "model": model,
        "n": int(data.get("n", 0) or 0),
        "score": float(metrics["mean_score"]),
        "breakdown": breakdown if isinstance(breakdown, dict) else {},
    }


def load_all(results_dir: Path) -> dict[tuple[str, str], dict]:
    """Load all 8 canonical (eval, model) cells."""
    cells: dict[tuple[str, str], dict] = {}
    for eval_name in EVALS:
        for model, _ in MODELS:
            cells[(eval_name, model)] = load_cell(results_dir, eval_name, model)
    return cells


# --- SVG helpers ----------------------------------------------------------
def _text(x: float, y: float, s: str, *, size: float, fill: str = INK,
          anchor: str = "start", weight: int = 400) -> str:
    weight_attr = f' font-weight="{weight}"' if weight != 400 else ""
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" fill="{fill}" '
        f'text-anchor="{anchor}"{weight_attr}>{escape(s)}</text>'
    )


def _bar(x: float, y: float, w: float, h: float, fill: str) -> str:
    return f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" fill="{fill}" rx="2"/>'


def render_svg(cells: dict[tuple[str, str], dict], title: str) -> str:
    """Build the two-panel results figure as an SVG string."""
    width = 1080
    height = 620
    p: list[str] = []
    p.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="{FONT}">'
    )
    p.append(f'<rect width="{width}" height="{height}" fill="{BG}"/>')

    # --- header -----------------------------------------------------------
    p.append(_text(48, 44, title, size=27, weight=700))
    p.append(
        _text(
            48, 68,
            "Mean honesty score per eval. Y-axis zoomed to 0.50-1.00 -- the "
            "scores cluster high, so small gaps matter.",
            size=13.5, fill=SUBTLE,
        )
    )

    # ============ panel A: headline grouped bar chart =====================
    ax_left = 64
    ax_top = 104
    ax_w = 612
    ax_h = 372
    ax_bottom = ax_top + ax_h
    y_lo, y_hi = 0.50, 1.00

    def y_of(score: float) -> float:
        clamped = min(max(score, y_lo), y_hi)
        return ax_bottom - (clamped - y_lo) / (y_hi - y_lo) * ax_h

    # y gridlines + labels every 0.10
    ticks = [0.50, 0.60, 0.70, 0.80, 0.90, 1.00]
    for t in ticks:
        y = y_of(t)
        p.append(
            f'<line x1="{ax_left}" y1="{y:.1f}" x2="{ax_left + ax_w}" y2="{y:.1f}" '
            f'stroke="{GRID}" stroke-width="1"/>'
        )
        p.append(_text(ax_left - 10, y + 4, f"{t:.2f}", size=11.5, fill=AXIS, anchor="end"))

    # axis lines
    p.append(
        f'<line x1="{ax_left}" y1="{ax_top}" x2="{ax_left}" y2="{ax_bottom}" '
        f'stroke="{AXIS}" stroke-width="1.5"/>'
    )
    p.append(
        f'<line x1="{ax_left}" y1="{ax_bottom}" x2="{ax_left + ax_w}" y2="{ax_bottom}" '
        f'stroke="{AXIS}" stroke-width="1.5"/>'
    )
    # break marker on the y-axis: the axis starts at 0.50, not 0.
    p.append(
        f'<text x="{ax_left - 10}" y="{ax_bottom + 16:.1f}" font-size="11.5" '
        f'fill="{AXIS}" text-anchor="end">0 ⤓</text>'
    )

    n_groups = len(EVALS)
    n_series = len(MODELS)
    group_w = ax_w / n_groups
    group_pad = group_w * 0.16
    bars_w = group_w - 2 * group_pad
    bar_w = bars_w / n_series

    for gi, eval_name in enumerate(EVALS):
        gx = ax_left + gi * group_w
        # group separators (skip the first)
        if gi > 0:
            p.append(
                f'<line x1="{gx:.1f}" y1="{ax_top}" x2="{gx:.1f}" y2="{ax_bottom}" '
                f'stroke="{GRID}" stroke-width="1"/>'
            )
        # n is the same across models for a given eval; read it once.
        n_eval = cells[(eval_name, MODELS[0][0])]["n"]
        p.append(
            _text(gx + group_w / 2, ax_bottom + 22, eval_name, size=14, weight=600,
                  anchor="middle")
        )
        p.append(
            _text(gx + group_w / 2, ax_bottom + 38, f"n = {n_eval}", size=11,
                  fill=FAINT, anchor="middle")
        )
        for si, (model, _label) in enumerate(MODELS):
            cell = cells[(eval_name, model)]
            color = MODEL_COLORS[model]
            x = gx + group_pad + si * bar_w
            bw = bar_w - 3
            y = y_of(cell["score"])
            p.append(_bar(x, y, bw, ax_bottom - y, color))
            p.append(
                _text(x + bw / 2, y - 6, f"{cell['score']:.3f}", size=11,
                      weight=600, anchor="middle")
            )

    # ============ panel B: where the models diverge =======================
    bx_left = 740
    bx_right = width - 40
    bx_w = bx_right - bx_left
    p.append(
        f'<line x1="{bx_left - 32}" y1="{ax_top - 4}" x2="{bx_left - 32}" '
        f'y2="{ax_bottom + 40}" stroke="{PANEL_RULE}" stroke-width="1"/>'
    )
    p.append(_text(bx_left, ax_top + 8, "Where the models diverge", size=16, weight=700))
    p.append(
        _text(
            bx_left, ax_top + 28,
            "Per-kind slices, full 0-1 scale.",
            size=12, fill=SUBTLE,
        )
    )

    sub_top = ax_top + 52
    sub_h = 116  # height of each mini-chart's plot area
    sub_gap = 60
    bar_h = 26
    bar_gap = 10

    for di, (eval_name, kind, slice_label) in enumerate(DIVERGENCE_SLICES):
        block_top = sub_top + di * (sub_h + sub_gap)
        p.append(_text(bx_left, block_top, slice_label, size=12.5, weight=600))
        # one sample-n note (same kind n across models)
        kind_n = cells[(eval_name, MODELS[0][0])]["breakdown"].get(kind, {}).get("n")
        if kind_n is not None:
            p.append(
                _text(bx_right, block_top, f"n = {int(kind_n)}", size=11,
                      fill=FAINT, anchor="end")
            )
        # horizontal bars, one per model (labels sit inside the track)
        row_y = block_top + 14
        track_x = bx_left
        track_w = bx_w - 44  # leave room for the value label
        for si, (model, label) in enumerate(MODELS):
            cell = cells[(eval_name, model)]
            kscore = cell["breakdown"].get(kind, {}).get("mean_score")
            color = MODEL_COLORS[model]
            ry = row_y + si * (bar_h + bar_gap)
            # faint full-width track
            p.append(
                f'<rect x="{track_x:.1f}" y="{ry:.1f}" width="{track_w:.1f}" '
                f'height="{bar_h}" fill="{GRID}" rx="2"/>'
            )
            if isinstance(kscore, (int, float)):
                fill_w = max(track_w * float(kscore), 1.5)
                p.append(_bar(track_x, ry, fill_w, bar_h, color))
                p.append(
                    _text(track_x + track_w + 8, ry + bar_h / 2 + 4,
                          f"{float(kscore):.3f}", size=11.5, weight=600)
                )
            # model label sits inside the track, left-aligned. Dark ink so it
            # stays legible on a very short bar (where it spills onto the
            # faint track) as well as on the coloured fill.
            p.append(
                _text(track_x + 8, ry + bar_h / 2 + 4, label, size=11,
                      fill=INK, weight=600)
            )

    # ============ legend ==================================================
    legend_y = height - 52
    swatch = 14
    # measure entries to centre the legend under panel A
    entries = [(MODEL_COLORS[m], lbl) for m, lbl in MODELS]
    entry_w = [swatch + 7 + len(lbl) * 7.4 + 26 for _, lbl in entries]
    total = sum(entry_w) - 26
    lx = ax_left + (ax_w - total) / 2
    for (color, label), ew in zip(entries, entry_w, strict=True):
        p.append(
            f'<rect x="{lx:.1f}" y="{legend_y:.1f}" width="{swatch}" '
            f'height="{swatch}" fill="{color}" rx="2"/>'
        )
        p.append(_text(lx + swatch + 7, legend_y + swatch - 2, label, size=13))
        lx += ew

    # ============ caption =================================================
    p.append(
        _text(
            48, height - 18,
            "candor-bench v0.1  ·  xAI clean-adapter runs  ·  higher = more honest",
            size=12, fill=FAINT,
        )
    )
    p.append(
        _text(
            width - 40, height - 18,
            "github.com/teddytennant/candor-bench",
            size=11, fill=FAINT, anchor="end",
        )
    )

    p.append("</svg>")
    return "\n".join(p)


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=repo_root / "results",
        help="directory holding the canonical EvalReport JSON files (default: results/)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="output SVG path (default: <results-dir>/figure.svg)",
    )
    parser.add_argument(
        "--title",
        default="candor-bench: honesty under pressure",
        help="chart title",
    )
    args = parser.parse_args()

    results_dir: Path = args.results_dir
    out_path: Path = args.out or (results_dir / "figure.svg")

    if not results_dir.is_dir():
        print(f"error: results dir not found: {results_dir}", file=sys.stderr)
        return 1

    cells = load_all(results_dir)
    svg = render_svg(cells, args.title)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(svg + "\n", encoding="utf-8")

    print(f"wrote {out_path}")
    print(f"  {len(cells)} cells  |  {len(EVALS)} evals  |  {len(MODELS)} models")
    for eval_name in EVALS:
        row = " ".join(
            f"{lbl}={cells[(eval_name, m)]['score']:.3f}" for m, lbl in MODELS
        )
        print(f"  {eval_name:12s} {row}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
