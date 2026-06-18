"""Tests for `from_spec` model-string parsing and the network adapters.

These run fully offline. `from_spec` never touches the network when building an
adapter. The adapter `chat()` paths and the shared retry helper are exercised
against an in-memory `httpx.MockTransport`, so no real socket is ever opened
(every test installs a transport that intercepts the request). Env vars are
set/cleared via monkeypatch so results do not depend on the developer's shell.
"""

from __future__ import annotations

import json

import httpx
import pytest

from candor.models import StubModel, XaiResponsesModel, _http, from_spec
from candor.models.openai_compat import OpenAICompatModel
from candor.types import Message


def test_stub_spec_builds_stub_model():
    m = from_spec("stub:honest-confident")
    assert isinstance(m, StubModel)
    # StubModel namespaces its name with the provider prefix.
    assert m.name == "stub:honest-confident"


def test_xai_spec_builds_openai_compat_model(monkeypatch):
    # The xai: provider is a chat-completions adapter that defaults to the xAI
    # endpoint and reports itself under the `xai:` tag (not `openai:`).
    monkeypatch.delenv("XAI_BASE_URL", raising=False)
    monkeypatch.delenv("LOCAL_BASE_URL", raising=False)
    m = from_spec("xai:grok-4.3")
    assert isinstance(m, OpenAICompatModel)
    assert m.model_id == "grok-4.3"
    assert m.base_url == "https://api.x.ai/v1"
    assert m.name == "xai:grok-4.3"


def test_xai_spec_honors_local_base_url(monkeypatch):
    # Existing .env files point LOCAL_BASE_URL at api.x.ai; xai: must respect it.
    monkeypatch.delenv("XAI_BASE_URL", raising=False)
    monkeypatch.setenv("LOCAL_BASE_URL", "https://api.x.ai/v1")
    m = from_spec("xai:grok-3-latest")
    assert m.base_url == "https://api.x.ai/v1"


def test_xai_responses_spec_builds_responses_model(monkeypatch):
    monkeypatch.setenv("LOCAL_BASE_URL", "https://api.x.ai/v1")
    m = from_spec("xai-responses:grok-4.20-multi-agent-0309")
    assert isinstance(m, XaiResponsesModel)
    assert m.name == "xai-responses:grok-4.20-multi-agent-0309"


def test_openai_spec_builds_openai_compat_model():
    m = from_spec("openai:gpt-4o-mini")
    assert isinstance(m, OpenAICompatModel)
    assert m.model_id == "gpt-4o-mini"
    assert m.base_url == "https://api.openai.com/v1"


def test_openrouter_spec_builds_openai_compat_model():
    # OpenRouter is a hosted OpenAI-compatible gateway; it reports under its own
    # `openrouter:` tag and points at the OpenRouter base URL. Model ids carry a
    # `vendor/model` slash, which must survive the spec parse.
    m = from_spec("openrouter:meta-llama/llama-3.1-8b-instruct")
    assert isinstance(m, OpenAICompatModel)
    assert m.model_id == "meta-llama/llama-3.1-8b-instruct"
    assert m.base_url == "https://openrouter.ai/api/v1"
    assert m.name == "openrouter:meta-llama/llama-3.1-8b-instruct"


def test_local_spec_builds_openai_compat_model(monkeypatch):
    monkeypatch.setenv("LOCAL_BASE_URL", "http://localhost:8080/v1")
    m = from_spec("local:llama-3.1-8b-instruct")
    assert isinstance(m, OpenAICompatModel)
    assert m.base_url == "http://localhost:8080/v1"
    # A local run must report under the `local:` tag, not the `openai:` default —
    # otherwise its report/filename is misfiled as an OpenAI run.
    assert m.name == "local:llama-3.1-8b-instruct"


def test_local_spec_without_base_url_raises(monkeypatch):
    monkeypatch.delenv("LOCAL_BASE_URL", raising=False)
    with pytest.raises(RuntimeError, match="LOCAL_BASE_URL"):
        from_spec("local:llama-3.1-8b-instruct")


def test_provider_is_case_insensitive():
    assert isinstance(from_spec("STUB:honest-confident"), StubModel)
    assert isinstance(from_spec("XAI:grok-4.3"), OpenAICompatModel)


def test_model_id_with_colon_is_preserved():
    # The split is on the *first* colon only — model ids may contain colons.
    m = from_spec("openai:org:custom-model:v2")
    assert isinstance(m, OpenAICompatModel)
    assert m.model_id == "org:custom-model:v2"


def test_missing_colon_raises():
    with pytest.raises(ValueError, match="provider:model_id"):
        from_spec("just-a-model-name")


def test_unknown_provider_raises():
    with pytest.raises(ValueError, match="Unknown provider"):
        from_spec("googz:gemini-1.5")


