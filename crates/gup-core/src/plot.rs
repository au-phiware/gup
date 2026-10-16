// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Plot`: shared x/y scales, data layers, axes and a title, resolved
//! into a [`Layout`] and a [`Scene`] (RFC-001 §8–9, S0a subset).
//!
//! Any number of layers share the x/y scales, whose automatic domains
//! union across layers and channels. Left and bottom axes with tick
//! labels, an optional title, axis titles, a [`Theme`](crate::Theme) and
//! colour legends at the right (found through
//! [`ShaderFn::legend`](crate::ShaderFn::legend)); grids and the
//! object-safe `Chart` trait are later RFC-001 S7 steps.

use crate::channel::{Mark, Px};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};
use crate::scale::{DynPositionScale, PositionScale, ScaleRef};
use crate::scene::{
    Anchor, HAlign, Item, ItemKind, Rule, Scene, TextRole, TextRun, TextStyle, VAlign, Z_AXES,
    Z_GRID, Z_TITLE,
};
use crate::selection::{Layer, Selection};
#[cfg(not(target_arch = "wasm32"))]
use crate::target::{ImageTarget, save_png};
use crate::theme::Theme;
use gup_text::TextSystem;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// A text run placed by layout, with its ink box.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedText {
    /// The run, as it appears in the scene.
    pub run: TextRun,
    /// Where its glyphs are drawn.
    pub bounds: Rect,
}

/// The resolved layout: where everything went, in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
    /// The plot (data) rectangle. Marks are clipped to it.
    pub plot: Rect,
    /// Every text run with its ink box.
    pub texts: Vec<PlacedText>,
    /// The rectangles covered by guide rules.
    pub guides: Vec<Rect>,
    /// How far marks extend past their positions (e.g. a radius).
    pub mark_overhang: f32,
    /// x tick values and their pixel positions.
    pub x_ticks: Vec<(f64, f32)>,
    /// y tick values and their pixel positions.
    pub y_ticks: Vec<(f64, f32)>,
    /// The legend column, `Some` exactly when a legend is drawn. Lies
    /// inside the canvas, right of [`plot`](Self::plot).
    pub legend: Option<Rect>,
}

/// A layout plus the scene drawn from it.
#[derive(Debug)]
pub struct Resolved {
    /// Where everything went.
    pub layout: Layout,
    /// What to draw.
    pub scene: Scene,
}

/// A chart with shared x/y position scales, one data layer, axes and an
/// optional title.
///
/// ```
/// use gup_core::prelude::*;
///
/// #[derive(Clone)]
/// struct Reading { gdp: f64, life_exp: f64, temp: f64 }
/// let readings: Vec<Reading> = (1..=50)
///     .map(|i| {
///         let i = f64::from(i);
///         Reading { gdp: i * 1_000.0, life_exp: 40.0 + i, temp: i % 30.0 }
///     })
///     .collect();
///
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
/// let heat = Sequential::viridis();
/// plot.title("Wealth and health")
///     .add(Selection::<Reading, Circle>::new(readings))
///     .attr(Circle::X, x.encode(|r: &Reading| r.gdp))
///     .attr(Circle::Y, y.encode(|r: &Reading| r.life_exp))
///     .attr(Circle::FILL, heat.encode(|r: &Reading| r.temp))
///     .attr(Circle::RADIUS, Px(4.0));
/// // .attr(Circle::RADIUS, heat.encode(..))  // ✗ Sequential produces Color, RADIUS needs Px
/// let image = plot.render(&cx, 640, 400)?;
/// assert_eq!(image.dimensions(), (640, 400));
/// # Ok::<(), gup_core::Error>(())
/// ```
#[derive(Default)]
pub struct Plot {
    title: Option<String>,
    x: Option<Box<dyn DynPositionScale>>,
    y: Option<Box<dyn DynPositionScale>>,
    layers: Vec<Box<dyn Layer>>,
    x_title: Option<String>,
    y_title: Option<String>,
    theme: Theme,
    /// Set by `legends(false)`.
    legends_off: bool,
}

impl std::fmt::Debug for Plot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plot")
            .field("title", &self.title)
            .field("layers", &self.layers.len())
            .finish_non_exhaustive()
    }
}

/// `s`, or its longest prefix followed by "…" that fits in `max` pixels
/// at `size` (U+2026 is in the bundled font subset).
fn fit_text(text: &TextSystem, s: &str, size: f32, max: f32) -> String {
    if text.measure(s, size).width <= max {
        return s.to_owned();
    }
    let mut kept: Vec<char> = s.chars().collect();
    while !kept.is_empty() {
        kept.pop();
        let candidate: String = kept.iter().collect::<String>().trim_end().to_owned() + "…";
        if text.measure(&candidate, size).width <= max {
            return candidate;
        }
    }
    "…".to_owned()
}

impl Plot {
    /// An empty plot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the title.
    pub fn title(&mut self, title: impl Into<String>) -> &mut Self {
        self.title = Some(title.into());
        self
    }

