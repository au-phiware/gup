// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The scatter builder: [`scatter`] and [`ScatterChart`].

use std::any::Any;
use std::sync::Arc;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::Appender;
use crate::column::DictionaryKey;
use crate::geom::Point;
use crate::prelude::*;
use crate::scale::ColorValueScale;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Num<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Op<T> = Box<dyn FnOnce(&mut Selection<T, Circle>) + Send + Sync>;

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
    Value(Num<T>),
}

enum SizeSpec<T> {
    Default,
    Radius(f32),
    Value(Num<T>),
}

/// Start a scatter chart over `rows`: discs at `(x, y)`.
///
/// See the [module docs](crate::builder) for the pattern and an example.
pub fn scatter<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> ScatterChart<T> {
    let mut plot = Plot::new();
    plot.x(Linear::new());
    plot.y(Linear::new());
    ScatterChart {
        plot,
        rows: Some(rows.into_iter().collect()),
        ops: Vec::new(),
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        color_value_scale: Sequential::viridis().into(),
        size: SizeSpec::Default,
        size_scale: Pow::sqrt().range(Px(2.0), Px(12.0)),
        legend_title: None,
    }
}

/// A scatter chart under construction, and a [`Chart`].
///
/// # Panics
///
/// The layer setters (`x`, `y`, `color*`, `fill`, `size*`, `radius`,
/// `opacity`, `tooltip`, `legend_title`) panic once the chart has been
/// drawn, with "set layer encodings before the chart is first drawn; change
/// a drawn chart through layer_mut()". The plot-level setters work at any
/// time.
#[must_use]
pub struct ScatterChart<T> {
    plot: Plot,
    rows: Option<Vec<T>>,
    ops: Vec<Op<T>>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    color_value_scale: ColorValueScale,
    size: SizeSpec<T>,
    size_scale: Pow,
    legend_title: Option<String>,
}

