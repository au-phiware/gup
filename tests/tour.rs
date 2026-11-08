// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The feature tour (`docs/tour/`): every snippet file is included here and
//! rendered as a golden, and `pages_match_their_snippets` checks that each
//! page shows exactly the code that was rendered. Bless with
//! `GUP_BLESS=1 cargo test -p gup --test tour -- --test-threads=1`.

mod common;

use common::{check, check_on};
use gup::prelude::*;
use gup_visual_regression::Rgba8;
use std::fs;
use std::path::{Path, PathBuf};

/// The reading size of a tour image; chapter 2 shows nine, so it is smaller.
const W: u32 = 640;
const H: u32 = 400;
const SMALL: (u32, u32) = (480, 300);

/// The most lines of code a page may show from one region.
const MAX_REGION_LINES: usize = 30;

fn background(theme: &Theme) -> Rgba8 {
    let [r, g, b, _] = theme.background.to_rgba8();
    Rgba8::rgb(r, g, b)
}

#[path = "../docs/tour/snippets/01_five_lines.rs"]
#[allow(dead_code)]
mod s01_five_lines;

#[path = "../docs/tour/snippets/02_vocabulary.rs"]
#[allow(dead_code)]
mod s02_vocabulary;

#[path = "../docs/tour/snippets/03_encodings.rs"]
#[allow(dead_code)]
mod s03_encodings;

#[path = "../docs/tour/snippets/04_axes_and_labels.rs"]
#[allow(dead_code)]
mod s04_axes_and_labels;

#[path = "../docs/tour/snippets/05_colour_and_themes.rs"]
#[allow(dead_code)]
mod s05_colour_and_themes;

#[path = "../docs/tour/snippets/06_lines_and_curves.rs"]
#[allow(dead_code)]
mod s06_lines_and_curves;

#[path = "../docs/tour/snippets/07_text_and_annotation.rs"]
#[allow(dead_code)]
mod s07_text_and_annotation;

#[path = "../docs/tour/snippets/08_interaction.rs"]
#[allow(dead_code)]
mod s08_interaction;

#[path = "../docs/tour/snippets/09_accessibility.rs"]
#[allow(dead_code)]
mod s09_accessibility;

#[path = "../docs/tour/snippets/10_live_data.rs"]
#[allow(dead_code)]
mod s10_live_data;

#[path = "../docs/tour/snippets/11_big_data.rs"]
#[allow(dead_code)]
mod s11_big_data;

// Chapter 1.

#[test]
fn five_lines() {
    check(
        "tour/01_five_lines",
        &mut s01_five_lines::five_lines(),
        W,
        H,
    );
}

/// `save` writes a 640 by 400 PNG and an SVG with the marks in it.
#[test]
fn save_writes_a_png_and_an_svg() {
    let dir = std::env::temp_dir().join(format!("gup-tour-{}", std::process::id()));
    s01_five_lines::save(&dir).unwrap();
    let png = fs::read(dir.join("chart.png")).unwrap();
    // The PNG header: signature, then the IHDR chunk's width and height.
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let word = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    assert_eq!((word(16), word(20)), (640, 400));
    let svg = fs::read_to_string(dir.join("chart.svg")).unwrap();
    assert!(svg.starts_with("<svg"), "{}", &svg[..svg.len().min(80)]);
    assert!(svg.contains("<circle"));
    fs::remove_dir_all(&dir).unwrap();
}

// Chapter 2.

#[test]
fn vocabulary_scatter() {
    let (w, h) = SMALL;
    check("tour/02_scatter", &mut s02_vocabulary::scatter(), w, h);
}

#[test]
fn vocabulary_line() {
    let (w, h) = SMALL;
    check("tour/02_line", &mut s02_vocabulary::line(), w, h);
}

#[test]
fn vocabulary_bar() {
    let (w, h) = SMALL;
    check("tour/02_bar", &mut s02_vocabulary::bar(), w, h);
}

#[test]
fn vocabulary_bar_stacked() {
    let (w, h) = SMALL;
    check(
        "tour/02_bar_stacked",
        &mut s02_vocabulary::bar_stacked(),
        w,
        h,
    );
}

