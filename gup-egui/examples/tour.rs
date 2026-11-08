// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The feature tour (`docs/tour/`) as an app: a chapter list, and for each
//! chart its code beside the live chart. Every chart is the function the page
//! and the golden image come from, in `docs/tour/snippets/`, and the code shown
//! is that file's region, found as `tests/tour.rs` finds it.
//!
//! Run it with `cargo run -p gup-egui --example tour`. Page Up and Page Down
//! (or `[` and `]`) change chapter. `GUP_SCREENSHOT_PATH=/some/dir` saves a
//! screenshot of every chapter as `NN.png` (and `NN_K.png` for the chapter's
//! other charts) and exits.

// The snippets are written against the `gup` facade, which is `gup-core`
// re-exported; the alias keeps gup-egui from depending on the facade.
extern crate gup_core as gup;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use eframe::egui_wgpu::RenderState;
use egui::text::{LayoutJob, TextFormat};
use gup::prelude::*;
use gup::{Input, Key};
use gup_egui::GupWidget;

#[path = "../../docs/tour/region.rs"]
mod region;

#[path = "../../docs/tour/snippets/01_five_lines.rs"]
#[allow(dead_code)]
mod s01;
#[path = "../../docs/tour/snippets/02_vocabulary.rs"]
#[allow(dead_code)]
mod s02;
#[path = "../../docs/tour/snippets/03_encodings.rs"]
#[allow(dead_code)]
mod s03;
#[path = "../../docs/tour/snippets/04_axes_and_labels.rs"]
#[allow(dead_code)]
mod s04;
#[path = "../../docs/tour/snippets/05_colour_and_themes.rs"]
#[allow(dead_code)]
mod s05;
#[path = "../../docs/tour/snippets/06_lines_and_curves.rs"]
#[allow(dead_code)]
mod s06;
#[path = "../../docs/tour/snippets/07_text_and_annotation.rs"]
#[allow(dead_code)]
mod s07;
#[path = "../../docs/tour/snippets/08_interaction.rs"]
#[allow(dead_code)]
mod s08;
#[path = "../../docs/tour/snippets/09_accessibility.rs"]
#[allow(dead_code)]
mod s09;
#[path = "../../docs/tour/snippets/10_live_data.rs"]
#[allow(dead_code)]
mod s10;
#[path = "../../docs/tour/snippets/11_big_data.rs"]
#[allow(dead_code)]
mod s11;
#[path = "../../docs/tour/snippets/12_under_the_builders.rs"]
#[allow(dead_code)]
mod s12;

/// The most lines this app shows from one region (the pages' limit is 30; a
/// chapter's page may raise it with `max=N`).
const MAX_REGION_LINES: usize = 120;

type Boxed = Box<dyn Chart>;
/// Builds a chart. The flag is true while the chart is on screen; a thread
/// that feeds a live chart holds it back otherwise.
type Build = fn(Arc<AtomicBool>) -> gup::Result<Boxed>;

fn boxed(chart: impl Chart + 'static) -> gup::Result<Boxed> {
    Ok(Box::new(chart))
}

/// The chart as a reader should first meet it: fitted, nothing hovered,
/// focused or brushed, even where the golden image applies inputs first.
fn fitted(chart: impl Chart + 'static) -> gup::Result<Boxed> {
    let mut chart: Boxed = Box::new(chart);
    for input in [
        Input::key(Key::ResetView),
        Input::key(Key::Escape),
        Input::PointerLeft,
    ] {
        let _ = chart.handle(input);
    }
    Ok(chart)
}

/// What a tab shows beside its code.
#[derive(Clone, Copy)]
enum Body {
    /// A live chart.
    Chart(Build),
    /// Text that a function of the snippet file returns.
    Text(fn() -> gup::Result<String>),
    /// Chapter 1's chart, with buttons that write it to a PNG and an SVG.
    Save,
    /// Text for a part of the chapter that cannot be live.
    Note(&'static str),
}

/// One chart (or one thing to read) of a chapter.
struct Tab {
    label: &'static str,
    /// The regions of the chapter's snippet file whose code is shown, in order.
    regions: &'static [&'static str],
    /// What to try.
    hint: &'static str,
    body: Body,
}

