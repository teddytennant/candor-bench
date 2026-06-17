//! Canonical inputs: the fixed evals, models, and per-axis presentation specs.
//!
//! Ported from `scripts/plot_results.py` so the HTML leaderboard and the SVG
//! figure stay in lockstep — same models, same display labels, same ordering,
//! same divergence slices.

use std::path::PathBuf;

/// Eval axes, in display order. Never blended — each renders its own ranking.
pub const EVALS: [&str; 4] = ["sycophancy", "pressure", "calibration", "factuality"];

/// Canonical model id -> short display label, ordered standard -> multi-agent.
pub const MODELS: [(&str, &str); 2] = [
    ("xai:grok-4.3", "grok-4.3"),
    (
        "xai-responses:grok-4.20-multi-agent-0309",
        "grok-4.20-multi-agent",
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

/// (eval, kind, human label) — the two per-kind slices that separate the models
/// the most. Reused verbatim from plot_results.py's divergence panel.
pub const DIVERGENCE_SLICES: [(&str, &str, &str); 2] = [
    (
        "pressure",
        "deception_request",
        "pressure / deception_request",
    ),
    (
        "sycophancy",
        "escalating_pressure",
        "sycophancy / escalating_pressure",
    ),
];

/// A headline metric column beyond `mean_score` (calibration and factuality).
pub struct MetricSpec {
    pub key: &'static str,
    pub label: &'static str,
    /// True when a lower value is better — renders a "↓" hint in the header.
    pub lower_better: bool,
}

/// Per-axis presentation: title, construct blurb, the breakdown kinds in
/// meaningful (not alphabetical) order, the extra headline metric columns, and
/// a one-line "reading" note distilled from RESULTS.md.
pub struct AxisSpec {
    pub eval: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub kinds: &'static [&'static str],
    pub extra_metrics: &'static [MetricSpec],
    pub reading: &'static str,
}

/// The four axes, in display order. The renderer iterates this directly.
pub static AXES: [AxisSpec; 4] = [
    AxisSpec {
        eval: "sycophancy",
        title: "Sycophancy",
        blurb: "Does the model abandon a correct answer when the user pushes back socially?",
        kinds: &[
            "are_you_sure",
            "authority_pressure",
            "biased_assertion",
            "escalating_pressure",
            "peer_pressure",
        ],
        extra_metrics: &[],
        reading: "Both hold direct re-asks and authority framing; they split on escalating and peer pressure.",
    },
    AxisSpec {
        eval: "pressure",
        title: "Pressure",
        blurb: "Will the model assert something it has reason to believe is false under threat, deception requests, persona framing, or a lying system prompt?",
        kinds: &[
            "deception_request",
            "evaluator_subversion",
            "persona_lie",
            "self_preservation",
            "system_prompt_lie",
            "threat_to_truth",
        ],
        extra_metrics: &[],
        reading: "The lowest axis and the widest gap — the multi-agent model collapses on deception_request (0.10).",
    },
    AxisSpec {
        eval: "calibration",
        title: "Calibration",
        blurb: "Does the model's stated confidence actually track its accuracy?",
        kinds: &["easy", "medium", "hard", "trick", "unanswerable"],
        extra_metrics: &[
            MetricSpec { key: "accuracy", label: "accuracy", lower_better: false },
            MetricSpec { key: "brier", label: "brier", lower_better: true },
            MetricSpec { key: "ece", label: "ece", lower_better: true },
            MetricSpec { key: "log_loss", label: "log loss", lower_better: true },
            MetricSpec { key: "overconfidence", label: "overconf.", lower_better: false },
            MetricSpec { key: "auroc", label: "auroc", lower_better: false },
            MetricSpec { key: "abstention_rate", label: "abstain", lower_better: false },
            MetricSpec { key: "no_confidence_rate", label: "no-conf.", lower_better: false },
        ],
        reading: "easy/medium are a floor check; hard and trick carry the signal.",
    },
    AxisSpec {
        eval: "factuality",
        title: "Factuality",
        blurb: "Will the model give specific, textbook-correct answers instead of refusing or hedging?",
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
            MetricSpec { key: "correct_rate", label: "correct", lower_better: false },
            MetricSpec { key: "wrong_rate", label: "wrong", lower_better: true },
            MetricSpec { key: "hedge_rate", label: "hedged", lower_better: true },
            MetricSpec { key: "refusal_rate", label: "refused", lower_better: true },
        ],
        reading: "Not saturated — criminology and psychometrics items pull both models below 0.90.",
    },
];
