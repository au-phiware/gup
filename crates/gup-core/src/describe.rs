// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A chart's text alternative (GUP-592): what a screen reader announces for
//! it. Built at resolve from what the resolve already computed (the titles,
//! the tick labels as drawn, the legends and each data layer's row count), so
//! it cannot disagree with the picture.

use crate::scale::Legend;
use crate::transform::BoxStats;
use std::sync::Arc;

/// A chart's text alternative: what a screen reader announces for it.
///
/// Read it with [`Plot::describe`](crate::Plot::describe) after a resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    /// A short name: the chart's title, else its kind ("Scatter chart"),
    /// else "Chart".
    pub title: String,
    /// What it shows: layers, axes and legends, in sentences.
    pub text: String,
}

impl Description {
    /// `title` and `text` as one string, for an `alt` or `aria-label`:
    /// joined with ". ", or with " " when `title` already ends in `.`, `!`
    /// or `?`; `title` alone when `text` is empty.
    pub fn alt_text(&self) -> String {
        if self.text.is_empty() {
            return self.title.clone();
        }
        if ends_sentence(&self.title) {
            format!("{} {}", self.title, self.text)
        } else {
            format!("{}. {}", self.title, self.text)
        }
    }
}

/// A chart's data as a table: what a screen reader browses with table
/// navigation (GUP-624).
///
/// Read it with [`Plot::data_table`](crate::Plot::data_table) after a
/// resolve. Cells are the row fields as the axes label them
/// ([`Plot::row_fields`](crate::Plot::row_fields)).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataTable {
    /// The description's title, plus " (first {k} of {n} rows)" when rows
    /// were omitted.
    pub caption: String,
    /// Field names, in first-seen order over the rows included.
    pub headers: Vec<String>,
    /// One entry per row, aligned with `headers` ("" where a row lacks a
    /// field).
    pub rows: Vec<Vec<String>>,
    /// Rows not included because of `max_rows`.
    pub omitted: usize,
}

