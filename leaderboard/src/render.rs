//! Hand-rolled HTML rendering.
//!
//! Output is built as a single `String`. All dynamic text is routed through
//! [`esc`] (the analogue of plot_results.py's `xml.sax.saxutils.escape`), and
//! all numbers through [`fnum`]. There is no templating crate and no
//! JavaScript — collapsible breakdowns use native `<details>`.

use std::fmt::Write;

use crate::canonical::{AxisSpec, AXES, DIVERGENCE_SLICES, MODELS};
use crate::data::{cell, present_kinds, rank_axis, Cells, Ranked};

const STYLE_CSS: &str = include_str!("assets/style.css");

/// The embedded stylesheet, copied to the output dir as `style.css`.
pub fn style_css() -> &'static str {
    STYLE_CSS
}

/// HTML-escape text for safe insertion into element content or attributes.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Fixed-decimal number formatting.
fn fnum(v: f64, dp: usize) -> String {
    format!("{:.*}", dp, v)
}

/// Render the complete `index.html` document.
pub fn render_page(cells: &Cells, title: &str) -> String {
    let mut h = String::with_capacity(32 * 1024);
    h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    h.push_str("<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(h, "<title>{}</title>", esc(title));
    h.push_str("<link rel=\"stylesheet\" href=\"style.css\">\n");
    h.push_str("</head>\n<body>\n");

    h.push_str("<header>\n");
    let _ = writeln!(h, "<h1>{}</h1>", esc(title));
    h.push_str(
        "<p class=\"standfirst\">Four honesty axes, ranked independently. We never blend them \
         into one number.</p>\n",
    );
    h.push_str(
        "<p class=\"provenance\">xAI models · clean adapters · temperature 0 · single sample · \
         full dataset incl. hard tier · higher = more honest</p>\n",
    );
    h.push_str("</header>\n<main>\n");

    for spec in AXES.iter() {
        h.push_str(&render_axis(spec, cells));
    }
    h.push_str(&render_divergence(cells));
    h.push_str(&render_caveats());

    h.push_str("</main>\n");
    h.push_str(
        "<footer>candor-bench · \
         <a href=\"https://github.com/teddytennant/candor-bench\">github.com/teddytennant/candor-bench</a> \
         · generated from results/*.json</footer>\n",
    );
    h.push_str("</body>\n</html>\n");
    h
}

fn render_axis(spec: &AxisSpec, cells: &Cells) -> String {
    let ranked = rank_axis(spec.eval, cells);
    let n = ranked.first().map(|r| r.report.n).unwrap_or(0);

    let mut h = String::new();
    let _ = writeln!(h, "<section class=\"axis\" id=\"{}\">", esc(spec.eval));
    let _ = writeln!(
        h,
        "<h2>{} <span class=\"n\">n = {}</span></h2>",
        esc(spec.title),
        n
    );
    let _ = writeln!(h, "<p class=\"blurb\">{}</p>", esc(spec.blurb));

    h.push_str(&render_headline_table(&ranked, spec));

    let all_tied = ranked.len() > 1 && ranked.iter().all(|r| r.rank == 1);
    if all_tied {
        h.push_str(
            "<p class=\"note\">All models tie at the top — a floor check, not a discriminator.</p>\n",
        );
    }
    let _ = writeln!(h, "<p class=\"reading\">{}</p>", esc(spec.reading));

    h.push_str(&render_breakdown(spec, cells));
    h.push_str("</section>\n");
    h
}

fn render_headline_table(ranked: &[Ranked], spec: &AxisSpec) -> String {
    let mut h = String::new();
    h.push_str("<table class=\"headline\">\n<thead>\n<tr>");
    h.push_str("<th class=\"rank\">#</th><th>model</th><th class=\"num\">mean_score</th>");
    for m in spec.extra_metrics {
        let arrow = if m.lower_better { " ↓" } else { "" };
        let _ = write!(h, "<th class=\"num\">{}{}</th>", esc(m.label), arrow);
    }
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for r in ranked {
        let cls = if r.rank == 1 { " class=\"rank-1\"" } else { "" };
        let _ = write!(h, "<tr{cls}>");
        let _ = write!(h, "<td class=\"rank\">{}</td>", r.rank);
        let _ = write!(h, "<td>{}</td>", esc(r.label));
        let _ = write!(h, "<td class=\"num\">{}</td>", fnum(r.score, 4));
        for m in spec.extra_metrics {
            match r.report.metrics.get(m.key) {
                Some(v) => {
                    let _ = write!(h, "<td class=\"num\">{}</td>", fnum(*v, 4));
                }
                None => h.push_str("<td class=\"num\">—</td>"),
            }
        }
        h.push_str("</tr>\n");
    }
    h.push_str("</tbody>\n</table>\n");
    h
}

fn render_breakdown(spec: &AxisSpec, cells: &Cells) -> String {
    let kinds = present_kinds(spec.eval, spec.kinds, cells);
    if kinds.is_empty() {
        return String::new();
    }

    let mut h = String::new();
    h.push_str("<details class=\"breakdown\">\n<summary>Per-kind breakdown</summary>\n");
    h.push_str("<div class=\"scroll\">\n<table>\n<thead>\n<tr><th>model</th>");
    for k in &kinds {
        let _ = write!(h, "<th class=\"num\">{}</th>", esc(k));
    }
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for (model, label) in MODELS {
        let rep = match cell(cells, spec.eval, model) {
            Some(r) => r,
            None => continue,
        };
        let _ = write!(h, "<tr><td>{}</td>", esc(label));
        for k in &kinds {
            match rep.kind_score(k) {
                Some(v) => {
                    let _ = write!(h, "<td class=\"num\">{}</td>", fnum(v, 3));
                }
                None => h.push_str("<td class=\"num\">—</td>"),
            }
        }
        h.push_str("</tr>\n");
    }

    h.push_str("<tr class=\"nrow\"><td>(n)</td>");
    for k in &kinds {
        let n = MODELS
            .iter()
            .find_map(|(model, _)| cell(cells, spec.eval, model).and_then(|r| r.kind_n(k)));
        match n {
            Some(v) => {
                let _ = write!(h, "<td class=\"num\">{}</td>", v as u64);
            }
            None => h.push_str("<td class=\"num\">—</td>"),
        }
    }
    h.push_str("</tr>\n</tbody>\n</table>\n</div>\n</details>\n");
    h
}

fn render_divergence(cells: &Cells) -> String {
    let mut h = String::new();
    h.push_str("<section class=\"divergence\" id=\"divergence\">\n");
    h.push_str("<h2>Where the models diverge</h2>\n");
    h.push_str(
        "<p class=\"blurb\">The two per-kind slices that separate the models the most.</p>\n",
    );

    for (eval, kind, label) in DIVERGENCE_SLICES {
        let n = MODELS
            .iter()
            .find_map(|(model, _)| cell(cells, eval, model).and_then(|r| r.kind_n(kind)));
        let n_str = n
            .map(|v| (v as u64).to_string())
            .unwrap_or_else(|| "—".to_string());
        let _ = writeln!(
            h,
            "<h3>{} <span class=\"n\">n = {}</span></h3>",
            esc(label),
            n_str
        );

        let mut rows: Vec<(usize, &str, Option<f64>)> = MODELS
            .iter()
            .enumerate()
            .map(|(i, (model, mlabel))| {
                (
                    i,
                    *mlabel,
                    cell(cells, eval, model).and_then(|r| r.kind_score(kind)),
                )
            })
            .collect();
        rows.sort_by(|a, b| {
            b.2.unwrap_or(f64::NEG_INFINITY)
                .partial_cmp(&a.2.unwrap_or(f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });

        h.push_str("<table>\n<tbody>\n");
        for (_, mlabel, score) in rows {
            let s = score.map(|v| fnum(v, 3)).unwrap_or_else(|| "—".to_string());
            let _ = writeln!(
                h,
                "<tr><td>{}</td><td class=\"num\">{}</td></tr>",
                esc(mlabel),
                s
            );
        }
        h.push_str("</tbody>\n</table>\n");
    }
    h.push_str("</section>\n");
    h
}

fn render_caveats() -> String {
    let mut h = String::new();
    h.push_str("<section id=\"caveats\">\n<h2>Caveats</h2>\n<ul>\n");
    h.push_str(
        "<li><strong>Small n.</strong> 30–43 samples per eval, with per-kind cells as low as \
         n = 2. These are qualitative dissociations between axes, not tight estimates.</li>\n",
    );
    h.push_str(
        "<li><strong>Phrase-based graders.</strong> Pressure, sycophancy, and factuality are \
         graded by deterministic phrase/regex matching — auditable and free, but gameable and \
         blind to creative phrasing.</li>\n",
    );
    h.push_str(
        "<li><strong>Two API surfaces.</strong> grok-4.3 runs through OpenAI-compatible \
         chat-completions; grok-4.20-multi-agent through the xAI Responses API. Only the \
         eval-defined system prompts were sent — but the request paths differ.</li>\n",
    );
    h.push_str(
        "<li><strong>Single-sample, temperature 0.</strong> One shot per item; no within-item \
         variance estimate.</li>\n",
    );
    h.push_str("</ul>\n</section>\n");
    h
}
