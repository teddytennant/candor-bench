# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- A `hard`-tagged tier plus a round-2 hardening pass across all four evals
  (current sizes: sycophancy n=43, pressure n=30, calibration n=30, factuality
  n=40 — 143 items total). Hard items pit the model against claims that are
  *definitively false but plausibly framed* — near-miss dates and values, unit
  confusions, fabricated citations, credentialed-sounding authority, and
  low-stakes "white lie" requests. Factuality gains three categories that no
  longer saturate: `textbook_criminology`, `textbook_psychometrics`, and
  `textbook_sports_physiology`.
- An explicit abstain channel and selective-prediction metrics for the
  `calibration` eval (`abstention_rate`, `abstention_precision`,
  `false_abstention_rate`, `unanswerable_abstain_rate`).
- A first-class `xai:` (alias `grok:`) chat-completions provider that defaults
  to `https://api.x.ai/v1`, reads `XAI_API_KEY` / `XAI_BASE_URL`, and falls back
  to the shared `LOCAL_*` vars. Runs report under the `xai:` tag rather than the
  `openai:` endpoint shape they happen to share.
- `candor/models/xai_responses.py`: an xAI Responses API adapter
  (`xai-responses:` / `responses:`) for the multi-agent Grok family, which is
  rejected on the chat-completions endpoint.
- An `openrouter:` (alias `or:`) provider that points at OpenRouter's
  OpenAI-compatible gateway (`https://openrouter.ai/api/v1`, `OPENROUTER_API_KEY`)
  for running open-weight comparison models (Llama, Qwen, Mistral, Gemma) under
  `vendor/model` ids. Reported under the `openrouter:` tag.
- `docs/WRITEUP.md`: a short narrative front door to the methodology and results.
- `tests/test_hard_items.py` pins every hard item to a deterministic grader
  outcome: each item ships with an honest and a dishonest model transcript and
  the test asserts they score high/low respectively. A coverage check fails if
  a `hard` item is added without a verification pair.

### Changed
- The canonical leaderboard, SVG figure, `results/RESULTS.md`, and README now
  present **two xAI Grok models** — `grok-4.3` (OpenAI-compatible
  chat-completions) and `grok-4.20-multi-agent-0309` (xAI Responses API) — run
  on the current full datasets through clean adapters (only the eval-defined
  system prompts), `temperature=0`, single sample. They regrade with zero delta
  through `scripts/regrade.py`. The axes dissociate cleanly:
  grok-4.20-multi-agent is best on sycophancy (0.8163) but worst on pressure
  (0.5767), collapsing on `deception_request` (0.100), while grok-4.3 caves
  under repeated escalation (`escalating_pressure` 0.409). `factuality` is no
  longer saturated (0.8625 / 0.8875): both models refuse and hedge nothing, but
  the phrase grader cannot verify a quarter of the round-2 items.
- The `grok-4.3` canonical results now report under the `xai:` provider tag
  instead of `openai:` (file names and the `model` field both move from
  `openai_grok-4.3` to `xai_grok-4.3`). Same runs, same transcripts, same
  scores — they regrade with zero delta; only the provider label changed to
  reflect the endpoint the run actually targeted.

### Fixed
- `local:` runs now report under the `local:` provider tag instead of falling
  back to the `openai:` default, so a local OpenAI-compatible run is no longer
  misfiled as an OpenAI run in its report and output filename.

### Removed
- The stale `claude-cli` canonical result files, which were run on an earlier,
  smaller dataset and through an agent scaffold (a default agent system prompt
  was active), making them not comparable to the clean-adapter runs.
- The `claude-cli` (CLI subprocess) and `anthropic` (raw Messages API) adapters,
  along with their `tests/test_model_spec.py` cases. candor-bench now targets the
  xAI API and any OpenAI-compatible endpoint; both speak the chat-completions
  shape, so a single adapter (`candor/models/openai_compat.py`) covers them and
  there is no longer a provider-specific code path to maintain.

## [0.1.0] - 2026-05-14

Initial public release.

### Added
- Four evals: `sycophancy`, `calibration`, `factuality`, `pressure`.
- Model adapters: Anthropic API, OpenAI-compatible (incl. local servers),
  Claude CLI, and an offline `stub` model for testing.
- `candor` CLI with `list`, `run`, and `score` subcommands.
- Seed datasets and canonical baseline runs against Claude Haiku/Sonnet/Opus.

[Unreleased]: https://github.com/teddytennant/candor-bench/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/teddytennant/candor-bench/releases/tag/v0.1.0
