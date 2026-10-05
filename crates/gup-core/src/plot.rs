// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Plot`: shared x/y scales, one data layer, axes and a title, resolved
//! into a [`Layout`] and a [`Scene`] (RFC-001 §8–9, S0a subset).
//!
//! S0a scope: exactly one layer, left and bottom axes with tick labels, an
//! optional title. Multiple layers with unioned domains, legends, grids,
//! axis titles, themes and the object-safe `Chart` trait are RFC-001 S7.

use crate::channel::{Mark, Px};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};
use crate::scale::{DynPositionScale, PositionScale, ScaleRef};
use crate::scene::{
    Anchor, HAlign, Item, ItemKind, Rule, Scene, TextRole, TextRun, TextStyle, VAlign, Z_AXES,
    Z_TITLE,
};
use crate::selection::{Layer, Selection};
use crate::target::{ImageTarget, save_png};
use std::path::Path;

/// Fixed S0a styling (a `Theme` is RFC-001 S7 / GUP-392).
mod style {
    use crate::channel::Color;

    pub const BACKGROUND: Color = Color::WHITE;
    pub const AXIS: Color = Color::hex(0x333333);
    pub const LABEL: Color = Color::hex(0x333333);
    pub const TITLE: Color = Color::hex(0x111111);
    pub const TITLE_SIZE: f32 = 18.0;
    pub const TICK_SIZE: f32 = 12.0;
    pub const TICK_LEN: f32 = 5.0;
    pub const LABEL_GAP: f32 = 4.0;
    pub const PAD: f32 = 12.0;
    /// Target spacing between ticks.
    pub const X_TICK_SPACING: f32 = 80.0;
    pub const Y_TICK_SPACING: f32 = 45.0;
}

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
}

impl std::fmt::Debug for Plot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plot")
            .field("title", &self.title)
            .field("layers", &self.layers.len())
            .finish_non_exhaustive()
    }
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

    /// Resolve domains, ticks, text, the plot rect and GPU state for a
    /// `width × height` logical-pixel chart.
    pub fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<Resolved> {
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
        let [layer] = self.layers.as_mut_slice() else {
            return Err(Error::config(
                "plot layers",
                format!(
                    "{} layers; S0a resolves exactly one (shared domains across layers arrive \
                     in RFC-001 S7)",
                    self.layers.len()
                ),
            ));
        };

        // 1–2. Domains from column stats, then nice.
        layer.fit_domains()?;
        for s in [x, y] {
            if s.is_auto() {
                s.nice();
            }
        }

        // 3. Ticks and label measurements give the margins.
        let tick_style = TextStyle {
            size: Px(style::TICK_SIZE),
            color: style::LABEL,
        };
        let x_ticks = x.ticks(((width / style::X_TICK_SPACING) as usize).max(2));
        let y_ticks = y.ticks(((height / style::Y_TICK_SPACING) as usize).max(2));
        let (title_h, y_label_w, x_label_h, last_x_label_w) = {
            let text = cx.text();
            let title_h = self.title.as_ref().map_or(0.0, |t| {
                let m = text.measure(t, style::TITLE_SIZE);
                m.cap_height + m.descent
            });
            let y_label_w = y_ticks
                .labels
                .iter()
                .map(|l| text.measure(l, style::TICK_SIZE).width)
                .fold(0.0, f32::max);
            let m = text.measure("0", style::TICK_SIZE);
            let last = x_ticks
                .labels
                .last()
                .map_or(0.0, |l| text.measure(l, style::TICK_SIZE).width);
            (title_h, y_label_w, m.cap_height + m.descent, last)
        };

        // 4. The plot rect, on whole pixels.
        let top = style::PAD
            + if self.title.is_some() {
                title_h + style::PAD
            } else {
                0.0
            };
        let left = style::PAD + y_label_w + style::LABEL_GAP + style::TICK_LEN;
        let bottom = height - (style::PAD + x_label_h + style::LABEL_GAP + style::TICK_LEN);
        let right = width - style::PAD.max(last_x_label_w / 2.0 + 2.0);
        let plot = Rect::from_edges(left.round(), top.round(), right.round(), bottom.round());
        if plot.width < 10.0 || plot.height < 10.0 {
            return Err(Error::config(
                "plot size",
                format!("{width}×{height} leaves no room for the plot area"),
            ));
        }

        // 5. Ranges, inset so marks on the domain edge stay inside.
        let mark_overhang = layer.overhang();
        let inset = mark_overhang + 1.0;
        x.set_range(Px(plot.left() + inset), Px(plot.right() - inset));
        y.set_range(Px(plot.bottom() - inset), Px(plot.top() + inset));

        // 6. Uniforms and columns.
        let batch = layer.prepare(cx)?;

        // 7. The scene.
        let mut scene = Scene::new(width, height, style::BACKGROUND);
        let mut rules = Vec::new();
        let mut labels = Vec::new();
        let axis = |p0: Point, p1: Point| Rule {
            p0,
            p1,
            width: Px(1.0),
            color: style::AXIS,
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
                Point::new(cx_, ax_y + style::TICK_LEN),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(px, ax_y + style::TICK_LEN + style::LABEL_GAP),
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
                Point::new(ay_x - style::TICK_LEN, cy),
                Point::new(ay_x, cy),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(ay_x - style::TICK_LEN - style::LABEL_GAP, py),
                anchor: Anchor::new(HAlign::End, VAlign::Middle),
                style: tick_style,
                role: TextRole::TickLabel,
            });
            y_placed.push((*v, py));
        }
        let title = self.title.as_ref().map(|t| TextRun {
            text: t.as_str().into(),
            at: Point::new(plot.x + plot.width / 2.0, style::PAD),
            anchor: Anchor::new(HAlign::Middle, VAlign::Top),
            style: TextStyle {
                size: Px(style::TITLE_SIZE),
                color: style::TITLE,
            },
            role: TextRole::Title,
        });

        let texts = {
            let text = cx.text();
            labels
                .iter()
                .chain(&title)
                .map(|run| PlacedText {
                    run: run.clone(),
                    bounds: text.ink_bounds(run),
                })
                .collect()
        };
        let guides = rules.iter().map(Rule::bounds).collect();

        let clip = scene.add_clip(plot);
        scene.push(Item {
            z: 0,
            clip: Some(clip),
            kind: ItemKind::Marks(batch),
        });
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
        if let Some(title) = title {
            scene.push(Item {
                z: Z_TITLE,
                clip: None,
                kind: ItemKind::Text(vec![title]),
            });
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
