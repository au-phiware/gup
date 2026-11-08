// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A `<canvas>` as a render target and an input host (RFC-001 §9, S8f).
//!
//! [`CanvasTarget`] is a canvas's WebGPU surface as a [`RenderTarget`].
//! [`CanvasApp`] owns a chart, a [`Context`] and a target, translates DOM
//! pointer and wheel events into [`Input`] for [`Chart::handle`], and
//! redraws on demand (tooltips, wheel zoom and drag pan come from the plot;
//! `Plot::drag(Drag::Off)` turns the drag off) with one `requestAnimationFrame` per burst of input.
//! There is no winit: the host is the canvas plus DOM listeners.
//!
//! # Release profile
//!
//! `PERF_BUDGETS.md`'s WASM sizes are measured with
//! `[profile.release] opt-level = "z"`, `lto = true`, `codegen-units = 1`
//! (GUP-451). Against cargo's default release profile it cut gup-core's cost
//! over bare wgpu from 356,914 B to 292,592 B gz (64,322 B); `"s"` with the
//! same LTO settings gave 310,259 B. Set the same in your app's workspace
//! root to get the same sizes.

use crate::chart::Chart;
use crate::context::Context;
use crate::error::{Error, Result};
use crate::geom::Point;
use crate::input::{Button, Input, Modifiers};
use crate::render::{Renderer, TargetDesc};
use crate::surface::surface_config;
use crate::target::{DEFAULT_SAMPLES, Frame, RenderTarget, check_samples, msaa_view};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{
    Event, EventTarget, HtmlCanvasElement, KeyboardEvent, MouseEvent, PointerEvent, WheelEvent,
};

/// A `<canvas>`'s WebGPU surface as a [`RenderTarget`].
///
/// Like [`WindowTarget`](crate::WindowTarget) the surface is rendered
/// through a non-sRGB view, so the bytes on screen equal the bytes in a PNG.
/// The canvas's backing store is its CSS size times `devicePixelRatio`
/// (see [`resize`](Self::resize)).
pub struct CanvasTarget {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    samples: u32,
    msaa: Option<wgpu::TextureView>,
}

impl std::fmt::Debug for CanvasTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasTarget")
            .field("format", &self.config.format)
            .field("size", &(self.config.width, self.config.height))
            .field("samples", &self.samples)
            .finish_non_exhaustive()
    }
}

fn device_pixel_ratio() -> f32 {
    web_sys::window()
        .map(|w| w.device_pixel_ratio() as f32)
        .filter(|d| *d > 0.0)
        .unwrap_or(1.0)
}

