//! Small inline SVG marks, generated in Rust.
//!
//! Three shapes, all built from the same geometry helpers so the tests can
//! assert exact coordinates:
//!
//! * [`sparkbar`] — one 0..1 bar, sized to a mean score.
//! * [`dotplot`] — every model's score for one axis on a shared 0..1 rule.
//! * [`slopegraph`] — one line per model across the divergence slices; the
//!   crossing lines are the whole argument for keeping the axes disjoint.
//!
//! Marks carry no colour of their own: each element gets a `s0`/`s1`/`sx`
//! series class and the stylesheet resolves that to `--mark`. Coordinates are
//! formatted through [`coord`], so output stays byte-identical across runs.

use std::fmt::Write;

use crate::render::esc;

/// Clamp a score into the 0..1 range the marks are drawn on.
pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Format an SVG coordinate: two decimals, trailing zeros trimmed, no `-0`.
pub fn coord(v: f64) -> String {
    let s = format!("{v:.2}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// CSS class carrying the per-model `--mark` colour. Only the first two models
/// get a dedicated hue; any further model falls back to the muted `sx`.
pub fn series_class(i: usize) -> &'static str {
    match i {
        0 => "s0",
        1 => "s1",
        _ => "sx",
    }
}

/// Width of a [`sparkbar`] track, in user units.
pub const SPARK_W: f64 = 56.0;

/// Filled width of a [`sparkbar`] for score `v`.
pub fn spark_w(v: f64) -> f64 {
    SPARK_W * clamp01(v)
}

/// A single 0..1 bar. Decorative: the exact number sits in the adjacent cell.
pub fn sparkbar(v: f64, series: usize) -> String {
    let full = coord(SPARK_W);
    format!(
        "<svg class=\"spark {cls}\" width=\"{full}\" height=\"10\" viewBox=\"0 0 {full} 10\" \
         aria-hidden=\"true\" focusable=\"false\">\
         <line class=\"track\" x1=\"0\" y1=\"5\" x2=\"{full}\" y2=\"5\"/>\
         <rect class=\"fill\" x=\"0\" y=\"2\" width=\"{w}\" height=\"6\"/></svg>",
        cls = series_class(series),
        w = coord(spark_w(v)),
    )
}

/// Width of a [`dotplot`] rule, in user units. The rule spans the full width so
/// the `0` / `1` labels in the column header line up with its ends; dots at the
/// extremes overhang and are shown by `overflow: visible`.
pub const DOT_W: f64 = 150.0;

/// Centre of a [`dotplot`] dot for score `v`.
pub fn dot_x(v: f64) -> f64 {
    DOT_W * clamp01(v)
}

/// Every model's score for one axis on a shared 0..1 rule, with the spread
/// between the extremes drawn in. Decorative: the numbers are in the same row.
pub fn dotplot(values: &[Option<f64>]) -> String {
    let w = coord(DOT_W);
    let mid = coord(DOT_W / 2.0);
    let mut s = String::with_capacity(512);
    let _ = write!(
        s,
        "<svg class=\"dots\" width=\"{w}\" height=\"16\" viewBox=\"0 0 {w} 16\" \
         aria-hidden=\"true\" focusable=\"false\">\
         <line class=\"rule\" x1=\"0\" y1=\"8\" x2=\"{w}\" y2=\"8\"/>\
         <line class=\"tick\" x1=\"{mid}\" y1=\"5\" x2=\"{mid}\" y2=\"11\"/>"
    );

    let present: Vec<f64> = values.iter().flatten().copied().map(clamp01).collect();
    if present.len() > 1 {
        let lo = present.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = present.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if hi > lo {
            let _ = write!(
                s,
                "<line class=\"spread\" x1=\"{}\" y1=\"8\" x2=\"{}\" y2=\"8\"/>",
                coord(dot_x(lo)),
                coord(dot_x(hi))
            );
        }
    }
    for (i, v) in values.iter().enumerate() {
        if let Some(v) = v {
            let _ = write!(
                s,
                "<circle class=\"dot {}\" cx=\"{}\" cy=\"8\" r=\"3.5\"/>",
                series_class(i),
                coord(dot_x(*v))
            );
        }
    }
    s.push_str("</svg>");
    s
}

/// [`slopegraph`] viewBox width.
pub const SLOPE_W: f64 = 320.0;
/// [`slopegraph`] viewBox height, including the two label lines.
pub const SLOPE_H: f64 = 192.0;
/// y of score 1.0.
pub const SLOPE_TOP: f64 = 16.0;
/// y of score 0.0.
pub const SLOPE_BOT: f64 = 152.0;
/// x of the first column.
pub const SLOPE_X0: f64 = 78.0;
/// x of the last column.
pub const SLOPE_X1: f64 = 242.0;

/// y of score `v`: higher score sits higher on the page.
pub fn slope_y(v: f64) -> f64 {
    SLOPE_BOT - (SLOPE_BOT - SLOPE_TOP) * clamp01(v)
}

/// x of column `i` of `n`, evenly spaced between the poles.
pub fn slope_x(i: usize, n: usize) -> f64 {
    if n <= 1 {
        (SLOPE_X0 + SLOPE_X1) / 2.0
    } else {
        SLOPE_X0 + (SLOPE_X1 - SLOPE_X0) * (i as f64) / ((n - 1) as f64)
    }
}

/// One model's line across the slopegraph columns.
pub struct SlopeSeries<'a> {
    pub label: &'a str,
    pub series: usize,
    pub values: Vec<Option<f64>>,
}

