"""Command-line interface for candor-bench."""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
from pathlib import Path

from . import evals  # noqa: F401  importing registers all evals
from .eval import get_eval, list_evals
from .models import from_spec
from .report import render_summary, render_text, write_json
from .runner import run_eval
from .types import EvalReport

# Load environment variables from .env file if python-dotenv is available.
# This makes it easy to manage API keys (xAI, OpenAI, any OpenAI-compatible
# endpoint) without exporting them in every shell session.
try:
    from dotenv import load_dotenv
    load_dotenv()
except ImportError:
    pass  # python-dotenv not installed — user can still use shell env vars


def _cmd_list(_args: argparse.Namespace) -> int:
    print("Registered evals:")
    for n in list_evals():
        e = get_eval(n)
        try:
            samples = e.load_samples()
            n_samples = len(samples)
        except Exception as exc:  # noqa: BLE001
            print(f"  {n:>14s}  v{e.version}  (dataset error: {exc})")
            continue
        print(f"  {n:>14s}  v{e.version}  n={n_samples}")
    return 0


def _cmd_run(args: argparse.Namespace) -> int:
    if args.concurrency < 1:
        print(f"error: --concurrency must be >= 1 (got {args.concurrency})", file=sys.stderr)
        return 2
    out_dir = Path(args.output)
    eval_names = args.eval if args.eval else list_evals()
    if not eval_names:
        print("error: no evals registered or selected", file=sys.stderr)
        return 1

    known = set(list_evals())
    unknown = [e for e in eval_names if e not in known]
    if unknown:
        print(
            f"error: unknown eval(s): {', '.join(unknown)}. "
            f"Available: {', '.join(sorted(known))}",
            file=sys.stderr,
        )
        return 2

    try:
        model = from_spec(args.model)
    except (ValueError, RuntimeError) as exc:
        print(f"error: could not load model {args.model!r}: {exc}", file=sys.stderr)
        return 2
    reports = []

    async def go() -> None:
        for ename in eval_names:
            eval_obj = get_eval(ename)
            samples = eval_obj.load_samples()
            if args.limit > 0:
                samples = samples[: args.limit]
            print(f"\n>>> running {ename} on {model.name} (n={len(samples)})")
            report = await run_eval(
                eval_obj, model, samples, concurrency=args.concurrency, progress=args.progress
            )
            reports.append(report)
            print(render_text(report))
            path = write_json(report, out_dir)
            print(f"  -> {path}")

    try:
        asyncio.run(go())
    except FileNotFoundError as exc:
        print(f"error: dataset not found: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\ninterrupted", file=sys.stderr)
        # Write any completed reports before exiting
        for r in reports:
            try:
                path = write_json(r, out_dir)
                print(f"  saved partial: {path}", file=sys.stderr)
            except Exception as e:
                print(f"  failed to save {r.eval}: {e}", file=sys.stderr)
        return 130
    print(render_summary(reports))
    return 0


def _cmd_score(args: argparse.Namespace) -> int:
    """Re-aggregate and pretty-print an existing JSON run.

    Useful when you've already paid for the model calls and just want to
    re-render or re-aggregate.
    """
    path = Path(args.path)
    try:
        raw = path.read_text(encoding="utf-8")
    except (FileNotFoundError, IsADirectoryError, PermissionError) as exc:
        print(f"error: cannot read {path}: {exc}", file=sys.stderr)
        return 1
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as exc:
        print(f"error: {path} is not valid JSON: {exc}", file=sys.stderr)
        return 1
    try:
        report = EvalReport.model_validate(data)
    except ValueError as exc:
        print(f"error: {path} is not a valid candor report: {exc}", file=sys.stderr)
        return 1
    print(render_text(report))
    return 0


def _cmd_validate(_args: argparse.Namespace) -> int:
    """Validate all registered evals: check kinds, duplicate IDs, and eval field."""
    errors_found = False
    for name in list_evals():
        eval_obj = get_eval(name)
        try:
            samples = eval_obj.load_samples()
        except Exception as exc:
            print(f"{name:>14s}: LOAD ERROR: {exc}")
            errors_found = True
            continue

        # Check for duplicate IDs
        seen: dict[str, int] = {}
        for s in samples:
            seen[s.id] = seen.get(s.id, 0) + 1
        dups = [k for k, v in seen.items() if v > 1]

        # Check eval field matches
        wrong_eval = [s for s in samples if s.eval != name]

        # KINDS validation happens inside load_samples() and raises on invalid

        if dups or wrong_eval:
            print(f"{name:>14s}: ERRORS")
            if dups:
                print(f"    duplicate IDs: {dups}")
            if wrong_eval:
                print(f"    wrong eval field on {len(wrong_eval)} samples")
            errors_found = True
        else:
            print(f"{name:>14s}: OK ({len(samples)} samples)")

    return 1 if errors_found else 0


def main(argv: list[str] | None = None) -> int:
    # Force line-buffered stdout so progress lines appear in real time even
    # when output is piped to a file (default Python full-buffering hides them).
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(line_buffering=True)  # type: ignore[attr-defined]
    os.environ.setdefault("PYTHONUNBUFFERED", "1")
    parser = argparse.ArgumentParser(prog="candor", description="Honesty evals for LLMs.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_list = sub.add_parser("list", help="list registered evals")
    p_list.set_defaults(func=_cmd_list)

    p_run = sub.add_parser("run", help="run an eval against a model")
    p_run.add_argument(
        "--model",
        required=True,
        help="model spec, e.g. 'xai:grok-4.3' or 'stub:honest-confident'",
    )
    p_run.add_argument(
        "--eval",
        action="append",
        default=None,
        help="eval to run (repeatable). Default: all.",
    )
    p_run.add_argument("--output", default="results", help="output directory")
    p_run.add_argument("--concurrency", type=int, default=4)
    p_run.add_argument("--limit", type=int, default=0, help=">0 to limit samples per eval")
    p_run.add_argument("--progress", action="store_true")
    p_run.set_defaults(func=_cmd_run)

    p_score = sub.add_parser("score", help="re-render an existing report JSON")
    p_score.add_argument("path", help="path to a run-output JSON file")
    p_score.set_defaults(func=_cmd_score)

    p_validate = sub.add_parser("validate", help="validate dataset integrity for all evals")
    p_validate.set_defaults(func=_cmd_validate)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