struct Chapter {
    /// The page, `docs/tour/NN_*.md`.
    page: &'static str,
    /// The snippet file the regions are in.
    source: &'static str,
    source_name: &'static str,
    tabs: &'static [Tab],
}

const FREE: &str = "Scroll to zoom, drag to pan, 0 resets the view.";

macro_rules! tab {
    ($label:expr, [$($region:expr),+], $hint:expr, $f:expr) => {
        Tab {
            label: $label,
            regions: &[$($region),+],
            hint: $hint,
            body: Body::Chart(|_| boxed($f())),
        }
    };
}

macro_rules! chapter {
    ($page:expr, $file:expr, $tabs:expr) => {
        Chapter {
            page: include_str!(concat!("../../docs/tour/", $page, ".md")),
            source: include_str!(concat!("../../docs/tour/snippets/", $file, ".rs")),
            source_name: concat!("docs/tour/snippets/", $file, ".rs"),
            tabs: $tabs,
        }
    };
}

static CHAPTERS: &[Chapter] = &[
    chapter!(
        "01_five_lines",
        "01_five_lines",
        &[
            tab!(
                "The chart",
                ["country", "five_lines"],
                FREE,
                s01::five_lines
            ),
            Tab {
                label: "Save to a file",
                regions: &["save"],
                hint: "The same chart, written at 640 by 400.",
                body: Body::Save,
            },
        ]
    ),
    chapter!(
        "02_vocabulary",
        "02_vocabulary",
        &[
            tab!("scatter", ["scatter"], FREE, s02::scatter),
            tab!("line", ["line"], FREE, s02::line),
            tab!("bar", ["bar"], FREE, s02::bar),
            tab!("bar, stacked", ["bar_stacked"], FREE, s02::bar_stacked),
            tab!("barh", ["barh"], FREE, s02::barh),
            tab!("area", ["area"], FREE, s02::area),
            tab!("histogram", ["histogram"], FREE, s02::histogram),
            tab!("heatmap", ["heatmap"], FREE, s02::heatmap),
            tab!("box plot", ["boxplot"], FREE, s02::boxplot),
        ]
    ),
    chapter!(
        "03_encodings",
        "03_encodings",
        &[tab!(
            "Encodings",
            ["encodings"],
            "Zoom in on the crowded corner: the sizes and colours stay with their rows.",
            s03::encodings
        )]
    ),
    chapter!(
        "04_axes_and_labels",
        "04_axes_and_labels",
        &[
            tab!(
                "A time axis",
                ["time_axis"],
                "Scroll to zoom along the time axis, drag to pan, 0 resets the view.",
                s04::time_axis
            ),
            tab!("A second y axis", ["dual_axis"], FREE, s04::dual_axis),
            tab!(
                "Tick labels",
                ["dollars", "bar_labels"],
                FREE,
                s04::bar_labels
            ),
        ]
    ),
    chapter!(
        "05_colour_and_themes",
        "05_colour_and_themes",
        &[
            tab!("The palette", ["palette"], FREE, s05::palette),
            tab!("Dark", ["dark"], FREE, s05::dark),
            tab!("High contrast", ["high_contrast"], FREE, s05::high_contrast),
        ]
    ),
    chapter!(
        "06_lines_and_curves",
        "06_lines_and_curves",
        &[tab!("Curves", ["curves"], FREE, s06::curves)]
    ),
    chapter!(
        "07_text_and_annotation",
        "07_text_and_annotation",
        &[tab!(
            "Labels",
            ["labels"],
            "Scroll to zoom in and out and watch which labels are kept; drag to pan, 0 resets.",
            s07::labels
        )]
    ),
    chapter!(
        "08_interaction",
        "08_interaction",
        &[
            Tab {
                label: "Tooltip, zoom, pan",
                regions: &["pixel", "tooltip"],
                hint: "Hover a bird for its tooltip. Scroll over the flock to zoom, drag to pan, 0 resets. The page sends these as Input values; here the window does.",
                body: Body::Chart(|_| fitted(s08::tooltip()?)),
            },
            Tab {
                label: "Brush",
                regions: &["pixel", "brush"],
                hint: "Drag a rectangle to brush it, Escape clears it. Scroll to zoom.",
                body: Body::Chart(|_| fitted(s08::brush()?)),
            },
        ]
    ),
    chapter!(
        "09_accessibility",
        "09_accessibility",
        &[
            Tab {
                label: "Keys",
                regions: &["keys"],
                hint: "Click the chart or Tab to it, then Right and Left walk the points (the tooltip follows), + and - zoom, Shift with an arrow pans, 0 resets, Escape leaves.",
                body: Body::Chart(|_| fitted(s09::keys()?)),
            },
            Tab {
                label: "Read aloud",
                regions: &["read_aloud"],
                hint: "What a screen reader is given, from the same chart.",
                body: Body::Text(s09::read_aloud_output),
            },
        ]
    ),
    chapter!(
        "10_live_data",
        "10_live_data",
        &[
            Tab {
                label: "Append",
                regions: &["append"],
                hint: "Live: a thread pushes a row through the Appender every 50 ms and the axes refit. Scroll to zoom, drag to pan, 0 refits.",
                body: Body::Chart(live_append),
            },
            Tab {
                label: "Window",
                regions: &["window"],
                hint: "Live: a thread pushes 600 rows a second; the chart keeps the newest 2,000, so the window slides. Scroll to zoom, drag to pan.",
                body: Body::Chart(live_window),
            },
        ]
    ),
    chapter!(
        "11_big_data",
        "11_big_data",
        &[Tab {
            label: "200,000 points",
            regions: &["columns"],
            hint: "Scroll to zoom into the spiral, drag to pan, 0 resets the view. The first view builds 200,000 points.",
            body: Body::Chart(|_| boxed(s11::columns()?)),
        }]
    ),
    chapter!(
        "12_under_the_builders",
        "12_under_the_builders",
        &[
            tab!("A colour ramp", ["ember", "function"], FREE, s12::function),
            tab!("A mark", ["mark"], FREE, s12::mark),
        ]
    ),
    chapter!(
        "13_everywhere",
        "01_five_lines",
        &[Tab {
            label: "Targets",
            regions: &["save"],
            hint: "",
            body: Body::Note(
                "This window is the egui target: a GupWidget draws each chart into a texture on egui's own device and forwards the pointer and keys.\n\n\
                 The same chart value also reaches:\n\
                 \u{2022} a window, with gup::show;\n\
                 \u{2022} a PNG and an SVG file, with save_png and save_svg (chapter 1's \"Save to a file\" tab has buttons);\n\
                 \u{2022} a texture, with TextureTarget;\n\
                 \u{2022} Bevy, with gup_bevy::GupPlugin;\n\
                 \u{2022} a browser canvas, with web::CanvasApp (WebGPU).\n\n\
                 Not live here: Bevy needs its own app, and the browser needs a wasm build, which this window cannot host. The page lists what proves each target; `mask wasm-browser` renders the browser one.",
            ),
        }]
    ),
];

