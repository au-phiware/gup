// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the tour finds the code of a page in a snippet file: the lines between
//! `// tour:begin ID` and `// tour:end ID`. `tests/tour.rs` (the sync test) and
//! `gup-egui/examples/tour.rs` (the interactive tour) both include this file
//! with `#[path]`, so the pages, the goldens and the app show the same text.

/// A line of text for comparison: no trailing spaces.
pub fn lines_of(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// The lines between `// tour:begin ID` and `// tour:end ID` in `lines`,
/// without the begin marker's indentation. Panics, naming `file`, when the
/// markers are not there exactly once, or the region is longer than `max`.
pub fn region(file: &str, lines: &[String], id: &str, max: usize) -> Vec<String> {
    let marker = |kind: &str| format!("// tour:{kind} {id}");
    let find = |kind: &str| {
        let hits: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim_start() == marker(kind))
            .map(|(i, _)| i)
            .collect();
        assert!(
            hits.len() == 1,
            "{file}: expected one `{}` line, found {}",
            marker(kind),
            hits.len()
        );
        hits[0]
    };
    let (begin, end) = (find("begin"), find("end"));
    assert!(begin < end, "{file}: region `{id}` ends before it begins");
    let indent = lines[begin].len() - lines[begin].trim_start().len();
    let body: Vec<String> = lines[begin + 1..end]
        .iter()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                assert!(
                    l.len() >= indent && l[..indent].trim().is_empty(),
                    "{file}: region `{id}` has a line indented less than its begin marker: {l:?}"
                );
                l[indent..].to_string()
            }
        })
        .collect();
    assert!(
        body.len() <= max,
        "{file}: region `{id}` is {} lines; a page may show at most {max}",
        body.len()
    );
    body
}
