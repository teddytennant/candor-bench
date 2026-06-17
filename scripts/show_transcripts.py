#!/usr/bin/env python3
"""Surface the most interesting candor-bench transcripts for launch material.

Loads result JSON files (the EvalReport dump written by candor/report.py) and
prints the FAILURE / ambiguous cases as readable transcripts: the prompt(s),
the model's response(s), the score, the label, and the grader's notes.

These are the shareable bits — e.g. a model capitulating under pressure, or
asserting a planted lie. Output is plain text, sized to copy into a tweet or
screenshot.

Usage:
    python scripts/show_transcripts.py                       # all failures, all files
    python scripts/show_transcripts.py --eval sycophancy
    python scripts/show_transcripts.py --model grok-4.3
    python scripts/show_transcripts.py --label capitulated
    python scripts/show_transcripts.py --worst-only          # only score == 0.0
    python scripts/show_transcripts.py --results-dir results-regraded

Notes on data:
    The result files DO persist full transcripts: each GradedResult carries a
    `calls` list of ModelCall objects with `messages` (system/user/assistant)
    and `response`. The one exception is samples that errored during the run —
    the runner's exception path stores `calls=[]`, so those have no transcript
    to show (we surface them as a one-line note instead).
"""

from __future__ import annotations

import argparse
import json
import sys
import textwrap
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# terminal formatting helpers
# ---------------------------------------------------------------------------

WIDTH = 78


def _supports_color() -> bool:
    return sys.stdout.isatty()


_COLOR = _supports_color()


def c(text: str, code: str) -> str:
    if not _COLOR:
        return text
    return f"\033[{code}m{text}\033[0m"


def bold(t: str) -> str:
    return c(t, "1")


def dim(t: str) -> str:
    return c(t, "2")


def red(t: str) -> str:
    return c(t, "31")


def yellow(t: str) -> str:
    return c(t, "33")


def cyan(t: str) -> str:
    return c(t, "36")


def green(t: str) -> str:
    return c(t, "32")


def rule(char: str = "=") -> str:
    return char * WIDTH


def wrap(text: str, indent: str = "") -> str:
    """Wrap text to WIDTH, preserving blank lines, with a hanging indent."""
    out_lines: list[str] = []
    for para in text.split("\n"):
        if not para.strip():
            out_lines.append("")
            continue
        wrapped = textwrap.wrap(
            para,
            width=WIDTH - len(indent),
            break_long_words=False,
            break_on_hyphens=False,
        )
        out_lines.extend(indent + line for line in (wrapped or [""]))
    return "\n".join(out_lines)


# ---------------------------------------------------------------------------
# loading + filtering
# ---------------------------------------------------------------------------


def load_reports(results_dir: Path) -> list[dict[str, Any]]:
    reports: list[dict[str, Any]] = []
    for path in sorted(results_dir.glob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError) as exc:
            print(dim(f"  (skipping {path.name}: {exc})"), file=sys.stderr)
            continue
        # A run currently in progress may write partial/empty files.
        if not isinstance(data, dict) or "results" not in data:
            print(dim(f"  (skipping {path.name}: not an EvalReport)"), file=sys.stderr)
            continue
        data["_path"] = str(path)
        reports.append(data)
    return reports


# calibration scores are continuous (Brier-style, e.g. 0.9996) — a 0.97 there
# is essentially a pass, not launch material. Only flag clear shortfalls.
PASS_THRESHOLD = 0.9


def is_interesting(result: dict[str, Any], worst_only: bool) -> bool:
    """Failure cases worth showing: score 0.0, or ambiguous/partial shortfalls.

    A "clean pass" is score >= PASS_THRESHOLD (covers both the discrete 1.0
    evals and calibration's continuous near-1.0 scores).
    """
    score = result.get("score", 1.0)
    if worst_only:
        return score == 0.0
    return score < PASS_THRESHOLD


def match_filter(value: str, needle: str | None) -> bool:
    if needle is None:
        return True
    return needle.lower() in value.lower()


# ---------------------------------------------------------------------------
# rendering one transcript
# ---------------------------------------------------------------------------

ROLE_LABEL = {
    "system": yellow("SYSTEM"),
    "user": cyan("USER"),
    "assistant": green("ASSISTANT"),
}


def score_tag(score: float) -> str:
    if score == 0.0:
        return red(f"score={score:.2f}  <-- FAILURE")
    if score < 1.0:
        return yellow(f"score={score:.2f}  <-- ambiguous/partial")
    return f"score={score:.2f}"


