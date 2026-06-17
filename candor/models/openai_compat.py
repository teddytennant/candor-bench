"""OpenAI-compatible chat completions adapter.

Works with any server that implements `/v1/chat/completions` — xAI's API,
OpenAI itself, and local servers (llama.cpp's server, vLLM, ollama in OpenAI
mode, LM Studio, etc.). `name_prefix` controls the `provider:` tag the run is
reported under, so an xAI run reads as `xai:grok-4.3` rather than the endpoint
shape it happens to share with OpenAI.
"""

from __future__ import annotations

import os

from ..types import Message
from ._http import post_json_with_retry


class OpenAICompatModel:
    def __init__(
        self,
        model_id: str,
        *,
        base_url: str | None = None,
        env_base_url: str | None = None,
        api_key: str | None = None,
        env_key: str = "OPENAI_API_KEY",
        timeout: float = 120.0,
        max_retries: int = 4,
        name_prefix: str = "openai",
    ):
        self.name = f"{name_prefix}:{model_id}"
        self.model_id = model_id
        # Resolve base URL: explicit arg > env var > OpenAI default. If an
        # `env_base_url` was requested (e.g. `local:` -> LOCAL_BASE_URL) but is
        # unset, fail loudly rather than silently hitting api.openai.com.
        if base_url is None and env_base_url:
            base_url = os.environ.get(env_base_url)
            if not base_url:
                raise RuntimeError(
                    f"{env_base_url} not set. Point it at your server's base URL "
                    f"(e.g. http://localhost:8080/v1)."
                )
        if base_url is None:
            base_url = "https://api.openai.com/v1"
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key or os.environ.get(env_key) or "sk-none"
        self.timeout = timeout
        self.max_retries = max_retries

    async def chat(
        self,
        messages: list[Message],
        *,
        temperature: float = 0.0,
        max_tokens: int = 1024,
    ) -> str:
        body = {
            "model": self.model_id,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        data = await post_json_with_retry(
            f"{self.base_url}/chat/completions",
            json=body,
            headers=headers,
            timeout=self.timeout,
            max_retries=self.max_retries,
        )
        choices = data.get("choices") or []
        if not choices:
            raise RuntimeError(f"OpenAI-compatible response had no choices: {data!r}")
        message = choices[0].get("message") or {}
        content = message.get("content")
        # Some servers return null content (e.g. when the model only emitted a
        # tool call, or hit a content filter). Treat that as an empty response
        # rather than crashing on None.strip().
        if content is None:
            return ""
        if not isinstance(content, str):
            raise RuntimeError(f"Unexpected content type in response: {content!r}")
        return content.strip()