impl DataTable {
    /// Assemble a table from each row's fields; `omitted` is the number of
    /// further rows left out.
    pub(crate) fn build(
        title: &str,
        fields: Vec<Vec<(String, String)>>,
        omitted: usize,
    ) -> DataTable {
        let mut headers: Vec<String> = Vec::new();
        for row in &fields {
            for (name, _) in row {
                if !headers.contains(name) {
                    headers.push(name.clone());
                }
            }
        }
        let rows: Vec<Vec<String>> = fields
            .into_iter()
            .map(|row| {
                headers
                    .iter()
                    .map(|h| {
                        row.iter()
                            .find(|(n, _)| n == h)
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        let mut caption = title.to_owned();
        if omitted > 0 {
            caption.push_str(" (first ");
            push_count(&mut caption, rows.len());
            caption.push_str(" of ");
            push_count(&mut caption, rows.len() + omitted);
            caption.push_str(" rows)");
        }
        DataTable {
            caption,
            headers,
            rows,
            omitted,
        }
    }
}

/// What [`describe`] reads.
pub(crate) struct DescribeInput<'a> {
    pub title: Option<&'a str>,
    pub subtitle: Option<&'a str>,
    pub kind: Option<&'static str>,
    pub text_override: Option<&'a str>,
    /// A builder's own account of the chart (GUP-600), in place of the
    /// generic row clause. It names the kind itself.
    pub summary: Option<&'a str>,
    /// (mark name, rows) per data layer, in plot order.
    pub layers: Vec<(&'static str, usize)>,
    /// x, y, right y; `None` where the plot has no such axis.
    pub axes: [Option<AxisText<'a>>; 3],
    pub legends: &'a [(Option<String>, Legend)],
}

/// One axis, as drawn.
pub(crate) struct AxisText<'a> {
    pub title: Option<&'a str>,
    /// Band or point axis (`DynPositionScale::reads_keys`).
    pub keyed: bool,
    /// The tick labels as drawn, in order.
    pub labels: &'a [String],
}

fn ends_sentence(s: &str) -> bool {
    s.ends_with(['.', '!', '?'])
}

/// Append `n` with `,` thousands separators (`12,000`).
pub(crate) fn push_count(out: &mut String, n: usize) {
    let mut digits = [0u8; 20];
    let mut len = 0;
    let mut rest = n;
    loop {
        digits[len] = b'0' + (rest % 10) as u8;
        len += 1;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    for i in (0..len).rev() {
        out.push(char::from(digits[i]));
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
    }
}

#[cfg(test)]
fn thousands(n: usize) -> String {
    let mut out = String::new();
    push_count(&mut out, n);
    out
}

/// `v` to three significant digits, without an exponent (`54`, `0.125`,
/// `1230`), for a builder's summary.
pub(crate) fn sig3(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() {
            "n/a".into()
        } else {
            format!("{v}")
        };
    }
    let p = 2 - v.abs().log10().floor() as i32;
    let r = if p >= 0 {
        let k = 10f64.powi(p);
        (v * k).round() / k
    } else {
        let k = 10f64.powi(-p);
        (v / k).round() * k
    };
    format!("{}", if r == 0.0 { 0.0 } else { r })
}

/// Append `{n} {one|many}`.
fn push_plural(out: &mut String, n: usize, one: &str, many: &str) {
    push_count(out, n);
    out.push(' ');
    out.push_str(if n == 1 { one } else { many });
}

/// Append the items: `a`; `a and b`; `a, b and c` (up to eight); else the
/// first six and `and {n - 6} more`.
pub(crate) fn push_list<S: AsRef<str>>(out: &mut String, items: &[S]) {
    let n = items.len();
    let shown = if n > 8 { 6 } else { n.saturating_sub(1) };
    for (i, item) in items[..shown].iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(item.as_ref());
    }
    if n > 8 {
        out.push_str(" and ");
        push_count(out, n - 6);
        out.push_str(" more");
    } else if n > 0 {
        if n > 1 {
            out.push_str(" and ");
        }
        out.push_str(items[n - 1].as_ref());
    }
}

/// A box plot's account of itself: its groups with their quartiles, whiskers
/// and outlier counts, the first eight, then how many more.
pub(crate) fn box_summary(groups: &[(&str, &BoxStats)]) -> String {
    let mut out = String::from("Box plot of ");
    push_plural(&mut out, groups.len(), "group", "groups");
    out.push_str(": ");
    for (i, (key, s)) in groups.iter().take(8).enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        out.push_str(key);
        out.push_str(", median ");
        out.push_str(&sig3(s.median));
        out.push_str(", middle half ");
        out.push_str(&sig3(s.q1));
        out.push_str(" to ");
        out.push_str(&sig3(s.q3));
        out.push_str(", whiskers ");
        out.push_str(&sig3(s.whisker_lo));
        out.push_str(" to ");
        out.push_str(&sig3(s.whisker_hi));
        if !s.outliers.is_empty() {
            out.push_str(", ");
            push_plural(&mut out, s.outliers.len(), "outlier", "outliers");
        }
    }
    if groups.len() > 8 {
        out.push_str("; and ");
        push_count(&mut out, groups.len() - 8);
        out.push_str(" more");
    }
    out
}

/// A histogram's account of itself: values counted, bins and their span, and
/// the stack groups (`stack_title` names what they are keyed by).
pub(crate) fn histogram_summary(
    values: usize,
    bins: usize,
    span: (f64, f64),
    stack_title: Option<&str>,
    stacks: &[Arc<str>],
) -> String {
    let mut out = String::from("Histogram of ");
    push_plural(&mut out, values, "value", "values");
    out.push_str(" in ");
    push_plural(&mut out, bins, "bin", "bins");
    out.push_str(" from ");
    out.push_str(&sig3(span.0));
    out.push_str(" to ");
    out.push_str(&sig3(span.1));
    if !stacks.is_empty() {
        out.push_str(", stacked");
        if let Some(t) = stack_title {
            out.push_str(" by ");
            out.push_str(t);
        }
        out.push_str(": ");
        push_list(&mut out, stacks);
    }
    out
}

fn quoted(out: &mut String, title: Option<&str>) {
    if let Some(t) = title {
        out.push_str(" \"");
        out.push_str(t);
        out.push('"');
    }
}

/// Build the description of a resolved chart.
pub(crate) fn describe(input: &DescribeInput<'_>) -> Description {
    let kind = input.kind.unwrap_or("Chart");
    let title = input.title.unwrap_or(kind).to_string();

    if let Some(text) = input.text_override {
        return Description {
            title,
            text: text.to_string(),
        };
    }

    let mut text = String::new();
    if let Some(sub) = input.subtitle.filter(|s| !s.is_empty()) {
        text.push_str(sub);
        if !ends_sentence(sub) {
            text.push('.');
        }
        text.push(' ');
    }

    let titled = input.title.is_some();
    if let Some(summary) = input.summary {
        text.push_str(summary);
        text.push('.');
    } else if input.layers.is_empty() {
        if titled {
            text.push_str(kind);
            text.push_str(" with no data.");
        } else {
            text.push_str("No data.");
        }
    } else {
        if titled {
            text.push_str(kind);
            text.push_str(", ");
        }
        let rows: usize = input.layers.iter().map(|l| l.1).sum();
        push_plural(&mut text, rows, "data row", "data rows");
        // Layers of one mark merge, in first-seen order.
        let mut kinds: Vec<(&str, usize)> = Vec::new();
        for &(mark, n) in &input.layers {
            match kinds.iter_mut().find(|k| k.0 == mark) {
                Some(k) => k.1 += n,
                None => kinds.push((mark, n)),
            }
        }
        if kinds.len() > 1 {
            text.push_str(": ");
            for (i, (mark, n)) in kinds.iter().enumerate() {
                if i > 0 {
                    text.push_str(", ");
                }
                text.push_str(&mark.to_ascii_lowercase());
                text.push(' ');
                push_count(&mut text, *n);
            }
        }
        text.push('.');
    }

    for (i, (axis, name)) in input
        .axes
        .iter()
        .zip(["X axis", "Y axis", "Right y axis"])
        .enumerate()
    {
        let Some(axis) = axis else { continue };
        if axis.labels.is_empty() && axis.title.is_none() {
            continue;
        }
        text.push(' ');
        text.push_str(name);
        quoted(&mut text, axis.title);
        match (axis.labels, axis.keyed) {
            ([], _) => {}
            (labels, true) => {
                text.push_str(": ");
                push_plural(&mut text, labels.len(), "category", "categories");
                text.push_str(", ");
                if i > 0 {
                    // A vertical axis labels bottom to top; a reader scans
                    // top to bottom (GUP-601).
                    let top_down: Vec<&String> = labels.iter().rev().collect();
                    push_list(&mut text, &top_down);
                } else {
                    push_list(&mut text, labels);
                }
            }
            ([only], false) => {
                text.push_str(": ");
                text.push_str(only);
            }
            ([first, .., last], false) => {
                text.push_str(": ");
                text.push_str(first);
                text.push_str(" to ");
                text.push_str(last);
            }
        }
        text.push('.');
    }

    for (legend_title, legend) in input.legends {
        text.push_str(" Colour legend");
        quoted(&mut text, legend_title.as_deref());
        match legend {
            Legend::Swatches(swatches) if !swatches.is_empty() => {
                let labels: Vec<&str> = swatches.iter().map(|w| &*w.label).collect();
                text.push_str(": ");
                push_list(&mut text, &labels);
            }
            Legend::Ramp(ramp) if !ramp.ticks().is_empty() => {
                let ticks = ramp.ticks();
                text.push_str(": ");
                text.push_str(&ticks[0].label);
                if ticks.len() > 1 {
                    text.push_str(" to ");
                    text.push_str(&ticks[ticks.len() - 1].label);
                }
            }
            _ => {}
        }
        text.push('.');
    }

    Description {
        title,
        text: text.trim_start().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::scale::{Ramp, RampTick, Swatch};
    use std::sync::Arc;

    fn labels(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn input<'a>() -> DescribeInput<'a> {
        DescribeInput {
            title: Some("T"),
            subtitle: None,
            kind: None,
            text_override: None,
            summary: None,
            layers: vec![("Circle", 3)],
            axes: [None, None, None],
            legends: &[],
        }
    }

    #[test]
    fn singular_and_plural() {
        let mut i = input();
        i.layers = vec![("Circle", 1)];
        assert_eq!(describe(&i).text, "Chart, 1 data row.");
        i.layers = vec![("Circle", 12_000), ("Circle", 5)];
        assert_eq!(describe(&i).text, "Chart, 12,005 data rows.");
    }

    #[test]
    fn mixed_layers_list_marks() {
        let mut i = input();
        i.layers = vec![("Circle", 200), ("Line", 10)];
        assert_eq!(
            describe(&i).text,
            "Chart, 210 data rows: circle 200, line 10."
        );
    }

    #[test]
    fn same_marks_merge() {
        let mut i = input();
        i.layers = vec![("Rect", 2), ("Segment", 4), ("Segment", 4), ("Segment", 2)];
        assert_eq!(
            describe(&i).text,
            "Chart, 12 data rows: rect 2, segment 10."
        );
    }

    #[test]
    fn summary_replaces_clause() {
        let mut i = input();
        i.summary = Some("Box plot of 1 group: a");
        assert_eq!(describe(&i).text, "Box plot of 1 group: a.");
    }

    fn stats(median: f64, outliers: Vec<f64>) -> BoxStats {
        BoxStats {
            n: 10,
            q1: median - 10.0,
            median,
            q3: median + 16.0,
            whisker_lo: median - 34.0,
            whisker_hi: median + 41.123,
            outliers,
        }
    }

    #[test]
    fn box_summaries() {
        let (a, b) = (stats(54.0, vec![200.0]), stats(60.0, vec![]));
        assert_eq!(
            box_summary(&[("api", &a), ("web", &b)]),
            "Box plot of 2 groups: api, median 54, middle half 44 to 70, whiskers 20 to 95.1, 1 outlier; \
             web, median 60, middle half 50 to 76, whiskers 26 to 101"
        );
        let many: Vec<String> = (0..11).map(|n| format!("g{n}")).collect();
        let groups: Vec<(&str, &BoxStats)> = many.iter().map(|k| (k.as_str(), &a)).collect();
        assert!(box_summary(&groups).ends_with("; and 3 more"));
    }

    #[test]
    fn histogram_summaries() {
        assert_eq!(
            histogram_summary(1200, 14, (10.0, 80.0), None, &[]),
            "Histogram of 1,200 values in 14 bins from 10 to 80"
        );
        let stacks: Vec<Arc<str>> = vec!["Morning".into(), "Evening".into()];
        assert_eq!(
            histogram_summary(1, 1, (0.0, 0.5), Some("Session"), &stacks),
            "Histogram of 1 value in 1 bin from 0 to 0.5, stacked by Session: Morning and Evening"
        );
    }

    #[test]
    fn sig3_rounds() {
        assert_eq!(sig3(54.0), "54");
        assert_eq!(sig3(0.12345), "0.123");
        assert_eq!(sig3(12345.0), "12300");
        assert_eq!(sig3(-0.0), "0");
        assert_eq!(sig3(1.0 / 3.0), "0.333");
    }

    #[test]
    fn vertical_keyed_axes_read_top_down() {
        let l = labels(&["c", "b", "a"]);
        let mut i = input();
        i.axes[0] = Some(AxisText {
            title: None,
            keyed: true,
            labels: &l,
        });
        i.axes[1] = Some(AxisText {
            title: None,
            keyed: true,
            labels: &l,
        });
        let t = describe(&i).text;
        assert!(t.contains("X axis: 3 categories, c, b and a."), "{t}");
        assert!(t.contains("Y axis: 3 categories, a, b and c."), "{t}");
    }

    #[test]
    fn untitled_drops_kind() {
        let mut i = input();
        i.title = None;
        i.layers = vec![("Circle", 6)];
        let d = describe(&i);
        assert_eq!(d.title, "Chart");
        assert_eq!(d.text, "6 data rows.");
        assert_eq!(d.alt_text(), "Chart. 6 data rows.");
        i.kind = Some("Scatter chart");
        let d = describe(&i);
        assert_eq!(d.title, "Scatter chart");
        assert_eq!(d.text, "6 data rows.");
    }

    #[test]
    fn titled_with_kind() {
        let mut i = input();
        i.kind = Some("Bar chart");
        assert_eq!(describe(&i).text, "Bar chart, 3 data rows.");
    }

    #[test]
    fn no_layers() {
        let mut i = input();
        i.layers.clear();
        i.kind = Some("Heatmap");
        assert_eq!(describe(&i).text, "Heatmap with no data.");
        i.title = None;
        assert_eq!(describe(&i).text, "No data.");
    }

    #[test]
    fn subtitle() {
        let mut i = input();
        i.subtitle = Some("Per capita");
        assert!(describe(&i).text.starts_with("Per capita. Chart,"));
        i.subtitle = Some("Is it?");
        assert!(describe(&i).text.starts_with("Is it? Chart,"));
        i.subtitle = Some("Done.");
        assert!(describe(&i).text.starts_with("Done. Chart,"));
    }

    #[test]
    fn keyed_axes() {
        let four = labels(&["a", "b", "c", "d"]);
        let twelve: Vec<String> = (1..=12).map(|n| format!("k{n}")).collect();
        let mut i = input();
        i.axes[0] = Some(AxisText {
            title: Some("Fruit"),
            keyed: true,
            labels: &four,
        });
        i.axes[1] = Some(AxisText {
            title: None,
            keyed: true,
            labels: &twelve,
        });
        i.axes[2] = Some(AxisText {
            title: None,
            keyed: true,
            labels: &four[..1],
        });
        let t = describe(&i).text;
        assert!(
            t.contains("X axis \"Fruit\": 4 categories, a, b, c and d."),
            "{t}"
        );
        assert!(
            t.contains("Y axis: 12 categories, k12, k11, k10, k9, k8, k7 and 6 more."),
            "{t}"
        );
        assert!(t.contains("Right y axis: 1 category, a."), "{t}");
    }

    #[test]
    fn continuous_axes() {
        let l = labels(&["0", "50", "100"]);
        let mut i = input();
        i.axes[0] = Some(AxisText {
            title: Some("GDP"),
            keyed: false,
            labels: &l,
        });
        i.axes[1] = Some(AxisText {
            title: None,
            keyed: false,
            labels: &l[1..2],
        });
        let t = describe(&i).text;
        assert!(t.contains("X axis \"GDP\": 0 to 100."), "{t}");
        assert!(t.contains("Y axis: 50."), "{t}");
    }

    #[test]
    fn axis_without_ticks() {
        let mut i = input();
        i.axes[0] = Some(AxisText {
            title: Some("X"),
            keyed: false,
            labels: &[],
        });
        i.axes[1] = Some(AxisText {
            title: None,
            keyed: false,
            labels: &[],
        });
        let t = describe(&i).text;
        assert!(t.ends_with("X axis \"X\"."), "{t}");
        assert!(!t.contains("Y axis"), "{t}");
    }

    #[test]
    fn legends() {
        let sw = |l: &str| Swatch {
            label: Arc::from(l),
            color: Color::BLACK,
        };
        let ramp = Ramp {
            lut: Arc::from(vec![[0u8; 4]; 2]),
            reverse: false,
            stops: [0.0, 5.0, 10.0],
            ticks: vec![
                RampTick {
                    t: 0.0,
                    value: 0.0,
                    label: "0".into(),
                },
                RampTick {
                    t: 1.0,
                    value: 10.0,
                    label: "10 °C".into(),
                },
            ],
        };
        let legends = vec![
            (
                Some("Continent".to_string()),
                Legend::Swatches(vec![sw("Africa"), sw("Europe"), sw("Asia")]),
            ),
            (None, Legend::Ramp(ramp)),
        ];
        let mut i = input();
        i.legends = &legends;
        let t = describe(&i).text;
        assert!(
            t.ends_with(
                "Colour legend \"Continent\": Africa, Europe and Asia. Colour legend: 0 to 10 °C."
            ),
            "{t}"
        );
    }

    #[test]
    fn override_replaces_sentences() {
        let mut i = input();
        i.subtitle = Some("sub");
        i.text_override = Some("Custom words.");
        let d = describe(&i);
        assert_eq!(d.title, "T");
        assert_eq!(d.text, "Custom words.");
        assert_eq!(d.alt_text(), "T. Custom words.");
    }

    #[test]
    fn alt_text_joins() {
        let d = |t: &str, x: &str| Description {
            title: t.into(),
            text: x.into(),
        };
        assert_eq!(d("A", "b.").alt_text(), "A. b.");
        assert_eq!(d("A?", "b.").alt_text(), "A? b.");
        assert_eq!(d("A!", "b.").alt_text(), "A! b.");
        assert_eq!(d("A.", "b.").alt_text(), "A. b.");
        assert_eq!(d("A", "").alt_text(), "A");
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