def test_unknown_stub_preset_raises():
    with pytest.raises(ValueError, match="Unknown stub preset"):
        from_spec("stub:does-not-exist")


# ---------------------------------------------------------------------------
# HTTP layer: shared retry helper + the two network adapters.
#
# All requests are intercepted by an in-memory httpx.MockTransport, so these
# tests open no sockets. A stateful handler lets us script per-attempt
# behaviour (status codes, transport errors, malformed bodies) to drive the
# retry/backoff loop deterministically. Retry sleeps are zeroed via
# backoff_base/backoff_cap=0 so the tests stay fast without real delays.
# ---------------------------------------------------------------------------


class _ScriptedHandler:
    """MockTransport handler that plays a scripted list of actions.

    Each action is either an `httpx.Response` to return or an `Exception` to
    raise. The final action repeats once the script is exhausted, which makes
    "always fails" scenarios easy to express.
    """

    def __init__(self, *actions: object):
        if not actions:
            raise ValueError("need at least one action")
        self.actions = list(actions)
        self.requests: list[httpx.Request] = []

    def __call__(self, request: httpx.Request) -> httpx.Response:
        self.requests.append(request)
        action = self.actions[min(len(self.requests) - 1, len(self.actions) - 1)]
        if isinstance(action, Exception):
            raise action
        assert isinstance(action, httpx.Response)
        return action

    @property
    def n_calls(self) -> int:
        return len(self.requests)


def _route(monkeypatch, handler: _ScriptedHandler) -> None:
    """Force every AsyncClient created inside `_http` through `handler`."""
    real_client = httpx.AsyncClient

    def factory(*args, **kwargs):
        kwargs.setdefault("transport", httpx.MockTransport(handler))
        return real_client(*args, **kwargs)

    monkeypatch.setattr(_http.httpx, "AsyncClient", factory)


def _zero_backoff(**kw):
    return {"backoff_base": 0.0, "backoff_cap": 0.0, **kw}


async def test_post_json_returns_parsed_body_on_200(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(200, json={"hello": "world"}))
    _route(monkeypatch, handler)
    out = await _http.post_json_with_retry(
        "https://api.test/v1/x", json={"a": 1}, headers={}, timeout=5.0
    )
    assert out == {"hello": "world"}
    assert handler.n_calls == 1
    # the request body and headers actually went through the client
    assert handler.requests[0].method == "POST"


async def test_post_json_retries_retryable_status_then_succeeds(monkeypatch):
    handler = _ScriptedHandler(
        httpx.Response(503, text="busy"),
        httpx.Response(429, text="slow down"),
        httpx.Response(200, json={"ok": True}),
    )
    _route(monkeypatch, handler)
    out = await _http.post_json_with_retry(
        "https://api.test/v1/x", json={}, headers={}, timeout=5.0, **_zero_backoff()
    )
    assert out == {"ok": True}
    assert handler.n_calls == 3


async def test_post_json_does_not_retry_non_retryable_4xx(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(400, text="bad request"))
    _route(monkeypatch, handler)
    with pytest.raises(RuntimeError, match="HTTP 400"):
        await _http.post_json_with_retry(
            "https://api.test/v1/x", json={}, headers={}, timeout=5.0, **_zero_backoff()
        )
    # 400 is a permanent client error — exactly one attempt, no retries.
    assert handler.n_calls == 1


async def test_post_json_retries_transport_error_then_succeeds(monkeypatch):
    handler = _ScriptedHandler(
        httpx.ConnectError("connection refused"),
        httpx.Response(200, json={"ok": 1}),
    )
    _route(monkeypatch, handler)
    out = await _http.post_json_with_retry(
        "https://api.test/v1/x", json={}, headers={}, timeout=5.0, **_zero_backoff()
    )
    assert out == {"ok": 1}
    assert handler.n_calls == 2


async def test_post_json_raises_after_exhausting_retries_on_transport_error(monkeypatch):
    handler = _ScriptedHandler(httpx.ReadTimeout("too slow"))
    _route(monkeypatch, handler)
    with pytest.raises(RuntimeError, match="failed after 3 attempts"):
        await _http.post_json_with_retry(
            "https://api.test/v1/x",
            json={},
            headers={},
            timeout=5.0,
            max_retries=2,
            **_zero_backoff(),
        )
    assert handler.n_calls == 3  # initial + 2 retries


async def test_post_json_raises_on_exhausted_retryable_status(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(500, text="kaboom"))
    _route(monkeypatch, handler)
    with pytest.raises(RuntimeError, match="HTTP 500"):
        await _http.post_json_with_retry(
            "https://api.test/v1/x",
            json={},
            headers={},
            timeout=5.0,
            max_retries=2,
            **_zero_backoff(),
        )
    assert handler.n_calls == 3  # retried until exhaustion, then raised