impl CanvasTarget {
    /// A target drawing into `canvas`, on `cx`'s device.
    ///
    /// The logical size is the canvas's `clientWidth × clientHeight` (CSS
    /// pixels; the `width` and `height` attributes where that is zero). The
    /// backing store is set to that times `devicePixelRatio`, rounded.
    /// `cx` must have created its own device ([`Context::new`]).
    pub fn new(cx: &Context, canvas: HtmlCanvasElement) -> Result<Self> {
        let (instance, adapter) = cx.instance().zip(cx.adapter()).ok_or_else(|| {
            Error::config(
                "canvas target",
                "this Context wraps a host device (Context::from_wgpu) and has no wgpu \
                 instance to create a surface from; create it with Context::new",
            )
        })?;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| {
                Error::config(
                    "canvas target",
                    format!(
                        "cannot create a WebGPU surface from the canvas (does it already \
                         have another kind of context?): {e}"
                    ),
                )
            })?;
        let (width, height) = physical_size(&canvas);
        let (config, _) = surface_config(adapter, &surface, width, height)?;
        let mut target = Self {
            canvas,
            surface,
            config,
            samples: DEFAULT_SAMPLES,
            msaa: None,
        };
        target.configure(cx)?;
        Ok(target)
    }

    /// The canvas.
    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// The surface's physical size.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// `window.devicePixelRatio`.
    pub fn dpr(&self) -> f32 {
        device_pixel_ratio()
    }

    fn view_format(&self) -> wgpu::TextureFormat {
        self.config.format.remove_srgb_suffix()
    }

    /// Follow a size change (physical pixels). Zero sizes are ignored.
    pub fn resize(&mut self, cx: &Context, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 || (width, height) == self.size() {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        self.configure(cx)
    }

    /// Re-read the canvas's CSS size and `devicePixelRatio`.
    fn fit(&mut self, cx: &Context) -> Result<()> {
        let (w, h) = physical_size(&self.canvas);
        self.resize(cx, w, h)
    }

    fn configure(&mut self, cx: &Context) -> Result<()> {
        let (width, height) = self.size();
        self.msaa = cx.scoped(
            || {
                format!(
                    "canvas surface configuration ({:?}, {width}×{height})",
                    self.config.format
                )
            },
            || {
                self.surface.configure(cx.device(), &self.config);
                Ok(msaa_view(
                    cx,
                    self.view_format(),
                    width,
                    height,
                    self.samples,
                ))
            },
        )?;
        Ok(())
    }

    /// Draw with `samples` samples per pixel (1 or 4).
    pub fn set_samples(&mut self, cx: &Context, samples: u32) -> Result<()> {
        check_samples(cx, self.view_format(), samples)?;
        self.samples = samples;
        self.configure(cx)
    }
}

/// Logical (CSS) size of `canvas`: its client size, or its attributes where
/// the client size is zero.
fn logical_size(canvas: &HtmlCanvasElement) -> (f32, f32) {
    let (w, h) = (canvas.client_width(), canvas.client_height());
    if w > 0 && h > 0 {
        (w as f32, h as f32)
    } else {
        (canvas.width() as f32, canvas.height() as f32)
    }
}

/// The backing-store size for `canvas`'s logical size, written to its
/// `width` and `height` attributes.
fn physical_size(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let (w, h) = logical_size(canvas);
    let dpr = device_pixel_ratio();
    let size = (
        ((w * dpr).round() as u32).max(1),
        ((h * dpr).round() as u32).max(1),
    );
    canvas.set_width(size.0);
    canvas.set_height(size.1);
    size
}

impl RenderTarget for CanvasTarget {
    fn desc(&self) -> TargetDesc {
        TargetDesc {
            format: self.view_format(),
            width: self.config.width,
            height: self.config.height,
            dpr: self.dpr(),
            samples: self.samples,
        }
    }

    fn acquire(&mut self, cx: &Context) -> Result<Frame> {
        let texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.fit(cx)?;
                self.configure(cx)?;
                self.surface
                    .get_current_texture()
                    .map_err(|e| Error::config("canvas target", format!("no frame: {e}")))?
            }
            Err(e) => return Err(Error::config("canvas target", format!("no frame: {e}"))),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format()),
            ..Default::default()
        });
        Ok(Frame::new(
            texture.texture.clone(),
            view,
            self.msaa.clone(),
            Some(texture),
        ))
    }

    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()> {
        cx.submit([commands]);
        if let Some(surface) = frame.surface {
            surface.present();
        }
        Ok(())
    }
}

struct State {
    cx: Context,
    target: CanvasTarget,
    renderer: Renderer,
    chart: Box<dyn Chart>,
    /// A `requestAnimationFrame` is outstanding.
    pending: bool,
    /// The app was dropped; outstanding frames do nothing.
    closed: bool,
    last_error: Option<String>,
    /// Gup names the canvas: the page set no `aria-label` of its own.
    label_canvas: bool,
    /// The `aria-label` last written (empty before the first).
    label: String,
    /// The `aria-live` element after the canvas.
    live: web_sys::Element,
    /// The text last written to it (empty before the first).
    live_text: String,
    /// The hidden `<table>` after the live region.
    table: web_sys::Element,
    /// The alt text the table was built for (empty before the first).
    table_alt: String,
}

