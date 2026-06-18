//! Integration tests: write a throwaway results dir, then exercise loading,
//! ranking, escaping, and a full render against it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use candor_leaderboard::canonical::{result_relpath, EVALS, MODELS};
use candor_leaderboard::data::{load_all, load_report, rank_axis, Cells};
use candor_leaderboard::render::{esc, render_page};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique temp dir per call (tests run in parallel threads).
fn fresh_dir(tag: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("candor_lb_{tag}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_report(dir: &Path, eval: &str, model: &str, score: f64) {
    let path = dir.join(result_relpath(eval, model).unwrap());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Include a `results` array to prove parsing skips it.
    let json = format!(
        r#"{{"eval":"{eval}","model":"{model}","n":10,
            "metrics":{{"mean_score":{score}}},
            "breakdown":{{"k1":{{"n":5.0,"mean_score":{score}}}}},
            "results":[{{"sample_id":"x","calls":[]}}]}}"#
    );
    fs::write(path, json).unwrap();
}

fn score_for(eval: &str, idx: usize) -> f64 {
    // MODELS order is [grok-4.3, grok-4.20-multi-agent] -> idx 0,1.
    match eval {
        "sycophancy" => [0.80, 1.00][idx], // distinct -> multi-agent first
        "factuality" => 1.00,              // both tied at the top
        _ => 0.85 + 0.01 * idx as f64,
    }
}

#[test]
fn ranks_and_renders() {
    let dir = fresh_dir("full");
    for eval in EVALS {
        for (i, (model, _)) in MODELS.iter().enumerate() {
            write_report(&dir, eval, model, score_for(eval, i));
        }
    }
    let cells = load_all(&dir).unwrap();
    assert_eq!(cells.len(), EVALS.len() * MODELS.len());

    // Distinct scores -> grok-4.20-multi-agent(1) > grok-4.3(2).
    let syc = rank_axis("sycophancy", &cells);
    assert_eq!((syc[0].rank, syc[0].label), (1, "grok-4.20-multi-agent"));
    assert_eq!((syc[1].rank, syc[1].label), (2, "grok-4.3"));

    // All equal -> everyone shares rank 1.
    let fac = rank_axis("factuality", &cells);
    assert!(fac.iter().all(|r| r.rank == 1));

    let html = render_page(&cells, "candor-bench: per-axis honesty leaderboard");
    assert!(html.starts_with("<!doctype html>"));
    for title in ["Sycophancy", "Pressure", "Calibration", "Factuality"] {
        assert!(
            html.contains(&format!("<h2>{title}")),
            "missing axis {title}"
        );
    }
    assert!(html.contains("grok-4.3"));
    assert!(html.contains("Per-kind breakdown"));
    // The page must never present a blended/overall honesty number.
    assert!(!html.to_lowercase().contains("overall honesty"));

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

    let _ = fs::remove_dir_all(&dir);
}

/// An empty/partial result set must not panic the renderer. `rank_axis`
/// returns an empty vec and the axis header falls back to `n = 0`.
#[test]
fn render_empty_cells_does_not_panic() {
    let cells = Cells::new();
    assert!(rank_axis("sycophancy", &cells).is_empty());
    let html = render_page(&cells, "empty");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.ends_with("</html>\n"));
    // Every axis still renders, with an n = 0 floor.
    assert!(html.contains("n = 0"));
    for title in ["Sycophancy", "Pressure", "Calibration", "Factuality"] {
        assert!(
            html.contains(&format!("<h2>{title}")),
            "missing axis {title}"
        );
    }
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

#[test]
fn esc_escapes_all_metachars() {
    assert_eq!(esc("<a>&\"'"), "&lt;a&gt;&amp;&quot;&#39;");
}

#[test]
fn render_escapes_injected_label() {
    // A title with markup must not leak a raw tag into the output.
    let dir = fresh_dir("xss");
    for eval in EVALS {
        for (i, (model, _)) in MODELS.iter().enumerate() {
            write_report(&dir, eval, model, score_for(eval, i));
        }
    }
    let cells = load_all(&dir).unwrap();
    let html = render_page(&cells, "<script>boom</script>");
    assert!(!html.contains("<script>boom</script>"));
    assert!(html.contains("&lt;script&gt;boom&lt;/script&gt;"));
    let _ = fs::remove_dir_all(&dir);
}
