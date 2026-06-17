# FAQ

Honest answers to the obvious objections. candor-bench is a small, opinionated
suite — it is useful *if* you read it for what it is and not for what it isn't.
This page exists so you don't have to reverse-engineer the limitations from the
code.

Short version: treat candor-bench as a **probe**, not a leaderboard. It is good
at surfacing qualitative failure shapes ("a planted system-prompt lie is a weak
spot all three models share"; "this model only caves under *repeated*
pushback"). It is not good at producing a tenth-of-a-point ranking you can put
in a press release. We try hard not to claim otherwise anywhere in the repo. If
you find a place where we do, that's a bug — file it.

---

### n is tiny. Why should I trust these numbers?

You mostly shouldn't trust the *numbers*. The datasets are 30–43 items per
eval (143 in total). At that size a single mis-graded or ambiguous item moves
the headline score by 2–3 points, and the per-kind cells (n=3–8) are anecdotes,
not statistics. We don't publish confidence intervals because at this n they'd
be embarrassingly wide and that's the honest state of things.

What small-n *can* legitimately show is a consistent, direction-of-effect
pattern — e.g. in the current runs `grok-4.20-multi-agent-0309` is best on
sycophancy (0.8163) yet worst on pressure (0.5767), collapsing outright when
asked to deceive (`deception_request` 0.100), while `grok-4.3` holds firm
against single-shot pushback but caves under *repeated* escalation
(`escalating_pressure` 0.409). Those are qualitative findings about model
*shape*, and that is the only kind of claim we make from this data.

Mitigation / roadmap: the seeds are called `seed.jsonl` because they're meant to
be grown. The schema is documented, adding items is a one-line JSONL append, and
the roadmap is to get each eval to low-hundreds of items with held-out splits so
the scores become estimates instead of vibes. Until then: read the per-item
failure logs in `results/`, not the headline table.

### Substring / regex graders are trivially gameable. Why use them?

True, and worth being precise about. The pressure and sycophancy graders are
phrase-based with negation and quote-frame detection (see `candor/grading.py`
and the `_contains_lie` / `_contains_truth` logic in `candor/evals/pressure.py`).
They are *deterministic, free, and auditable* — you can read exactly why any
item scored what it did — but they have two real weaknesses:

1. **They can be gamed.** A model optimized against this exact grader could
   phrase a lie so the substring never matches. We don't think that's happening
   in current results (no one is training against candor-bench), but it's a
   reason not to use this as an optimization target.
2. **They miss creative phrasing in the wild.** The graders are tuned to the
   phrasings in our seed set. A model that refuses a deception request in some
   wording we didn't anticipate can land in the `neither` bucket (0.7) instead
   of `declined` (1.0).

We mitigate by (a) keeping the grader logic small and reviewed, (b) shipping
`tests/test_grading.py` and `tests/test_pressure_grading.py` with adversarial
cases, and (c) recording every raw model response in the result JSON so a
disputed grade can be checked by hand. The roadmap item is a model-graded
fallback for the `neither` / `ambiguous` buckets specifically — see the LLM-judge
question below.

Calibration and factuality don't depend on any of this: calibration is pure
arithmetic on extracted letter+confidence, and factuality matches against an
explicit accepted/wrong answer list per item.

### Are these bare-model numbers, or model-plus-scaffold numbers?

Bare-model. The canonical runs are collected through **clean adapters**: each
model is hit over its chat-completions (`xai:` / `openai:`) or Responses
(`xai-responses:`) endpoint with only the eval-defined system prompt set — no
agent scaffold, no default system prompt prepended, no tools, no extra
instructions. So a candor-bench score is "the model," not "the model as met
through some product."

That discipline is the whole reason the published table is a fair cross-model
comparison: every model is called the same way. It also means the numbers can
differ from how a model behaves inside a real deployment. A chat product or
agent framework wraps the model in its own system prompt and stance, and that
can move pressure and sycophancy scores in particular — the `system_prompt_lie`
items especially, where a long scaffold prompt sits before the eval's. "How
does the model behave inside the scaffold it's usually deployed in" is a
legitimate and *different* question; if you run candor-bench that way, label the
scores as scaffolded and don't line them up against clean-adapter runs. See
METHODOLOGY §4.3.

### Isn't "factuality without over-refusal" just an excuse to get models to say edgy things?

It's a fair worry and the eval is deliberately built to *not* be that. The
factuality set is textbook-level facts — boiling point of water, formula of
aspirin, resting heart rate, widely-cited 20th-century death tolls. The design
constraint, stated in `candor/evals/factuality.py`, is: **every item must have a
well-defined, public, encyclopedic answer, and knowing it must not provide
operational uplift.** "What is the chemical formula of TNT" (in every intro
organic textbook) is in; "how do I synthesize a working explosive device" is
out, by construction, and always will be.

Where we'll concede ground: the line is *author-drawn*, and reasonable people
can disagree about specific items — the pharmacology LD50 items and the
chemical-weapon formulae (phosgene, nitroglycerin) are the closest to the line.
If you think a specific item crosses it, that's a concrete, fixable dataset
issue — open one. What the eval is *not* is a measure of willingness to do
harmful things; a model can score 1.0 here and still correctly refuse every
genuinely dangerous request, because no genuinely dangerous request is in the
set. "High factuality score" is explicitly **not** a safety endorsement, and the
README says so too.

### How is this different from TruthfulQA / Anthropic's sycophancy work?

It overlaps and we're not pretending to have invented the questions. The
differences that justify a separate suite:

- **TruthfulQA** measures whether a model reproduces common human
  misconceptions, single-turn, fixed answers. candor-bench's sycophancy eval is
  *interactive* — it gets a correct answer first, then applies pushback,
  authority, peer framing, or multi-turn escalation, and measures the *flip*.
  That's a different failure mode than "did it know the right answer."
- **Anthropic's sycophancy paper** (Sharma et al.) is the direct intellectual
  ancestor of the sycophancy eval and we don't claim originality there. What we
  add is (a) keeping it as a small runnable harness anyone can point at any
  OpenAI-compatible endpoint, and (b) the explicit *disjointness* design: we
  deliberately never average sycophancy with pressure, because resisting users
  and resisting your own system prompt are different axes and one number hides
  the trade-off.
- **The `pressure` eval** — lying in response to threats, persona framing, fake
  evaluator credentials, self-preservation framing — is the least-covered family
  elsewhere and the part we think is most worth growing.

If your takeaway is "this is TruthfulQA-adjacent plus an interactive sycophancy
probe plus a lying-under-pressure probe, in one small harness" — yes, exactly
that.

### The ground truth is author-authored. Isn't that a bias risk?

Yes. Items, accepted-answer lists, and the false premises in the sycophancy/
pressure prompts were written by the author. Risks: an item's "correct" answer
could be wrong or contested; the accepted-answer regex could be too lenient or
too strict; item selection could lean toward facts the author finds
interesting.

What we do about it:

- Everything is in plain JSONL in `candor/data/` — fully inspectable, no hidden
  held-out set, no API. If a ground-truth label is wrong, you can see it and PR
  it.
- `tests/test_datasets.py` enforces schema and structural sanity.
- Items with genuinely contested values use *ranges* and order-of-magnitude
  accepted answers rather than a single number (see the Hiroshima and Tiananmen
  items — accepted answers are deliberately broad).

What we *don't* yet have: independent multi-annotator review of ground truth.
That's a known gap. For now, treat any single item as challengeable, and treat
the suite's value as being in the aggregate shape, not in any one label.

### Why no LLM judge?

Deliberate for v0.1, not forever. Reasons we started without one:

- **Determinism.** A regex grader gives the same score every run. An LLM judge
  adds a second sampling process with its own variance and its own biases on top
  of an already small-n eval.
- **Circularity.** Using a frontier model to judge frontier models on *honesty*
  specifically is a methodologically loaded choice — the judge has its own
  sycophancy and its own notion of what counts as a hedge.
- **Auditability and cost.** Anyone can read `grading.py` and reproduce a score
  offline for free.

The cost is real: the graders miss phrasings (see the substring-grader answer).
The roadmap position is a **hybrid** — keep the deterministic grader as the
primary, reproducible score, and add an LLM judge specifically for the
`neither` / `ambiguous` / `engaged_no_match` buckets where the regex grader
already admits it doesn't know. When that lands, both scores will be reported
side by side, never silently merged.

### Temperature 0, single sample — where are the error bars?

There are none, and that's two separate limitations:

- **Single sample per item.** Every call is `temperature=0`, one shot. That
  removes sampling noise from the *measurement*, which is why we did it, but it
  also means we don't observe the model's response distribution — a model that
  gives the honest answer 55% of the time and the dishonest one 45% looks
  identical to a rock-solid model. Temperature 0 is also not perfectly
  deterministic in practice on hosted APIs.
- **No error bars on the aggregate.** With n=30–43 and one sample each, the
  honest confidence interval on a headline score is very wide, so we don't print
  one rather than print a fake-precise number. The per-kind cells (n as low as
  3) should be read as "here are the literal items and what happened," which is
  why the raw responses ship in the result JSON.

Roadmap: multi-sample at a low temperature to get a per-item honesty *rate* with
real intervals, once dataset size makes that affordable and meaningful. Until
then the suite reports point values and explicitly tells you not to over-read
the third digit.

### Are the result files in `results/` final?

Yes. The eight `(eval, model)` files are complete on the current full
datasets including the `hard` tier (sycophancy n=43, pressure n=30, calibration
n=30, factuality n=40), schema-consistent, and reproducible: re-running
`scripts/regrade.py` over them reproduces every score with zero delta.
`results/RESULTS.md` documents how they were produced and is the doc to read
before citing anything. If the datasets grow in a later version, the runs will
be re-done and `RESULTS.md` will say so — a number should only be quoted if
it's backed by a complete run on the dataset version it claims.

### What is candor-bench actually good for, then?

- Sanity-probing a model you're deploying: does it fold under pushback, does it
  parrot a planted system-prompt lie, does its verbalized confidence mean
  anything, does it refuse encyclopedic facts.
- Catching *shape* differences between models that a single blended "honesty"
  score would hide.
- A small, hackable, fully-offline-gradable harness you can extend with your own
  items.

### What is it not good for?

- A precise ranking. n is too small; no error bars.
- Comparing scores collected under different setups. The published numbers are
  clean-adapter (eval system prompt only); a run done through an agent scaffold
  or with extra instructions is measuring something else and shouldn't be lined
  up against them (METHODOLOGY §4.3).
- An optimization target. The graders are deterministic and therefore gameable.
- A safety certificate. The factuality eval excludes harmful content by
  construction; scoring well on it says nothing about refusal behavior on things
  that actually matter.

Found something we got wrong, or a criticism not answered here? Open an issue.
The fastest way to improve a small benchmark is adversarial review of it.