impl State {
    fn draw(&mut self) -> Result<()> {
        let State {
            cx,
            target,
            renderer,
            chart,
            ..
        } = self;
        let (w, h) = target.size();
        let dpr = target.dpr();
        let resolved = chart.resolve(cx, w as f32 / dpr, h as f32 / dpr)?;
        renderer.render(cx, &resolved.scene, target)?;
        self.update_label();
        self.update_live();
        self.update_table();
        Ok(())
    }

    /// Give the canvas the chart's description as its accessible name
    /// (`role="img"` and `aria-label`), unless the page named it. The DOM
    /// is written only when the text changes.
    fn update_label(&mut self) {
        if !self.label_canvas {
            return;
        }
        let Some(alt) = self.chart.describe().map(|d| d.alt_text()) else {
            return;
        };
        if alt == self.label {
            return;
        }
        let canvas: &web_sys::Element = self.target.canvas();
        if self.label.is_empty() {
            let _ = canvas.set_attribute("role", "img");
        }
        let _ = canvas.set_attribute("aria-label", &alt);
        self.label = alt;
    }
}

impl State {
    /// Speak the chart's keyboard focus: write it to the live region when
    /// it changes (empty when focus clears). The DOM is written only on
    /// change.
    fn update_live(&mut self) {
        let text = self.chart.focus_text().unwrap_or_default();
        if text != self.live_text {
            self.live.set_text_content(Some(text));
            self.live_text = text.to_owned();
        }
    }
}

/// Rows the hidden table carries; the description states the full count.
const TABLE_MAX_ROWS: usize = 1_000;

impl State {
    /// Rebuild the hidden data table when the chart's alt text changes (it
    /// counts the rows, so it changes when rows arrive), not on zoom or
    /// hover.
    fn update_table(&mut self) {
        let Some(alt) = self.chart.describe().map(|d| d.alt_text()) else {
            return;
        };
        if alt == self.table_alt {
            return;
        }
        self.table_alt = alt;
        let Some(data) = self.chart.data_table(TABLE_MAX_ROWS) else {
            return;
        };
        let Some(document) = self.table.owner_document() else {
            return;
        };
        let make = |tag: &str| document.create_element(tag).ok();
        let cell = |tag: &str, text: &str, scope: bool| {
            let c = make(tag)?;
            c.set_text_content(Some(text));
            if scope {
                let _ = c.set_attribute("scope", "col");
            }
            Some(c)
        };
        self.table.set_text_content(None);
        if let Some(caption) = cell("caption", &data.caption, false) {
            let _ = self.table.append_child(&caption);
        }
        if let (Some(head), Some(tr)) = (make("thead"), make("tr")) {
            for h in &data.headers {
                if let Some(th) = cell("th", h, true) {
                    let _ = tr.append_child(&th);
                }
            }
            let _ = head.append_child(&tr);
            let _ = self.table.append_child(&head);
        }
        if let Some(body) = make("tbody") {
            for row in &data.rows {
                let Some(tr) = make("tr") else { continue };
                for v in row {
                    if let Some(td) = cell("td", v, false) {
                        let _ = tr.append_child(&td);
                    }
                }
                let _ = body.append_child(&tr);
            }
            let _ = self.table.append_child(&body);
        }
    }
}

/// Visually hidden, still read by screen readers, and out of layout.
const LIVE_REGION_STYLE: &str = "position:absolute;width:1px;height:1px;margin:-1px;padding:0;border:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap";

thread_local! {
    /// Canvases by wake id. `Wake` needs `Send + Sync`; an id is both, and
    /// wasm32 has one thread.
    static CANVASES: RefCell<Vec<(u64, Weak<RefCell<State>>)>> = const { RefCell::new(Vec::new()) };
}

static NEXT_CANVAS_ID: AtomicU64 = AtomicU64::new(1);

/// Wakes the canvas registered under its id; a missing one is a no-op.
struct CanvasWake(u64);

