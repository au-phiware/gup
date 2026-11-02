// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A chart's text alternative (GUP-592): what a screen reader announces for
//! it. Built at resolve from what the resolve already computed (the titles,
//! the tick labels as drawn, the legends and each data layer's row count), so
//! it cannot disagree with the picture.

use crate::scale::Legend;

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

/// What [`describe`] reads.
pub(crate) struct DescribeInput<'a> {
    pub title: Option<&'a str>,
    pub subtitle: Option<&'a str>,
    pub kind: Option<&'static str>,
    pub text_override: Option<&'a str>,
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
fn push_count(out: &mut String, n: usize) {
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

/// Append `{n} {one|many}`.
fn push_plural(out: &mut String, n: usize, one: &str, many: &str) {
    push_count(out, n);
    out.push(' ');
    out.push_str(if n == 1 { one } else { many });
}

/// Append the items: `a`; `a and b`; `a, b and c` (up to eight); else the
/// first six and `and {n - 6} more`.
fn push_list<S: AsRef<str>>(out: &mut String, items: &[S]) {
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

    let rows: usize = input.layers.iter().map(|l| l.1).sum();
    let layers = input.layers.len();
    let titled = input.title.is_some();
    if layers == 0 {
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
        push_plural(&mut text, rows, "data row", "data rows");
        text.push_str(" in ");
        push_plural(&mut text, layers, "layer", "layers");
        if input.layers.iter().any(|l| l.0 != input.layers[0].0) {
            let mut parts: Vec<String> = Vec::with_capacity(layers);
            for (mark, n) in &input.layers {
                let mut part = mark.to_ascii_lowercase();
                part.push(' ');
                push_count(&mut part, *n);
                parts.push(part);
            }
            text.push_str(" (");
            push_list(&mut text, &parts);
            text.push(')');
        }
        text.push('.');
    }

    for (axis, name) in input.axes.iter().zip(["X axis", "Y axis", "Right y axis"]) {
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
                push_list(&mut text, labels);
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
            layers: vec![("Circle", 3)],
            axes: [None, None, None],
            legends: &[],
        }
    }

    #[test]
    fn singular_and_plural() {
        let mut i = input();
        i.layers = vec![("Circle", 1)];
        assert_eq!(describe(&i).text, "Chart, 1 data row in 1 layer.");
        i.layers = vec![("Circle", 12_000), ("Circle", 5)];
        assert_eq!(describe(&i).text, "Chart, 12,005 data rows in 2 layers.");
    }

    #[test]
    fn mixed_layers_list_marks() {
        let mut i = input();
        i.layers = vec![("Circle", 200), ("Line", 10)];
        assert_eq!(
            describe(&i).text,
            "Chart, 210 data rows in 2 layers (circle 200 and line 10)."
        );
    }

    #[test]
    fn untitled_drops_kind() {
        let mut i = input();
        i.title = None;
        i.layers = vec![("Circle", 6)];
        let d = describe(&i);
        assert_eq!(d.title, "Chart");
        assert_eq!(d.text, "6 data rows in 1 layer.");
        assert_eq!(d.alt_text(), "Chart. 6 data rows in 1 layer.");
        i.kind = Some("Scatter chart");
        let d = describe(&i);
        assert_eq!(d.title, "Scatter chart");
        assert_eq!(d.text, "6 data rows in 1 layer.");
    }

    #[test]
    fn titled_with_kind() {
        let mut i = input();
        i.kind = Some("Bar chart");
        assert_eq!(describe(&i).text, "Bar chart, 3 data rows in 1 layer.");
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
            t.contains("Y axis: 12 categories, k1, k2, k3, k4, k5, k6 and 6 more."),
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
