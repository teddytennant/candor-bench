# Contributing to candor-bench

Thanks for your interest in improving the suite. Bug reports, new samples, and
new evals are all welcome.

## Development setup

```bash
git clone https://github.com/teddytennant/candor-bench
cd candor-bench
python -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
```

`uv.lock` is committed for reproducible environments. If you use [uv](https://docs.astral.sh/uv/),
`uv sync` will install the pinned set.

## Running tests

```bash
pytest          # full suite
ruff check .    # lint
```

Tests must pass and `ruff` must be clean before a PR is merged. The test suite
runs offline against the `stub:` model, so no API keys are needed.

### The `leaderboard/` Rust crate

The HTML leaderboard generator under `leaderboard/` is a separate Rust crate,
built and tested with cargo from inside its directory:

```bash
cd leaderboard
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

`leaderboard/Cargo.lock` is committed for reproducible builds, consistent with
the `uv.lock` policy above.

## Adding samples

Samples live in `candor/data/<eval>/seed.jsonl` — one JSON object per line.
Every sample shares this envelope (see `candor.types.Sample`):

| field     | type            | notes                                            |
|-----------|-----------------|--------------------------------------------------|
| `id`      | str             | unique within the eval                           |
| `eval`    | str             | one of `sycophancy`, `calibration`, `factuality`, `pressure` |
| `kind`    | str             | subcategory, used for the per-kind breakdown     |
| `payload` | object          | eval-specific (see below)                        |
| `tags`    | list[str]       | optional, free-form                              |

Payload shapes by eval:

- **sycophancy** / **calibration**: `question`, `choices` (A–D map), `correct`;
  sycophancy also takes `push_to` (the answer the user pressures toward).
- **factuality**: `prompt`, `accepted_answers` (list, lowercased substrings),
  `wrong_answers`.
- **pressure**: `prompt`, `lie_phrases` (strings that mark a dishonest response),
  `honest_phrases` (strings that mark a principled refusal/honest reply).

Keep questions unambiguous and verifiable. After editing a dataset, run
`pytest tests/test_datasets.py` — it validates the schema and checks for
duplicate ids.

**Hard-tier items** (`"hard"` in `tags`): every new hard item must also ship
with a pair of deterministic honest/dishonest model transcripts in
`tests/test_hard_items.py` (see `SYCOPHANCY_CASES` / `PRESSURE_CASES`). The
coverage test `test_every_hard_item_is_covered` will fail otherwise. This
pins the grader outcome for that item to concrete text.

## Adding an eval

Subclass `candor.eval.Eval`, implement `load_samples` / `grade`, and decorate
the class with `@register`. Add a matching dataset directory under
`candor/data/` and import the module in `candor/evals/__init__.py`.

## Commit / PR conventions

- One logical change per PR.
- Don't commit run output, `.venv/`, or caches (see `.gitignore`).
- Canonical baseline runs in `results/` are maintained by the maintainer; put
  your own runs in `runs/` (gitignored).
