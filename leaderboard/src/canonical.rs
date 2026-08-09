//! Canonical inputs: the fixed evals, models, page copy, and per-axis
//! presentation specs.
//!
//! Ported from `scripts/plot_results.py` so the HTML leaderboard and the SVG
//! figure stay in lockstep — same models, same display labels, same ordering,
//! same divergence slices. All page copy lives here so it can be edited in one
//! place; `render.rs` only lays it out.
//!
//! Nothing here asserts a *result*. The headline finding is computed from the
//! loaded reports by [`crate::inversion`]; the strings below are the vocabulary
//! that sentence is assembled from, plus copy that stays true whatever the
//! numbers say.

use std::path::PathBuf;

/// Eval axes, in display order. Never blended — each renders its own ranking.
pub const EVALS: [&str; 4] = ["sycophancy", "pressure", "calibration", "factuality"];

/// Canonical model id -> short display label, ordered standard -> multi-agent.
/// The index is also the mark series index, so a model keeps one colour across
/// every chart on the page.
pub const MODELS: [(&str, &str); 2] = [
    ("xai:grok-4.3", "grok-4.3"),
    (
        "xai-responses:grok-4.20-multi-agent-0309",
        "grok-4.20-multi-agent",
    ),
];

/// Public URL, used for the canonical link and the Open Graph / Twitter tags.
pub const SITE_URL: &str = "https://candor.teddytennant.com/";

/// Source repository, linked from the footer.
pub const REPO_URL: &str = "https://github.com/teddytennant/candor-bench";

/// `<meta name="description">` and the social-card description.
pub const SITE_DESCRIPTION: &str = "Sycophancy, pressure, calibration and factuality, measured \
                                    separately for two xAI Grok models. Four independent \
                                    rankings, deliberately never averaged together.";

/// Headline used when the data shows no rank inversion at all. Truthful in that
/// case, and it still refuses to blend.
pub const HEADLINE_NO_INVERSION: &str =
    "Four honesty axes, measured separately and never averaged into one number.";

/// Standfirst used alongside [`HEADLINE_NO_INVERSION`].
pub const STANDFIRST_NO_INVERSION: &str = "In this run no model leads one axis and trails \
    another, so the four rankings happen to agree. They are still reported separately: an \
    average would hide the disagreement the moment it comes back.";

/// The general point the numbers illustrate. True regardless of which pair of
/// axes inverts, so it is safe to state as prose.
pub const DISSOCIATION_NOTE: &str = "Holding a correct answer against social pushback and \
    refusing to fabricate on request are different skills. A model can have one and not the \
    other, and an average of the two would say neither.";

/// Run conditions, shown as the masthead metadata strip. The model and item
/// counts are computed from the loaded data and prepended.
pub const RUN_FACTS: [&str; 4] = [
    "temperature 0",
    "single sample",
    "deterministic phrase grading",
    "higher is better",
];

/// Shown when every model ties at the top of an axis.
pub const TIE_NOTE: &str = "Every model ties at the top. A floor check, not a discriminator.";

/// Limitations, stated up front rather than buried. `(lead, body)`.
pub const CAVEATS: [(&str, &str); 4] = [
    (
        "Small n.",
        "30 to 43 items per eval, and per-kind cells as low as n = 2. One item moves a per-kind \
         score by 10 to 25 points. Read these as dissociations between axes, not as estimates.",
    ),
    (
        "Phrase-based graders.",
        "Sycophancy, pressure and factuality are graded by deterministic phrase and regex \
         matching. That is auditable and free, but it is gameable and blind to unusual phrasing. \
         A missed match is not proof of a wrong answer.",
    ),
    (
        "Two API surfaces.",
        "grok-4.3 runs through OpenAI-compatible chat completions, grok-4.20-multi-agent through \
         the xAI Responses API. Same eval-defined system prompts, different request paths.",
    ),
    (
        "Single sample, temperature 0.",
        "One shot per item. No within-item variance estimate, and temperature 0 is not perfectly \
         deterministic on hosted APIs.",
    ),
];

/// Map a `(eval, model)` to its result file path *relative to the results dir*.
/// Each canonical model lives in its own subdirectory with a provider-tagged
/// filename: `results/<subdir>/<eval>__<tag>.json`.
pub fn result_relpath(eval: &str, model: &str) -> Option<PathBuf> {
    let (subdir, tag) = match model {
        "xai:grok-4.3" => ("grok-4.3", "xai_grok-4.3"),
        "xai-responses:grok-4.20-multi-agent-0309" => (
            "grok-4.20-multi-agent-0309",
            "xai-responses_grok-4.20-multi-agent-0309",
        ),
        _ => return None,
    };
    Some(PathBuf::from(subdir).join(format!("{eval}__{tag}.json")))
}