    /// Set the x axis title, centred under the x tick labels.
    pub fn x_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.x_title = Some(title.into());
        self
    }

    /// Set the y axis title: horizontal, left-aligned above the y axis.
    pub fn y_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.y_title = Some(title.into());
        self
    }

    /// Draw the colour legends beside the plot (default), or not: with
    /// `false` the plot rect is as if there were none, for callers who
    /// place their own.
    pub fn legends(&mut self, on: bool) -> &mut Self {
        self.legends_off = !on;
        self
    }

    /// Style the plot with `theme`.
    pub fn theme(&mut self, theme: Theme) -> &mut Self {
        self.theme = theme;
        self
    }

    /// The title, if set.
    pub fn title_text(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Use `scale` for the x axis and return a handle to encode with.
    pub fn x<S: PositionScale>(&mut self, scale: S) -> ScaleRef<S> {
        let handle = ScaleRef::new(scale);
        self.x = Some(Box::new(handle.clone()));
        handle
    }

    /// Use `scale` for the y axis and return a handle to encode with.
    pub fn y<S: PositionScale>(&mut self, scale: S) -> ScaleRef<S> {
        let handle = ScaleRef::new(scale);
        self.y = Some(Box::new(handle.clone()));
        handle
    }

    /// Add a data layer and return it for further `attr` calls.
    pub fn add<T: Send + Sync + 'static, M: Mark>(
        &mut self,
        layer: Selection<T, M>,
    ) -> &mut Selection<T, M> {
        self.layers.push(Box::new(layer));
        self.layers
            .last_mut()
            .and_then(|l| l.as_any_mut().downcast_mut())
            .expect("the layer just pushed has this type")
    }

    /// Zoom both position scales by `factor` about the logical-pixel point
    /// `at` (`factor < 1` zooms in). The values under `at` stay where they
    /// are; a log scale zooms in log space. Only the scales' domains
    /// change, so the next [`resolve`](Self::resolve) writes uniforms and
    /// no column bytes. Does nothing before the first resolve has fitted
    /// and ranged the scales.
    pub fn zoom(&mut self, at: Point, factor: f64) -> Result<()> {
        if !(factor.is_finite() && factor > 0.0) {
            return Err(Error::config(
                "zoom factor",
                format!("{factor} must be finite and positive"),
            ));
        }
        if let Some(x) = &self.x {
            x.zoom(f64::from(at.x), factor)?;
        }
        if let Some(y) = &self.y {
            y.zoom(f64::from(at.y), factor)?;
        }
        Ok(())
    }

    /// Resolve domains, ticks, text, the plot rect and GPU state for a
    /// `width × height` logical-pixel chart.
    pub fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<Resolved> {
        // Errors that a browser reported after an earlier call returned.
        cx.take_gpu_errors()?;
        let (x, y) = match (&self.x, &self.y) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                return Err(Error::config(
                    "plot scales",
                    "set both position scales with `plot.x(..)` and `plot.y(..)` and encode \
                     Circle::X / Circle::Y through the returned handles",
                ));
            }
        };
        if self.layers.is_empty() {
            return Err(Error::config(
                "plot layers",
                "the plot has no layers; add one with `plot.add(..)`",
            ));
        }
        for (i, layer) in self.layers.iter().enumerate() {
            if !(Z_GRID + 1..Z_AXES).contains(&layer.z()) {
                return Err(Error::config(
                    "layer z",
                    format!(
                        "layer {i} has z {}; marks must stay between the grid and the axes, \
                         so z is in {}..={}",
                        layer.z(),
                        Z_GRID + 1,
                        Z_AXES - 1
                    ),
                ));
            }
        }

        let th = self.theme.clone();
        for layer in &mut self.layers {
            layer.apply_theme(&th);
        }

        // 1–2. Domains from column stats, then nice.
        // Every scale clears its union once, then every layer fits into it.
        for layer in &self.layers {
            layer.begin_fit();
        }
        for layer in &mut self.layers {
            layer.fit_domains(cx)?;
        }
        for s in [x, y] {
            if s.is_auto() {
                s.nice();
            }
        }

        // Legends read the fitted scales (a categorical's keys).
        let legends = if self.legends_off {
            Vec::new()
        } else {
            crate::guide::dedup(self.layers.iter().flat_map(|l| l.legends()))
        };

        // 3. Ticks and label measurements give the margins.
        let tick_style = TextStyle {
            size: Px(th.tick_size.0),
            color: th.label,
        };
        let x_ticks = x.ticks(((width / th.x_tick_spacing.0) as usize).max(2));
        let y_ticks = y.ticks(((height / th.y_tick_spacing.0) as usize).max(2));
        let legend_w = if legends.is_empty() {
            0.0
        } else {
            crate::guide::column_width(&cx.text(), &th, &legends) + crate::guide::COLUMN_GAP
        };
        let (title_h, axis_title_h, y_label_w, x_label_h, last_x_label_w) = {
            let text = cx.text();
            let axis_title_h = {
                let m = text.measure("0", th.axis_title_size.0);
                m.cap_height + m.descent
            };
            let title_h = self.title.as_ref().map_or(0.0, |t| {
                let m = text.measure(t, th.title_size.0);
                m.cap_height + m.descent
            });
            let y_label_w = y_ticks
                .labels
                .iter()
                .map(|l| text.measure(l, th.tick_size.0).width)
                .fold(0.0, f32::max);
            let m = text.measure("0", th.tick_size.0);
            let last = x_ticks
                .labels
                .last()
                .map_or(0.0, |l| text.measure(l, th.tick_size.0).width);
            (
                title_h,
                axis_title_h,
                y_label_w,
                m.cap_height + m.descent,
                last,
            )
        };

        // 4. The plot rect, on whole pixels.
        let top = th.pad.0
            + if self.title.is_some() {
                title_h + th.pad.0
            } else {
                0.0
            }
            + if self.y_title.is_some() {
                axis_title_h + th.label_gap.0
            } else {
                0.0
            };
        let left = th.pad.0 + y_label_w + th.label_gap.0 + th.tick_len.0;
        let x_title_h = if self.x_title.is_some() {
            axis_title_h + th.label_gap.0
        } else {
            0.0
        };
        let bottom = height - (th.pad.0 + x_label_h + th.label_gap.0 + th.tick_len.0 + x_title_h);
        let right = width - th.pad.0.max(last_x_label_w / 2.0 + 2.0) - legend_w;
        let plot = Rect::from_edges(left.round(), top.round(), right.round(), bottom.round());
        if plot.width < 10.0 || plot.height < 10.0 {
            return Err(Error::config(
                "plot size",
                format!("{width}×{height} leaves no room for the plot area"),
            ));
        }

        // 5. Ranges, inset so marks on the domain edge stay inside.
        let mark_overhang = self.layers.iter().map(|l| l.overhang()).fold(0.0, f32::max);
        let inset = mark_overhang + 1.0;
        x.set_range(Px(plot.left() + inset), Px(plot.right() - inset));
        y.set_range(Px(plot.bottom() - inset), Px(plot.top() + inset));

        // 6. Uniforms and columns, inside a GPU error scope (GUP-410).
        let mut batches = Vec::with_capacity(self.layers.len());
        for layer in &mut self.layers {
            let batch = cx.scoped(
                || "layer resolution (column upload, uniforms)".to_owned(),
                || layer.prepare(cx, &th),
            )?;
            // Only after the upload succeeded: CPU data the layer's
            // `Retain` policy does not keep.
            layer.release();
            batches.push((layer.z(), batch));
        }

        // 7. The scene.
        let mut scene = Scene::new(width, height, th.background);
        let mut rules = Vec::new();
        let mut labels = Vec::new();
        let axis = |p0: Point, p1: Point| Rule {
            p0,
            p1,
            width: Px(1.0),
            color: th.axis,
        };
        // Axis lines just outside the plot rect, on pixel centres.
        let (ax_y, ay_x) = (plot.bottom() + 0.5, plot.left() - 0.5);
        rules.push(axis(
            Point::new(ay_x, ax_y),
            Point::new(plot.right() - 0.5, ax_y),
        ));
        rules.push(axis(
            Point::new(ay_x, plot.top() + 0.5),
            Point::new(ay_x, ax_y),
        ));
        let mut x_placed = Vec::new();
        for (v, label) in x_ticks.values.iter().zip(&x_ticks.labels) {
            let px = x.eval(*v) as f32;
            let cx_ = px.floor() + 0.5;
            rules.push(axis(
                Point::new(cx_, ax_y),
                Point::new(cx_, ax_y + th.tick_len.0),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(px, ax_y + th.tick_len.0 + th.label_gap.0),
                anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                style: tick_style,
                role: TextRole::TickLabel,
            });
            x_placed.push((*v, px));
        }
        let mut y_placed = Vec::new();
        for (v, label) in y_ticks.values.iter().zip(&y_ticks.labels) {
            let py = y.eval(*v) as f32;
            let cy = py.floor() + 0.5;
            rules.push(axis(
                Point::new(ay_x - th.tick_len.0, cy),
                Point::new(ay_x, cy),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(ay_x - th.tick_len.0 - th.label_gap.0, py),
                anchor: Anchor::new(HAlign::End, VAlign::Middle),
                style: tick_style,
                role: TextRole::TickLabel,
            });
            y_placed.push((*v, py));
        }
        let (title, axis_titles) = {
            let text = cx.text();
            let title = self.title.as_ref().map(|t| TextRun {
                text: fit_text(&text, t, th.title_size.0, plot.width).into(),
                at: Point::new(plot.x + plot.width / 2.0, th.pad.0),
                anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                style: TextStyle {
                    size: th.title_size,
                    color: th.title,
                },
                role: TextRole::Title,
            });
            let axis_style = TextStyle {
                size: th.axis_title_size,
                color: th.label,
            };
            let mut axis_titles = Vec::new();
            if let Some(t) = &self.x_title {
                axis_titles.push(TextRun {
                    text: fit_text(&text, t, th.axis_title_size.0, plot.width).into(),
                    at: Point::new(
                        plot.x + plot.width / 2.0,
                        ax_y + th.tick_len.0 + th.label_gap.0 + x_label_h + th.label_gap.0,
                    ),
                    anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                    style: axis_style,
                    role: TextRole::AxisTitle,
                });
            }
            if let Some(t) = &self.y_title {
                axis_titles.push(TextRun {
                    text: fit_text(&text, t, th.axis_title_size.0, width - 2.0 * th.pad.0).into(),
                    at: Point::new(th.pad.0, plot.top() - th.label_gap.0),
                    anchor: Anchor::new(HAlign::Start, VAlign::Bottom),
                    style: axis_style,
                    role: TextRole::AxisTitle,
                });
            }
            (title, axis_titles)
        };

        let legend = (!legends.is_empty()).then(|| {
            crate::guide::place(
                &cx.text(),
                &th,
                &legends,
                plot.right() + crate::guide::COLUMN_GAP,
                plot,
            )
        });
        let legend_texts: &[TextRun] = legend.as_ref().map_or(&[], |l| &l.texts);
        let texts = {
            let text = cx.text();
            labels
                .iter()
                .chain(&title)
                .chain(&axis_titles)
                .chain(legend_texts)
                .map(|run| PlacedText {
                    run: run.clone(),
                    bounds: text.ink_bounds(&run.layout_run()).into(),
                })
                .collect()
        };
        let mut guides: Vec<Rect> = rules.iter().map(Rule::bounds).collect();
        guides.extend(legend.iter().flat_map(|l| l.guides.iter().copied()));

        let clip = scene.add_clip(plot);
        // Insertion order; `Scene::push` breaks z ties by it.
        for (z, batch) in batches {
            scene.push(Item {
                z,
                clip: Some(clip),
                kind: ItemKind::Marks(batch),
            });
        }
        scene.push(Item {
            z: Z_AXES,
            clip: None,
            kind: ItemKind::Rules(rules),
        });
        scene.push(Item {
            z: Z_AXES,
            clip: None,
            kind: ItemKind::Text(labels),
        });
        let titles: Vec<TextRun> = title.into_iter().chain(axis_titles).collect();
        if !titles.is_empty() {
            scene.push(Item {
                z: Z_TITLE,
                clip: None,
                kind: ItemKind::Text(titles),
            });
        }

        let legend_bounds = legend.as_ref().map(|l| l.bounds);
        if let Some(l) = legend {
            for item in l.items {
                scene.push(item);
            }
        }

        Ok(Resolved {
            layout: Layout {
                width,
                height,
                plot,
                texts,
                guides,
                mark_overhang,
                x_ticks: x_placed,
                y_ticks: y_placed,
                legend: legend_bounds,
            },
            scene,
        })
    }

    /// Resolve and render to an image of `width × height` pixels (dpr 1).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render(&mut self, cx: &Context, width: u32, height: u32) -> Result<image::RgbaImage> {
        Ok(self.render_resolved(cx, width, height)?.0)
    }

    /// Like [`render`](Self::render), also returning the layout.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_resolved(
        &mut self,
        cx: &Context,
        width: u32,
        height: u32,
    ) -> Result<(image::RgbaImage, Layout)> {
        let resolved = self.resolve(cx, width as f32, height as f32)?;
        let image = ImageTarget::new(cx, width, height)?.render_blocking(cx, &resolved.scene)?;
        Ok((image, resolved.layout))
    }

    /// Resolve, render and write a PNG.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_png(
        &mut self,
        cx: &Context,
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        save_png(&self.render(cx, width, height)?, path)
    }

    /// The generated glue WGSL of each layer (diagnostics).
    pub fn glue_sources(&self) -> Vec<(String, String)> {
        self.layers
            .iter()
            .map(|l| {
                let g = l.glue_source();
                (g.signature, g.source)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{CpuMirror, EncodeFn};
    use crate::marks::Circle;
    use crate::scale::{Linear, Log, ScaleRef};
    use crate::scene::ItemKind;
    use crate::selection::Retain;
    use std::sync::Arc;

    #[test]
    fn fit_text_keeps_what_fits_and_truncates_the_rest() {
        let cx = Context::new_blocking().unwrap();
        let text = cx.text();
        assert_eq!(fit_text(&text, "short", 12.0, 500.0), "short");
        let long = "x".repeat(100);
        for max in [30.0, 80.0, 200.0] {
            let fitted = fit_text(&text, &long, 12.0, max);
            assert!(fitted.ends_with('…') && fitted.len() < long.len());
            assert!(text.measure(&fitted, 12.0).width <= max);
        }
        assert_eq!(fit_text(&text, &long, 12.0, 1.0), "…");
    }

    fn batch(r: &Resolved) -> Arc<crate::render::LayerGpu> {
        r.scene
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::Marks(b) => Some(Arc::clone(&b.gpu)),
                _ => None,
            })
            .unwrap()
    }

    /// Changing a domain re-resolves with new uniforms only: the glue is
    /// not relinked, the pipeline is a cache hit, the layer's GPU state
    /// (column chunk, uniform buffers, bind groups) is reused and the
    /// upload counter shows 0 column bytes and only uniform writes.
    #[test]
    fn rescale_reuses_program_pipeline_and_columns() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (1.0, 1.0),
            (5.0, 100.0),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
        let first = plot.resolve(&cx, 300.0, 200.0).unwrap();
        let mut target = ImageTarget::new(&cx, 300, 200).unwrap();
        let image = target.render_blocking(&cx, &first.scene).unwrap();
        let before = cx.pipelines().stats;
        let uploads = cx.upload_stats();

        x.write().set_domain(2.0, 3.0).unwrap();
        let second = plot.resolve(&cx, 300.0, 200.0).unwrap();
        let zoomed = target.render_blocking(&cx, &second.scene).unwrap();
        let after = cx.pipelines().stats;
        let written = cx.upload_stats() - uploads;

        assert_eq!(before.programs_linked, 1);
        assert_eq!(after.programs_linked, 1, "glue relinked");
        assert_eq!(after.pipelines_created, before.pipelines_created);
        assert!(after.pipeline_hits > before.pipeline_hits);
        assert!(
            Arc::ptr_eq(&batch(&first), &batch(&second)),
            "layer GPU state rebuilt"
        );
        assert_ne!(first.layout.x_ticks, second.layout.x_ticks);
        assert_ne!(image, zoomed, "the new domain did not reach the GPU");

        assert_eq!(written.columns, Default::default(), "{written:?}");
        assert_eq!(written.textures, Default::default(), "{written:?}");
        // Encodings + chunk uniforms, then the view uniform.
        assert_eq!(written.uniforms.writes, 3, "{written:?}");
    }

    /// Zooming the plot about a point keeps that point's data values fixed
    /// on both axes and narrows both domains.
    #[test]
    fn zoom_narrows_both_domains_about_the_anchor() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (0.0, 1.0),
            (100.0, 1e4),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
        let r = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let at = Point::new(
            r.layout.plot.x + 0.3 * r.layout.plot.width,
            r.layout.plot.y + 0.6 * r.layout.plot.height,
        );
        let (xv, yv) = (
            x.read().invert(f64::from(at.x)),
            y.read().invert(f64::from(at.y)),
        );
        let (xd, yd) = (
            x.read().current_domain().unwrap(),
            y.read().current_domain().unwrap(),
        );
        plot.zoom(at, 0.5).unwrap();
        let (zx, zy) = (
            x.read().current_domain().unwrap(),
            y.read().current_domain().unwrap(),
        );
        assert!(((zx.1 - zx.0) / (xd.1 - xd.0) - 0.5).abs() < 1e-9, "{zx:?}");
        assert!(((zy.1 / zy.0).log10() / (yd.1 / yd.0).log10() - 0.5).abs() < 1e-9);
        assert!((x.read().invert(f64::from(at.x)) - xv).abs() < 1e-9);
        assert!((y.read().invert(f64::from(at.y)) / yv - 1.0).abs() < 1e-9);
        // Domains are now explicit, so re-resolving keeps them.
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(x.read().current_domain().unwrap(), zx);
        assert!(plot.zoom(at, 0.0).is_err());
    }

    type Pt = (f64, f64);

    /// A scatter of `rows` (linear x, log y) in chunks of at most 64 rows.
    fn chunked(rows: Vec<Pt>) -> (Plot, ScaleRef<Linear>) {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<Pt, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Pt| r.0))
            .attr(Circle::Y, y.encode(|r: &Pt| r.1))
            .max_chunk_rows(64);
        (plot, x)
    }

    fn points(range: std::ops::Range<u32>) -> Vec<Pt> {
        range
            .map(|i| {
                let i = f64::from(i);
                (1.7e9 + i * 3.5, 1.0 + (i * 0.37) % 9.0 * 100.0)
            })
            .collect()
    }

    /// RFC-001 S4a (AC5): appending rows in batches writes exactly their
    /// column bytes (no chunk is uploaded again), grows the auto x domain
    /// through the merged stats, and renders exactly what a plot built
    /// from every row at once renders.
    #[test]
    fn appended_rows_upload_only_their_bytes() {
        let cx = Context::new_blocking().unwrap();
        let (mut plot, x) = chunked(points(0..100));
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let before = target.render_blocking(&cx, &first.scene).unwrap();
        let domain = x.read().current_domain().unwrap();
        let start = cx.upload_stats();

        let mut appended = 0;
        let mut end = 100;
        for n in [1, 27, 36, 100, 300, 5] {
            let layer: &mut Selection<Pt, Circle> =
                plot.layers[0].as_any_mut().downcast_mut().unwrap();
            layer.append(points(end..end + n)).unwrap();
            end += n;
            appended += u64::from(n);
            let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
            assert_eq!(batch(&resolved).instances(), u64::from(end));
            target.render_blocking(&cx, &resolved.scene).unwrap();
        }
        let written = cx.upload_stats() - start;
        // Two f32 columns: exactly the appended rows' bytes.
        assert_eq!(written.columns.bytes, appended * 8, "{written:?}");
        let grown = x.read().current_domain().unwrap();
        assert_eq!(grown.0, domain.0);
        assert!(grown.1 > domain.1 + 1000.0, "{domain:?} → {grown:?}");

        let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(&resolved).chunks.len(), (end as usize).div_ceil(64));
        let after = target.render_blocking(&cx, &resolved.scene).unwrap();
        assert_ne!(before, after);
        let (mut fresh, _) = chunked(points(0..end));
        let expected = fresh.render(&cx, 400, 300).unwrap();
        assert!(after == expected, "appending differs from building at once");
    }

    /// RFC-001 S7d (GUP-433): `DictionarySlot` hands every user the same
    /// dictionary, and reports a key type mismatch.
    #[test]
    fn dictionary_slot_shares_one_dictionary_per_key_type() {
        use crate::encoding::DictionarySlot;
        let slot = DictionarySlot::default();
        let first = slot.get_or_insert::<Arc<str>>().unwrap();
        first.lock().unwrap().encode(Arc::from("a")).unwrap();
        let second = slot.get_or_insert::<Arc<str>>().unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(second.lock().unwrap().labels().len(), 1);
        let (existing, requested) = slot.get_or_insert::<u32>().unwrap_err();
        assert!(existing.contains("str") && requested == "u32");
    }

    /// RFC-001 S7d (GUP-433): a new key appended to one layer of a shared
    /// band axis uploads only that layer's new rows.
    #[test]
    fn appending_a_new_key_uploads_only_the_appended_layer() {
        use crate::scale::Band;
        type R = (&'static str, f64);
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
        for rows in [vec![("a", 1.0), ("b", 2.0)], vec![("b", 3.0), ("a", 4.0)]] {
            plot.add(Selection::<R, Circle>::new(rows))
                .attr(Circle::X, x.encode_key(|r: &R| r.0))
                .attr(Circle::Y, y.encode(|r: &R| r.1));
        }
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        let start = cx.upload_stats();
        let layer: &mut Selection<R, Circle> = plot.layers[1].as_any_mut().downcast_mut().unwrap();
        layer
            .append(vec![("c", 5.0), ("a", 6.0), ("c", 7.0)])
            .unwrap();
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        let written = cx.upload_stats() - start;
        // A u32 code column and an f32 column: only the three new rows.
        assert_eq!(written.columns.bytes, 3 * 8, "{written:?}");
        assert_eq!(x.read().keys().len(), 3);
    }

    #[test]
    fn resolve_errors_name_what_is_missing() {
        let cx = Context::new_blocking().unwrap();
        let err = Plot::new().resolve(&cx, 300.0, 200.0).unwrap_err();
        assert!(err.to_string().contains("plot.x(..)"), "{err}");
        let mut plot = Plot::new();
        plot.x(Linear::new());
        plot.y(Log::new());
        let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err();
        assert!(err.to_string().contains("plot.add(..)"), "{err}");
    }

    /// A row that counts its live copies through a shared token.
    struct Tracked {
        x: f64,
        y: f64,
        _token: Arc<()>,
    }

    fn tracked(range: std::ops::Range<u32>, token: &Arc<()>) -> Vec<Tracked> {
        points(range)
            .into_iter()
            .map(|(x, y)| Tracked {
                x,
                y,
                _token: Arc::clone(token),
            })
            .collect()
    }

    /// A scatter of tracked rows in chunks of at most 64 rows.
    fn tracked_plot(rows: Vec<Tracked>, retain: Retain) -> (Plot, ScaleRef<Linear>) {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<Tracked, Circle>::new(rows))
            .retain(retain)
            .attr(Circle::X, x.encode(|r: &Tracked| r.x))
            .attr(Circle::Y, y.encode(|r: &Tracked| r.y))
            .max_chunk_rows(64);
        (plot, x)
    }

    fn tracked_layer(plot: &mut Plot) -> &mut Selection<Tracked, Circle> {
        plot.layers[0].as_any_mut().downcast_mut().unwrap()
    }

    /// Which chunks of the layer still have their CPU copy.
    fn kept(plot: &mut Plot) -> Vec<bool> {
        let store = tracked_layer(plot).store().unwrap();
        store.chunks().iter().map(|c| c.bytes().is_some()).collect()
    }

    #[test]
    fn retain_policies_keep_what_they_say() {
        use Retain::*;
        let max = Retain::AUTO_MAX_ROWS;
        assert_eq!(Retain::default(), Auto);
        for (policy, rows, keeps) in [
            (Auto, max, (true, true)),
            (Auto, max + 1, (false, false)),
            (Rows, max + 1, (true, false)),
            (Columns, 1, (false, true)),
            (GpuOnly, 1, (false, false)),
        ] {
            assert_eq!(
                (policy.keeps_rows(rows), policy.keeps_columns(rows)),
                keeps,
                "{policy:?} at {rows} rows"
            );
        }
    }

    /// S4b (AC5): under `GpuOnly`, a successful resolve drops every row
    /// (each `T` is dropped, not just unused) and the CPU copy of every
    /// full chunk, keeping stats and GPU buffers: zooming still works and
    /// renders what a plot that kept everything renders. Appended rows are
    /// dropped after their upload too. Resolving on another context, or
    /// re-encoding a channel, is an error naming the policy.
    #[test]
    fn gpu_only_drops_rows_and_full_chunks_after_upload() {
        let cx = Context::new_blocking().unwrap();
        let token = Arc::new(());
        let (mut plot, x) = tracked_plot(tracked(0..150, &token), Retain::GpuOnly);
        assert_eq!(Arc::strong_count(&token), 151);
        let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(Arc::strong_count(&token), 1, "every row was dropped");
        assert!(tracked_layer(&mut plot).rows_released());
        assert_eq!(tracked_layer(&mut plot).len(), 150);
        assert_eq!(kept(&mut plot), [false, false, true]);
        let stats = tracked_layer(&mut plot).store().unwrap().stats(0).unwrap();
        assert_eq!(stats.extent(), (points(0..1)[0].0, points(149..150)[0].0));

        // The same picture as a plot that keeps everything.
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let released = target.render_blocking(&cx, &resolved.scene).unwrap();
        let (mut keep, _) = tracked_plot(tracked(0..150, &token), Retain::Auto);
        let kept_image = keep.render(&cx, 400, 300).unwrap();
        assert!(released == kept_image, "releasing changed the render");
        drop(keep);

        // Zooming writes uniforms only.
        let start = cx.upload_stats();
        plot.zoom(Point::new(200.0, 150.0), 0.5).unwrap();
        let zoomed = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(&zoomed).instances(), 150);
        assert_eq!((cx.upload_stats() - start).columns, Default::default());

        // Appending: the new rows are evaluated, uploaded, then dropped.
        tracked_layer(&mut plot)
            .append(tracked(150..170, &token))
            .unwrap();
        assert_eq!(Arc::strong_count(&token), 21);
        let grown = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(&grown).instances(), 170);
        assert_eq!(Arc::strong_count(&token), 1);
        assert_eq!(kept(&mut plot), [false, false, true]);

        let other = Context::from_wgpu(cx.device().clone(), cx.queue().clone());
        let err = plot.resolve(&other, 400.0, 300.0).unwrap_err().to_string();
        assert!(
            err.contains("on another context and their CPU copy was dropped")
                && err.contains("Retain::GpuOnly"),
            "{err}"
        );

        tracked_layer(&mut plot).attr(Circle::X, x.encode(|r: &Tracked| r.x * 2.0));
        let err = plot.resolve(&cx, 400.0, 300.0).unwrap_err().to_string();
        assert!(
            err.contains("Circle layer: its columns must be evaluated again")
                && err.contains("170 rows were dropped after upload (Retain::GpuOnly)"),
            "{err}"
        );
    }

    /// S4b: `Rows` keeps the rows and re-evaluates them for another
    /// context; `Columns` drops the rows but uploads its CPU columns to
    /// another context; `Auto` keeps both at this size.
    #[test]
    fn rows_and_columns_policies_survive_another_context() {
        let cx = Context::new_blocking().unwrap();
        let other = Context::from_wgpu(cx.device().clone(), cx.queue().clone());
        let token = Arc::new(());
        for (policy, rows_kept, chunks_kept) in [
            (Retain::Rows, true, [false, false, true]),
            (Retain::Columns, false, [true, true, true]),
            (Retain::Auto, true, [true, true, true]),
        ] {
            let (mut plot, _) = tracked_plot(tracked(0..150, &token), policy);
            let here = plot.render(&cx, 400, 300).unwrap();
            assert_eq!(Arc::strong_count(&token) > 1, rows_kept, "{policy:?}");
            assert_eq!(kept(&mut plot), chunks_kept, "{policy:?}");
            let there = plot.render(&other, 400, 300).unwrap();
            assert!(
                here == there,
                "{policy:?}: another context renders differently"
            );
        }
    }

    /// S4b (AC1): the vertex stage hides a row by its validity bit, never
    /// by testing the value. Row 0 keeps a finite position, but with its x
    /// bit cleared (the store already has a null, row 2, so it uploads
    /// bits) its disc vanishes while row 1 still draws.
    #[test]
    fn a_cleared_validity_bit_hides_a_finite_row() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<Pt, Circle>::new(vec![
            (2.0, 5.0),
            (8.0, 5.0),
            (f64::NAN, 5.0),
        ]))
        .attr(Circle::X, x.encode(|r: &Pt| r.0))
        .attr(Circle::Y, y.encode(|r: &Pt| r.1))
        .attr(Circle::RADIUS, Px(10.0));
        let white = image::Rgba([255, 255, 255, 255]);
        let (before, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        let centre = |v: f64| (x.read().eval(v) as u32, y.read().eval(5.0) as u32);
        let ((x0, y0), (x1, y1)) = (centre(2.0), centre(8.0));
        assert_ne!(*before.get_pixel(x0, y0), white);
        assert_ne!(*before.get_pixel(x1, y1), white);

        let layer: &mut Selection<Pt, Circle> = plot.layers[0].as_any_mut().downcast_mut().unwrap();
        layer.store_mut().unwrap().clear_valid(0, 0);
        let (after, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        assert_eq!(*after.get_pixel(x0, y0), white, "row 0 still drawn");
        assert_eq!(*after.get_pixel(x1, y1), *before.get_pixel(x1, y1));
    }

    /// GUP-419 AC1: the dictionary hook end to end. Appending rows with a
    /// new key grows the categorical domain and its legend; the GPU learns
    /// of it through one uniform (the key count): the column writes are
    /// exactly the appended rows' bytes, no texture (palette) is written,
    /// and the layer's GPU state is kept. The new key draws in its palette
    /// colour, not the null colour a code outside the domain gets, so the
    /// uniform did reach the GPU.
    #[test]
    fn a_new_key_grows_the_domain_with_a_uniform_write_only() {
        use crate::ShaderFn;
        use crate::scale::{Categorical, OKABE_ITO};
        type Row = (f64, f64, &'static str);
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<Row> = (0..30)
            .map(|i| {
                (
                    f64::from(i),
                    f64::from(i % 5),
                    ["a", "b", "c"][i as usize % 3],
                )
            })
            .collect();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(-1.0, 40.0)),
            plot.y(Linear::new().domain(-1.0, 5.0)),
        );
        let colour = ScaleRef::new(Categorical::okabe_ito());
        // No legend: a new key's label would write glyphs to the atlas,
        // and this test counts texture writes.
        plot.legends(false);
        plot.add(Selection::<Row, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(4.0))
            .attr(Circle::FILL, colour.encode_key(|r: &Row| r.2));
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap();
        target.render_blocking(&cx, &first.scene).unwrap();
        let labels = |c: &ScaleRef<Categorical>| -> Vec<String> {
            let legend = c.read().legend().unwrap();
            legend
                .swatches()
                .iter()
                .map(|s| s.label.to_string())
                .collect()
        };
        assert_eq!(labels(&colour), ["a", "b", "c"]);

        let start = cx.upload_stats();
        let linked = cx.pipelines().stats.programs_linked;
        let layer: &mut Selection<Row, Circle> =
            plot.layers[0].as_any_mut().downcast_mut().unwrap();
        layer.append([(35.0, 2.0, "d"), (36.0, 3.0, "a")]).unwrap();
        let second = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let image = target.render_blocking(&cx, &second.scene).unwrap();
        let written = cx.upload_stats() - start;

        assert_eq!(labels(&colour), ["a", "b", "c", "d"]);
        assert_eq!(
            colour.read().legend().unwrap().swatches()[3].color,
            OKABE_ITO[3]
        );
        // x, y (4 bytes each) and the code (4 bytes) of 2 rows.
        assert_eq!(written.columns.bytes, 2 * 12, "{written:?}");
        assert_eq!(written.textures, Default::default(), "{written:?}");
        // Same program and pipeline: the domain is a uniform, not WGSL.
        assert_eq!(cx.pipelines().stats.programs_linked, linked);
        assert_eq!(
            batch(&first).program.glue.signature,
            batch(&second).program.glue.signature
        );
        let at = |v: (f64, f64)| {
            image
                .get_pixel(x.read().eval(v.0) as u32, y.read().eval(v.1) as u32)
                .0
        };
        assert_eq!(at((35.0, 2.0)), OKABE_ITO[3].to_rgba8(), "the new key");
        assert_eq!(at((36.0, 3.0)), OKABE_ITO[0].to_rgba8(), "an old key");
    }

    /// GUP-429 AC3: a bar's width comes from the band scale each resolve.
    /// Appending a row with a new key narrows every bar through a uniform
    /// write: the second resolve's column writes are the appended rows'
    /// bytes only, and `width` has no column.
    #[test]
    fn a_bar_width_follows_an_appended_key() {
        type Row = (&'static str, f64);
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<Row> = vec![("A", 5.0), ("B", 8.0), ("C", 6.0)];
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(crate::Band::new()),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        let floor = y.clone();
        plot.add(Selection::<Row, crate::Rect>::new(rows))
            .attr(crate::Rect::X, x.encode_key(|r: &Row| r.0))
            .attr(crate::Rect::WIDTH, x.band_width_px())
            .attr(crate::Rect::Y, y.encode(|r: &Row| r.1))
            .attr(
                crate::Rect::Y_END,
                crate::Derived::new(move || Px(floor.read().eval(0.0) as f32)),
            );
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        // Ink widths along a row just above the baseline, one run per bar.
        let widths = |image: &image::RgbaImage| -> Vec<u32> {
            let row = (y.read().eval(0.0) - 3.0) as u32;
            let mut runs = Vec::new();
            let mut start = None;
            for px in 0..image.width() {
                let ink = image.get_pixel(px, row).0[0] < 128;
                match (ink, start) {
                    (true, None) => start = Some(px),
                    (false, Some(s)) => {
                        // Runs of a few pixels are the axis, not a bar.
                        if px - s > 5 {
                            runs.push(px - s);
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
            runs
        };
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let before = target.render_blocking(&cx, &first.scene).unwrap();
        let w3 = x.read().band_width();
        let runs = widths(&before);
        assert_eq!(runs.len(), 3, "{runs:?}");
        for r in &runs {
            assert!((f64::from(*r) - w3.round()).abs() <= 1.0, "{r} vs {w3}");
        }

        let start = cx.upload_stats();
        let layer: &mut Selection<Row, crate::Rect> =
            plot.layers[0].as_any_mut().downcast_mut().unwrap();
        layer.append([("D", 7.0), ("A", 9.0)]).unwrap();
        let second = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let after = target.render_blocking(&cx, &second.scene).unwrap();
        let written = cx.upload_stats() - start;
        // y (4 bytes) and the key code (4 bytes) of 2 rows.
        assert_eq!(written.columns.bytes, 2 * 8, "{written:?}");
        let w4 = x.read().band_width();
        assert!(w4 < w3, "{w4} vs {w3}");
        let runs = widths(&after);
        assert_eq!(runs.len(), 4, "{runs:?}");
        for r in &runs {
            assert!((f64::from(*r) - w4.round()).abs() <= 1.0, "{r} vs {w4}");
        }
    }

    /// GUP-419 AC4: a null key on a band position is hidden by the glue's
    /// `NULL_CODE` comparison, not by landing off-screen. The x channel is
    /// a `Band` with an empty range (step 0), so the GPU maps every code,
    /// the null code too, to x = 150, as the dispatch shows. Rows with a
    /// key draw there; the row without one does not, on x and on y.
    #[test]
    fn a_null_band_key_is_hidden_not_placed() {
        use crate::scale::Band;
        let cx = Context::new_blocking().unwrap();
        let on_screen = Band::new().range(Px(150.0), Px(150.0));
        let gpu = crate::scale::conformance::dispatch(
            &cx,
            &on_screen,
            &[0.0, f64::from(crate::column::NULL_CODE)],
            None,
        );
        assert_eq!(gpu, [150.0, 150.0], "the null code maps on-screen");

        type Row = (Option<&'static str>, f64);
        let rows: Vec<Row> = vec![(Some("a"), 2.0), (None, 5.0), (Some("b"), 8.0)];
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<Row, Circle>::new(rows))
            .attr(Circle::X, on_screen.encode_nullable_key(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(8.0));
        let resolved = plot.resolve(&cx, 300.0, 200.0).unwrap();
        let glue = &plot.glue_sources()[0].1;
        assert!(
            glue.contains("let drawn = u32(col.x != 4294967295u);"),
            "{glue}"
        );
        let image = ImageTarget::new(&cx, 300, 200)
            .unwrap()
            .render_blocking(&cx, &resolved.scene)
            .unwrap();
        let white = image::Rgba([255, 255, 255, 255]);
        let at = |v: f64| *image.get_pixel(150, y.read().eval(v) as u32);
        assert_ne!(at(2.0), white, "key a");
        assert_ne!(at(8.0), white, "key b");
        assert_eq!(at(5.0), white, "the null key was drawn");
        let _ = x;

        // The same on y, with the x column holding a null too: both
        // compares AND together.
        let rows: Vec<(f64, Option<&'static str>)> = vec![(2.0, Some("p")), (5.0, None)];
        let mut plot = Plot::new();
        let (x, _y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<(f64, Option<&'static str>), Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &(f64, Option<&str>)| r.0))
            .attr(
                Circle::Y,
                Band::new()
                    .range(Px(100.0), Px(100.0))
                    .encode_nullable_key(|r: &(f64, Option<&'static str>)| r.1),
            )
            .attr(Circle::RADIUS, Px(8.0));
        let image = plot.render(&cx, 300, 200).unwrap();
        let at = |v: f64| *image.get_pixel(x.read().eval(v) as u32, 100);
        assert_ne!(at(2.0), white);
        assert_eq!(at(5.0), white, "the null y key was drawn");
    }

    /// S4b: a non-finite value driving a numeric colour scale draws the
    /// point in the null colour (a `select` on its validity bit), not a
    /// palette colour, and the point is still drawn.
    #[test]
    fn a_null_sequential_input_draws_in_the_null_colour() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        let rows: Vec<(f64, f64)> = vec![(2.0, 1.0), (5.0, f64::NAN), (8.0, 3.0)];
        plot.add(Selection::<Pt, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Pt| r.0))
            .attr(Circle::Y, y.encode(|r: &Pt| r.0))
            .attr(
                Circle::FILL,
                crate::scale::Sequential::viridis().encode(|r: &Pt| r.1),
            )
            .attr(Circle::RADIUS, Px(8.0));
        let (image, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        let at = |v: f64| {
            let (px, py) = (x.read().eval(v), y.read().eval(v));
            image.get_pixel(px as u32, py as u32).0
        };
        assert_eq!(at(5.0), crate::scale::NULL_COLOR.to_rgba8());
        let viridis = crate::scale::Sequential::viridis().domain(1.0, 3.0);
        assert_eq!(at(2.0), viridis.eval(1.0).to_rgba8());
        assert_eq!(at(8.0), viridis.eval(3.0).to_rgba8());
    }
}