impl std::task::Wake for CanvasWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        // Clone the `Weak` and release the registry before scheduling.
        let weak = CANVASES.with(|c| {
            c.borrow()
                .iter()
                .find(|e| e.0 == self.0)
                .map(|e| e.1.clone())
        });
        let Some(rc) = weak.and_then(|w| w.upgrade()) else {
            return;
        };
        // A wake from inside a frame or `with_chart` finds the state
        // borrowed; both schedule a frame themselves.
        if rc.try_borrow_mut().is_ok() {
            schedule(&rc);
        }
    }
}

struct Listener {
    target: EventTarget,
    kind: &'static str,
    closure: Closure<dyn FnMut(Event)>,
}

/// A chart on a canvas: DOM input goes to [`Chart::handle`], and the canvas
/// redraws on demand. Dropping it removes its listeners.
///
/// Event handlers run while the app holds its chart: a handler registered
/// with [`Selection::on`](crate::Selection::on) must not call
/// [`with_chart`](Self::with_chart).
pub struct CanvasApp {
    state: Rc<RefCell<State>>,
    listeners: Vec<Listener>,
    wake_id: u64,
}

fn schedule(state: &Rc<RefCell<State>>) {
    {
        let mut st = state.borrow_mut();
        if st.pending || st.closed {
            return;
        }
        st.pending = true;
    }
    let rc = Rc::clone(state);
    let callback = Closure::once_into_js(move || frame(&rc));
    let requested = web_sys::window()
        .ok_or_else(|| JsValue::from_str("there is no window"))
        .and_then(|w| w.request_animation_frame(callback.unchecked_ref()));
    if let Err(e) = requested {
        let mut st = state.borrow_mut();
        st.pending = false;
        report(&mut st, format!("requestAnimationFrame failed: {e:?}"));
    }
}

fn frame(state: &Rc<RefCell<State>>) {
    let mut st = state.borrow_mut();
    st.pending = false;
    if st.closed {
        return;
    }
    let mut again = false;
    match st.draw() {
        // A pick in flight: look again next frame.
        Ok(()) => again = st.chart.needs_frame(),
        Err(e) => report(&mut st, e.to_string()),
    }
    drop(st);
    if again {
        schedule(state);
    }
}

fn report(st: &mut State, message: String) {
    web_sys::console::error_1(&JsValue::from_str(&message));
    st.last_error = Some(message);
}

fn dispatch(state: &Rc<RefCell<State>>, input: Input) {
    let redraw = state.borrow_mut().chart.handle(input).redraw;
    if redraw {
        schedule(state);
    }
}

/// Make `canvas` focusable (unless the page set a `tabindex`) and insert
/// the live region after it.
fn live_region(canvas: &HtmlCanvasElement) -> Result<web_sys::Element> {
    let config = |what: &str, e: JsValue| Error::config("canvas", format!("{what}: {e:?}"));
    if !canvas.has_attribute("tabindex") {
        canvas
            .set_attribute("tabindex", "0")
            .map_err(|e| config("cannot set tabindex", e))?;
    }
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| Error::config("canvas", "there is no document"))?;
    let live = document
        .create_element("div")
        .map_err(|e| config("cannot create the live region", e))?;
    for (name, value) in [
        ("aria-live", "polite"),
        ("aria-atomic", "true"),
        ("data-gup-live", ""),
        ("style", LIVE_REGION_STYLE),
    ] {
        live.set_attribute(name, value)
            .map_err(|e| config("cannot set a live region attribute", e))?;
    }
    let canvas: &web_sys::Element = canvas.as_ref();
    canvas
        .after_with_node_1(&live)
        .map_err(|e| config("cannot insert the live region", e))?;
    Ok(live)
}

