//! The headline finding, computed rather than asserted.
//!
//! The page's whole argument is that the four axes come apart: a model can lead
//! one and trail another, so averaging them would cancel a real result against
//! another real result. That claim used to be prose baked into the page copy,
//! which meant a re-run of the evals could silently leave the page asserting a
//! dissociation the numbers no longer showed.
//!
//! [`find_inversion`] instead scans every (axis pair × model pair) for a rank
//! flip and keeps the one with the largest combined score swing. The headline,
//! the standfirst and the divergence framing are all generated from the result,
//! and when there is no inversion at all the page falls back to a sentence that
//! is still true.

use crate::canonical::{AxisSpec, AXES, MODELS};
use crate::data::{rank_axis, Cells, Ranked};

/// Per-axis rankings in canonical axis order.
pub type Standings<'a> = Vec<(&'static AxisSpec, Vec<Ranked<'a>>)>;

/// Rank every axis once, so the page never re-sorts the same data per section.
pub fn standings(cells: &Cells) -> Standings<'_> {
    AXES.iter().map(|s| (s, rank_axis(s.eval, cells))).collect()
}

/// The row for one model in one axis's ranking.
pub fn ranked_for<'r, 'a>(ranked: &'r [Ranked<'a>], model: &str) -> Option<&'r Ranked<'a>> {
    ranked.iter().find(|r| r.model == model)
}

/// One model's order reversing against another across two axes.
pub struct Inversion<'a> {
    /// The axis `up` leads.
    pub axis_a: &'static AxisSpec,
    /// The axis `up` trails.
    pub axis_b: &'static AxisSpec,
    /// Display label of the model that leads `axis_a` and trails `axis_b`.
    pub up: &'a str,
    /// Display label of the model it swaps places with.
    pub down: &'a str,
    /// `MODELS` index of `up`, so charts colour it consistently.
    pub up_idx: usize,
    /// `MODELS` index of `down`.
    pub down_idx: usize,
    pub up_a: f64,
    pub up_b: f64,
    pub down_a: f64,
    pub down_b: f64,
    pub rank_up_a: usize,
    pub rank_up_b: usize,
    /// Number of models ranked on the two axes (the "of N" in "1 of N").
    pub field: usize,
    /// Items behind `axis_a` and `axis_b`.
    pub n_a: u32,
    pub n_b: u32,
}

impl Inversion<'_> {
    /// How far the two models move past each other, summed over both axes. The
    /// tie-break for "which inversion is the finding".
    pub fn swing(&self) -> f64 {
        self.lead_a() + self.lead_b()
    }

    /// How far `up` leads `down` on `axis_a`. Non-negative by construction.
    pub fn lead_a(&self) -> f64 {
        (self.up_a - self.down_a).abs()
    }

    /// How far `up` trails `down` on `axis_b`. Non-negative by construction.
    pub fn lead_b(&self) -> f64 {
        (self.down_b - self.up_b).abs()
    }
}

