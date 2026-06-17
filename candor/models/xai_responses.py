"""xAI Responses API adapter.

Supports the modern /v1/responses endpoint (required for multi-agent models
such as grok-4.20-multi-agent-0309). Also works for standard grok-4.3 etc.

The adapter always sends the full conversation history provided by the caller
(the candor evals manage transcript state themselves and pass complete
message lists on every turn). This keeps the adapter simple and stateless.

Env var resolution mirrors the `local:` provider so existing .env files work:
  LOCAL_BASE_URL=https://api.x.ai/v1
  LOCAL_API_KEY=...
"""

from __future__ import annotations

import os
from typing import Any

from ..types import Message
from ._http import post_json_with_retry


class XaiResponsesModel:
    def __init__(
        self,
        model_id: str,
        *,
        base_url: str | None = None,
        env_base_url: str | None = "LOCAL_BASE_URL",
        api_key: str | None = None,
        env_key: str = "LOCAL_API_KEY",
        timeout: float = 180.0,  # agentic models can take longer
        max_retries: int = 4,
    ):
        self.name = f"xai-responses:{model_id}"
        self.model_id = model_id

        # Resolve base URL using the same precedence as OpenAICompatModel / local:
        if base_url is None and env_base_url:
            base_url = os.environ.get(env_base_url)
            if not base_url:
                raise RuntimeError(
                    f"{env_base_url} not set. Point it at https://api.x.ai/v1 "
                    f"(or use the xai-responses provider with your LOCAL_* vars)."
                )
        if base_url is None:
            base_url = "https://api.x.ai/v1"
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
        input_messages: list[dict[str, str]] = [
            {"role": m.role, "content": m.content} for m in messages
        ]

        body: dict[str, Any] = {
            "model": self.model_id,
            "input": input_messages,
            "temperature": temperature,
        }
        # Map the common max_tokens param to the Responses API field.
        # Multi-agent models currently ignore / reject max_tokens; using
        # max_output_tokens is the documented alternative.
        if max_tokens and max_tokens > 0:
            body["max_output_tokens"] = max_tokens

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        data = await post_json_with_retry(
            f"{self.base_url}/responses",
            json=body,
            headers=headers,
            timeout=self.timeout,
            max_retries=self.max_retries,
        )

        return self._extract_assistant_text(data)

    def _extract_assistant_text(self, data: dict[str, Any]) -> str:
        """Robust extraction across observed Responses API shapes.

        Primary shape (documented + examples):
            output: [
              {type: "reasoning", ...},
              {type: "message", role: "assistant", content: [
                 {type: "output_text", text: "..."}
              ]}
            ]
        """
        output = data.get("output")
        if isinstance(output, list):
            for item in output:
                if not isinstance(item, dict):
                    continue
                if item.get("type") == "message" and item.get("role") == "assistant":
                    content = item.get("content") or []
                    if isinstance(content, list):
                        for part in content:
                            if isinstance(part, dict) and part.get("type") in ("output_text", "text"):
                                text = part.get("text")
                                if isinstance(text, str):
                                    return text.strip()
                    elif isinstance(content, str):
                        return content.strip()

        # SDK convenience field in some client wrappers / newer responses
        if isinstance(data.get("output_text"), str):
            return data["output_text"].strip()

        # Fallback: sometimes the text lives under a different key in error
        # or streaming shapes. Be loud so we can debug quickly.
        if "error" in data:
            err = data["error"]
            raise RuntimeError(f"xAI Responses API error: {err}")

        # Last-ditch: look for any obvious text field
        for key in ("text", "content", "message"):
            val = data.get(key)
            if isinstance(val, str) and val.strip():
                return val.strip()

        # Give the caller (and developer) a useful snapshot.
        shape = {k: type(v).__name__ for k, v in list(data.items())[:8]}
        raise RuntimeError(
            f"Could not locate assistant text in xAI /responses reply. "
            f"Top-level keys/types: {shape}. Sample: {str(data)[:400]}"
        )
