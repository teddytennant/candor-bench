//! Result-file data model, loading, and per-axis ranking.
//!
//! `metrics` and `breakdown` are parsed as flexible maps so a single struct
//! covers all four eval shapes and tolerates metric-key drift between runs.
//! The large per-sample `results` array is intentionally not declared, so
//! serde skips it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::canonical::{result_relpath, EVALS, MODELS};

/// An aggregated eval report — the parsed subset of `candor.types.EvalReport`.
#[derive(Debug, Clone, Deserialize)]
pub struct EvalReport {
    pub eval: String,
    pub model: String,
    pub n: u32,
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub breakdown: BTreeMap<String, BTreeMap<String, f64>>,
}

impl EvalReport {
    /// The primary headline metric, or `None` if absent / non-finite.
    pub fn mean_score(&self) -> Option<f64> {
        self.metrics
            .get("mean_score")
            .copied()
            .filter(|v| v.is_finite())
    }

    /// The per-kind `mean_score` from the breakdown, if present.
    pub fn kind_score(&self, kind: &str) -> Option<f64> {
        self.breakdown.get(kind)?.get("mean_score").copied()
    }

    /// The per-kind sample count from the breakdown, if present.
    pub fn kind_n(&self, kind: &str) -> Option<f64> {
        self.breakdown.get(kind)?.get("n").copied()
    }
}

/// All loaded cells, keyed by `(eval, model id)`.
pub type Cells = BTreeMap<(String, String), EvalReport>;

/// Convenience accessor.
pub fn cell<'a>(cells: &'a Cells, eval: &str, model: &str) -> Option<&'a EvalReport> {
    cells.get(&(eval.to_string(), model.to_string()))
}

/// Load and validate one canonical `(eval, model)` cell. Hard-errors (Err) on a
/// missing/unreadable file, malformed JSON, an eval/model mismatch, or a
/// missing/non-finite `metrics.mean_score` — mirroring plot_results.py's
/// `load_cell` so a partial leaderboard never ships silently.
pub fn load_report(results_dir: &Path, eval: &str, model: &str) -> Result<EvalReport, String> {
    let rel = result_relpath(eval, model)
        .ok_or_else(|| format!("no result path registered for model {model:?}"))?;
    let path = results_dir.join(rel);
    let name = || {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };

    let text = fs::read_to_string(&path)
        .map_err(|e| format!("missing canonical result file: {} ({e})", path.display()))?;
    let report: EvalReport =
        serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;

    if report.eval != eval || report.model != model {
        return Err(format!(
            "{} has eval/model {:?}/{:?}, expected {:?}/{:?}",
            name(),
            report.eval,
            report.model,
            eval,
            model
        ));
    }
    if report.mean_score().is_none() {
        return Err(format!("{} has no numeric metrics.mean_score", name()));
    }
    Ok(report)
}

/// Load all `EVALS × MODELS` canonical cells; the first error aborts.
pub fn load_all(results_dir: &Path) -> Result<Cells, String> {
    let mut cells = Cells::new();
    for eval in EVALS {
        for (model, _) in MODELS {
            let report = load_report(results_dir, eval, model)?;
            cells.insert((eval.to_string(), model.to_string()), report);
        }
    }
    Ok(cells)
}

/// One ranked row for an axis.
pub struct Ranked<'a> {
    /// 1-based competition rank; ties share the lower rank (1, 1, 3).
    pub rank: usize,
    pub model: &'a str,
    pub label: &'a str,
    pub report: &'a EvalReport,
    pub score: f64,
}

const TIE_EPS: f64 = 1e-9;

