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

/// `12000` as `12,000`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

/// `a`; `a and b`; `a, b and c` (up to eight); else the first six and
/// `and {n - 6} more`.
fn list<S: AsRef<str>>(items: &[S]) -> String {
    let at = |i: usize| items[i].as_ref();
    match items.len() {
        0 => String::new(),
        1 => at(0).to_string(),
        2 => format!("{} and {}", at(0), at(1)),
        n if n <= 8 => format!("{} and {}", join(&items[..n - 1]), at(n - 1)),
        n => format!("{} and {} more", join(&items[..6]), n - 6),
    }
}

fn join<S: AsRef<str>>(items: &[S]) -> String {
    items
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build the description of a resolved chart.
pub(crate) fn describe(input: &DescribeInput<'_>) -> Description {
    let kind = input.kind.unwrap_or("Chart");
    let title = input
        .title
        .map(str::to_string)
        .unwrap_or_else(|| kind.to_string());

    if let Some(text) = input.text_override {
        return Description {
            title,
            text: text.to_string(),
        };
    }

    let mut sentences: Vec<String> = Vec::new();
    if let Some(sub) = input.subtitle.filter(|s| !s.is_empty()) {
        sentences.push(if ends_sentence(sub) {
            sub.to_string()
        } else {
            format!("{sub}.")
        });
    }

    let rows: usize = input.layers.iter().map(|l| l.1).sum();
    let layers = input.layers.len();
    let titled = input.title.is_some();
    sentences.push(if layers == 0 {
        if titled {
            format!("{kind} with no data.")
        } else {
            "No data.".to_string()
        }
    } else {
        let mut s = format!(
            "{} in {}",
            plural(rows, "data row", "data rows"),
            plural(layers, "layer", "layers")
        );
        if titled {
            s = format!("{kind}, {s}");
        }
        if input.layers.iter().any(|l| l.0 != input.layers[0].0) {
            let parts: Vec<String> = input
                .layers
                .iter()
                .map(|(m, n)| format!("{} {}", m.to_lowercase(), thousands(*n)))
                .collect();
            s = format!("{s} ({})", list(&parts));
        }
        s.push('.');
        s
    });

    for (axis, name) in input.axes.iter().zip(["X axis", "Y axis", "Right y axis"]) {
        let Some(axis) = axis else { continue };
        let mut s = name.to_string();
        if let Some(t) = axis.title {
            s = format!("{s} \"{t}\"");
        }
        match (axis.labels, axis.keyed) {
            ([], _) if axis.title.is_none() => continue,
            ([], _) => s.push('.'),
            (labels, true) => {
                s = format!(
                    "{s}: {}, {}.",
                    plural(labels.len(), "category", "categories"),
                    list(labels)
                );
            }
            ([only], false) => s = format!("{s}: {only}."),
            ([first, .., last], false) => s = format!("{s}: {first} to {last}."),
        }
        sentences.push(s);
    }

    for (legend_title, legend) in input.legends {
        let mut s = "Colour legend".to_string();
        if let Some(t) = legend_title {
            s = format!("{s} \"{t}\"");
        }
        match legend {
            Legend::Swatches(swatches) if !swatches.is_empty() => {
                let labels: Vec<&str> = swatches.iter().map(|w| &*w.label).collect();
                s = format!("{s}: {}.", list(&labels));
            }
            Legend::Ramp(ramp) if !ramp.ticks().is_empty() => {
                let ticks = ramp.ticks();
                let (first, last) = (&ticks[0].label, &ticks[ticks.len() - 1].label);
                s = if ticks.len() == 1 {
                    format!("{s}: {first}.")
                } else {
                    format!("{s}: {first} to {last}.")
                };
            }
            _ => s.push('.'),
        }
        sentences.push(s);
    }

    Description {
        title,
        text: sentences.join(" "),
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