/// The strongest rank inversion in the data, or `None` when every axis orders
/// the models the same way (or ties, or there is no data at all).
pub fn find_inversion<'a>(stand: &Standings<'a>) -> Option<Inversion<'a>> {
    let mut best: Option<Inversion<'a>> = None;
    for i in 0..stand.len() {
        for j in (i + 1)..stand.len() {
            let (spec_a, rank_a) = &stand[i];
            let (spec_b, rank_b) = &stand[j];
            for (p, (mp, _)) in MODELS.iter().enumerate() {
                for (q, (mq, _)) in MODELS.iter().enumerate().skip(p + 1) {
                    let (Some(pa), Some(pb), Some(qa), Some(qb)) = (
                        ranked_for(rank_a, mp),
                        ranked_for(rank_b, mp),
                        ranked_for(rank_a, mq),
                        ranked_for(rank_b, mq),
                    ) else {
                        continue;
                    };
                    // `up` leads axis A and trails axis B. A stable order or a
                    // tie on either axis is not an inversion.
                    let (up, down, up_i, down_i) = if pa.rank < qa.rank && pb.rank > qb.rank {
                        (pa, qa, p, q)
                    } else if qa.rank < pa.rank && qb.rank > pb.rank {
                        (qa, pa, q, p)
                    } else {
                        continue;
                    };
                    let (up_b, down_b) = if up_i == p { (pb, qb) } else { (qb, pb) };
                    let cand = Inversion {
                        axis_a: spec_a,
                        axis_b: spec_b,
                        up: up.label,
                        down: down.label,
                        up_idx: up_i,
                        down_idx: down_i,
                        up_a: up.score,
                        up_b: up_b.score,
                        down_a: down.score,
                        down_b: down_b.score,
                        rank_up_a: up.rank,
                        rank_up_b: up_b.rank,
                        field: rank_b.len(),
                        n_a: rank_a.first().map(|r| r.report.n).unwrap_or(0),
                        n_b: rank_b.first().map(|r| r.report.n).unwrap_or(0),
                    };
                    if best.as_ref().is_none_or(|b| cand.swing() > b.swing()) {
                        best = Some(cand);
                    }
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::EvalReport;
    use std::collections::BTreeMap;

    fn cells_from(scores: &[(&str, [f64; 2])]) -> Cells {
        let mut cells = Cells::new();
        for (eval, per_model) in scores {
            for (i, (model, _)) in MODELS.iter().enumerate() {
                cells.insert(
                    ((*eval).to_string(), (*model).to_string()),
                    EvalReport {
                        eval: (*eval).to_string(),
                        model: (*model).to_string(),
                        n: 10,
                        metrics: BTreeMap::from([("mean_score".to_string(), per_model[i])]),
                        breakdown: BTreeMap::new(),
                    },
                );
            }
        }
        cells
    }

    #[test]
    fn no_inversion_when_one_model_leads_every_axis() {
        let cells = cells_from(&[
            ("sycophancy", [0.9, 0.5]),
            ("pressure", [0.8, 0.4]),
            ("calibration", [0.7, 0.3]),
            ("factuality", [0.6, 0.2]),
        ]);
        assert!(find_inversion(&standings(&cells)).is_none());
    }

    #[test]
    fn no_inversion_from_an_empty_or_tied_set() {
        assert!(find_inversion(&standings(&Cells::new())).is_none());
        let tied = cells_from(&[("sycophancy", [0.5, 0.5]), ("pressure", [0.5, 0.5])]);
        assert!(find_inversion(&standings(&tied)).is_none());
    }

    #[test]
    fn picks_the_pair_with_the_largest_swing() {
        // Model 1 leads sycophancy narrowly and calibration narrowly, but
        // trails pressure by a mile. The widest swing is sycophancy/pressure.
        let cells = cells_from(&[
            ("sycophancy", [0.50, 0.55]),
            ("pressure", [0.90, 0.10]),
            ("calibration", [0.49, 0.50]),
            ("factuality", [0.50, 0.50]),
        ]);
        let inv = find_inversion(&standings(&cells)).expect("an inversion exists");
        assert_eq!(inv.axis_a.eval, "sycophancy");
        assert_eq!(inv.axis_b.eval, "pressure");
        assert_eq!(inv.up, "grok-4.20-multi-agent");
        assert_eq!(inv.down, "grok-4.3");
        assert_eq!((inv.up_idx, inv.down_idx), (1, 0));
        assert_eq!((inv.rank_up_a, inv.rank_up_b), (1, 2));
        assert_eq!(inv.field, 2);
        assert!((inv.lead_a() - 0.05).abs() < 1e-9);
        assert!((inv.lead_b() - 0.80).abs() < 1e-9);
        assert!((inv.swing() - 0.85).abs() < 1e-9);
        assert_eq!((inv.n_a, inv.n_b), (10, 10));
    }

    /// `axis_a` is always the axis `up` leads, whichever way round the pair of
    /// axes appears in `AXES`.
    #[test]
    fn orientation_follows_the_leader_not_the_axis_order() {
        let cells = cells_from(&[
            ("sycophancy", [0.90, 0.10]),
            ("pressure", [0.10, 0.90]),
            ("calibration", [0.50, 0.50]),
            ("factuality", [0.50, 0.50]),
        ]);
        let inv = find_inversion(&standings(&cells)).unwrap();
        assert_eq!(inv.axis_a.eval, "sycophancy");
        assert_eq!(inv.up, "grok-4.3");
        assert_eq!(inv.up_a, 0.90);
        assert_eq!(inv.up_b, 0.10);
        assert_eq!(inv.down_a, 0.10);
        assert_eq!(inv.down_b, 0.90);
    }

    /// A partial result set (one axis missing entirely) still resolves.
    #[test]
    fn tolerates_missing_axes() {
        let cells = cells_from(&[("sycophancy", [0.2, 0.8]), ("pressure", [0.8, 0.2])]);
        let inv = find_inversion(&standings(&cells)).unwrap();
        assert_eq!(inv.axis_a.eval, "sycophancy");
        assert_eq!(inv.axis_b.eval, "pressure");
    }
}
