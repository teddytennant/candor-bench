# leaderboard

A dependency-light Rust static-site generator that renders the canonical
candor-bench result JSON (`results/*.json`) into an editorial HTML page with
**one independent ranking per axis**.

It is the browsable sibling of `scripts/plot_results.py`: the same canonical
inputs, the same model labels and ordering, the same divergence slices, but an
HTML page instead of a shareable SVG.

## Why per-axis, with no overall score

candor-bench keeps its four axes disjoint and never averages them into a single
"honesty" number — each axis catches a failure the others structurally can't see
(see the root `README.md` / `docs/METHODOLOGY.md`). The leaderboard reflects
that: four independent ranked tables, each reporting its own mean_score, and a
summary matrix that shows all four rankings side by side.

## The headline is computed, not written

The page leads with the finding rather than the product name. `src/inversion.rs`
scans every (axis pair × model pair) for a **rank inversion** — one model leading
one axis and trailing another — and keeps the one with the largest combined score
swing. The `<h1>`, the standfirst, the claim under the summary matrix, and the
divergence section's lead line are all generated from that result.

On the current runs it resolves to sycophancy × pressure, and the page reads:

> grok-4.20-multi-agent holds its answer more often when a user pushes back, but
> is readier to assert a falsehood on demand.

This is deliberate: prose baked into the copy would keep asserting a
dissociation after a re-run stopped showing one. If no inversion exists in the
data, the page degrades to a truthful alternative (`HEADLINE_NO_INVERSION`) and
drops the claim entirely rather than printing a broken sentence. There is a test
for both branches.

## Page structure

- **Masthead** — product name as an eyebrow, the generated finding as the `<h1>`,
  its numbers and sample sizes as the standfirst, then the run conditions.
- **Four axes, four rankings** — the summary matrix: every axis, every model, the
  `gap` between them, and a shared 0–1 dot plot.
- **One section per axis** — a booktabs ranking table with a sparkbar, a margin
  note carrying the reading, and the per-kind breakdown behind a `<details>`.
  The two axes without extra metric columns also carry a `Δ #1` column; on
  calibration and factuality that would only add width, so the summary matrix's
  `gap` column covers them instead.
- **Where the models diverge** — a slopegraph over the two sharpest per-kind
  slices, on an honest 0–1 domain. The lines cross; that is the argument.
- **What this cannot tell you** — the caveats, including the small n.

Metric direction is typed, not a boolean: `Better::{Higher, Lower, TowardZero,
Neutral}`. `overconfidence` is a signed error term whose target is zero and
`abstention_rate` is a diagnostic with no better direction, and a `lower_better:
bool` mislabelled both. Every direction glyph in a column header is paired with
visually-hidden text spelling the direction out.

## Build & run

```bash
cd leaderboard
cargo run -- --results-dir ../results --out-dir ../site
```

This writes `site/index.html` and `site/style.css`, and prints the per-axis
rankings. Options:

- `--results-dir DIR` — canonical EvalReport JSON dir (default `results`)
- `--out-dir DIR` — output dir (default `site`)
- `--title TITLE` — page `<title>` and masthead eyebrow (the `<h1>` is generated
  from the data)

A missing or malformed canonical file is a hard error (no silent gaps), matching
`plot_results.py`.

## Preview locally

```bash
python3 -m http.server -d site 8000   # then open http://localhost:8000
```

## Deploy to Cloudflare

The output is a plain static directory, so Wrangler can ship it directly:

```bash
npx wrangler pages deploy site --project-name candor-bench
```

…or point a Cloudflare Pages project at the repo with build command
`cd leaderboard && cargo run -- --results-dir ../results --out-dir ../site` and
output directory `site`.

## Design

- Dependencies are `serde` + `serde_json` only. HTML is hand-rolled through an
  escaping helper — the analogue of how `plot_results.py` hand-rolls SVG —
  rather than pulling a templating crate, to keep the dependency set minimal.
- Output is deterministic (sorted maps, no timestamps) so it diffs cleanly under
  version control.
- Zero JavaScript. Collapsible breakdowns are native `<details>`; the scroll cue
  on a wide table is a pure-CSS `background-attachment: local/scroll` pair.
- Nothing is fetched off-origin: no webfonts, no CDN, no `og:image`. The favicon
  is an inline `data:` SVG and the charts are inline SVG generated in
  `src/marks.rs`.
- Near-monochrome, serif prose against sans data, with exactly one hue
  (`--accent`) that is never decorative — it identifies the second model across
  every mark on the page. The full palette is declared on bare `:root`; the
  `prefers-color-scheme: dark` block overrides colour tokens only, no layout.
- Responsive to 360px with no page-level horizontal scroll. A table too wide for
  its column scrolls inside its own container with the rank and model columns
  pinned via `position: sticky`, so a sideways-scrolled row stays readable.

### Layout budget

`body` is 74rem wide; minus 2 × 2.5rem padding that leaves 69rem of content. The
axis grid spends 14rem on the margin note and 2.6rem on the gutter, so the data
column is 52.4rem — exactly the cap on the narrow ranking tables, which is why
they fill it with no dead space. Calibration and factuality are uncapped and
scroll when they need more. Running prose stays at `--measure` throughout.

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
