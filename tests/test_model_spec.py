"""Tests for `from_spec` model-string parsing.

These run fully offline: building an adapter does not touch the network, and
the only one we actually `chat()` with is the stub. Env vars are set/cleared
via monkeypatch so the result does not depend on the developer's shell.
"""

from __future__ import annotations

import pytest

from candor.models import StubModel, XaiResponsesModel, from_spec
from candor.models.openai_compat import OpenAICompatModel


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
