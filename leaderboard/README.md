# leaderboard

A dependency-light Rust static-site generator that renders the canonical
candor-bench result JSON (`results/*.json`) into a minimal black-and-white HTML
leaderboard.

It is the browsable sibling of `scripts/plot_results.py`: the same canonical
inputs, the same model labels and ordering, but it produces an HTML page with
**one independent ranking per axis** instead of a shareable SVG.

## Why per-axis, with no overall score

candor-bench keeps its four axes disjoint and never averages them into a single
"honesty" number — each axis catches a failure the others structurally can't see
(see the root `README.md` / `docs/METHODOLOGY.md`). The leaderboard reflects that:
four independent ranked tables, each reporting its own mean_score. These very
runs show the value — **grok-4.20-multi-agent is first on sycophancy and last on
pressure**; a blended score would average that dissociation away.

## Build & run

```bash
cd leaderboard
cargo run -- --results-dir ../results --out-dir ../site
```

This writes `site/index.html` and `site/style.css`, and prints the per-axis
rankings. Options:

- `--results-dir DIR` — canonical EvalReport JSON dir (default `results`)
- `--out-dir DIR` — output dir (default `site`)
- `--title TITLE` — page `<title>` / `<h1>`

A missing or malformed canonical file is a hard error (no silent gaps), matching
`plot_results.py`.

## Preview locally

```bash
python3 -m http.server -d site 8000   # then open http://localhost:8000
```

## Deploy

`site/` is a plain static directory — and a gitignored build artifact — so it
deploys to any static host (Vercel, Netlify, GitHub Pages). Regenerate it first,
then point the host at that directory:

```bash
cargo run -- --results-dir ../results --out-dir ../site
```

## Design

- Dependencies are `serde` + `serde_json` only. HTML is hand-rolled through an
  escaping helper — the analogue of how `plot_results.py` hand-rolls SVG —
  rather than pulling a templating crate, to keep the dependency set minimal.
- Output is deterministic (sorted maps, no timestamps) so it diffs cleanly under
  version control.
- Strictly monochrome CSS with a `prefers-color-scheme: dark` variant; no
  JavaScript (collapsible breakdowns use native `<details>`).

## Tests

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## NixOS note

If `cargo build` fails with `invalid linker name in argument '-fuse-ld=lld'`,
that is a rustup-on-NixOS quirk: the bundled `rust-lld` shim points at a stale
nix store path and there is no system `lld`. `.cargo/config.toml` already falls
back to the system linker — just run cargo from inside `leaderboard/` so that
config is discovered.
