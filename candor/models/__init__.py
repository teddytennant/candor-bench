"""Model adapters."""

from __future__ import annotations

import os
from typing import Any

from ..model import Model
from .openai_compat import OpenAICompatModel
from .stub import StubModel
from .xai_responses import XaiResponsesModel

_KNOWN = "xai, xai-responses, openai, openrouter, kaggle, local, stub"


def from_spec(spec: str, **kwargs: Any) -> Model:
    """Build a model adapter from a `provider:model_id` spec string.

    Examples:
        xai:grok-4.3                               (xAI chat-completions; uses XAI_* / LOCAL_*)
        xai-responses:grok-4.20-multi-agent-0309   (xAI Responses API; multi-agent family)
        openai:gpt-4o-mini                         (OpenAI; uses OPENAI_API_KEY)
        openrouter:meta-llama/llama-3.1-8b-instruct (OpenRouter gateway; uses OPENROUTER_API_KEY)
        kaggle:anthropic/claude-haiku-4-5@20251001 (Kaggle Benchmarks Model Proxy; uses MODEL_PROXY_*)
        local:llama-3.1-8b-instruct                (any OpenAI-compatible server; uses LOCAL_*)
        stub:always-honest
    """
    if ":" not in spec:
        raise ValueError(
            f"Model spec must be 'provider:model_id', got {spec!r}. "
            f"Known providers: {_KNOWN}."
        )
    provider, model_id = spec.split(":", 1)
    provider = provider.strip().lower()
    model_id = model_id.strip()
    if not model_id:
        raise ValueError(f"Model spec {spec!r} has an empty model_id")

    if provider in ("xai", "grok"):
        # First-class xAI chat-completions path. Defaults to the xAI endpoint;
        # honors XAI_* and falls back to the shared LOCAL_* vars so existing
        # .env files keep working. Multi-agent Grok models need xai-responses.
        base_url = (
            os.environ.get("XAI_BASE_URL")
            or os.environ.get("LOCAL_BASE_URL")
            or "https://api.x.ai/v1"
        )
        api_key = os.environ.get("XAI_API_KEY") or os.environ.get("LOCAL_API_KEY")
        return OpenAICompatModel(
            model_id, base_url=base_url, api_key=api_key, name_prefix="xai", **kwargs
        )
    if provider in ("xai-responses", "xai_responses", "responses"):
        return XaiResponsesModel(
            model_id, env_base_url="LOCAL_BASE_URL", env_key="LOCAL_API_KEY", **kwargs
        )
    if provider == "openai":
        return OpenAICompatModel(
            model_id, base_url="https://api.openai.com/v1", env_key="OPENAI_API_KEY", **kwargs
        )
    if provider in ("openrouter", "or"):
        # OpenRouter is a hosted OpenAI-compatible gateway; it fronts open-weight
        # models (Llama, Qwen, Mistral, Gemma, ...) under `vendor/model` ids.
        # Reported under the `openrouter:` tag so runs read as the gateway, not
        # the endpoint shape it shares with OpenAI.
        return OpenAICompatModel(
            model_id,
            base_url="https://openrouter.ai/api/v1",
            env_key="OPENROUTER_API_KEY",
            name_prefix="openrouter",
            **kwargs,
        )
    if provider == "kaggle":
        # Kaggle Benchmarks Model Proxy: an OpenAI-compatible gateway whose
        # base URL and key are provisioned by `kaggle benchmarks auth`
        # (MODEL_PROXY_URL / MODEL_PROXY_API_KEY). The OpenAI surface lives under
        # the `/openapi` suffix; the proxy hands back a bare base, so append it
        # (and tolerate a base that already carries the /openapi or /genai tail).
        base = os.environ.get("MODEL_PROXY_URL")
        if not base:
            raise RuntimeError(
                "MODEL_PROXY_URL not set. Run `kaggle benchmarks auth` (or "
                "`kaggle benchmarks init`) to provision Model Proxy credentials, "
                "then load the generated .env."
            )
        base = base.rstrip("/")
        for suffix in ("/openapi", "/genai"):
            if base.endswith(suffix):
                base = base[: -len(suffix)]
                break
        return OpenAICompatModel(
            model_id,
            base_url=f"{base}/openapi",
            env_key="MODEL_PROXY_API_KEY",
            name_prefix="kaggle",
            **kwargs,
        )
    if provider == "local":
        return OpenAICompatModel(
            model_id,
            env_base_url="LOCAL_BASE_URL",
            env_key="LOCAL_API_KEY",
            name_prefix="local",
            **kwargs,
        )
    if provider == "stub":
        return StubModel(model_id, **kwargs)
    raise ValueError(
        f"Unknown provider {provider!r} in spec {spec!r}. Known providers: {_KNOWN}."
    )


__all__ = ["OpenAICompatModel", "StubModel", "XaiResponsesModel", "from_spec"]
