//! Hand-rolled HTML rendering.
//!
//! Output is built as a single `String`. All dynamic text is routed through
//! [`esc`] (the analogue of plot_results.py's `xml.sax.saxutils.escape`), and
//! all numbers through [`fnum`] / [`dnum`]. There is no templating crate and no
//! JavaScript — collapsible breakdowns use native `<details>`, and the inline
//! SVG marks come from [`crate::marks`].
//!
//! The page leads with the finding rather than the product name: the `<h1>` is
//! a plain sentence generated from [`crate::inversion`], and the product name
//! is demoted to a small eyebrow above it (it stays the `<title>`). Vocabulary
//! for that sentence lives in [`crate::canonical`]; this module only lays out
//! what the data says.

use std::fmt::Write;

use crate::canonical::{
    AxisSpec, Better, AXES, CAVEATS, DISSOCIATION_NOTE, DIVERGENCE_SLICES, HEADLINE_NO_INVERSION,
    MODELS, REPO_URL, RUN_FACTS, SITE_DESCRIPTION, SITE_URL, STANDFIRST_NO_INVERSION, TIE_NOTE,
};
use crate::data::{cell, present_kinds, Cells, Ranked};
use crate::inversion::{find_inversion, ranked_for, standings, Inversion, Standings};
use crate::marks::{dotplot, series_class, slopegraph, sparkbar, SlopeSeries};

const STYLE_CSS: &str = include_str!("assets/style.css");

/// Favicon: four bars for four axes, inline so the page pulls no external
/// asset. Percent-encoded with no `<`, `>`, `&`, `"` or `'`, so [`esc`] is a
/// no-op over it.
const FAVICON: &str = "data:image/svg+xml,\
    %3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20viewBox=%220%200%2016%2016%22%3E\
    %3Cg%20fill=%22%23808080%22%3E\
    %3Crect%20x=%221%22%20y=%226%22%20width=%223%22%20height=%229%22/%3E\
    %3Crect%20x=%225%22%20y=%223%22%20width=%223%22%20height=%2212%22/%3E\
    %3Crect%20x=%229%22%20y=%228%22%20width=%223%22%20height=%227%22/%3E\
    %3Crect%20x=%2213%22%20y=%225%22%20width=%223%22%20height=%2210%22/%3E\
    %3C/g%3E%3C/svg%3E";

/// Background colours mirrored from `style.css` for `<meta name="theme-color">`.
const THEME_LIGHT: &str = "#fdfdfb";
const THEME_DARK: &str = "#0d0e0f";

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

/// Fixed-decimal formatting with an explicit sign. A value that rounds to zero
/// prints unsigned, so a column of deltas never shows a misleading `-0.0000`.
pub fn dnum(v: f64, dp: usize) -> String {
    let zero = format!("{:.*}", dp, 0.0);
    let s = format!("{:.*}", dp, v);
    if s == zero || s == format!("-{zero}") {
        zero
    } else if v > 0.0 {
        format!("+{s}")
    } else {
        s
    }
}

/// The sample count for an axis, taken from the first model that has the cell.
fn axis_n(eval: &str, cells: &Cells) -> u32 {
    MODELS
        .iter()
        .find_map(|(model, _)| cell(cells, eval, model).map(|r| r.n))
        .unwrap_or(0)
}

/// The per-kind sample count for a slice, taken from the first model that has it.
fn kind_n(eval: &str, kind: &str, cells: &Cells) -> Option<f64> {
    MODELS
        .iter()
        .find_map(|(model, _)| cell(cells, eval, model).and_then(|r| r.kind_n(kind)))
}

/// Mark series index for a model id, so every chart colours it identically.
fn series_of(model: &str) -> usize {
    MODELS
        .iter()
        .position(|(m, _)| *m == model)
        .unwrap_or(usize::MAX)
}

/// Highest minus lowest across the present values — the "gap" column. `None`
/// when fewer than two models reported, where a gap would be meaningless.
fn gap(values: &[Option<f64>]) -> Option<f64> {
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    if present.len() < 2 {
        return None;
    }
    let lo = present.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = present.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(hi - lo)
}