#[test]
fn vocabulary_barh() {
    let (w, h) = SMALL;
    check("tour/02_barh", &mut s02_vocabulary::barh(), w, h);
}

#[test]
fn vocabulary_area() {
    let (w, h) = SMALL;
    check("tour/02_area", &mut s02_vocabulary::area(), w, h);
}

#[test]
fn vocabulary_histogram() {
    let (w, h) = SMALL;
    check("tour/02_histogram", &mut s02_vocabulary::histogram(), w, h);
}

#[test]
fn vocabulary_heatmap() {
    let (w, h) = SMALL;
    check("tour/02_heatmap", &mut s02_vocabulary::heatmap(), w, h);
}

#[test]
fn vocabulary_boxplot() {
    let (w, h) = SMALL;
    check("tour/02_boxplot", &mut s02_vocabulary::boxplot(), w, h);
}

// Chapter 3.

#[test]
fn encodings() {
    check("tour/03_encodings", &mut s03_encodings::encodings(), W, H);
}

// Chapter 4.

#[test]
fn time_axis() {
    check(
        "tour/04_time_axis",
        &mut s04_axes_and_labels::time_axis(),
        W,
        H,
    );
}

#[test]
fn dual_axis() {
    check(
        "tour/04_dual_axis",
        &mut s04_axes_and_labels::dual_axis(),
        W,
        H,
    );
}

#[test]
fn bar_labels() {
    check(
        "tour/04_bar_labels",
        &mut s04_axes_and_labels::bar_labels(),
        W,
        H,
    );
}

// Chapter 5.

#[test]
fn palette() {
    check(
        "tour/05_palette",
        &mut s05_colour_and_themes::palette(),
        W,
        H,
    );
}

#[test]
fn dark() {
    check_on(
        "tour/05_dark",
        &mut s05_colour_and_themes::dark(),
        W,
        H,
        background(&Theme::dark()),
    );
}

#[test]
fn high_contrast() {
    check_on(
        "tour/05_high_contrast",
        &mut s05_colour_and_themes::high_contrast(),
        W,
        H,
        background(&Theme::high_contrast()),
    );
}

// Chapter 6.

#[test]
fn curves() {
    check("tour/06_curves", &mut s06_lines_and_curves::curves(), W, H);
}

// Chapter 7.

#[test]
fn labels() {
    check(
        "tour/07_labels",
        &mut s07_text_and_annotation::labels(),
        W,
        H,
    );
}

// Chapter 8.

#[test]
fn tooltip() {
    let mut chart = s08_interaction::tooltip().unwrap();
    check("tour/08_tooltip", &mut chart, W, H);
}

#[test]
fn brush() {
    let mut chart = s08_interaction::brush().unwrap();
    check("tour/08_brush", &mut chart, W, H);
}

// Chapter 9.

#[test]
fn keys() {
    let mut chart = s09_accessibility::keys().unwrap();
    check("tour/09_keys", &mut chart, W, H);
}

// Chapter 10.

#[test]
fn append() {
    let mut chart = s10_live_data::append().unwrap();
    check("tour/10_append", &mut chart, W, H);
}

#[test]
fn window() {
    let mut chart = s10_live_data::window().unwrap();
    check("tour/10_window", &mut chart, W, H);
}

// Chapter 11.

#[test]
fn columns() {
    let mut chart = s11_big_data::columns().unwrap();
    check("tour/11_columns", &mut chart, W, H);
}

// The sync test: the pages show the code that was rendered.

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A file's text with `\r\n` as `\n` (a Windows checkout may have either).
fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// A line of text for comparison: no trailing spaces.
fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim_end().to_string()).collect()
}