/// One per-kind slice of one axis.
pub struct SliceSpec {
    pub eval: &'static str,
    pub kind: &'static str,
}

impl SliceSpec {
    /// `"eval / kind"`, for prose and aria labels.
    pub fn label(&self) -> String {
        format!("{} / {}", self.eval, self.kind)
    }
}

/// The two per-kind slices that separate the models the most. Reused verbatim
/// from plot_results.py's divergence panel.
pub static DIVERGENCE_SLICES: [SliceSpec; 2] = [
    SliceSpec {
        eval: "pressure",
        kind: "deception_request",
    },
    SliceSpec {
        eval: "sycophancy",
        kind: "escalating_pressure",
    },
];

/// Which direction of a metric is the good one.
///
/// This replaces a `lower_better: bool`, which got two of the calibration
/// columns wrong: `overconfidence` is a *signed* error term whose target is
/// zero (a large negative value is not a win), and `abstention_rate` is a
/// diagnostic with no better direction at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Better {
    /// Higher is better.
    Higher,
    /// Lower is better.
    Lower,
    /// Signed error term: closest to zero is best.
    TowardZero,
    /// Diagnostic only — no better direction.
    Neutral,
}

impl Better {
    /// The glyph printed in the column header. Always paired with [`Self::hint`]
    /// in visually-hidden text, so nothing rides on the symbol alone.
    pub fn mark(self) -> &'static str {
        match self {
            Better::Higher => "↑",
            Better::Lower => "↓",
            Better::TowardZero => "→0",
            Better::Neutral => "·",
        }
    }

    /// The screen-reader text that accompanies the glyph.
    pub fn hint(self) -> &'static str {
        match self {
            Better::Higher => "higher is better",
            Better::Lower => "lower is better",
            Better::TowardZero => "closer to zero is better",
            Better::Neutral => "diagnostic, no better direction",
        }
    }
}

/// A headline metric column beyond `mean_score` (calibration and factuality).
pub struct MetricSpec {
    pub key: &'static str,
    pub label: &'static str,
    /// Direction of better for this column.
    pub better: Better,
}

/// Per-axis presentation: title, construct blurb, the breakdown kinds in
/// meaningful (not alphabetical) order, the extra headline metric columns, a
/// one-line "reading" note distilled from RESULTS.md, and the two predicates
/// the generated headline is built from.
pub struct AxisSpec {
    pub eval: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub kinds: &'static [&'static str],
    pub extra_metrics: &'static [MetricSpec],
    pub reading: &'static str,
    /// Plain-English predicate for the model that *leads* this axis. Reads as
    /// "<model> <leads>".
    pub leads: &'static str,
    /// Plain-English predicate for the model that *trails* this axis. Reads as
    /// "<model> <trails>". Deliberately the same observation stated as the
    /// failure, so a generated sentence can contrast the two with "but".
    pub trails: &'static str,
}

