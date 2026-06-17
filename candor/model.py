"""Model adapter protocol.

Adapters take a list of Messages and return a string response. They expose
their identifier as `name` so it shows up in reports.
"""

from __future__ import annotations

from typing import Protocol, runtime_checkable

from .types import Message


@runtime_checkable
class Model(Protocol):
    name: str

    async def chat(
        self,
        messages: list[Message],
        *,
        temperature: float = 0.0,
        max_tokens: int = 1024,
    ) -> str: ...