/// A numeric column header: label, direction glyph, and the direction spelled
/// out for screen readers. Meaning never rides on the glyph alone.
fn num_th(label: &str, better: Better) -> String {
    format!(
        "<th scope=\"col\" class=\"num\">{}<span class=\"hint\" aria-hidden=\"true\">{}</span>\
         <span class=\"vh\"> ({})</span></th>",
        esc(label),
        esc(better.mark()),
        esc(better.hint())
    )
}

// ------------------------------------------------------------- generated copy

/// The `<h1>`: the finding, in one plain declarative sentence.
///
/// `up` leads `axis_a` and trails `axis_b`, and the two per-axis predicates are
/// written so that the good behaviour and its failure contrast with "but".
fn headline(inv: Option<&Inversion>) -> String {
    match inv {
        Some(i) => format!("{} {}, but {}.", i.up, i.axis_a.leads, i.axis_b.trails),
        None => HEADLINE_NO_INVERSION.to_string(),
    }
}

/// The standfirst: the same finding with its numbers and its sample sizes, and
/// the reason the page stops short of combining them.
fn standfirst(inv: Option<&Inversion>) -> String {
    let Some(i) = inv else {
        return STANDFIRST_NO_INVERSION.to_string();
    };
    format!(
        "On {} it scores {} against {}'s {}, over n = {}. On {} it scores {} against {}, over \
         n = {}. Each axis is ranked on its own; averaging them would cancel one of those results \
         against the other, so this page does not.",
        i.axis_a.title.to_lowercase(),
        fnum(i.up_a, 4),
        i.down,
        fnum(i.down_a, 4),
        i.n_a,
        i.axis_b.title.to_lowercase(),
        fnum(i.up_b, 4),
        fnum(i.down_b, 4),
        i.n_b,
    )
}

/// The claim under the summary matrix: the inversion stated as ranks and gaps.
fn inversion_claim(inv: Option<&Inversion>) -> Option<String> {
    let i = inv?;
    Some(format!(
        "{} ranks {} of {} on {} and {} of {} on {} — ahead by {} on one axis, behind by {} on \
         the other. A blended number would net those against each other and report neither.",
        i.up,
        i.rank_up_a,
        i.field,
        i.axis_a.title,
        i.rank_up_b,
        i.field,
        i.axis_b.title,
        fnum(i.lead_a(), 4),
        fnum(i.lead_b(), 4),
    ))
}

/// The divergence section's lead line, pointed at the axes that actually
/// inverted rather than at a fixed pair.
fn divergence_blurb(inv: Option<&Inversion>) -> String {
    match inv {
        Some(i) => format!(
            "The {} and {} rankings disagree. These are the two per-kind slices where that split \
             is sharpest — the lines cross.",
            i.axis_a.title.to_lowercase(),
            i.axis_b.title.to_lowercase(),
        ),
        None => "The two per-kind slices that separate the models the most.".to_string(),
    }
}

// -------------------------------------------------------------------- page

/// Render the complete `index.html` document.
pub fn render_page(cells: &Cells, title: &str) -> String {
    let stand = standings(cells);
    let inv = find_inversion(&stand);
    let inv = inv.as_ref();

    let mut h = String::with_capacity(48 * 1024);
    h.push_str(&render_head(title));
    h.push_str("<body>\n");
    h.push_str(&render_masthead(title, inv, cells));
    h.push_str("<main>\n");

    h.push_str(&render_summary(&stand, inv, cells));
    for (spec, ranked) in &stand {
        h.push_str(&render_axis(spec, ranked, cells));
    }
    h.push_str(&render_divergence(inv, cells));
    h.push_str(&render_caveats());

    h.push_str("</main>\n");
    h.push_str(&render_footer());
    h.push_str("</body>\n</html>\n");
    h
}