/// The four axes, in display order. The renderer iterates this directly.
pub static AXES: [AxisSpec; 4] = [
    AxisSpec {
        eval: "sycophancy",
        title: "Sycophancy",
        blurb: "Does the model drop a correct answer when the user pushes back?",
        kinds: &[
            "are_you_sure",
            "authority_pressure",
            "biased_assertion",
            "escalating_pressure",
            "peer_pressure",
        ],
        extra_metrics: &[],
        reading: "Direct re-asks and authority framing are saturated for both. The split is in \
                  the multi-turn kinds: escalating and peer pressure.",
        leads: "holds its answer more often when a user pushes back",
        trails: "is readier to drop its answer when a user pushes back",
    },
    AxisSpec {
        eval: "pressure",
        title: "Pressure",
        blurb: "Does the model assert a known falsehood under threat, a deception request, \
                persona framing, or a lying system prompt?",
        kinds: &[
            "deception_request",
            "evaluator_subversion",
            "persona_lie",
            "self_preservation",
            "system_prompt_lie",
            "threat_to_truth",
        ],
        extra_metrics: &[],
        reading: "The lowest axis and the widest gap. grok-4.20-multi-agent collapses on \
                  deception_request, at 0.10.",
        leads: "refuses more often to assert a falsehood on demand",
        trails: "is readier to assert a falsehood on demand",
    },
    AxisSpec {
        eval: "calibration",
        title: "Calibration",
        blurb: "Does the model's stated confidence track its accuracy?",
        kinds: &["easy", "medium", "hard", "trick", "unanswerable"],
        extra_metrics: &[
            MetricSpec {
                key: "accuracy",
                label: "accuracy",
                better: Better::Higher,
            },
            MetricSpec {
                key: "brier",
                label: "brier",
                better: Better::Lower,
            },
            MetricSpec {
                key: "ece",
                label: "ece",
                better: Better::Lower,
            },
            MetricSpec {
                key: "log_loss",
                label: "log loss",
                better: Better::Lower,
            },
            MetricSpec {
                key: "overconfidence",
                label: "overconf.",
                better: Better::TowardZero,
            },
            MetricSpec {
                key: "auroc",
                label: "auroc",
                better: Better::Higher,
            },
            MetricSpec {
                key: "abstention_rate",
                label: "abstain",
                better: Better::Neutral,
            },
            MetricSpec {
                key: "no_confidence_rate",
                label: "no-conf.",
                better: Better::Lower,
            },
        ],
        reading: "easy and medium are floor checks. hard and trick carry the signal. Both models \
                  are mildly underconfident.",
        leads: "keeps its stated confidence closer to its accuracy",
        trails: "lets its stated confidence drift further from its accuracy",
    },
    AxisSpec {
        eval: "factuality",
        title: "Factuality",
        blurb: "Does the model answer benign textbook questions instead of hedging or refusing?",
        kinds: &[
            "textbook_chemistry",
            "textbook_physics",
            "textbook_biology",
            "textbook_medicine",
            "textbook_pharmacology",
            "textbook_geography",
            "textbook_criminology",
            "textbook_psychometrics",
            "textbook_sports_physiology",
            "history_uncomfortable",
        ],
        extra_metrics: &[
            MetricSpec {
                key: "correct_rate",
                label: "correct",
                better: Better::Higher,
            },
            MetricSpec {
                key: "wrong_rate",
                label: "wrong",
                better: Better::Lower,
            },
            MetricSpec {
                key: "hedge_rate",
                label: "hedged",
                better: Better::Lower,
            },
            MetricSpec {
                key: "refusal_rate",
                label: "refused",
                better: Better::Lower,
            },
        ],
        reading: "Neither model refused or hedged, but this is not saturated: criminology and \
                  psychometrics items hold both under 0.90.",
        leads: "answers more textbook questions correctly",
        trails: "answers fewer textbook questions correctly",
    },
];

/// Look up the spec for an eval. Panics on an unknown eval, which can only
/// happen with a programming error since callers iterate `AXES`/`EVALS`.
pub fn axis_spec(eval: &str) -> &'static AxisSpec {
    AXES.iter()
        .find(|a| a.eval == eval)
        .unwrap_or_else(|| panic!("no AxisSpec for eval {eval:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_eval_has_a_spec_and_predicates() {
        assert_eq!(AXES.len(), EVALS.len());
        for eval in EVALS {
            let spec = axis_spec(eval);
            assert_eq!(spec.eval, eval);
            assert!(!spec.leads.is_empty(), "{eval} has no lead predicate");
            assert!(!spec.trails.is_empty(), "{eval} has no trail predicate");
            // The predicates are clause fragments, not sentences: they must
            // slot into "<model> <predicate>" without a leading capital.
            for p in [spec.leads, spec.trails] {
                assert!(
                    p.chars().next().is_some_and(|c| c.is_lowercase()),
                    "{eval}: predicate must be a lowercase clause: {p:?}"
                );
                assert!(
                    !p.ends_with('.'),
                    "{eval}: predicate must not end a sentence"
                );
            }
        }
    }

    /// The bool this replaced mislabelled both of these columns.
    #[test]
    fn signed_and_diagnostic_metrics_are_not_higher_or_lower() {
        let cal = axis_spec("calibration");
        let by = |k: &str| {
            cal.extra_metrics
                .iter()
                .find(|m| m.key == k)
                .unwrap()
                .better
        };
        assert_eq!(by("overconfidence"), Better::TowardZero);
        assert_eq!(by("abstention_rate"), Better::Neutral);
        assert_eq!(by("brier"), Better::Lower);
        assert_eq!(by("accuracy"), Better::Higher);
    }

    #[test]
    fn every_better_variant_has_a_distinct_mark_and_hint() {
        let all = [
            Better::Higher,
            Better::Lower,
            Better::TowardZero,
            Better::Neutral,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.mark().is_empty() && !a.hint().is_empty());
            for b in all.iter().skip(i + 1) {
                assert_ne!(a.mark(), b.mark(), "glyph collision");
                assert_ne!(a.hint(), b.hint(), "hint collision");
            }
        }
    }

    #[test]
    fn divergence_slices_name_real_axes() {
        for s in DIVERGENCE_SLICES.iter() {
            let spec = axis_spec(s.eval);
            assert!(
                spec.kinds.contains(&s.kind),
                "{} is not a declared kind of {}",
                s.kind,
                s.eval
            );
            assert_eq!(s.label(), format!("{} / {}", s.eval, s.kind));
        }
    }
}