/// The page's title: its `# ` line.
fn title(page: &str) -> &str {
    page.lines()
        .find_map(|l| l.strip_prefix("# "))
        .expect("a tour page starts with a title")
}

/// The page's first paragraph, as one line of plain text.
fn summary(page: &str) -> String {
    let mut lines = page
        .lines()
        .skip_while(|l| !l.starts_with("# "))
        .skip(1)
        .skip_while(|l| l.trim().is_empty());
    lines
        .by_ref()
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The code of a tab: its regions, a blank line between them.
fn code(chapter: &Chapter, tab: &Tab) -> String {
    let lines = region::lines_of(chapter.source);
    tab.regions
        .iter()
        .map(|id| region::region(chapter.source_name, &lines, id, MAX_REGION_LINES).join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

// The live charts of chapter 10. The builders repeat the setters of the
// snippet regions (the code shown); a thread does what the snippet's batches
// stand in for, at a steady rate.

fn live_append(running: Arc<AtomicBool>) -> gup::Result<Boxed> {
    let mut chart = gup::line(Vec::<s10::Sample>::new())
        .x(|s| s.t)
        .y(|s| s.level)
        .width(2.0)
        .title("Signal")
        .x_title("Time (s)")
        .y_title("Level");
    let feed = chart.appender();
    feed.extend((0..120).map(s10::sample))
        .expect("the chart is alive");
    std::thread::spawn(move || {
        for i in 120..2_000 {
            std::thread::sleep(Duration::from_millis(50));
            while !running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            if feed.push(s10::sample(i)).is_err() {
                break;
            }
        }
    });
    boxed(chart)
}

fn live_window(running: Arc<AtomicBool>) -> gup::Result<Boxed> {
    let mut chart = gup::line(Vec::<s10::Sample>::new())
        .x(|s| s.t)
        .y(|s| s.level)
        .width(1.5)
        .title("A window on the newest readings")
        .x_title("Time (s)")
        .y_title("Level")
        .window(Window::LastRows(2_000));
    chart.layers_mut()[0].retain(Retain::GpuOnly);
    let feed = chart.appender();
    feed.extend((0..2_000).map(s10::reading))
        .expect("the chart is alive");
    std::thread::spawn(move || {
        let mut i = 2_000;
        loop {
            std::thread::sleep(Duration::from_millis(16));
            while !running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            if feed.extend((i..i + 10).map(s10::reading)).is_err() {
                break;
            }
            i += 10;
        }
    });
    boxed(chart)
}

/// A tab once it has been opened.
enum Built {
    Chart {
        chart: Boxed,
        widget: GupWidget,
        running: Arc<AtomicBool>,
    },
    /// A chart the tour could not build, with the reason.
    Failed(String),
    Text(String),
    Save {
        chart: Boxed,
        widget: GupWidget,
        saved: Option<String>,
    },
}

fn open(tab: &Tab, render_state: &RenderState) -> Built {
    let attempt = || -> gup::Result<Built> {
        Ok(match tab.body {
            Body::Chart(build) => {
                let running = Arc::new(AtomicBool::new(true));
                Built::Chart {
                    chart: build(running.clone())?,
                    widget: GupWidget::new(render_state),
                    running,
                }
            }
            Body::Text(f) => Built::Text(f()?),
            Body::Save => Built::Save {
                chart: boxed(s01::five_lines())?,
                widget: GupWidget::new(render_state),
                saved: None,
            },
            Body::Note(text) => Built::Text(text.to_string()),
        })
    };
    attempt().unwrap_or_else(|e| Built::Failed(format!("This chart could not be built: {e}")))
}

/// Write the chapter 1 chart to `name` in a temporary directory, as a PNG or
/// an SVG; the message to show.
fn save(svg: bool) -> String {
    let dir = std::env::temp_dir().join("gup-tour");
    let path: PathBuf = dir.join(if svg { "chart.svg" } else { "chart.png" });
    let result = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|()| {
            let cx = Context::shared().map_err(|e| e.to_string())?;
            let mut chart = s01::five_lines();
            if svg {
                chart.save_svg(&cx, &path, 640, 400)
            } else {
                chart.save_png(&cx, &path, 640, 400)
            }
            .map_err(|e| e.to_string())
        });
    match result {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

// Highlighting: comments, strings (raw ones may span lines), keywords, types
// and numbers. Enough to read by; not a parser.

const KEYWORDS: &[&str] = &[
    "as", "const", "else", "enum", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "trait",
    "true", "type", "use", "where", "while",
];

fn highlight(code: &str, font: egui::FontId, base: egui::Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut add = |text: &str, color: egui::Color32| {
        if !text.is_empty() {
            job.append(
                text,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        }
    };
    let comment = egui::Color32::from_rgb(110, 118, 110);
    let string = egui::Color32::from_rgb(25, 120, 60);
    let keyword = egui::Color32::from_rgb(150, 45, 160);
    let ty = egui::Color32::from_rgb(20, 105, 135);
    let number = egui::Color32::from_rgb(30, 90, 170);
    let bytes = code.as_bytes();
    let (mut plain, mut i) = (0, 0);
    while i < bytes.len() {
        let rest = &code[i..];
        let (end, color) = if rest.starts_with("//") {
            (i + rest.find('\n').unwrap_or(rest.len()), comment)
        } else if rest.starts_with("r#\"") {
            (i + rest.find("\"#").map_or(rest.len(), |at| at + 2), string)
        } else if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let mut end = (j + 1).min(bytes.len());
            while !code.is_char_boundary(end) {
                end += 1;
            }
            (end, string)
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &code[i..j];
            let color = if KEYWORDS.contains(&word) {
                keyword
            } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
                ty
            } else {
                base
            };
            (j, color)
        } else if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.')
            {
                j += 1;
            }
            (j, number)
        } else {
            i += code[i..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        if color == base {
            i = end;
            continue;
        }
        add(&code[plain..i], base);
        add(&code[i..end], color);
        plain = end;
        i = end;
    }
    add(&code[plain..], base);
    job
}

/// What the screenshot run is doing.
struct Shots {
    dir: PathBuf,
    /// Every (chapter, tab) in order, and where in it we are.
    next: usize,
    frames: u32,
    asked: bool,
}

impl Shots {
    fn steps() -> Vec<(usize, usize)> {
        let mut steps = Vec::new();
        for (c, chapter) in CHAPTERS.iter().enumerate() {
            for t in 0..chapter.tabs.len() {
                steps.push((c, t));
            }
        }
        steps
    }
}

struct App {
    render_state: RenderState,
    chapter: usize,
    tab: usize,
    /// The opened tabs, by chapter and tab.
    built: Vec<Vec<Option<Built>>>,
    shots: Option<Shots>,
}

impl App {
    fn new(render_state: RenderState, dir: Option<PathBuf>) -> Self {
        let built = CHAPTERS
            .iter()
            .map(|c| c.tabs.iter().map(|_| None).collect())
            .collect();
        Self {
            render_state,
            chapter: 0,
            tab: 0,
            built,
            shots: dir.map(|dir| Shots {
                dir,
                next: 0,
                frames: 0,
                asked: false,
            }),
        }
    }

    fn go(&mut self, chapter: usize) {
        if chapter != self.chapter {
            self.chapter = chapter;
            self.tab = 0;
        }
    }

    fn chapters_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("chapters")
            .exact_width(250.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("A Tour of Gup");
                ui.label("Page Up / Page Down, or [ and ]");
                ui.add_space(8.0);
                for (i, chapter) in CHAPTERS.iter().enumerate() {
                    let label = egui::RichText::new(title(chapter.page)).size(15.0);
                    let selected = i == self.chapter;
                    if ui
                        .add(
                            egui::Button::selectable(selected, label)
                                .min_size(egui::vec2(ui.available_width(), 26.0))
                                .wrap_mode(egui::TextWrapMode::Truncate),
                        )
                        .clicked()
                    {
                        self.go(i);
                    }
                }
            });
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (prev, next) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::OpenBracket),
                i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::CloseBracket),
            )
        });
        if prev && self.chapter > 0 {
            self.go(self.chapter - 1);
        }
        if next && self.chapter + 1 < CHAPTERS.len() {
            self.go(self.chapter + 1);
        }
    }

    fn main_panel(&mut self, ctx: &egui::Context) {
        let chapter = &CHAPTERS[self.chapter];
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading(egui::RichText::new(title(chapter.page)).size(24.0));
            ui.add_space(2.0);
            ui.label(egui::RichText::new(summary(chapter.page)).size(15.0));
            ui.add_space(6.0);
            if chapter.tabs.len() > 1 {
                ui.horizontal_wrapped(|ui| {
                    for (i, tab) in chapter.tabs.iter().enumerate() {
                        if ui.selectable_label(i == self.tab, tab.label).clicked() {
                            self.tab = i;
                        }
                    }
                });
                ui.add_space(4.0);
            }
            ui.separator();

            let tab = &chapter.tabs[self.tab];
            let slot = &mut self.built[self.chapter][self.tab];
            if slot.is_none() {
                *slot = Some(open(tab, &self.render_state));
            }
            // A live chart's thread runs only while its tab is on screen.
            for (c, row) in self.built.iter().enumerate() {
                for (t, built) in row.iter().enumerate() {
                    if let Some(Built::Chart { running, .. }) = built {
                        running.store(c == self.chapter && t == self.tab, Ordering::Relaxed);
                    }
                }
            }

            let avail = ui.available_size();
            let has_code = !tab.regions.is_empty() && !matches!(tab.body, Body::Note(_));
            let chart_w = if has_code {
                (avail.x * 0.52).clamp(380.0, 900.0)
            } else {
                avail.x
            };
            let panel_h = (avail.y - 74.0).max(240.0);
            let chart_h = panel_h.min(chart_w * 0.85);
            let built = self.built[self.chapter][self.tab].as_mut().expect("opened above");
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(chart_w);
                    match built {
                        Built::Chart { chart, widget, .. } => {
                            widget.show(ui, egui::vec2(chart_w, chart_h), chart.as_mut());
                        }
                        Built::Save { chart, widget, saved } => {
                            widget.show(ui, egui::vec2(chart_w, chart_h - 34.0), chart.as_mut());
                            ui.horizontal(|ui| {
                                if ui.button("Save PNG").clicked() {
                                    *saved = Some(save(false));
                                }
                                if ui.button("Save SVG").clicked() {
                                    *saved = Some(save(true));
                                }
                                if let Some(message) = saved {
                                    ui.label(message.as_str());
                                }
                            });
                        }
                        Built::Text(text) => {
                            egui::ScrollArea::vertical()
                                .id_salt("text")
                                .max_height(panel_h)
                                .show(ui, |ui| {
                                    if matches!(tab.body, Body::Note(_)) {
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(text.as_str()).size(15.0));
                                        ui.add_space(8.0);
                                        ui.hyperlink_to(
                                            "docs/tour/13_everywhere.md, the page with the table of targets",
                                            "https://github.com/au-phiware/gup/blob/main/docs/tour/13_everywhere.md",
                                        );
                                    } else {
                                        ui.label(egui::RichText::new(text.as_str()).monospace().size(13.0));
                                    }
                                });
                        }
                        Built::Failed(reason) => {
                            ui.colored_label(egui::Color32::from_rgb(180, 30, 30), reason.as_str());
                        }
                    }
                    if !tab.hint.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(tab.hint).size(14.0).italics());
                    }
                });
                if has_code {
                    ui.add_space(8.0);
                    let source = code(chapter, tab);
                    let font = egui::FontId::monospace(13.0);
                    let base = ui.visuals().text_color();
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().extreme_bg_color)
                        .show(ui, |ui| {
                            let width = (ui.available_width() - 8.0).max(120.0);
                            ui.set_width(width);
                            egui::ScrollArea::both()
                                .id_salt("code")
                                .max_height(panel_h - 14.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(egui::Label::new(highlight(&source, font, base)).extend());
                                });
                        });
                }
            });
        });
    }

    /// Advance the screenshot run; true while it is still going.
    fn screenshots(&mut self, ctx: &egui::Context) {
        let Some(shots) = &mut self.shots else {
            return;
        };
        ctx.request_repaint();
        let steps = Shots::steps();
        let Some(&(chapter, tab)) = steps.get(shots.next) else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        if (self.chapter, self.tab) != (chapter, tab) {
            self.chapter = chapter;
            self.tab = tab;
            shots.frames = 0;
            shots.asked = false;
            return;
        }
        shots.frames += 1;
        // Live charts get time to move; the 200,000 points get a few frames.
        let wait = if chapter == 9 { 120 } else { 30 };
        if shots.frames == wait && !shots.asked {
            shots.asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let name = if tab == 0 {
                format!("{:02}.png", chapter + 1)
            } else {
                format!("{:02}_{tab}.png", chapter + 1)
            };
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let [w, h] = img.size;
            let path = shots.dir.join(name);
            image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8)
                .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
            shots.next += 1;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.keys(ctx);
        self.chapters_panel(ctx);
        self.main_panel(ctx);
        self.screenshots(ctx);
    }
}

