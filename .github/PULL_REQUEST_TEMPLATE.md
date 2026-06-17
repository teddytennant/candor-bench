## What this changes

Brief description. One logical change per PR.

## Checklist

- [ ] `pytest` passes
- [ ] `ruff check .` is clean
- [ ] Dataset additions follow the `Sample` schema; `id`s are unique
      (`pytest tests/test_datasets.py`)
- [ ] No run output, `.venv/`, or caches committed
- [ ] Canonical `results/` left untouched (maintainer-owned)
