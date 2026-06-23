#!/usr/bin/env python3
"""Benchmark Kaggle Model Proxy models on candor-bench.

The Kaggle Benchmarks Model Proxy (https://www.kaggle.com/benchmarks) is an
OpenAI-compatible gateway. `kaggle benchmarks auth` provisions a short-lived
credential pair -- MODEL_PROXY_URL and MODEL_PROXY_API_KEY -- and lists the
models the credential may call (LLMS_AVAILABLE). candor's `kaggle:` adapter
speaks to that gateway directly, so this script is a thin sweep driver: for each
model x eval it shells out to `candor run --model kaggle:<id>`.

The credential is short-lived (about an hour). Pass --reauth to re-run
`kaggle benchmarks auth` before each model so a long sweep can't expire
mid-run; the refreshed .env is reloaded between models.

Note the Model Proxy exposes two tiers. The locally-provisioned credential can
only reach the handful of models in LLMS_AVAILABLE. The full frontier catalog
(`kaggle benchmarks tasks models`) is reachable only by pushing a task and
running it on Kaggle's own infrastructure -- a different integration than this
local sweep.

Usage:
    # Provision once, then sweep every granted model across all four evals:
    kaggle benchmarks auth
    python scripts/run_kaggle_proxy.py --output results-kaggle

    # A specific subset, one eval, refreshing the credential per model:
    python scripts/run_kaggle_proxy.py \
        --models anthropic/claude-haiku-4-5@20251001 zai/glm-5 \
        --evals calibration sycophancy --reauth --output results-kaggle

    python scripts/run_kaggle_proxy.py --help
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

ENV_KEYS = ("MODEL_PROXY_URL", "MODEL_PROXY_API_KEY", "LLMS_AVAILABLE")


def parse_env_file(path: Path) -> dict[str, str]:
    """Read a dotenv-style file into a dict (KEY=VALUE lines; # comments)."""
    out: dict[str, str] = {}
    if not path.exists():
        return out
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        out[key.strip()] = value.strip()
    return out


def reauth(env_file: Path) -> None:
    """Refresh Model Proxy credentials into `env_file` via the Kaggle CLI."""
    print(f"  [auth] refreshing Model Proxy credentials -> {env_file}", flush=True)
    subprocess.run(
        ["kaggle", "benchmarks", "auth", "-y", "--env-file", str(env_file)],
        check=True,
    )


def load_proxy_env(env_file: Path) -> dict[str, str]:
    """Resolve proxy vars from the process env, falling back to `env_file`."""
    env = {k: os.environ[k] for k in ENV_KEYS if k in os.environ}
    file_env = parse_env_file(env_file)
    for key in ENV_KEYS:
        env.setdefault(key, file_env.get(key, ""))
    return {k: v for k, v in env.items() if v}


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--models",
        nargs="+",
        help="Model ids to run (e.g. anthropic/claude-haiku-4-5@20251001). "
        "Defaults to every id in LLMS_AVAILABLE.",
    )
    parser.add_argument(
        "--evals",
        nargs="+",
        help="Eval names to run (default: all registered evals).",
    )
    parser.add_argument(
        "--output",
        default="results-kaggle",
        help="Directory for candor report JSONs (default: results-kaggle).",
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=Path(".env"),
        help="dotenv file holding MODEL_PROXY_* vars (default: ./.env).",
    )
    parser.add_argument(
        "--reauth",
        action="store_true",
        help="Run `kaggle benchmarks auth` before each model so a long sweep "
        "cannot expire mid-run.",
    )
    parser.add_argument(
        "--candor",
        default="candor",
        help="candor executable to invoke (default: candor on PATH).",
    )
    args = parser.parse_args()

    if args.reauth:
        reauth(args.env_file)

    proxy = load_proxy_env(args.env_file)
    if "MODEL_PROXY_URL" not in proxy or "MODEL_PROXY_API_KEY" not in proxy:
        print(
            "error: MODEL_PROXY_URL / MODEL_PROXY_API_KEY not found in the "
            f"environment or {args.env_file}. Run `kaggle benchmarks auth` "
            "first (or pass --reauth).",
            file=sys.stderr,
        )
        return 2

    models = args.models
    if not models:
        available = proxy.get("LLMS_AVAILABLE", "")
        models = [m for m in available.split(",") if m]
    if not models:
        print(
            "error: no models to run. Pass --models or ensure LLMS_AVAILABLE is "
            "set (re-run `kaggle benchmarks auth`).",
            file=sys.stderr,
        )
        return 2

    print(f"Sweeping {len(models)} model(s) into {args.output}/", flush=True)
    failures: list[str] = []
    for model in models:
        if args.reauth and model is not models[0]:
            reauth(args.env_file)
            proxy = load_proxy_env(args.env_file)

        # Hand the proxy credentials to candor's kaggle: adapter via the env.
        run_env = dict(os.environ)
        run_env["MODEL_PROXY_URL"] = proxy["MODEL_PROXY_URL"]
        run_env["MODEL_PROXY_API_KEY"] = proxy["MODEL_PROXY_API_KEY"]

        cmd = [args.candor, "run", "--model", f"kaggle:{model}", "--output", args.output]
        if args.evals:
            for name in args.evals:
                cmd += ["--eval", name]
        print(f"\n=== {model} ===\n$ {' '.join(cmd)}", flush=True)
        result = subprocess.run(cmd, env=run_env)
        if result.returncode != 0:
            failures.append(model)
            print(f"  [warn] candor exited {result.returncode} for {model}", flush=True)

    if failures:
        print(f"\nCompleted with failures: {', '.join(failures)}", file=sys.stderr)
        return 1
    print("\nAll model runs completed.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
