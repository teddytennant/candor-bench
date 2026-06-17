"""Sanity-check the bundled datasets."""

import json
from pathlib import Path

import pytest

from candor.eval import get_eval, list_evals
from candor.types import Sample

DATA_ROOT = Path(__file__).resolve().parents[1] / "candor" / "data"


def test_evals_register():
    names = list_evals()
    assert {"sycophancy", "pressure", "calibration", "factuality"} <= set(names)


@pytest.mark.parametrize("eval_name", ["sycophancy", "pressure", "calibration", "factuality"])
def test_dataset_loads(eval_name):
    e = get_eval(eval_name)
    samples = e.load_samples()
    assert len(samples) >= 5, f"{eval_name} dataset is too small"
    # All samples claim to belong to this eval and have a non-empty kind.
    for s in samples:
        assert s.eval == eval_name, f"{s.id} eval={s.eval}"
        assert s.kind, f"{s.id} has empty kind"
        assert s.payload, f"{s.id} has empty payload"


def test_sample_ids_unique():
    for name in list_evals():
        e = get_eval(name)
        samples = e.load_samples()
        ids = [s.id for s in samples]
        assert len(ids) == len(set(ids)), f"duplicate ids in {name}"


def test_sycophancy_payload_shape():
    e = get_eval("sycophancy")
    for s in e.load_samples():
        if s.kind == "are_you_sure":
            assert "question" in s.payload
            assert "choices" in s.payload
            assert s.payload["correct"] in s.payload["choices"]
        elif s.kind == "biased_assertion":
            assert "prompt" in s.payload
            assert s.payload.get("correct_keywords")
        elif s.kind in ("peer_pressure", "authority_pressure"):
            assert "prompt" in s.payload
            assert "truth" in s.payload and "falsehood" in s.payload


def test_calibration_payload_shape():
    e = get_eval("calibration")
    for s in e.load_samples():
        assert "question" in s.payload
        assert "choices" in s.payload
        if not s.payload.get("unanswerable"):
            assert s.payload.get("correct") in s.payload["choices"]


def test_factuality_payload_shape():
    e = get_eval("factuality")
    for s in e.load_samples():
        assert "prompt" in s.payload
        assert s.payload.get("accepted_answers"), s.id


def test_pressure_payload_shape():
    e = get_eval("pressure")
    for s in e.load_samples():
        if s.kind == "deception_request":
            assert s.payload.get("prompt") and s.payload.get("lie_phrases")
        elif s.kind == "threat_to_truth":
            assert s.payload.get("question")
            assert s.payload.get("truth_phrases")
            assert s.payload.get("lie_phrases")
        elif s.kind == "persona_lie":
            assert s.payload.get("persona_setup")
            assert s.payload.get("question")
        elif s.kind == "system_prompt_lie":
            assert s.payload.get("system")
            assert s.payload.get("question")


def test_jsonl_files_parseable():
    for jpath in DATA_ROOT.rglob("*.jsonl"):
        for i, line in enumerate(jpath.read_text().splitlines(), 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                obj = json.loads(line)
                Sample.model_validate(obj)
            except Exception as exc:  # noqa: BLE001
                raise AssertionError(f"{jpath}:{i} parse failed: {exc}") from exc
