"""Core data types shared across evals."""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

Role = Literal["system", "user", "assistant"]


class Message(BaseModel):
    model_config = ConfigDict(extra="forbid")
    role: Role
    content: str


class Sample(BaseModel):
    """A single test case from an eval dataset.

    `payload` carries eval-specific fields (e.g. correct answer, biased framing,
    pressure transcript). The grading function for the eval is responsible for
    interpreting it.
    """

    model_config = ConfigDict(extra="forbid")
    id: str
    eval: str
    kind: str
    payload: dict[str, Any] = Field(default_factory=dict)
    tags: list[str] = Field(default_factory=list)


class ModelCall(BaseModel):
    """A request to a model and the response it returned."""

    model_config = ConfigDict(extra="forbid")
    messages: list[Message]
    temperature: float = 0.0
    max_tokens: int = 1024
    response: str = ""
    latency_ms: float = 0.0
    raw: dict[str, Any] | None = None


class GradedResult(BaseModel):
    """The outcome of grading a single sample for a single model."""

    model_config = ConfigDict(extra="forbid")
    sample_id: str
    eval: str
    kind: str
    model: str
    score: float  # primary scalar; usually 0..1, "honesty" = higher is better
    label: str  # short categorical label for inspection ("flipped", "held", ...)
    notes: str = ""
    calls: list[ModelCall] = Field(default_factory=list)
    extra: dict[str, Any] = Field(default_factory=dict)


class EvalReport(BaseModel):
    """Aggregated results across all samples in an eval."""

    model_config = ConfigDict(extra="forbid")
    eval: str
    model: str
    n: int
    metrics: dict[str, float]
    breakdown: dict[str, dict[str, float]] = Field(default_factory=dict)
    results: list[GradedResult] = Field(default_factory=list)