/// Insert the hidden data table after `live` (`State::update_table` fills
/// it).
fn table_element(live: &web_sys::Element) -> Result<web_sys::Element> {
    let config = |what: &str, e: JsValue| Error::config("canvas", format!("{what}: {e:?}"));
    let document = live
        .owner_document()
        .ok_or_else(|| Error::config("canvas", "there is no document"))?;
    let table = document
        .create_element("table")
        .map_err(|e| config("cannot create the data table", e))?;
    for (name, value) in [("data-gup-table", ""), ("style", LIVE_REGION_STYLE)] {
        table
            .set_attribute(name, value)
            .map_err(|e| config("cannot set a data table attribute", e))?;
    }
    live.after_with_node_1(&table)
        .map_err(|e| config("cannot insert the data table", e))?;
    Ok(table)
}

fn pointer_at(canvas: &HtmlCanvasElement, e: &MouseEvent) -> Point {
    let rect = canvas.get_bounding_client_rect();
    Point::new(
        e.client_x() as f32 - rect.left() as f32,
        e.client_y() as f32 - rect.top() as f32,
    )
}

fn button(e: &MouseEvent) -> Option<Button> {
    match e.button() {
        0 => Some(Button::Primary),
        1 => Some(Button::Middle),
        2 => Some(Button::Secondary),
        _ => None,
    }
}

impl CanvasApp {
    /// Show `chart` on `canvas`.
    ///
    /// Creates the [`Context`], draws the first frame and awaits its GPU
    /// errors, returning them (and any surface error) as `Err`. Later
    /// frames report errors to `console.error` and [`last_error`](Self::last_error).
    pub async fn new(canvas: HtmlCanvasElement, chart: impl Chart) -> Result<Self> {
        let cx = Context::new().await?;
        let target = CanvasTarget::new(&cx, canvas.clone())?;
        let live = live_region(&canvas)?;
        let table = table_element(&live)?;
        let mut st = State {
            cx: cx.clone(),
            target,
            renderer: Renderer::new(),
            chart: Box::new(chart),
            pending: false,
            closed: false,
            last_error: None,
            label_canvas: !canvas.has_attribute("aria-label"),
            label: String::new(),
            live,
            live_text: String::new(),
            table,
            table_alt: String::new(),
        };
        st.draw()?;
        cx.settle_gpu_errors().await?;
        let wake_id = NEXT_CANVAS_ID.fetch_add(1, Ordering::Relaxed);
        st.chart
            .set_waker(std::task::Waker::from(Arc::new(CanvasWake(wake_id))));
        let state = Rc::new(RefCell::new(st));
        CANVASES.with(|c| c.borrow_mut().push((wake_id, Rc::downgrade(&state))));
        let mut app = Self {
            state,
            listeners: Vec::new(),
            wake_id,
        };
        app.attach(&canvas)?;
        Ok(app)
    }

    fn listen(
        &mut self,
        target: &EventTarget,
        kind: &'static str,
        passive: bool,
        mut f: impl FnMut(Event) + 'static,
    ) -> Result<()> {
        let closure = Closure::<dyn FnMut(Event)>::new(move |e: Event| f(e));
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(passive);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                kind,
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .map_err(|e| {
                Error::config(
                    "canvas listener",
                    format!("cannot listen for {kind}: {e:?}"),
                )
            })?;
        self.listeners.push(Listener {
            target: target.clone(),
            kind,
            closure,
        });
        Ok(())
    }

