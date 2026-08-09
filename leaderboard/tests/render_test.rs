//! Integration tests: write a throwaway results dir, then exercise loading,
//! ranking, escaping, mark geometry, generated copy, and a full render.
//!
//! Fixtures are built from a `(eval, model index) -> score` closure so a test
//! can plant exactly the shape of data it is about — a rank inversion, a stable
//! order, a tie, or a difference that vanishes at the printed precision.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use candor_leaderboard::canonical::{
    axis_spec, result_relpath, Better, AXES, DISSOCIATION_NOTE, DIVERGENCE_SLICES, EVALS,
    HEADLINE_NO_INVERSION, MODELS,
};
use candor_leaderboard::data::{load_all, load_report, rank_axis, Cells};
use candor_leaderboard::inversion::{find_inversion, standings};
use candor_leaderboard::marks::{coord, dot_x, slope_x, slope_y, spark_w, SLOPE_X0, SLOPE_X1};
use candor_leaderboard::render::{dnum, esc, render_page, style_css};

static COUNTER: AtomicU32 = AtomicU32::new(0);

const TITLE: &str = "candor-bench: per-axis honesty leaderboard";

/// A unique temp dir per call (tests run in parallel threads).
fn fresh_dir(tag: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("candor_lb_{tag}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write one canonical cell. `extra_kinds` lets a test plant the per-kind
/// values the divergence slopegraph reads.
fn write_report(dir: &Path, eval: &str, model: &str, score: f64, extra_kinds: &[(&str, f64)]) {
    let path = dir.join(result_relpath(eval, model).unwrap());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut breakdown = format!(r#""k1":{{"n":5.0,"mean_score":{score}}}"#);
    for (kind, v) in extra_kinds {
        breakdown.push_str(&format!(r#","{kind}":{{"n":3.0,"mean_score":{v}}}"#));
    }
    // Include a `results` array to prove parsing skips it.
    let json = format!(
        r#"{{"eval":"{eval}","model":"{model}","n":10,
            "metrics":{{"mean_score":{score}}},
            "breakdown":{{{breakdown}}},
            "results":[{{"sample_id":"x","calls":[]}}]}}"#
    );
    fs::write(path, json).unwrap();
}

/// The canonical fixture: sycophancy separates the models, pressure separates
/// them the other way (a real inversion), factuality is a dead tie.
fn score_for(eval: &str, idx: usize) -> f64 {
    // MODELS order is [grok-4.3, grok-4.20-multi-agent] -> idx 0,1.
    match eval {
        "sycophancy" => [0.80, 1.00][idx], // multi-agent first
        "pressure" => [0.50, 0.25][idx],   // grok-4.3 first: the inversion
        "factuality" => 1.00,              // both tied at the top
        _ => 0.85 + 0.01 * idx as f64,
    }
}

/// Per-kind values for the divergence slices, deliberately crossing: model 0 is
/// high on the first slice and low on the second, model 1 the inverse.
fn slice_score_for(kind: &str, idx: usize) -> f64 {
    match (kind, idx) {
        ("deception_request", 0) => 0.90,
        ("deception_request", _) => 0.10,
        (_, 0) => 0.20,
        (_, _) => 0.80,
    }
}

fn write_all(dir: &Path, score: impl Fn(&str, usize) -> f64) {
    for eval in EVALS {
        for (i, (model, _)) in MODELS.iter().enumerate() {
            let extra: Vec<(&str, f64)> = DIVERGENCE_SLICES
                .iter()
                .filter(|s| s.eval == eval)
                .map(|s| (s.kind, slice_score_for(s.kind, i)))
                .collect();
            write_report(dir, eval, model, score(eval, i), &extra);
        }
    }
}

/// Build a fixture, render it, and clean up.
fn render_fixture(tag: &str, score: impl Fn(&str, usize) -> f64) -> String {
    let dir = fresh_dir(tag);
    write_all(&dir, score);
    let cells = load_all(&dir).unwrap();
    let html = render_page(&cells, TITLE);
    let _ = fs::remove_dir_all(&dir);
    html
}

/// Every `<th>` in the document must declare a scope.
fn assert_every_th_has_scope(html: &str) {
    let mut rest = html;
    let mut seen = 0usize;
    while let Some(i) = rest.find("<th") {
        let tail = &rest[i..];
        let end = tail.find('>').expect("unterminated <th>");
        let tag = &tail[..end];
        // Skip `<thead>`: only the header cells themselves need a scope.
        if matches!(tag.as_bytes().get(3), Some(b' ') | None) {
            assert!(tag.contains("scope=\""), "header cell without scope: {tag}");
            seen += 1;
        }
        rest = &tail[end..];
    }
    assert!(
        seen > 20,
        "expected the tables to emit header cells, saw {seen}"
    );
}

// ------------------------------------------------------------------ loading

#[test]
fn ranks_and_renders() {
    let dir = fresh_dir("full");
    write_all(&dir, score_for);
    let cells = load_all(&dir).unwrap();
    assert_eq!(cells.len(), EVALS.len() * MODELS.len());

    // Distinct scores -> grok-4.20-multi-agent(1) > grok-4.3(2).
    let syc = rank_axis("sycophancy", &cells);
    assert_eq!((syc[0].rank, syc[0].label), (1, "grok-4.20-multi-agent"));
    assert_eq!((syc[1].rank, syc[1].label), (2, "grok-4.3"));

    // All equal -> everyone shares rank 1.
    let fac = rank_axis("factuality", &cells);
    assert!(fac.iter().all(|r| r.rank == 1));

    let html = render_page(&cells, TITLE);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.ends_with("</html>\n"));
    for spec in AXES.iter() {
        assert!(
            html.contains(&format!("<h2>{}", spec.title)),
            "missing axis {}",
            spec.title
        );
        assert!(
            html.contains(&format!("id=\"{}\"", spec.eval)),
            "missing section id {}",
            spec.eval
        );
    }
    for id in ["summary", "divergence", "caveats"] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "missing section {id}"
        );
    }
    assert!(html.contains("grok-4.3"));
    assert!(html.contains("Per-kind breakdown"));

    // The tie note fires only where every model ties.
    assert_eq!(html.matches("Every model ties at the top").count(), 1);

    assert_every_th_has_scope(&html);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_report_hard_errors() {
    let dir = fresh_dir("err");
    let grok = "xai:grok-4.3";

    // (a) missing file
    assert!(load_report(&dir, "sycophancy", grok).is_err());

    // (b) eval/model mismatch: a file whose contents claim the wrong eval.
    let p = dir.join(result_relpath("pressure", grok).unwrap());
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(
        &p,
        format!(r#"{{"eval":"sycophancy","model":"{grok}","n":1,"metrics":{{"mean_score":1.0}}}}"#),
    )
    .unwrap();
    assert!(load_report(&dir, "pressure", grok).is_err());

    // (c) missing mean_score
    let p2 = dir.join(result_relpath("calibration", grok).unwrap());
    fs::create_dir_all(p2.parent().unwrap()).unwrap();
    fs::write(
        &p2,
        format!(r#"{{"eval":"calibration","model":"{grok}","n":1,"metrics":{{}}}}"#),
    )
    .unwrap();
    assert!(load_report(&dir, "calibration", grok).is_err());

    // (d) malformed JSON: a file that is not valid JSON at all.
    let p3 = dir.join(result_relpath("factuality", grok).unwrap());
    fs::create_dir_all(p3.parent().unwrap()).unwrap();
    fs::write(&p3, "{ this is not json").unwrap();
    let err = load_report(&dir, "factuality", grok).unwrap_err();
    assert!(err.contains("cannot parse"), "unexpected error: {err}");

    // ...and one bad cell aborts the whole load.
    assert!(load_all(&dir).is_err());

    let _ = fs::remove_dir_all(&dir);
}

// --------------------------------------------------------------- no blending

/// The load-bearing invariant: four independent rankings, never a blend.
#[test]
fn never_presents_a_blended_score() {
    let html = render_fixture("noblend", score_for);
    let lower = html.to_lowercase();

    for phrase in [
        "overall",
        "composite score",
        "combined score",
        "honesty index",
        "average score",
        "aggregate score",
        "total score",
    ] {
        assert!(!lower.contains(phrase), "page presents a blend: {phrase:?}");
    }

    // Structurally: exactly one ranked table per axis, and the summary has one
    // row per axis with no extra roll-up row.
    assert_eq!(
        html.matches("<table class=\"headline").count(),
        AXES.len(),
        "expected one ranking table per axis"
    );
    assert_eq!(
        html.matches("<tr><th scope=\"row\"><a href=\"#").count(),
        AXES.len(),
        "summary must have exactly one row per axis"
    );

    // ...and the page says out loud why there is no single number.
    assert!(html.contains(&esc(DISSOCIATION_NOTE)));
}

// ------------------------------------------------------- the computed finding

/// The whole point of computing the claim: it is made when, and only when, the
/// data actually shows a rank flip. A page that keeps asserting a dissociation
/// after the evals stop showing one is the failure this guards against.
#[test]
fn inversion_is_claimed_only_when_the_ranking_actually_flips() {
    // grok-4.20-multi-agent leads sycophancy; grok-4.3 leads pressure.
    let flipped = render_fixture("inv", |eval, idx| match eval {
        "sycophancy" => [0.60, 0.90][idx],
        "pressure" => [0.90, 0.50][idx],
        _ => 0.70,
    });
    assert!(flipped.contains("grok-4.20-multi-agent ranks 1 of 2 on Sycophancy"));
    assert!(flipped.contains("and 2 of 2 on Pressure"));
    // The h1 is the finding, generated from that same inversion.
    let h1_at = flipped.find("<h1>").expect("no h1");
    let h1 = &flipped[h1_at..flipped[h1_at..].find("</h1>").unwrap() + h1_at];
    assert!(h1.contains("grok-4.20-multi-agent"), "h1 was {h1:?}");
    assert!(h1.contains(axis_spec("sycophancy").leads), "h1 was {h1:?}");
    assert!(h1.contains(axis_spec("pressure").trails), "h1 was {h1:?}");
    assert!(!flipped.contains(HEADLINE_NO_INVERSION));

    // One model ahead on every axis: nothing to invert, so no claim is made
    // and the page degrades to a sentence that is still true.
    let stable = render_fixture("noinv", |_, idx| [0.90, 0.50][idx]);
    assert!(!stable.contains("ranks 1 of 2 on"));
    assert!(stable.contains(&esc(HEADLINE_NO_INVERSION)));
    assert!(!stable.contains(&esc(DISSOCIATION_NOTE)));
    // ...and it is still a whole page, with every axis and no dangling sentence.
    assert!(stable.ends_with("</html>\n"));
    for spec in AXES.iter() {
        assert!(stable.contains(&format!("<h2>{}", spec.title)));
    }
    assert_every_th_has_scope(&stable);
}

/// The inversion drives the divergence framing too, not only the headline.
#[test]
fn divergence_framing_names_the_inverted_axes() {
    let html = render_fixture("divframe", score_for);
    assert!(html.contains("The sycophancy and pressure rankings disagree."));
}

/// The generated headline reports the model's *relative* standing, and never
/// claims more than a two-model, small-n comparison can support.
#[test]
fn headline_does_not_overclaim() {
    let html = render_fixture("claim", score_for);
    let h1_at = html.find("<h1>").unwrap();
    let h1 = &html[h1_at + 4..html[h1_at..].find("</h1>").unwrap() + h1_at];
    for hype in [
        "best",
        "worst",
        "safest",
        "most honest",
        "state of the art",
        "always",
        "never",
        "proves",
        "cannot",
    ] {
        assert!(
            !h1.to_lowercase().contains(hype),
            "headline overclaims with {hype:?}: {h1:?}"
        );
    }
    assert!(h1.ends_with('.'), "headline must be a sentence: {h1:?}");
    // Sample sizes travel with the claim, immediately under it.
    let stand_at = html.find("class=\"standfirst\"").unwrap();
    assert!(stand_at > h1_at);
    let standfirst = &html[stand_at..stand_at + 400];
    assert!(standfirst.contains("n = 10"), "{standfirst}");
}

/// The product name is demoted to an eyebrow but is still the document title.
#[test]
fn product_name_is_the_title_and_the_eyebrow_not_the_headline() {
    let html = render_fixture("eyebrow", score_for);
    assert!(html.contains(&format!("<title>{}</title>", esc(TITLE))));
    assert!(html.contains(&format!("<p class=\"eyebrow\">{}</p>", esc(TITLE))));
    let eyebrow_at = html.find("class=\"eyebrow\"").unwrap();
    let h1_at = html.find("<h1>").unwrap();
    assert!(eyebrow_at < h1_at, "the eyebrow sits above the headline");
    // Exactly one h1, and heading order never skips a level.
    assert_eq!(html.matches("<h1>").count(), 1);
    assert!(!html.contains("<h4"), "no heading level is skipped");
    assert!(!html.contains("<h3"), "the page has no h3 to skip to");
}

// ---------------------------------------------------------- direction of good

/// Every metric column declares which way is better, and the glyph is always
/// paired with visually-hidden text — meaning never rides on a symbol alone.
#[test]
fn direction_of_better_is_spelled_out_per_column() {
    let html = render_fixture("dirs", score_for);
    for (label, better) in [
        ("mean score", Better::Higher),
        ("brier", Better::Lower),
        ("overconf.", Better::TowardZero),
        ("abstain", Better::Neutral),
    ] {
        let expected = format!(
            "{label}<span class=\"hint\" aria-hidden=\"true\">{}</span>\
             <span class=\"vh\"> ({})</span>",
            better.mark(),
            better.hint()
        );
        assert!(
            html.contains(&expected),
            "missing direction marker for {label}"
        );
    }
    // Every variant's wording actually reaches the page.
    for b in [
        Better::Higher,
        Better::Lower,
        Better::TowardZero,
        Better::Neutral,
    ] {
        assert!(html.contains(b.hint()), "missing hint for {b:?}");
    }
    // Every aria-hidden glyph has an adjacent .vh span; none stands alone.
    assert_eq!(
        html.matches("<span class=\"hint\" aria-hidden=\"true\">")
            .count(),
        html.matches("</span><span class=\"vh\"> (").count()
    );
}

// ------------------------------------------------------------- gap and delta

#[test]
fn delta_column_is_signed_and_anchored_to_rank_one() {
    let html = render_fixture("delta", score_for);
    // The rank-1 row is the reference; the 0.80 row trails 1.00 by 0.2000.
    assert!(html.contains(">ref</td>"));
    assert!(
        html.contains(">-0.2000</td>"),
        "expected a signed delta against the leader"
    );
    // A tie renders an unsigned zero, never "-0.0000".
    assert!(!html.contains("-0.0000"));
    assert!(html.contains(">0.0000</td>"));

    // The delta is only where it earns its place: the two narrow axis tables.
    // Calibration and factuality already carry extra metric columns.
    assert_eq!(html.matches("class=\"num delta").count(), 2 * MODELS.len());
    let cal = &html[html.find("id=\"calibration\"").unwrap()..];
    let cal = &cal[..cal.find("</section>").unwrap()];
    assert!(
        !cal.contains("delta"),
        "delta must not bloat the wide table"
    );
}

#[test]
fn dnum_signs_every_value_including_negative_zero() {
    assert_eq!(dnum(0.5, 4), "+0.5000");
    assert_eq!(dnum(-0.5, 4), "-0.5000");
    assert_eq!(dnum(0.0, 4), "0.0000");
    assert_eq!(dnum(-0.0, 4), "0.0000");
    assert_eq!(dnum(-1e-9, 4), "0.0000");
    assert_eq!(dnum(1e-9, 4), "0.0000");
    assert_eq!(dnum(-0.12345, 3), "-0.123");
}

/// The summary matrix carries the size of the gap, so the reader never has to
/// subtract two four-decimal numbers by eye.
#[test]
fn summary_reports_the_gap_per_axis() {
    let html = render_fixture("gap", score_for);
    assert!(html.contains("<td class=\"num gap\">0.2000</td>")); // sycophancy
    assert!(html.contains("<td class=\"num gap\">0.2500</td>")); // pressure
    assert!(html.contains("<td class=\"num gap\">0.0000</td>")); // factuality tie
    assert!(html.contains("gap<span class=\"vh\"> (highest score minus lowest)</span>"));
    // One gap cell per axis in the summary, plus one per divergence slice.
    assert_eq!(
        html.matches("class=\"num gap\"").count(),
        AXES.len() + DIVERGENCE_SLICES.len()
    );
}

// ------------------------------------------------------------- summary order

/// The summary matrix lists the axes in canonical order, one row each.
#[test]
fn summary_rows_follow_canonical_axis_order() {
    let html = render_fixture("order", score_for);
    let start = html.find("<table class=\"summary\">").unwrap();
    let body = &html[start..html[start..].find("</table>").unwrap() + start];

    let mut cursor = 0usize;
    for spec in AXES.iter() {
        let at = body[cursor..]
            .find(&format!("<a href=\"#{}\">{}</a>", spec.eval, spec.title))
            .unwrap_or_else(|| panic!("summary row for {} out of order", spec.eval))
            + cursor;
        cursor = at;
    }
    assert_eq!(body.matches("<tr><th scope=\"row\">").count(), AXES.len());
    // And the axis sections themselves follow the same order.
    let mut cursor = 0usize;
    for spec in AXES.iter() {
        let at = html[cursor..]
            .find(&format!("<section class=\"axis\" id=\"{}\">", spec.eval))
            .unwrap_or_else(|| panic!("axis section for {} out of order", spec.eval))
            + cursor;
        cursor = at;
    }
}

// ------------------------------------------------------------- mark geometry

/// Summary dot plots must sit at the score's position on the shared 0..1 rule.
#[test]
fn summary_dotplot_geometry_matches_scores() {
    let html = render_fixture("dots", score_for);

    // sycophancy: grok-4.3 = 0.80 (series 0), multi-agent = 1.00 (series 1).
    assert!(html.contains(&format!(
        "<circle class=\"dot s0\" cx=\"{}\" cy=\"8\" r=\"3.5\"/>",
        coord(dot_x(0.80))
    )));
    assert!(html.contains(&format!(
        "<circle class=\"dot s1\" cx=\"{}\" cy=\"8\" r=\"3.5\"/>",
        coord(dot_x(1.00))
    )));
    // The spread rule connects the two extremes.
    assert!(html.contains(&format!(
        "<line class=\"spread\" x1=\"{}\" y1=\"8\" x2=\"{}\" y2=\"8\"/>",
        coord(dot_x(0.80)),
        coord(dot_x(1.00))
    )));
    // Every axis gets one plot, and every plot is decorative (numbers adjacent).
    assert_eq!(html.matches("<svg class=\"dots\"").count(), AXES.len());
    assert_eq!(
        html.matches("<svg class=\"dots\"").count(),
        html.matches("<svg class=\"dots\" width=\"150\" height=\"16\" viewBox=\"0 0 150 16\" aria-hidden=\"true\"").count()
    );
}

/// Headline sparkbars must be sized to the mean score they sit beside.
#[test]
fn headline_sparkbar_width_matches_score() {
    let html = render_fixture("spark", score_for);

    assert!(html.contains(
        "class=\"spark s0\" width=\"56\" height=\"10\" viewBox=\"0 0 56 10\" aria-hidden=\"true\""
    ));
    // grok-4.3 scores 0.80 on sycophancy, the multi-agent model 1.00.
    assert!(html.contains(&format!(
        "<rect class=\"fill\" x=\"0\" y=\"2\" width=\"{}\" height=\"6\"/>",
        coord(spark_w(0.80))
    )));
    assert!(html.contains(&format!(
        "<rect class=\"fill\" x=\"0\" y=\"2\" width=\"{}\" height=\"6\"/>",
        coord(spark_w(1.00))
    )));
    // One bar per ranked row across all four axes.
    assert_eq!(
        html.matches("<svg class=\"spark").count(),
        AXES.len() * MODELS.len()
    );
}

/// Marks are told apart by series class *and* by the direct label in the same
/// row or the legend, so identity is never carried by colour alone.
#[test]
fn every_series_is_named_in_the_legend() {
    let html = render_fixture("legend", score_for);
    for (i, (_, label)) in MODELS.iter().enumerate() {
        let key = format!("<li class=\"key s{i}\">{label}</li>");
        assert!(html.contains(&key), "missing legend key: {key}");
    }
    // The legend appears with the summary plots and again with the slopegraph.
    assert_eq!(html.matches("<ul class=\"legend\">").count(), 2);
}

/// The divergence slopegraph is the visual form of the argument: the two lines
/// must actually cross, on an honest 0..1 domain, and it must be exposed as a
/// labelled image.
#[test]
fn slopegraph_lines_cross_for_the_divergence_slices() {
    let html = render_fixture("slope", score_for);

    let a0 = slice_score_for(DIVERGENCE_SLICES[0].kind, 0);
    let a1 = slice_score_for(DIVERGENCE_SLICES[1].kind, 0);
    let b0 = slice_score_for(DIVERGENCE_SLICES[0].kind, 1);
    let b1 = slice_score_for(DIVERGENCE_SLICES[1].kind, 1);

    // Model 0 starts above model 1 and ends below it.
    assert!(slope_y(a0) < slope_y(b0));
    assert!(slope_y(a1) > slope_y(b1));

    for (cls, lo, hi) in [("s0", a0, a1), ("s1", b0, b1)] {
        let points = format!(
            "<polyline class=\"line {cls}\" points=\"{},{} {},{}\"/>",
            coord(slope_x(0, 2)),
            coord(slope_y(lo)),
            coord(slope_x(1, 2)),
            coord(slope_y(hi))
        );
        assert!(html.contains(&points), "missing slope line {cls}: {points}");
    }
    assert_eq!(coord(slope_x(0, 2)), coord(SLOPE_X0));
    assert_eq!(coord(slope_x(1, 2)), coord(SLOPE_X1));

    // The domain is the full 0..1, not a truncated window that would inflate
    // the crossing: score 1.0 and 0.0 map to the poles' own ends.
    assert!(html.contains("<text class=\"scale\" x=\"2\" y=\"19.5\">1.0</text>"));
    assert!(html.contains("<text class=\"scale\" x=\"2\" y=\"155.5\">0.0</text>"));

    // Meaningful graphic -> role + aria-label naming every model and slice.
    assert!(
        html.contains("<svg class=\"slope\" viewBox=\"0 0 320 192\" role=\"img\" aria-label=\"")
    );
    for (_, label) in MODELS {
        assert!(
            html.contains(&format!("{label}:")),
            "{label} not in the label"
        );
    }
    for s in DIVERGENCE_SLICES.iter() {
        assert!(html.contains(&esc(s.kind)), "slice {} missing", s.label());
    }
}

// --------------------------------------------------------------- self-contained

#[test]
fn head_carries_the_public_meta_tags() {
    let html = render_fixture("meta", score_for);

    for needle in [
        "<meta name=\"description\" content=\"",
        "<meta name=\"color-scheme\" content=\"light dark\">",
        "<link rel=\"canonical\" href=\"https://candor.teddytennant.com/\">",
        "<meta property=\"og:type\" content=\"website\">",
        "<meta property=\"og:description\" content=\"",
        "<meta property=\"og:url\" content=\"https://candor.teddytennant.com/\">",
        "<meta name=\"twitter:card\" content=\"summary\">",
        "<meta name=\"theme-color\" media=\"(prefers-color-scheme: light)\" content=\"#fdfdfb\">",
        "<meta name=\"theme-color\" media=\"(prefers-color-scheme: dark)\" content=\"#0d0e0f\">",
        "<link rel=\"icon\" href=\"data:image/svg+xml,",
    ] {
        assert!(html.contains(needle), "missing meta: {needle}");
    }
    // The card image would be a remote asset; there is none.
    assert!(!html.contains("og:image"));
    assert!(!html.contains("rel=\"icon\" href=\"http"));

    // The masthead counts are derived, not hardcoded: 4 axes x n = 10.
    assert!(html.contains("<li>40 items</li>"));
    assert!(html.contains(&format!("<li>{} xAI models</li>", MODELS.len())));
}

/// Zero JavaScript, and nothing fetched off-origin. The only absolute URLs are
/// the canonical link, the footer source link, and the SVG namespace inside the
/// inline data: favicon.
#[test]
fn output_is_scriptless_and_pulls_no_external_asset() {
    let html = render_fixture("assets", score_for);
    assert!(!html.contains("<script"), "page must ship no JavaScript");
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("onclick"));
    assert!(!html.contains("onerror="));
    assert!(
        !html.contains("src="),
        "no external or inline asset references"
    );
    assert_eq!(html.matches("<link rel=\"stylesheet\"").count(), 1);
    assert!(html.contains("<link rel=\"stylesheet\" href=\"style.css\">"));

    let external = html.matches("https://").count() + html.matches("http://").count();
    let accounted = html.matches("https://candor.teddytennant.com").count()
        + html.matches("https://github.com/teddytennant").count()
        + html.matches("http://www.w3.org/2000/svg").count();
    assert_eq!(external, accounted, "unexpected off-origin URL in the page");
}

#[test]
fn stylesheet_is_self_contained_and_defines_both_themes() {
    let css = style_css();
    let (light, rest) = css
        .split_once("@media (prefers-color-scheme: dark)")
        .expect("no dark-mode block");
    // The dark block runs to its own closing brace at column 0, not to the end
    // of the file — everything after that is shared layout.
    let dark = rest.split_once("\n}\n").expect("unterminated dark block").0;

    // The full palette is declared on bare :root...
    for var in [
        "--paper",
        "--ink",
        "--faint",
        "--rule",
        "--rule-strong",
        "--mark-rule",
        "--hover",
        "--accent",
    ] {
        assert!(light.contains(&format!("{var}:")), "{var} missing on :root");
        assert!(dark.contains(&format!("{var}:")), "{var} not overridden");
    }
    assert!(light.contains(":root {"));
    assert!(light.contains("color-scheme: light dark"));
    // ...and the dark block only re-points colours; it declares no layout.
    for layout in ["grid-template", "max-width:", "padding:", "font-size:"] {
        assert!(
            !dark.contains(layout),
            "dark block must override tokens only, found {layout:?}"
        );
    }

    // No external assets, no webfonts, no imports.
    assert!(!css.contains("@import"), "stylesheet must not @import");
    assert!(
        !css.contains("@font-face"),
        "stylesheet must not load fonts"
    );
    assert!(
        !css.contains("http"),
        "stylesheet must reference nothing remote"
    );
    assert!(!css.contains("url("), "stylesheet must embed no assets");

    // Wide tables scroll in their own container rather than the page, with the
    // identifying columns pinned.
    assert!(css.contains(".scroll") && css.contains("overflow-x: auto"));
    assert!(css.contains("position: sticky"));
}

// ------------------------------------------------------------------ escaping

#[test]
fn esc_escapes_all_metachars() {
    assert_eq!(esc("<a>&\"'"), "&lt;a&gt;&amp;&quot;&#39;");
}

#[test]
fn render_escapes_injected_label() {
    // A title with markup must not leak a raw tag into the output, in the body
    // or in any of the social-card meta attributes.
    let dir = fresh_dir("xss");
    write_all(&dir, score_for);
    let cells = load_all(&dir).unwrap();
    let html = render_page(&cells, "<script>boom</script>\" onload=\"x");
    assert!(!html.contains("<script>boom</script>"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("onload=\"x"));
    assert!(html.contains("&lt;script&gt;boom&lt;/script&gt;&quot; onload=&quot;x"));
    // <title>, description-free og:title, twitter:title, and the eyebrow.
    assert_eq!(html.matches("&lt;script&gt;boom").count(), 4);
    let _ = fs::remove_dir_all(&dir);
}

/// Breakdown kind names come straight from the result JSON — the one genuinely
/// data-driven string in the table headers. They must be HTML-escaped so a
/// hostile result file cannot inject markup into the page.
#[test]
fn render_escapes_breakdown_kind_from_json() {
    let dir = fresh_dir("kindxss");
    let payload = "<img src=x onerror=alert(1)>";
    for eval in EVALS {
        for (i, (model, _)) in MODELS.iter().enumerate() {
            let score = score_for(eval, i);
            let path = dir.join(result_relpath(eval, model).unwrap());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            // Use a hostile breakdown key on every cell.
            let json = format!(
                r#"{{"eval":"{eval}","model":"{model}","n":10,
                    "metrics":{{"mean_score":{score}}},
                    "breakdown":{{"{payload}":{{"n":5.0,"mean_score":{score}}}}}}}"#
            );
            fs::write(path, json).unwrap();
        }
    }
    let cells = load_all(&dir).unwrap();
    let html = render_page(&cells, "kinds");
    assert!(
        !html.contains(payload),
        "raw breakdown kind leaked into HTML"
    );
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- robustness

/// An empty/partial result set must not panic the renderer. `rank_axis`
/// returns an empty vec and the axis header falls back to `n = 0`.
#[test]
fn render_empty_cells_does_not_panic() {
    let cells = Cells::new();
    assert!(rank_axis("sycophancy", &cells).is_empty());
    assert!(find_inversion(&standings(&cells)).is_none());

    let html = render_page(&cells, "empty");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.ends_with("</html>\n"));
    // Every axis still renders, with an n = 0 floor.
    assert!(html.contains("n = 0"));
    assert!(html.contains("<li>0 items</li>"));
    for spec in AXES.iter() {
        assert!(
            html.contains(&format!("<h2>{}", spec.title)),
            "missing axis {}",
            spec.title
        );
    }
    // No data means no marks, no slope lines, and no invented claim.
    assert!(!html.contains("<polyline"));
    assert!(html.contains(&esc(HEADLINE_NO_INVERSION)));
    assert_every_th_has_scope(&html);
}

/// A win that vanishes at the printed precision is still ranked, but the two
/// cells print the same number — the page must not show one of two identical
/// numbers in bold with no visible reason.
#[test]
fn a_win_invisible_at_the_printed_precision_is_not_flagged() {
    // 0.85003 vs 0.84997: distinct scores, identical at 3dp, distinct at 4dp.
    let html = render_fixture("rounding", |_, idx| [0.8500_3, 0.8499_7][idx]);

    // The per-kind table prints 3dp: both cells read 0.850, and neither is
    // marked, because `lead` is only ever applied through the ranking.
    assert!(html.contains("<td class=\"num\">0.850</td>"));
    assert!(!html.contains("<td class=\"num lead\">0.850</td>"));

    // The headline table prints 4dp, where the difference is legible, and the
    // rank-1 row carries the emphasis.
    assert!(html.contains("<td class=\"num score\">0.8500</td>"));
    assert!(html.contains("<td class=\"num score\">0.8500</td>"));
    assert!(html.contains("<tr class=\"lead\">"));
    // The summary gap column shows the difference is real but tiny.
    assert!(html.contains("<td class=\"num gap\">0.0001</td>"));
}

// --------------------------------------------------------------- determinism

/// Output must diff cleanly in version control: same input, same bytes.
#[test]
fn render_is_deterministic() {
    let dir = fresh_dir("determ");
    write_all(&dir, score_for);
    let cells = load_all(&dir).unwrap();
    let a = render_page(&cells, "t");
    let b = render_page(&cells, "t");
    assert_eq!(a, b, "two renders of the same cells must be byte-identical");
    // ...and it survives a reload from disk, not just a re-render in memory.
    let reloaded = load_all(&dir).unwrap();
    assert_eq!(a, render_page(&reloaded, "t"));
    let _ = fs::remove_dir_all(&dir);
}