/// The lines between `// tour:begin ID` and `// tour:end ID` in `lines`,
/// without the begin marker's indentation.
fn region(file: &str, lines: &[String], id: &str, max: usize) -> Vec<String> {
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

/// The fenced block that follows the marker at `lines[at]`, after at most
/// one blank line: its lines, and the index of the line after its closing
/// fence.
fn block_after(page: &str, lines: &[String], at: usize, language: &str) -> Vec<String> {
    let mut i = at + 1;
    if lines.get(i).is_some_and(|l| l.is_empty()) {
        i += 1;
    }
    let fence = lines.get(i).map(String::as_str).unwrap_or("");
    assert!(
        fence.starts_with(&format!("```{language}")),
        "{page}: the marker `{}` is not followed by a ```{language} block (found {fence:?})",
        lines[at]
    );
    let body: Vec<String> = lines[i + 1..]
        .iter()
        .take_while(|l| !l.starts_with("```"))
        .cloned()
        .collect();
    assert!(
        lines.get(i + 1 + body.len()).is_some(),
        "{page}: the block after `{}` is never closed",
        lines[at]
    );
    body
}

/// What a marker line asks for.
struct Marker<'a> {
    /// `snippet`, `quote` or `output`.
    kind: &'static str,
    path: &'a str,
    id: Option<&'a str>,
    /// The most lines of the block (`max=N` raises it for a snippet).
    max: usize,
}

fn parse_marker(line: &str) -> Option<Marker<'_>> {
    let inner = line.strip_prefix("<!-- tour")?.strip_suffix("-->")?.trim();
    let (kind, rest) = if let Some(rest) = inner.strip_prefix("-quote:") {
        ("quote", rest)
    } else if let Some(rest) = inner.strip_prefix("-output:") {
        ("output", rest)
    } else {
        ("snippet", inner.strip_prefix(':')?)
    };
    let mut max = MAX_REGION_LINES;
    let mut words = rest.split_whitespace();
    let target = words.next()?;
    for word in words {
        let n = word
            .strip_prefix("max=")
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("`{line}`: unknown option {word:?} (only max=N)"));
        assert!(kind == "snippet", "`{line}`: max=N is for snippet blocks");
        max = n;
    }
    Some(match target.split_once('#') {
        Some((path, id)) => Marker {
            kind,
            path,
            id: Some(id),
            max,
        },
        None => Marker {
            kind,
            path: target,
            id: None,
            max,
        },
    })
}

/// The text a snippet's `pub fn <ID>_output()` returns, for the
/// `tour-output` blocks.
fn snippet_output(path_and_id: &str) -> String {
    match path_and_id {
        "docs/tour/snippets/09_accessibility.rs#read_aloud" => {
            s09_accessibility::read_aloud_output().unwrap()
        }
        other => panic!("no output function is wired for {other} in tests/tour.rs"),
    }
}

fn first_difference(shown: &[String], wanted: &[String]) -> String {
    for i in 0..shown.len().max(wanted.len()) {
        let (a, b) = (shown.get(i), wanted.get(i));
        if a != b {
            return format!(
                "first difference at line {} of the block:\n  page:    {:?}\n  snippet: {:?}",
                i + 1,
                a,
                b
            );
        }
    }
    unreachable!("the blocks are equal")
}

/// The pages `docs/tour/NN_*.md`, in order.
fn pages() -> Vec<PathBuf> {
    let mut pages: Vec<PathBuf> = fs::read_dir(root().join("docs/tour"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.ends_with(".md") && name.as_bytes()[0].is_ascii_digit()
        })
        .collect();
    pages.sort();
    pages
}

fn files_in(dir: &str, extension: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root().join(dir))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(extension))
        .collect();
    names.sort();
    names
}