def render_result(result: dict[str, Any], model: str) -> str:
    lines: list[str] = []
    sid = result.get("sample_id", "?")
    ev = result.get("eval", "?")
    kind = result.get("kind", "?")
    label = result.get("label", "?")
    score = result.get("score", float("nan"))
    notes = result.get("notes", "") or ""
    calls = result.get("calls") or []

    lines.append(rule("="))
    lines.append(bold(f"{ev} / {kind}") + dim(f"   [{sid}]"))
    lines.append(f"  model:  {model}")
    lines.append(f"  label:  {bold(label)}")
    lines.append(f"  {score_tag(score)}")
    if notes:
        lines.append(wrap(f"why:    {notes}", indent="  ").lstrip())
        # re-add the leading spaces stripped above
        lines[-1] = "  " + lines[-1]
    lines.append(rule("-"))

    if not calls:
        lines.append(dim("  (no transcript persisted for this sample — likely an"))
        lines.append(dim("   errored run; the runner stores calls=[] on exception)"))
        lines.append("")
        return "\n".join(lines)

    for i, call in enumerate(calls):
        if len(calls) > 1:
            lines.append(dim(f"  --- turn {i + 1}/{len(calls)} ---"))
        messages = call.get("messages") or []
        # Only show messages new to this call: each successive ModelCall in an
        # eval typically replays the prior turns, so skip the overlap.
        start = 0
        if i > 0 and calls[i - 1].get("messages"):
            start = len(calls[i - 1]["messages"])
            # the previous turn's response becomes an assistant msg here
            if start < len(messages):
                lines.append(dim(f"  (+{start} prior messages replayed, hidden)"))
        for msg in messages[start:]:
            role = msg.get("role", "?")
            tag = ROLE_LABEL.get(role, role.upper())
            lines.append(f"  {tag}")
            lines.append(wrap(msg.get("content", ""), indent="    "))
        resp = call.get("response", "")
        lines.append(f"  {green('ASSISTANT')} " + dim("(response)"))
        lines.append(wrap(resp, indent="    "))
        lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Print interesting (failure/ambiguous) candor-bench transcripts.",
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=Path("results"),
        help="Directory of EvalReport JSON files (default: results).",
    )
    parser.add_argument(
        "--eval",
        help="Only show this eval (substring match, e.g. 'sycophancy').",
    )
    parser.add_argument(
        "--model",
        help="Only show this model (substring match, e.g. 'grok').",
    )
    parser.add_argument(
        "--label",
        help="Only show this grader label (substring match, e.g. 'capitulated').",
    )
    parser.add_argument(
        "--kind",
        help="Only show this sample kind (substring match, e.g. 'system_prompt_lie').",
    )
    parser.add_argument(
        "--worst-only",
        action="store_true",
        help="Only score==0.0 outright failures (skip 0.5/ambiguous).",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=0,
        help="Max transcripts to print (0 = no limit).",
    )
    args = parser.parse_args()

    if not args.results_dir.is_dir():
        print(red(f"results dir not found: {args.results_dir}"), file=sys.stderr)
        return 1

    reports = load_reports(args.results_dir)
    if not reports:
        print(red(f"no EvalReport JSON files in {args.results_dir}"), file=sys.stderr)
        return 1

    # collect matching results
    selected: list[tuple[dict[str, Any], str]] = []
    for report in reports:
        model = report.get("model", "?")
        ev = report.get("eval", "?")
        if not match_filter(model, args.model):
            continue
        if not match_filter(ev, args.eval):
            continue
        for result in report.get("results", []):
            if not match_filter(result.get("label", ""), args.label):
                continue
            if not match_filter(result.get("kind", ""), args.kind):
                continue
            if not is_interesting(result, args.worst_only):
                continue
            selected.append((result, model))

    # worst first: 0.0 before 0.5 before higher partials
    selected.sort(key=lambda rm: rm[0].get("score", 1.0))

    if args.limit > 0:
        selected = selected[: args.limit]

    if not selected:
        print(dim("no matching failure/ambiguous transcripts."))
        print(dim("(try dropping a filter, or --worst-only off)"))
        return 0

    n_fail = sum(1 for r, _ in selected if r.get("score", 1.0) == 0.0)
    n_amb = len(selected) - n_fail
    print()
    print(bold(f"candor-bench transcripts: {len(selected)} interesting samples"))
    print(dim(f"  {n_fail} outright failures (score 0.0), {n_amb} ambiguous/partial"))
    print(dim(f"  source: {args.results_dir}/"))
    print()

    for result, model in selected:
        print(render_result(result, model))

    return 0


if __name__ == "__main__":
    sys.exit(main())
