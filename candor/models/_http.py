"""Shared HTTP helpers for network-backed model adapters.

Keeps the dependency surface at httpx only — the retry/backoff loop here is
deliberately small rather than pulling in tenacity or similar.
"""

from __future__ import annotations

import asyncio
import random

import httpx

# HTTP status codes worth retrying: rate limits + transient server errors.
RETRYABLE_STATUS = {408, 409, 429, 500, 502, 503, 504}


async def post_json_with_retry(
    url: str,
    *,
    json: dict,
    headers: dict[str, str],
    timeout: float,
    max_retries: int = 4,
    backoff_base: float = 1.0,
    backoff_cap: float = 30.0,
) -> dict:
    """POST `json` to `url` and return the parsed JSON response.

    Retries on transient network errors and retryable HTTP status codes with
    exponential backoff + jitter. Honors a `Retry-After` header when present.
    Raises a clear error (not a bare httpx exception) on permanent failure.
    """
    last_exc: Exception | None = None
    async with httpx.AsyncClient(timeout=timeout) as client:
        for attempt in range(max_retries + 1):
            try:
                r = await client.post(url, json=json, headers=headers)
            except (httpx.TimeoutException, httpx.TransportError) as exc:
                last_exc = exc
                if attempt >= max_retries:
                    raise RuntimeError(
                        f"HTTP request to {url} failed after {max_retries + 1} "
                        f"attempts: {type(exc).__name__}: {exc}"
                    ) from exc
                await _sleep_backoff(attempt, backoff_base, backoff_cap)
                continue

            if r.status_code in RETRYABLE_STATUS and attempt < max_retries:
                delay = _retry_after(r) or _backoff_delay(attempt, backoff_base, backoff_cap)
                await asyncio.sleep(delay)
                continue

            if r.status_code >= 400:
                body = r.text[:500]
                raise RuntimeError(
                    f"HTTP {r.status_code} from {url}: {body}"
                )

            try:
                return r.json()
            except ValueError as exc:  # json.JSONDecodeError subclasses ValueError
                raise RuntimeError(
                    f"Non-JSON response from {url} (HTTP {r.status_code}): "
                    f"{r.text[:300]!r}"
                ) from exc

    # Only reachable if the loop exhausted retries on retryable status codes.
    raise RuntimeError(
        f"HTTP request to {url} still failing after {max_retries + 1} attempts"
        + (f": {last_exc}" if last_exc else "")
    )


def _backoff_delay(attempt: int, base: float, cap: float) -> float:
    return min(cap, base * (2**attempt)) + random.uniform(0, base)


async def _sleep_backoff(attempt: int, base: float, cap: float) -> None:
    await asyncio.sleep(_backoff_delay(attempt, base, cap))


def _retry_after(r: httpx.Response) -> float | None:
    """Parse a Retry-After header (delta-seconds form) if present and sane."""
    raw = r.headers.get("retry-after")
    if not raw:
        return None
    try:
        val = float(raw)
    except ValueError:
        return None
    if val < 0 or val > 120:  # ignore absurd/HTTP-date values; fall back to backoff
        return None
    return val
