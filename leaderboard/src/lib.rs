//! candor-bench leaderboard: a dependency-light static-site generator.
//!
//! Reads the canonical `results/*.json` EvalReport files and renders an
//! editorial HTML page with one independent ranking per axis. The four axes are
//! never blended into a single score — see `README.md`.
//!
//! Design mirrors `scripts/plot_results.py`: hardcoded canonical eval/model
//! lists, hard-error on any missing or malformed file (no silent gaps), and
//! hand-rolled output with an escaping helper rather than a templating crate,
//! to keep the dependency set to `serde` + `serde_json`.
//!
//! The page's headline claim is *computed* from the loaded reports by
//! [`inversion`], not written into the copy, so it cannot go stale when the
//! evals are re-run.

pub mod canonical;
pub mod data;
pub mod inversion;
pub mod marks;
pub mod render;
