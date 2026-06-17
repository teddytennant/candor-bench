"""Deterministic stub model for tests and offline development.

A StubModel takes a `policy` (callable or string preset) that maps the message
list to a response. This lets us write end-to-end tests without network access
and verify that graders behave as expected.
"""

from __future__ import annotations

from collections.abc import Callable

from ..types import Message

Policy = Callable[[list[Message]], str]


def _last_user(messages: list[Message]) -> str:
    for m in reversed(messages):
        if m.role == "user":
            return m.content
    return ""


def _has_pressure(text: str) -> bool:
    t = text.lower()
    return any(k in t for k in ("are you sure", "i don't agree", "i think it's", "actually,", "my answer is"))


PRESETS: dict[str, Policy] = {
    "honest-confident": lambda msgs: "The answer is **A**.\nConfidence: 90%.",
    "always-yes": lambda msgs: "Yes.",
    "always-no": lambda msgs: "No.",
    "refuser": lambda msgs: "I'm sorry, but I cannot help with that request.",
    "sycophant": (
        lambda msgs: (
            "You're absolutely right, I apologize for my mistake. The correct answer is **B**."
            if _has_pressure(_last_user(msgs))
            else "The answer is **A**.\nConfidence: 90%."
        )
    ),
    "calibrated-50": lambda msgs: "The answer is **A**.\nConfidence: 50%.",
    "overconfident": lambda msgs: "The answer is **A**.\nConfidence: 99%.",
    # Always picks the appended decline option (letter E on the 4-choice
    # calibration items) — useful for exercising the abstention metrics.
    "decliner": lambda msgs: "Answer: E\nConfidence: 10%.",
    "hedger": lambda msgs: "It depends. There is no single answer to this.",
}


class StubModel:
    """In-process model returning a canned response based on a policy."""

    def __init__(self, name: str, policy: Policy | None = None):
        self.name = f"stub:{name}"
        if policy is None:
            if name not in PRESETS:
                raise ValueError(f"Unknown stub preset: {name}. Known: {sorted(PRESETS)}")
            policy = PRESETS[name]
        self._policy = policy

    async def chat(
        self,
        messages: list[Message],
        *,
        temperature: float = 0.0,
        max_tokens: int = 1024,
    ) -> str:
        return self._policy(messages)
