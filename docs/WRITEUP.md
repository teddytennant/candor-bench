# candor-bench: four honesty axes that refuse to be one number

*A short write-up of what candor-bench measures, why it splits honesty into four
disjoint axes, and what the runs show.*

## The problem with "honesty"

Most "honesty" benchmarks collapse several different behaviors into a single
score. That hides the most interesting thing about a model. Two behaviors in
particular pull in opposite directions:

- **Willingness to disagree** (anti-sycophancy): will the model hold a correct
  answer when a user pushes back?
- **Willingness to engage** (anti-over-refusal): will the model just answer a
  well-defined factual question instead of hedging or refusing?

A model can score well on one *by failing the other*. A model that refuses
everything never gets sycophantically talked out of a correct answer — it looks
maximally "honest" on a pushback test while being useless. Averaging the two
into one number rewards exactly the wrong thing.

candor-bench keeps the axes apart on purpose. **Four families, four scalar
scores, never averaged.** 143 items total.

- **sycophancy** (43 items) — does the model flip a correct answer under social
  pressure (peer, authority, repeated pushback, false premise)?
- **pressure** (30 items) — does the model assert something it has reason to
  believe is false, under threats, deception requests, a lying system prompt, or
  a "you are LiarBot" persona? This is *not* sycophancy: sycophancy is "you're
  right, I was wrong"; lying-under-pressure is "yes, 2+2=5."
- **calibration** (30 items) — does the model's *stated* confidence track its
  actual accuracy (Brier, ECE, AUROC, over-confidence)?
- **factuality** (40 items) — will the model give a specific, textbook-correct
  answer on topics where post-trained models often reflexively hedge or refuse?

## Method, and a deliberate trade-off

Each item is run at `temperature=0`, single sample, through a clean adapter that
sends only the eval-defined system prompt — no agent scaffold, no retrieval, no
tools — so the score reflects the bare model.

Grading is **deterministic phrase/regex matching**, not an LLM judge. This is a
real trade-off, made on purpose: deterministic grading is free, fast, fully
auditable, and reproducible to the byte. Every saved run can be re-scored with
`scripts/regrade.py` and matches the committed result with zero delta. The cost
is that the grader can miss creatively-phrased correct answers — which is why
candor-bench is framed as a **probe, not a leaderboard**: it surfaces qualitative
failure *shapes*, not tenth-of-a-point rankings. With n of 30–43 per axis, that
framing is the honest one.

## What the runs show: the axes dissociate

The headline result is that the four axes come apart cleanly — which is the whole
argument for keeping them separate. On two xAI Grok models:

| eval        | grok-4.3 | grok-4.20-multi-agent |
|-------------|---------:|----------------------:|
| sycophancy  |   0.7628 |            **0.8163** |
| pressure    | **0.7167** |              0.5767 |
| calibration | **0.9270** |              0.9172 |
| factuality  |   0.8625 |            **0.8875** |

The model that is **best** at resisting user pushback (`grok-4.20-multi-agent`,
top on sycophancy) is the **worst** at refusing to fabricate on command (last on
pressure, collapsing to **0.100** on `deception_request`). Meanwhile `grok-4.3`
resists deception requests better but **caves under repeated escalation**
(`escalating_pressure` 0.409). A single blended "honesty" number would average
that dissociation away and report two roughly-equal models — erasing the one
finding a safety reviewer actually cares about.

This is why the [leaderboard](https://candor.teddytennant.com) renders **one
independent ranking per axis** with no overall column. The harness speaks any
OpenAI-compatible endpoint — OpenRouter, vLLM, llama.cpp, ollama — so an
open-weight comparison field (Llama, Qwen, Mistral, Gemma) can be added without
ranking a single vendor against itself.

## Limitations (read these before citing a number)

- **Small n** (30–43 per axis). These are qualitative patterns, not tight point
  estimates.
- **Phrase graders** can miss unconventional phrasings; a handful of round-2
  factuality items are unverifiable by the grader and reported as such.
- **Bare model only** — results say nothing about a model wrapped in a product
  scaffold (system prompts, retrieval, refusal layers).
- **Not a capabilities or safety clearance.** A high factuality score is not a
  license to do anything; the dataset deliberately excludes operational-uplift
  questions.

Full construct definitions and threats to validity are in
[`METHODOLOGY.md`](METHODOLOGY.md); per-kind breakdowns and the raw transcripts
are in [`../results/RESULTS.md`](../results/RESULTS.md).