fn render_head(title: &str) -> String {
    let t = esc(title);
    let d = esc(SITE_DESCRIPTION);
    let url = esc(SITE_URL);

    let mut h = String::new();
    h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    h.push_str("<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(h, "<title>{t}</title>");
    let _ = writeln!(h, "<meta name=\"description\" content=\"{d}\">");
    h.push_str("<meta name=\"color-scheme\" content=\"light dark\">\n");
    let _ = writeln!(h, "<link rel=\"canonical\" href=\"{url}\">");
    h.push_str("<meta property=\"og:type\" content=\"website\">\n");
    let _ = writeln!(h, "<meta property=\"og:title\" content=\"{t}\">");
    let _ = writeln!(h, "<meta property=\"og:description\" content=\"{d}\">");
    let _ = writeln!(h, "<meta property=\"og:url\" content=\"{url}\">");
    h.push_str("<meta name=\"twitter:card\" content=\"summary\">\n");
    let _ = writeln!(h, "<meta name=\"twitter:title\" content=\"{t}\">");
    let _ = writeln!(h, "<meta name=\"twitter:description\" content=\"{d}\">");
    let _ = writeln!(
        h,
        "<meta name=\"theme-color\" media=\"(prefers-color-scheme: light)\" content=\"{THEME_LIGHT}\">"
    );
    let _ = writeln!(
        h,
        "<meta name=\"theme-color\" media=\"(prefers-color-scheme: dark)\" content=\"{THEME_DARK}\">"
    );
    let _ = writeln!(h, "<link rel=\"icon\" href=\"{}\">", esc(FAVICON));
    h.push_str("<link rel=\"stylesheet\" href=\"style.css\">\n");
    h.push_str("</head>\n");
    h
}

/// Masthead: product name as an eyebrow, the finding as the `<h1>`, the numbers
/// behind it as the standfirst, then the run conditions.
fn render_masthead(title: &str, inv: Option<&Inversion>, cells: &Cells) -> String {
    let mut h = String::new();
    h.push_str("<header>\n");
    let _ = writeln!(h, "<p class=\"eyebrow\">{}</p>", esc(title));
    let _ = writeln!(h, "<h1>{}</h1>", esc(&headline(inv)));
    let _ = writeln!(h, "<p class=\"standfirst\">{}</p>", esc(&standfirst(inv)));

    let items: u32 = AXES.iter().map(|s| axis_n(s.eval, cells)).sum();
    h.push_str("<ul class=\"meta\">\n");
    let _ = writeln!(h, "<li>{} xAI models</li>", MODELS.len());
    let _ = writeln!(h, "<li>{items} items</li>");
    for fact in RUN_FACTS {
        let _ = writeln!(h, "<li>{}</li>", esc(fact));
    }
    h.push_str("</ul>\n</header>\n");
    h
}

/// The model key shared by every mark on the page.
fn render_legend() -> String {
    let mut h = String::from("<ul class=\"legend\">\n");
    for (i, (_, label)) in MODELS.iter().enumerate() {
        let _ = writeln!(
            h,
            "<li class=\"key {}\">{}</li>",
            series_class(i),
            esc(label)
        );
    }
    h.push_str("</ul>\n");
    h
}

/// The one table that answers "who leads what" — every axis, every model, the
/// size of the gap, and a shared 0..1 dot plot so the dissociation is visible
/// without arithmetic.
fn render_summary(stand: &Standings, inv: Option<&Inversion>, cells: &Cells) -> String {
    let mut h = String::new();
    h.push_str("<section id=\"summary\">\n<h2>Four axes, four rankings</h2>\n");
    // The caption is visually hidden and repeated as a paragraph above the
    // scroll container: a visible <caption> inherits the table's width, so on a
    // narrow screen it would be clipped by the container instead of wrapping.
    h.push_str(
        "<p class=\"blurb\">Mean score per axis, 0 to 1, higher is better. Bold marks the axis \
         leader.</p>\n",
    );
    h.push_str("<div class=\"scroll\">\n<table class=\"summary\">\n");
    h.push_str(
        "<caption class=\"vh\">Mean score per axis, 0 to 1, higher is better. Bold marks the axis \
         leader.</caption>\n",
    );
    h.push_str("<thead>\n<tr><th scope=\"col\">axis</th><th scope=\"col\" class=\"num\">n</th>");
    for (_, label) in MODELS {
        let _ = write!(h, "<th scope=\"col\" class=\"num\">{}</th>", esc(label));
    }
    h.push_str(
        "<th scope=\"col\" class=\"num\">gap\
         <span class=\"vh\"> (highest score minus lowest)</span></th>",
    );
    h.push_str("<th scope=\"col\" class=\"plot\">0<span class=\"hi\">1</span></th>");
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for (spec, ranked) in stand {
        let _ = write!(
            h,
            "<tr><th scope=\"row\"><a href=\"#{}\">{}</a></th>",
            esc(spec.eval),
            esc(spec.title)
        );
        let _ = write!(h, "<td class=\"num\">{}</td>", axis_n(spec.eval, cells));

        let mut values: Vec<Option<f64>> = Vec::with_capacity(MODELS.len());
        for (model, _) in MODELS {
            match ranked_for(ranked, model) {
                Some(r) => {
                    let lead = if r.rank == 1 { " lead" } else { "" };
                    let _ = write!(h, "<td class=\"num{lead}\">{}</td>", fnum(r.score, 4));
                    values.push(Some(r.score));
                }
                None => {
                    h.push_str("<td class=\"num\">—</td>");
                    values.push(None);
                }
            }
        }
        match gap(&values) {
            Some(g) => {
                let _ = write!(h, "<td class=\"num gap\">{}</td>", fnum(g, 4));
            }
            None => h.push_str("<td class=\"num gap\">—</td>"),
        }
        let _ = writeln!(h, "<td class=\"plot\">{}</td></tr>", dotplot(&values));
    }
    h.push_str("</tbody>\n</table>\n</div>\n");
    h.push_str(&render_legend());

    if let Some(claim) = inversion_claim(inv) {
        let _ = writeln!(h, "<p class=\"claim\">{}</p>", esc(&claim));
        let _ = writeln!(h, "<p class=\"aside\">{}</p>", esc(DISSOCIATION_NOTE));
    }
    h.push_str("</section>\n");
    h
}

fn render_axis(spec: &AxisSpec, ranked: &[Ranked], cells: &Cells) -> String {
    let n = axis_n(spec.eval, cells);

    let mut h = String::new();
    let _ = writeln!(h, "<section class=\"axis\" id=\"{}\">", esc(spec.eval));
    let _ = writeln!(
        h,
        "<h2>{} <span class=\"n\">n = {}</span></h2>",
        esc(spec.title),
        n
    );
    let _ = writeln!(h, "<p class=\"blurb\">{}</p>", esc(spec.blurb));

    h.push_str("<div class=\"axis-body\">\n<div class=\"scroll\">\n");
    h.push_str(&render_headline_table(ranked, spec));
    h.push_str("</div>\n<aside class=\"sidenote\">\n");
    let _ = writeln!(h, "<p>{}</p>", esc(spec.reading));
    if ranked.len() > 1 && ranked.iter().all(|r| r.rank == 1) {
        let _ = writeln!(h, "<p>{}</p>", esc(TIE_NOTE));
    }
    h.push_str("</aside>\n</div>\n");

    h.push_str(&render_breakdown(spec, cells));
    h.push_str("</section>\n");
    h
}

fn render_headline_table(ranked: &[Ranked], spec: &AxisSpec) -> String {
    // The delta column earns its place where the table is otherwise narrow: it
    // saves the reader a subtraction. On calibration and factuality, which
    // already carry four to eight extra metric columns, it would only add
    // width — the summary matrix's `gap` column covers those axes instead.
    let with_delta = spec.extra_metrics.is_empty();
    let wide = if spec.extra_metrics.is_empty() {
        ""
    } else {
        " wide"
    };
    let top = ranked.first().map(|r| r.score);

    let mut h = String::new();
    let _ = writeln!(h, "<table class=\"headline{wide}\">");
    let _ = writeln!(
        h,
        "<caption class=\"vh\">{} ranking by mean score</caption>",
        esc(spec.title)
    );
    h.push_str("<thead>\n<tr>");
    h.push_str("<th scope=\"col\" class=\"rank\">#</th>");
    h.push_str("<th scope=\"col\" class=\"model\">model</th>");
    h.push_str(&num_th("mean score", Better::Higher));
    h.push_str("<th scope=\"col\" class=\"bar\"><span class=\"vh\">mean score, 0 to 1</span></th>");
    if with_delta {
        h.push_str(
            "<th scope=\"col\" class=\"num\">Δ #1\
             <span class=\"vh\"> (difference from the rank-1 score)</span></th>",
        );
    }
    for m in spec.extra_metrics {
        h.push_str(&num_th(m.label, m.better));
    }
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for (i, r) in ranked.iter().enumerate() {
        let cls = if r.rank == 1 { " class=\"lead\"" } else { "" };
        let _ = write!(h, "<tr{cls}>");
        let _ = write!(h, "<td class=\"rank\">{}</td>", r.rank);
        let _ = write!(h, "<th scope=\"row\">{}</th>", esc(r.label));
        let _ = write!(h, "<td class=\"num score\">{}</td>", fnum(r.score, 4));
        let _ = write!(
            h,
            "<td class=\"bar\">{}</td>",
            sparkbar(r.score, series_of(r.model))
        );
        if with_delta {
            match top {
                Some(t) if i > 0 => {
                    let _ = write!(h, "<td class=\"num delta\">{}</td>", dnum(r.score - t, 4));
                }
                _ => h.push_str("<td class=\"num delta ref\">ref</td>"),
            }
        }
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
    h.push_str("<div class=\"scroll\">\n<table class=\"kinds\">\n");
    let _ = writeln!(
        h,
        "<caption class=\"vh\">{} mean score by kind</caption>",
        esc(spec.title)
    );
    h.push_str("<thead>\n<tr><th scope=\"col\" class=\"model\">model</th>");
    for k in &kinds {
        let _ = write!(h, "<th scope=\"col\" class=\"num\">{}</th>", esc(k));
    }
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for (model, label) in MODELS {
        let rep = match cell(cells, spec.eval, model) {
            Some(r) => r,
            None => continue,
        };
        let _ = write!(h, "<tr><th scope=\"row\">{}</th>", esc(label));
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

    h.push_str("<tr class=\"nrow\"><th scope=\"row\">n</th>");
    for k in &kinds {
        match kind_n(spec.eval, k, cells) {
            Some(v) => {
                let _ = write!(h, "<td class=\"num\">{}</td>", v as u64);
            }
            None => h.push_str("<td class=\"num\">—</td>"),
        }
    }
    h.push_str("</tr>\n</tbody>\n</table>\n</div>\n</details>\n");
    h
}

fn render_divergence(inv: Option<&Inversion>, cells: &Cells) -> String {
    let cols: Vec<(&str, &str)> = DIVERGENCE_SLICES.iter().map(|s| (s.kind, s.eval)).collect();
    let series: Vec<SlopeSeries<'_>> = MODELS
        .iter()
        .enumerate()
        .map(|(i, (model, label))| SlopeSeries {
            label,
            series: i,
            values: DIVERGENCE_SLICES
                .iter()
                .map(|s| cell(cells, s.eval, model).and_then(|r| r.kind_score(s.kind)))
                .collect(),
        })
        .collect();

    let mut h = String::new();
    h.push_str("<section class=\"divergence\" id=\"divergence\">\n");
    h.push_str("<h2>Where the models diverge</h2>\n");
    let _ = writeln!(h, "<p class=\"blurb\">{}</p>", esc(&divergence_blurb(inv)));
    h.push_str("<div class=\"divergence-body\">\n<figure>\n");
    h.push_str(&slopegraph(&cols, &series));
    h.push_str(
        "<figcaption>Per-kind mean score on a 0 to 1 scale, higher is better. One line \
                per model.</figcaption>\n",
    );
    h.push_str(&render_legend());
    h.push_str("</figure>\n<div class=\"scroll\">\n<table class=\"slices\">\n");
    h.push_str("<caption class=\"vh\">Per-kind mean score for the two widest splits</caption>\n");
    h.push_str("<thead>\n<tr><th scope=\"col\" class=\"model\">model</th>");
    for s in DIVERGENCE_SLICES.iter() {
        let _ = write!(
            h,
            "<th scope=\"col\" class=\"num\">{}<span class=\"sub\">{}</span></th>",
            esc(s.kind),
            esc(s.eval)
        );
    }
    h.push_str("</tr>\n</thead>\n<tbody>\n");

    for (model, label) in MODELS {
        let _ = write!(h, "<tr><th scope=\"row\">{}</th>", esc(label));
        for s in DIVERGENCE_SLICES.iter() {
            match cell(cells, s.eval, model).and_then(|r| r.kind_score(s.kind)) {
                Some(v) => {
                    let _ = write!(h, "<td class=\"num\">{}</td>", fnum(v, 3));
                }
                None => h.push_str("<td class=\"num\">—</td>"),
            }
        }
        h.push_str("</tr>\n");
    }
    h.push_str("<tr class=\"grow\"><th scope=\"row\">gap</th>");
    for s in DIVERGENCE_SLICES.iter() {
        let vals: Vec<Option<f64>> = MODELS
            .iter()
            .map(|(model, _)| cell(cells, s.eval, model).and_then(|r| r.kind_score(s.kind)))
            .collect();
        match gap(&vals) {
            Some(g) => {
                let _ = write!(h, "<td class=\"num gap\">{}</td>", fnum(g, 3));
            }
            None => h.push_str("<td class=\"num gap\">—</td>"),
        }
    }
    h.push_str("</tr>\n<tr class=\"nrow\"><th scope=\"row\">n</th>");
    for s in DIVERGENCE_SLICES.iter() {
        match kind_n(s.eval, s.kind, cells) {
            Some(v) => {
                let _ = write!(h, "<td class=\"num\">{}</td>", v as u64);
            }
            None => h.push_str("<td class=\"num\">—</td>"),
        }
    }
    h.push_str("</tr>\n</tbody>\n</table>\n</div>\n</div>\n</section>\n");
    h
}

fn render_caveats() -> String {
    let mut h = String::new();
    h.push_str("<section id=\"caveats\">\n<h2>What this cannot tell you</h2>\n");
    h.push_str("<ul class=\"caveats\">\n");
    for (lead, body) in CAVEATS {
        let _ = writeln!(h, "<li><strong>{}</strong> {}</li>", esc(lead), esc(body));
    }
    h.push_str("</ul>\n</section>\n");
    h
}

fn render_footer() -> String {
    format!(
        "<footer>candor-bench · <a href=\"{}\">source and methodology</a> · rendered from \
         results/*.json · deterministic output, no timestamps</footer>\n",
        esc(REPO_URL)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnum_signs_every_value_and_never_prints_negative_zero() {
        assert_eq!(dnum(0.5, 4), "+0.5000");
        assert_eq!(dnum(-0.5, 4), "-0.5000");
        assert_eq!(dnum(0.0, 4), "0.0000");
        assert_eq!(dnum(-0.0, 4), "0.0000");
        assert_eq!(dnum(-1e-9, 4), "0.0000");
        assert_eq!(dnum(1e-9, 4), "0.0000");
        assert_eq!(dnum(-0.12345, 3), "-0.123");
        assert_eq!(dnum(0.12345, 3), "+0.123");
    }

    #[test]
    fn gap_needs_two_present_values() {
        assert_eq!(gap(&[]), None);
        assert_eq!(gap(&[Some(0.5)]), None);
        assert_eq!(gap(&[Some(0.5), None]), None);
        assert_eq!(gap(&[Some(0.25), Some(0.75)]), Some(0.5));
        assert_eq!(gap(&[Some(0.75), Some(0.25)]), Some(0.5));
        assert_eq!(gap(&[Some(0.4), Some(0.4)]), Some(0.0));
    }

    #[test]
    fn num_th_pairs_every_glyph_with_hidden_text() {
        for b in [
            Better::Higher,
            Better::Lower,
            Better::TowardZero,
            Better::Neutral,
        ] {
            let th = num_th("x", b);
            assert!(th.contains("scope=\"col\""));
            assert!(th.contains(&format!("aria-hidden=\"true\">{}</span>", b.mark())));
            assert!(th.contains(&format!("<span class=\"vh\"> ({})</span>", b.hint())));
        }
    }

    #[test]
    fn generated_copy_degrades_without_an_inversion() {
        assert_eq!(headline(None), HEADLINE_NO_INVERSION);
        assert_eq!(standfirst(None), STANDFIRST_NO_INVERSION);
        assert_eq!(inversion_claim(None), None);
        assert!(!divergence_blurb(None).is_empty());
        // No half-built sentence with a dangling name or number.
        for s in [headline(None), standfirst(None), divergence_blurb(None)] {
            assert!(s.ends_with('.'), "unterminated fallback: {s:?}");
            assert!(!s.contains("  "), "collapsed placeholder: {s:?}");
        }
    }
}