fn main() -> eframe::Result {
    let shots = std::env::var_os("GUP_SCREENSHOT_PATH").map(PathBuf::from);
    if let Some(dir) = &shots {
        std::fs::create_dir_all(dir).expect("the screenshot directory");
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "A Tour of Gup",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            let rs = cc.wgpu_render_state.as_ref().expect("the wgpu backend");
            Ok(Box::new(App::new(rs.clone(), shots)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tab's regions exist, every page has a title and a summary, and
    /// the summary is short.
    #[test]
    fn every_chapter_has_its_code_and_summary() {
        assert_eq!(CHAPTERS.len(), 13);
        for chapter in CHAPTERS {
            assert!(!title(chapter.page).is_empty());
            let summary = summary(chapter.page);
            assert!(
                (20..700).contains(&summary.len()),
                "{}: {summary:?}",
                chapter.page.lines().next().unwrap()
            );
            for tab in chapter.tabs {
                let text = code(chapter, tab);
                assert!(!text.is_empty(), "{}", tab.label);
                for id in tab.regions {
                    assert!(text.lines().count() > 0, "{id}");
                }
            }
        }
    }

    /// The code the app shows is the code the page shows: every region of every
    /// tab is a fenced block on the chapter's page (the way `tests/tour.rs`
    /// compares them is by marker; this checks the same text appears).
    #[test]
    fn the_code_is_on_the_page() {
        for chapter in CHAPTERS
            .iter()
            .filter(|c| c.page.contains("tour: docs/tour/snippets/"))
        {
            let page = region::lines_of(chapter.page).join("\n");
            let lines = region::lines_of(chapter.source);
            for tab in chapter.tabs {
                for id in tab.regions {
                    let body = region::region(chapter.source_name, &lines, id, MAX_REGION_LINES)
                        .join("\n");
                    assert!(
                        page.contains(&body),
                        "{}: region {id} is not on the page",
                        chapter.source_name
                    );
                }
            }
        }
    }

    /// The buttons of chapter 1 write a PNG and an SVG and say where.
    #[test]
    fn the_save_buttons_write_files() {
        for (svg, name) in [(false, "chart.png"), (true, "chart.svg")] {
            let message = save(svg);
            assert!(message.starts_with("Saved "), "{message}");
            let path = std::env::temp_dir().join("gup-tour").join(name);
            assert!(message.ends_with(&path.display().to_string()), "{message}");
            assert!(std::fs::metadata(&path).unwrap().len() > 100, "{name}");
        }
    }

    #[test]
    fn highlighting_keeps_every_character() {
        let text = "let x = \"a // b\"; // c\nr#\"raw\nmore\"# 1.5 é";
        let job = highlight(text, egui::FontId::monospace(12.0), egui::Color32::BLACK);
        assert_eq!(job.text, text);
    }
}