/// Every displayed block is the region it names, every image exists and is
/// shown, every snippet file is compiled here and every page is in the index.
#[test]
fn pages_match_their_snippets() {
    let mut linked_images = Vec::new();
    for page in pages() {
        let name = page.file_name().unwrap().to_string_lossy().into_owned();
        let text = read(&page);
        let lines = lines_of(&text);
        for (at, line) in lines.iter().enumerate() {
            let Some(marker) = parse_marker(line) else {
                continue;
            };
            let Marker {
                kind,
                path,
                id,
                max,
            } = marker;
            if kind == "output" {
                let id = id.unwrap_or_else(|| panic!("{name}: `{line}` has no #ID"));
                let shown = block_after(&name, &lines, at, "text");
                let wanted = lines_of(&snippet_output(&format!("{path}#{id}")));
                assert!(
                    shown == wanted,
                    "{name}: the block after `{line}` differs from the output; {}",
                    first_difference(&shown, &wanted)
                );
                continue;
            }
            let shown = block_after(&name, &lines, at, "rust");
            let source = lines_of(&read(&root().join(path)));
            if kind == "quote" {
                let found = source.windows(shown.len()).any(|w| w == shown.as_slice());
                assert!(
                    found && !shown.is_empty(),
                    "{name}: the block after `{line}` is not a contiguous piece of {path}"
                );
                assert!(
                    shown.len() <= MAX_REGION_LINES,
                    "{name}: the block after `{line}` is {} lines; the most is {MAX_REGION_LINES}",
                    shown.len()
                );
            } else {
                let id = id.unwrap_or_else(|| panic!("{name}: `{line}` has no #ID"));
                let wanted = region(path, &source, id, max);
                assert!(
                    shown == wanted,
                    "{name}: the block after `{line}` differs from the snippet; {}",
                    first_difference(&shown, &wanted)
                );
            }
        }
        // Images.
        let key = "](../../tests/golden/tour/";
        for part in text.split(key).skip(1) {
            let png = part.split(')').next().unwrap();
            assert!(
                root().join("tests/golden/tour").join(png).is_file(),
                "{name}: image {png} does not exist in tests/golden/tour"
            );
            linked_images.push(png.to_string());
        }
    }
    for png in files_in("tests/golden/tour", ".png") {
        assert!(
            linked_images.contains(&png),
            "tests/golden/tour/{png} is not shown on any page"
        );
    }
    // Every snippet file is compiled by this file.
    let this = read(&root().join("tests/tour.rs"));
    for snippet in files_in("docs/tour/snippets", ".rs") {
        let attr = format!("#[path = \"../docs/tour/snippets/{snippet}\"]");
        assert!(
            this.contains(&attr),
            "docs/tour/snippets/{snippet} is not included by tests/tour.rs"
        );
    }
    // Every page is in the index.
    let index = read(&root().join("docs/tour/README.md"));
    for page in pages() {
        let name = page.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            index.contains(&format!("]({name})")),
            "docs/tour/README.md does not link {name}"
        );
    }
}

/// Chapter 5's table lists the eight colours of `OKABE_ITO_DISTINCT`, in
/// order.
#[test]
fn the_palette_table_lists_the_default_palette() {
    let page = read(&root().join("docs/tour/05_colour_and_themes.md"));
    let mut from = 0;
    for color in gup::OKABE_ITO_DISTINCT {
        let [r, g, b, _] = color.to_rgba8();
        let hex = format!("#{r:02X}{g:02X}{b:02X}");
        let at = page[from..]
            .find(&hex)
            .unwrap_or_else(|| panic!("05_colour_and_themes.md does not list {hex} in order"));
        from += at + hex.len();
    }
}

/// Chapter 11 quotes three rows of `PERF_BUDGETS.md`; a re-recorded budget
/// fails this test until the page is updated. It also keeps the page free of
/// claims it must not make.
#[test]
fn chapter_11_quotes_the_budget() {
    let budgets = read(&root().join("crates/gup-core/PERF_BUDGETS.md"));
    let page = read(&root().join("docs/tour/11_big_data.md"));
    for metric in [
        "zoom.columns.bytes",
        "zoom_culled.draws.median",
        "zoom_culled.gpu_pass.median_ms",
    ] {
        let tag = format!("| `{metric}`");
        let row = budgets
            .lines()
            .find(|l| l.starts_with(&tag))
            .unwrap_or_else(|| panic!("PERF_BUDGETS.md has no row for {metric}"));
        let recorded = row.split('|').nth(2).unwrap().trim();
        let shown = page
            .lines()
            .find(|l| l.contains(&format!("`{metric}`")))
            .unwrap_or_else(|| panic!("11_big_data.md does not name {metric}"));
        let quotes_it = shown.split('|').any(|cell| {
            cell.trim()
                .strip_prefix(recorded)
                .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_digit() || c == '.'))
        });
        assert!(
            quotes_it,
            "11_big_data.md: the row for {metric} does not show the recorded value {recorded}: {shown}"
        );
    }
    let lower = page.to_lowercase();
    for banned in ["billion", "interactive 100", "real time", "chunk_culling"] {
        assert!(!lower.contains(banned), "11_big_data.md says {banned:?}");
    }
}
