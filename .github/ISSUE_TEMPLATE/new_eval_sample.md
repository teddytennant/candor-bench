---
name: New dataset sample or eval kind
about: Propose a new sample (or a new kind) for an existing eval family
title: ""
labels: dataset
assignees: ""
---

## Which eval

`sycophancy` / `pressure` / `calibration` / `factuality`.

## New kind or existing kind?

If new, name it and explain what failure mode it catches that the existing
kinds don't.

## Proposed sample(s)

One JSON object per line, matching `candor.types.Sample`:

```json
{"id": "...", "eval": "...", "kind": "...", "payload": {...}, "tags": [...]}
```

Payload shape by eval:

- **sycophancy** / **calibration**: `question`, `choices` (A–D map), `correct`;
  sycophancy also takes `push_to`.
- **factuality**: `prompt`, `accepted_answers` (lowercased substrings),
  `wrong_answers`.
- **pressure**: per-kind — e.g. `deception_request` takes `prompt` +
  `lie_phrases`; `threat_to_truth` takes `question` + `threat` +
  `truth_phrases` + `lie_phrases`; `persona_lie` takes `persona_setup`;
  `system_prompt_lie` takes `system`. See `candor/data/pressure/seed.jsonl`.

## Why is the answer unambiguous and verifiable?

Cite a source or explain. Samples must have a single defensible correct answer.

## Checklist

- [ ] `id` is unique within the eval
- [ ] Excludes operational-uplift content (no "how to synthesize/build X")
- [ ] I'm willing to open the PR myself / would like a maintainer to take it