impl<T> std::fmt::Debug for ScatterChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScatterChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static> ScatterChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn op(mut self, op: impl FnOnce(&mut Selection<T, Circle>) + Send + Sync + 'static) -> Self {
        self.check_open();
        self.ops.push(Box::new(op));
        self
    }

    /// The x position of a row, through the x scale.
    pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.op(move |s| {
            s.x(f);
        })
    }

    /// The y position of a row, through the y scale.
    pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.op(move |s| {
            s.y(f);
        })
    }

    /// The x scale (default [`Linear`]).
    pub fn x_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.x(scale);
        self
    }

    /// The y scale (default [`Linear`]).
    pub fn y_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.y(scale);
        self
    }

    /// Colour by a category key per row, through the colour scale. The last
    /// of `color`, `color_value` and `fill` wins.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn color<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.color = ColorSpec::Key(erase_key(key));
        self
    }

    /// The scale for [`color`](Self::color) (default
    /// [`Categorical::okabe_ito`]).
    pub fn color_scale(mut self, scale: Categorical) -> Self {
        self.check_open();
        self.color_scale = scale;
        self
    }

    /// Colour by a number per row, through the value scale. The last of
    /// `color`, `color_value` and `fill` wins.
    pub fn color_value<D: ColumnValue>(
        mut self,
        f: impl Fn(&T) -> D + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.color = ColorSpec::Value(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// The scale for [`color_value`](Self::color_value) (default
    /// [`Sequential::viridis`]): a [`Sequential`] or a [`Diverging`].
    pub fn color_value_scale(mut self, scale: impl Into<ColorValueScale>) -> Self {
        self.check_open();
        self.color_value_scale = scale.into();
        self
    }

    /// One fill colour for every disc. The last of `color`, `color_value`
    /// and `fill` wins.
    pub fn fill(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Fill(color);
        self
    }

    /// Title the colour legend.
    pub fn legend_title(mut self, title: impl Into<String>) -> Self {
        self.check_open();
        self.legend_title = Some(title.into());
        self
    }

    /// Size discs by a number per row: their area follows the value. The
    /// last of `size` and `radius` wins.
    pub fn size<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.size = SizeSpec::Value(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// The scale for [`size`](Self::size) (default `Pow::sqrt()` onto
    /// 2 to 12 px).
    pub fn size_scale(mut self, scale: Pow) -> Self {
        self.check_open();
        self.size_scale = scale;
        self
    }

    /// One radius, in logical pixels, for every disc (default 3.5). The
    /// last of `size` and `radius` wins.
    pub fn radius(mut self, px: f32) -> Self {
        self.check_open();
        self.size = SizeSpec::Radius(px);
        self
    }

    /// The fill opacity.
    ///
    /// # Panics
    ///
    /// If `opacity` is outside `0..=1`.
    pub fn opacity(self, opacity: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&opacity),
            "opacity must be within 0..=1, not {opacity}"
        );
        self.op(move |s| {
            s.fill_opacity(Unit(opacity));
        })
    }

    /// The tooltip text of a row.
    pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        self.op(move |s| {
            s.tooltip(text);
        })
    }

    /// How many rows to keep as rows are appended (default [`Window::All`]):
    /// see [`Selection::window`].
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// # fn main() -> gup_core::Result<()> {
    /// let cx = Context::new_blocking()?;
    /// let mut chart = gup_core::scatter(Vec::<(f64, f64)>::new())
    ///     .x(|p| p.0)
    ///     .y(|p| p.1)
    ///     .window(Window::LastRows(5_000));
    /// let feed = chart.appender();
    /// for k in 0..20 {
    ///     feed.extend((0..1_000).map(|i| (f64::from(k * 1_000 + i), 0.0))).unwrap();
    ///     chart.resolve(&cx, 320.0, 200.0)?;
    /// }
    /// let held = chart.layer_mut().len();
    /// assert!((5_000..5_000 + 2 * 1_250).contains(&held), "{held}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn window(self, window: Window) -> Self {
        self.op(move |s| {
            s.window(window);
        })
    }

    /// Build the layer from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let ops = std::mem::take(&mut self.ops);
        let sel = self.plot.circles(rows);
        for op in ops {
            op(sel);
        }
        match &self.color {
            ColorSpec::Default => {}
            ColorSpec::Fill(c) => {
                sel.fill(*c);
            }
            ColorSpec::Key(key) => {
                let key = Arc::clone(key);
                let scale = ScaleRef::new(self.color_scale.clone());
                sel.fill(scale.encode_owned_key(move |r: &T| key(r)));
            }
            ColorSpec::Value(f) => {
                let f = Arc::clone(f);
                match self.color_value_scale.clone() {
                    ColorValueScale::Sequential(s) => sel.fill(s.encode(move |r: &T| f(r))),
                    ColorValueScale::Diverging(s) => sel.fill(s.encode(move |r: &T| f(r))),
                };
            }
        }
        if let Some(title) = self.legend_title.take() {
            sel.legend_title(Circle::FILL, title);
        }
        match &self.size {
            SizeSpec::Default => {}
            SizeSpec::Radius(px) => {
                sel.radius(Px(*px));
            }
            SizeSpec::Value(f) => {
                let f = Arc::clone(f);
                let scale = ScaleRef::new(self.size_scale.clone());
                sel.radius(scale.encode(move |r: &T| f(r)));
            }
        }
    }

    /// The circle layer, for settings the builder has no method for (a
    /// stroke, say). Materialises the chart: layer setters panic after.
    pub fn layer_mut(&mut self) -> &mut Selection<T, Circle> {
        self.materialise();
        self.plot
            .layer_mut::<T, Circle>()
            .expect("the scatter layer was added by materialise")
    }

    /// A handle that appends rows to the chart from any thread; rows
    /// pushed between two frames are uploaded together (only they) by the
    /// next resolve. Materialises the chart: layer setters panic after.
    ///
    /// ```
    /// use gup_core::{ChartExt, Context};
    /// # fn main() -> gup_core::Result<()> {
    /// let mut chart = gup_core::scatter(vec![(0.0, 1.0)]).x(|p| p.0).y(|p| p.1);
    /// let feed = chart.appender();
    /// std::thread::spawn(move || {
    ///     feed.extend((1..10).map(|i| (f64::from(i), f64::from(i * i)))).unwrap();
    /// })
    /// .join()
    /// .unwrap();
    /// # let cx = Context::new_blocking()?;
    /// let image = chart.render(&cx, 320, 200)?;
    /// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    /// # Ok(())
    /// # }
    /// ```
    pub fn appender(&mut self) -> Appender<T> {
        self.layer_mut().appender()
    }

    /// The inner plot. Materialises the chart.
    pub fn plot_mut(&mut self) -> &mut Plot {
        self.materialise();
        &mut self.plot
    }

    /// The finished plot.
    pub fn into_plot(mut self) -> Plot {
        self.materialise();
        self.plot
    }

    /// The row drawn under `at` (logical pixels), once the chart has been
    /// resolved.
    pub fn pick(&mut self, at: impl Into<Point>) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for ScatterChart<T> {
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved> {
        self.materialise();
        self.plot.resolve(cx, width, height)
    }
    fn layout(&self) -> Option<&Layout> {
        self.plot.layout()
    }
    fn handle(&mut self, input: Input) -> Response {
        self.materialise();
        self.plot.handle(input)
    }
    fn pick_row(&mut self, at: Point) -> Option<RowId> {
        self.materialise();
        self.plot.pick_row(at)
    }
    fn needs_frame(&self) -> bool {
        Chart::needs_frame(&self.plot)
    }
    fn set_waker(&mut self, waker: std::task::Waker) {
        self.plot.set_waker(waker);
    }
    fn title_text(&self) -> Option<&str> {
        self.plot.title_text()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