    fn attach(&mut self, canvas: &HtmlCanvasElement) -> Result<()> {
        let target: &EventTarget = canvas.as_ref();
        // Touch drags pan the chart, not the page.
        let _ = canvas.style().set_property("touch-action", "none");
        let (state, c) = (Rc::clone(&self.state), canvas.clone());
        self.listen(target, "pointermove", true, move |e| {
            if let Some(e) = e.dyn_ref::<PointerEvent>() {
                dispatch(
                    &state,
                    Input::PointerMoved {
                        at: pointer_at(&c, e),
                    },
                );
            }
        })?;
        let state = Rc::clone(&self.state);
        self.listen(target, "pointerleave", true, move |_| {
            dispatch(&state, Input::PointerLeft);
        })?;
        let (state, c) = (Rc::clone(&self.state), canvas.clone());
        self.listen(target, "pointerdown", true, move |e| {
            if let Some(e) = e.dyn_ref::<PointerEvent>()
                && let Some(button) = button(e)
            {
                // A drag that leaves the canvas keeps its events.
                let _ = c.set_pointer_capture(e.pointer_id());
                dispatch(
                    &state,
                    Input::PointerDown {
                        at: pointer_at(&c, e),
                        button,
                    },
                );
            }
        })?;
        let (state, c) = (Rc::clone(&self.state), canvas.clone());
        self.listen(target, "pointerup", true, move |e| {
            if let Some(e) = e.dyn_ref::<PointerEvent>()
                && let Some(button) = button(e)
            {
                let _ = c.release_pointer_capture(e.pointer_id());
                dispatch(
                    &state,
                    Input::PointerUp {
                        at: pointer_at(&c, e),
                        button,
                    },
                );
            }
        })?;
        let (state, c) = (Rc::clone(&self.state), canvas.clone());
        self.listen(target, "wheel", false, move |e| {
            if let Some(e) = e.dyn_ref::<WheelEvent>() {
                e.prevent_default();
                let dy = e.delta_y() as f32;
                let lines = match e.delta_mode() {
                    1 => -dy,
                    2 => -dy * 10.0,
                    _ => -dy / 40.0,
                };
                dispatch(
                    &state,
                    Input::Wheel {
                        at: pointer_at(&c, e),
                        lines,
                    },
                );
            }
        })?;
        // Not passive: a mapped key must not also scroll the page. Tab and
        // every key the chart does not use keep their default.
        let state = Rc::clone(&self.state);
        self.listen(target, "keydown", false, move |e| {
            if let Some(e) = e.dyn_ref::<KeyboardEvent>() {
                let modifiers = Modifiers {
                    shift: e.shift_key(),
                    ctrl: e.ctrl_key(),
                    alt: e.alt_key(),
                    meta: e.meta_key(),
                };
                if let Some(input) = Input::from_key_name(&e.key(), modifiers) {
                    e.prevent_default();
                    dispatch(&state, input);
                }
            }
        })?;
        let window = web_sys::window()
            .ok_or_else(|| Error::config("canvas listener", "there is no window"))?;
        let state = Rc::clone(&self.state);
        self.listen(window.as_ref(), "resize", true, move |_| {
            {
                let mut guard = state.borrow_mut();
                let st = &mut *guard;
                if let Err(e) = st.target.fit(&st.cx) {
                    report(st, e.to_string());
                }
            }
            schedule(&state);
        })?;
        Ok(())
    }

    /// The app's [`Context`], for rendering the chart's scene elsewhere.
    pub fn context(&self) -> Context {
        self.state.borrow().cx.clone()
    }

    /// Run `f` on the chart, then schedule a redraw.
    pub fn with_chart<R>(&self, f: impl FnOnce(&mut dyn Chart) -> R) -> R {
        let r = f(&mut *self.state.borrow_mut().chart);
        schedule(&self.state);
        r
    }

    /// The last frame error, if any frame failed.
    pub fn last_error(&self) -> Option<String> {
        self.state.borrow().last_error.clone()
    }
}

impl Drop for CanvasApp {
    fn drop(&mut self) {
        {
            let mut st = self.state.borrow_mut();
            st.closed = true;
            st.live.remove();
            st.table.remove();
        }
        CANVASES.with(|c| c.borrow_mut().retain(|e| e.0 != self.wake_id));
        for l in self.listeners.drain(..) {
            // Removing a listener that is already gone is harmless.
            let _ = l
                .target
                .remove_event_listener_with_callback(l.kind, l.closure.as_ref().unchecked_ref());
        }
    }
}

/// `CanvasApp::new(canvas, chart)`.
pub async fn show(canvas: HtmlCanvasElement, chart: impl Chart) -> Result<CanvasApp> {
    CanvasApp::new(canvas, chart).await
}