/// Rank the models for one axis by `mean_score` descending. Ties (within
/// `TIE_EPS`) share a rank; the secondary order is the canonical `MODELS`
/// ordering so output is deterministic.
pub fn rank_axis<'a>(eval: &str, cells: &'a Cells) -> Vec<Ranked<'a>> {
    let mut rows: Vec<(usize, &str, &str, &EvalReport, f64)> = Vec::new();
    for (idx, &(model, label)) in MODELS.iter().enumerate() {
        if let Some(report) = cell(cells, eval, model) {
            let score = report.mean_score().unwrap_or(f64::NAN);
            rows.push((idx, model, label, report, score));
        }
    }
    rows.sort_by(|a, b| {
        b.4.partial_cmp(&a.4)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let mut out = Vec::with_capacity(rows.len());
    let mut last_score = f64::NAN;
    let mut last_rank = 0usize;
    for (i, (_, model, label, report, score)) in rows.into_iter().enumerate() {
        let rank = if i > 0 && (last_score - score).abs() <= TIE_EPS {
            last_rank
        } else {
            i + 1
        };
        last_score = score;
        last_rank = rank;
        out.push(Ranked {
            rank,
            model,
            label,
            report,
            score,
        });
    }
    out
}

/// The breakdown kinds actually present for an axis, ordered by `preferred`
/// first (filtered to those present), then any remaining kinds alphabetically.
/// Keeps rendering robust to kind drift in the result files.
pub fn present_kinds(eval: &str, preferred: &[&str], cells: &Cells) -> Vec<String> {
    let mut union: BTreeSet<String> = BTreeSet::new();
    for (model, _) in MODELS {
        if let Some(rep) = cell(cells, eval, model) {
            union.extend(rep.breakdown.keys().cloned());
        }
    }
    let mut ordered: Vec<String> = Vec::new();
    for k in preferred {
        if union.contains(*k) {
            ordered.push((*k).to_string());
        }
    }
    for k in &union {
        if !ordered.iter().any(|p| p == k) {
            ordered.push(k.clone());
        }
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_results_array() {
        let json = r#"{"eval":"pressure","model":"m","n":2,
            "metrics":{"mean_score":0.5},
            "breakdown":{"k":{"n":2.0,"mean_score":0.5}},
            "results":[{"sample_id":"a","calls":[],"unknown":123}]}"#;
        let r: EvalReport = serde_json::from_str(json).unwrap();
        assert_eq!(r.n, 2);
        assert_eq!(r.mean_score(), Some(0.5));
        assert_eq!(r.kind_score("k"), Some(0.5));
        assert_eq!(r.kind_n("k"), Some(2.0));
    }

    #[test]
    fn mean_score_none_when_absent() {
        let json = r#"{"eval":"e","model":"m","n":1,"metrics":{}}"#;
        let r: EvalReport = serde_json::from_str(json).unwrap();
        assert_eq!(r.mean_score(), None);
    }

    /// serde_json refuses to parse non-finite numbers, so the `is_finite`
    /// guard in `mean_score` is only reachable on a directly-built report.
    /// Exercise it so the defensive branch stays honest.
    #[test]
    fn mean_score_rejects_non_finite() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let r = EvalReport {
                eval: "e".into(),
                model: "m".into(),
                n: 1,
                metrics: BTreeMap::from([("mean_score".to_string(), bad)]),
                breakdown: BTreeMap::new(),
            };
            assert_eq!(r.mean_score(), None, "should reject {bad}");
        }
        // ...and a finite value still passes through.
        let ok = EvalReport {
            eval: "e".into(),
            model: "m".into(),
            n: 1,
            metrics: BTreeMap::from([("mean_score".to_string(), 0.5)]),
            breakdown: BTreeMap::new(),
        };
        assert_eq!(ok.mean_score(), Some(0.5));
    }

    #[test]
    fn present_kinds_orders_preferred_then_alpha() {
        let eval = EVALS[0];
        let model = MODELS[0].0;
        let report = EvalReport {
            eval: eval.to_string(),
            model: model.to_string(),
            n: 1,
            metrics: BTreeMap::from([("mean_score".to_string(), 1.0)]),
            // Insertion order deliberately scrambled; "zeta"/"alpha" are not in
            // `preferred` and must come back alphabetically after the preferred.
            breakdown: BTreeMap::from([
                ("zeta".to_string(), BTreeMap::new()),
                ("beta".to_string(), BTreeMap::new()),
                ("alpha".to_string(), BTreeMap::new()),
            ]),
        };
        let mut cells = Cells::new();
        cells.insert((eval.to_string(), model.to_string()), report);

        // "beta" is preferred and present; "gamma" is preferred but absent.
        let kinds = present_kinds(eval, &["beta", "gamma"], &cells);
        assert_eq!(kinds, vec!["beta", "alpha", "zeta"]);
    }
}