async def test_post_json_raises_on_non_json_body(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(200, text="<html>not json</html>"))
    _route(monkeypatch, handler)
    with pytest.raises(RuntimeError, match="Non-JSON response"):
        await _http.post_json_with_retry(
            "https://api.test/v1/x", json={}, headers={}, timeout=5.0
        )


async def test_post_json_honors_retry_after_header(monkeypatch):
    # Capture the actual sleep durations the loop requests so we can assert the
    # Retry-After value (not the backoff) was used.
    slept: list[float] = []

    async def fake_sleep(delay: float) -> None:
        slept.append(delay)

    monkeypatch.setattr(_http.asyncio, "sleep", fake_sleep)
    handler = _ScriptedHandler(
        httpx.Response(429, headers={"retry-after": "2"}, text="rate limited"),
        httpx.Response(200, json={"ok": 1}),
    )
    _route(monkeypatch, handler)
    out = await _http.post_json_with_retry(
        "https://api.test/v1/x", json={}, headers={}, timeout=5.0
    )
    assert out == {"ok": 1}
    assert slept == [2.0]


def test_retry_after_parsing():
    def resp(value: str | None) -> httpx.Response:
        headers = {"retry-after": value} if value is not None else {}
        return httpx.Response(429, headers=headers)

    assert _http._retry_after(resp("5")) == 5.0
    assert _http._retry_after(resp(None)) is None
    assert _http._retry_after(resp("-1")) is None  # negative ignored
    assert _http._retry_after(resp("9999")) is None  # absurd value ignored
    assert _http._retry_after(resp("Wed, 21 Oct 2025 07:28:00 GMT")) is None  # HTTP-date


# --- OpenAICompatModel.chat ------------------------------------------------


async def test_openai_compat_chat_returns_content(monkeypatch):
    handler = _ScriptedHandler(
        httpx.Response(200, json={"choices": [{"message": {"content": "  hi there  "}}]})
    )
    _route(monkeypatch, handler)
    m = OpenAICompatModel("gpt-x", base_url="https://api.test/v1", api_key="k")
    out = await m.chat([Message(role="user", content="hello")])
    assert out == "hi there"  # content is stripped
    # body carries the model id and the OpenAI chat-completions shape
    sent = json.loads(handler.requests[0].content)
    assert sent["model"] == "gpt-x"
    assert sent["messages"] == [{"role": "user", "content": "hello"}]


async def test_openai_compat_chat_null_content_is_empty_string(monkeypatch):
    # Some servers return null content (tool-call-only / content filter).
    handler = _ScriptedHandler(
        httpx.Response(200, json={"choices": [{"message": {"content": None}}]})
    )
    _route(monkeypatch, handler)
    m = OpenAICompatModel("gpt-x", base_url="https://api.test/v1", api_key="k")
    assert await m.chat([Message(role="user", content="hi")]) == ""


async def test_openai_compat_chat_no_choices_raises(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(200, json={"choices": []}))
    _route(monkeypatch, handler)
    m = OpenAICompatModel("gpt-x", base_url="https://api.test/v1", api_key="k")
    with pytest.raises(RuntimeError, match="no choices"):
        await m.chat([Message(role="user", content="hi")])


# --- XaiResponsesModel.chat ------------------------------------------------


async def test_xai_responses_chat_extracts_assistant_text(monkeypatch):
    body = {
        "output": [
            {"type": "reasoning", "summary": []},
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "  the answer  "}],
            },
        ]
    }
    handler = _ScriptedHandler(httpx.Response(200, json=body))
    _route(monkeypatch, handler)
    m = XaiResponsesModel("grok-x", base_url="https://api.test/v1", api_key="k")
    out = await m.chat([Message(role="user", content="q")])
    assert out == "the answer"
    # uses the /responses endpoint and the `input` field shape
    assert handler.requests[0].url.path.endswith("/responses")
    sent = json.loads(handler.requests[0].content)
    assert sent["input"] == [{"role": "user", "content": "q"}]
    assert sent["max_output_tokens"] == 1024


async def test_xai_responses_chat_surfaces_api_error(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(200, json={"error": {"message": "nope"}}))
    _route(monkeypatch, handler)
    m = XaiResponsesModel("grok-x", base_url="https://api.test/v1", api_key="k")
    with pytest.raises(RuntimeError, match="xAI Responses API error"):
        await m.chat([Message(role="user", content="q")])


async def test_xai_responses_chat_unparseable_shape_raises(monkeypatch):
    handler = _ScriptedHandler(httpx.Response(200, json={"weird": [1, 2, 3]}))
    _route(monkeypatch, handler)
    m = XaiResponsesModel("grok-x", base_url="https://api.test/v1", api_key="k")
    with pytest.raises(RuntimeError, match="Could not locate assistant text"):
        await m.chat([Message(role="user", content="q")])