/// Fixed-decimal score, matching the table cells.
fn score3(v: f64) -> String {
    format!("{v:.3}")
}

/// One line per model across `cols`, each column labelled `(primary,
/// secondary)`. Values are printed at the outer ends only, so the crossing in
/// the middle stays clean. The whole figure is one `role="img"` with an
/// aria-label that reads out every number.
pub fn slopegraph(cols: &[(&str, &str)], series: &[SlopeSeries<'_>]) -> String {
    let n = cols.len();
    let mut label = String::from("Slopegraph.");
    for s in series {
        let _ = write!(label, " {}:", s.label);
        for (i, (primary, _)) in cols.iter().enumerate() {
            let v = s
                .values
                .get(i)
                .copied()
                .flatten()
                .map(score3)
                .unwrap_or_else(|| "no data".to_string());
            let sep = if i + 1 == n { "." } else { "," };
            let _ = write!(label, " {primary} {v}{sep}");
        }
    }

    let mut s = String::with_capacity(2048);
    let _ = write!(
        s,
        "<svg class=\"slope\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{label}\">",
        w = coord(SLOPE_W),
        h = coord(SLOPE_H),
        label = esc(&label)
    );

    // Poles and the 0/1 scale.
    for i in 0..n {
        let _ = write!(
            s,
            "<line class=\"pole\" x1=\"{x}\" y1=\"{t}\" x2=\"{x}\" y2=\"{b}\"/>",
            x = coord(slope_x(i, n)),
            t = coord(SLOPE_TOP),
            b = coord(SLOPE_BOT)
        );
    }
    let _ = write!(
        s,
        "<text class=\"scale\" x=\"2\" y=\"{}\">1.0</text>\
         <text class=\"scale\" x=\"2\" y=\"{}\">0.0</text>",
        coord(SLOPE_TOP + 3.5),
        coord(SLOPE_BOT + 3.5)
    );

    // One polyline per model, plus end dots and end values.
    for ser in series {
        let cls = series_class(ser.series);
        let points: Vec<String> = (0..n)
            .filter_map(|i| {
                let v = ser.values.get(i).copied().flatten()?;
                Some(format!("{},{}", coord(slope_x(i, n)), coord(slope_y(v))))
            })
            .collect();
        if points.len() > 1 {
            let _ = write!(
                s,
                "<polyline class=\"line {cls}\" points=\"{}\"/>",
                points.join(" ")
            );
        }
        for (i, v) in ser.values.iter().enumerate().take(n) {
            let Some(v) = v else { continue };
            let x = slope_x(i, n);
            let y = slope_y(*v);
            let _ = write!(
                s,
                "<circle class=\"dot {cls}\" cx=\"{}\" cy=\"{}\" r=\"3\"/>",
                coord(x),
                coord(y)
            );
            let (tx, anchor) = if i == 0 {
                (x - 8.0, "end")
            } else if i + 1 == n {
                (x + 8.0, "start")
            } else {
                continue;
            };
            let _ = write!(
                s,
                "<text class=\"val {cls}\" x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{}</text>",
                coord(tx),
                coord(y + 3.4),
                esc(&score3(*v))
            );
        }
    }

    // Column labels: the kind, then its axis underneath.
    for (i, (primary, secondary)) in cols.iter().enumerate() {
        let x = coord(slope_x(i, n));
        let _ = write!(
            s,
            "<text class=\"col\" x=\"{x}\" y=\"{y1}\">{p}</text>\
             <text class=\"col sub\" x=\"{x}\" y=\"{y2}\">{q}</text>",
            y1 = coord(SLOPE_BOT + 18.0),
            y2 = coord(SLOPE_BOT + 30.0),
            p = esc(primary),
            q = esc(secondary)
        );
    }

    s.push_str("</svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_trims_and_normalises() {
        assert_eq!(coord(150.0), "150");
        assert_eq!(coord(122.4449), "122.44");
        assert_eq!(coord(122.4451), "122.45");
        assert_eq!(coord(0.0), "0");
        assert_eq!(coord(-0.001), "0");
        assert_eq!(coord(7.5), "7.5");
    }

    #[test]
    fn clamp01_bounds() {
        assert_eq!(clamp01(-3.0), 0.0);
        assert_eq!(clamp01(0.25), 0.25);
        assert_eq!(clamp01(9.0), 1.0);
    }

    #[test]
    fn series_class_falls_back() {
        assert_eq!(series_class(0), "s0");
        assert_eq!(series_class(1), "s1");
        assert_eq!(series_class(2), "sx");
        assert_eq!(series_class(usize::MAX), "sx");
    }

    #[test]
    fn spark_and_dot_scales_are_linear_and_clamped() {
        assert_eq!(spark_w(0.0), 0.0);
        assert_eq!(spark_w(0.5), SPARK_W / 2.0);
        assert_eq!(spark_w(1.0), SPARK_W);
        assert_eq!(spark_w(-1.0), 0.0);
        assert_eq!(spark_w(2.0), SPARK_W);

        assert_eq!(dot_x(0.0), 0.0);
        assert_eq!(dot_x(0.5), DOT_W / 2.0);
        assert_eq!(dot_x(1.0), DOT_W);
        assert_eq!(dot_x(-0.5), 0.0);
        assert_eq!(dot_x(4.0), DOT_W);
    }

    #[test]
    fn slope_y_is_inverted_and_clamped() {
        assert_eq!(slope_y(1.0), SLOPE_TOP);
        assert_eq!(slope_y(0.0), SLOPE_BOT);
        assert_eq!(slope_y(0.5), (SLOPE_TOP + SLOPE_BOT) / 2.0);
        // Higher score => smaller y => higher on the page.
        assert!(slope_y(0.9) < slope_y(0.1));
        assert_eq!(slope_y(5.0), SLOPE_TOP);
        assert_eq!(slope_y(-5.0), SLOPE_BOT);
    }

    #[test]
    fn slope_x_spaces_columns() {
        assert_eq!(slope_x(0, 2), SLOPE_X0);
        assert_eq!(slope_x(1, 2), SLOPE_X1);
        assert_eq!(slope_x(1, 3), (SLOPE_X0 + SLOPE_X1) / 2.0);
        assert_eq!(slope_x(0, 1), (SLOPE_X0 + SLOPE_X1) / 2.0);
    }

    #[test]
    fn sparkbar_geometry_matches_value() {
        let svg = sparkbar(0.5, 1);
        assert!(svg.contains("class=\"spark s1\""), "{svg}");
        assert!(
            svg.contains(&format!("width=\"{}\" height=\"6\"", coord(SPARK_W / 2.0))),
            "{svg}"
        );
        assert!(svg.contains("aria-hidden=\"true\""), "{svg}");
        // A zero score still renders the track, with an empty bar.
        assert!(sparkbar(0.0, 0).contains("width=\"0\" height=\"6\""));
    }

    #[test]
    fn dotplot_places_one_dot_per_present_value() {
        let svg = dotplot(&[Some(0.25), None, Some(1.0)]);
        assert_eq!(svg.matches("<circle").count(), 2, "{svg}");
        assert!(svg.contains(&format!("class=\"dot s0\" cx=\"{}\"", coord(dot_x(0.25)))));
        assert!(svg.contains(&format!("class=\"dot sx\" cx=\"{}\"", coord(dot_x(1.0)))));
        // The spread rule runs between the extremes.
        assert!(svg.contains(&format!(
            "class=\"spread\" x1=\"{}\" y1=\"8\" x2=\"{}\"",
            coord(dot_x(0.25)),
            coord(dot_x(1.0))
        )));
    }

    #[test]
    fn dotplot_omits_spread_when_values_coincide() {
        let svg = dotplot(&[Some(0.4), Some(0.4)]);
        assert!(!svg.contains("spread"), "{svg}");
        assert_eq!(svg.matches("<circle").count(), 2);
    }

    #[test]
    fn slopegraph_lines_cross_when_the_models_dissociate() {
        let cols = [
            ("deception_request", "pressure"),
            ("escalating", "sycophancy"),
        ];
        let series = [
            SlopeSeries {
                label: "a",
                series: 0,
                values: vec![Some(0.857), Some(0.409)],
            },
            SlopeSeries {
                label: "b",
                series: 1,
                values: vec![Some(0.100), Some(0.818)],
            },
        ];
        let svg = slopegraph(&cols, &series);

        // a starts above b and ends below it: the lines cross.
        assert!(slope_y(0.857) < slope_y(0.100));
        assert!(slope_y(0.409) > slope_y(0.818));

        let a = format!(
            "points=\"{},{} {},{}\"",
            coord(SLOPE_X0),
            coord(slope_y(0.857)),
            coord(SLOPE_X1),
            coord(slope_y(0.409))
        );
        assert!(svg.contains(&a), "{svg}");
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert!(svg.contains("role=\"img\""));
        assert!(svg.contains("aria-label=\"Slopegraph. a: deception_request 0.857,"));
        assert!(svg.contains("deception_request"));
    }

    #[test]
    fn slopegraph_skips_a_line_for_a_single_point() {
        let cols = [("k", "e")];
        let series = [SlopeSeries {
            label: "a",
            series: 0,
            values: vec![Some(0.5)],
        }];
        let svg = slopegraph(&cols, &series);
        assert!(!svg.contains("<polyline"), "{svg}");
        assert_eq!(svg.matches("<circle").count(), 1);
    }

    #[test]
    fn slopegraph_escapes_labels() {
        let cols = [("<kind>", "<eval>"), ("b", "e")];
        let series = [SlopeSeries {
            label: "<script>",
            series: 0,
            values: vec![Some(0.5), Some(0.5)],
        }];
        let svg = slopegraph(&cols, &series);
        assert!(!svg.contains("<kind>"), "{svg}");
        assert!(!svg.contains("<script>"), "{svg}");
        assert!(svg.contains("&lt;kind&gt;"));
    }
}
