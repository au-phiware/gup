# Accessibility

What Gup does for readers who cannot see a chart, cannot use a pointer or need
more contrast, what you must still do, and what it does not do. This guide
states what each feature does today. It makes no conformance claim (no "WCAG AA
compliant"): conformance depends on the page or app around the chart, which Gup
does not control.

Every Rust block below is compiled as a doctest of `gup` (one is marked
`ignore`: it is wasm-only).

## What Gup does

| Feature                                        | PNG | SVG | Window | egui | Browser |
| ---------------------------------------------- | --- | --- | ------ | ---- | ------- |
| Text alternative (`describe`)                  | API | yes | API    | API  | yes     |
| Keyboard focus over rows, zoom, pan, reset     | -   | -   | yes    | yes  | yes     |
| Focus ring (drawn into the chart)              | yes | -   | yes    | yes  | yes     |
| Row announcement text (`focus_text`)           | API | -   | API    | API  | yes     |
| Live region that speaks it                     | -   | -   | -      | -    | yes     |
| Hidden data table                              | API | -   | API    | API  | yes     |
| High-contrast theme, Okabe-Ito default palette | yes | yes | yes    | yes  | yes     |

"API" means Gup computes the text and returns it, and nothing there speaks it:
your application must route it to the platform's accessibility API (see
[Not supported](#not-supported)). A dash means the feature does not apply (a PNG
or SVG file has no keyboard) or does not exist there. The focus ring is drawn by
the chart whatever the host, so a PNG rendered after a key press shows it.

## Text alternatives

After a chart is resolved (drawn, rendered or exported), `describe()` returns a
`Description`: a short `title` (the chart's title, else its kind, such as
"Scatter chart") and a `text` of sentences naming the kind, the number of data
rows, each axis with its range or categories as the axis labels them, and each
colour legend. It is made from the chart's own state, so it follows the data,
titles and axis titles you set.

When the generated text is not enough, `.description(..)` replaces the `text`
(the title is still derived).

```rust
use gup::prelude::*;

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut chart = gup::scatter(vec![
    (1.0, 55.0, "Africa"),
    (20.0, 74.0, "Europe"),
    (60.0, 81.0, "Asia"),
])
.x(|r| r.0)
.y(|r| r.1)
.color(|r| r.2)
.title("Wealth and health")
.x_title("GDP ($k)")
.y_title("Life expectancy");

// Nothing is known before the chart is resolved.
assert!(chart.describe().is_none());

let svg = chart.to_svg(&cx, 640, 400)?;
let d = chart.describe().expect("resolved by to_svg");
assert_eq!(d.title, "Wealth and health");
assert!(d.text.starts_with("Scatter chart, 3 data rows."));
assert!(d.text.contains("Colour legend: Africa, Europe and Asia."));

// The SVG root has role="img" and the description as its first children.
assert!(svg.contains("role=\"img\""));
assert!(svg.contains("<title>Wealth and health</title>"));
assert!(svg.contains("<desc>Scatter chart, 3 data rows."));

// Your own words win over the generated sentences.
let mut chart = chart.description("Life expectancy rises with income in all three regions.");
chart.to_svg(&cx, 640, 400)?;
assert_eq!(
    chart.describe().unwrap().text,
    "Life expectancy rises with income in all three regions."
);
# Ok(())
# }
```

- **SVG**: the root element gets `role="img"` and escaped `<title>` and `<desc>`
  as its first children. A scene without a description (one built by hand) gets
  none.
- **Browser canvas**: `web::CanvasApp` sets `role="img"` and `aria-label` to the
  title and text after each successful draw, only when they change. It leaves an
  `aria-label` the page set itself alone. It uses `aria-label` and not
  `aria-labelledby` because element ids collide when an SVG is inlined into a
  page, or several charts share one.
- **PNG** is pixels: use `describe()` to write the image's alt text yourself.

## Keyboard

A chart answers keys sent as `Input::key(Key::..)`. The window (`gup::show`),
egui (`GupWidget`) and browser (`web::CanvasApp`) hosts translate their
platform's key events into those inputs for you:

| Key (host)          | `Key`               | Does                                                  |
| ------------------- | ------------------- | ----------------------------------------------------- |
| Right, Left         | `Right`, `Left`     | Next, previous row of the focused layer, in x order   |
| Up, Down            | `Up`, `Down`        | The nearest row (by x) of the next, previous layer    |
| Home, End           | `Home`, `End`       | First, last row of the focused layer                  |
| Enter, Space        | `Enter`             | Runs the focused row's `Click` handlers               |
| Escape              | `Escape`            | Clears focus                                          |
| `+` or `=`, `-`     | `ZoomIn`, `ZoomOut` | Zoom about the centre of the plot, as two wheel lines |
| Shift with an arrow | arrow, with Shift   | Pan by a tenth of the plot's width or height          |
| `0`                 | `ResetView`         | Undo zoom and pan                                     |

Keys with Ctrl, Alt or Meta held are ignored (they belong to the host: browser
shortcuts, for one), as are keys during a drag. Keys do nothing before the chart
has been resolved.

**Focus order.** The first arrow key focuses the first row of the first layer
(`End` the last). The focused row is hovered exactly as if the pointer were on
it: its `Enter` and `Leave` handlers run and its tooltip shows. A ring is drawn
around it in the theme's title colour with a contrasting edge, so it reads on
the light, dark and high-contrast themes. A bar, histogram bin, heatmap cell or
box plot box is outlined whole instead. Moving the pointer onto another row ends
keyboard focus. A key zoom or pan keeps focus while the row stays inside the
plot and ends it once the row has left.

```rust
use gup::prelude::*;
use gup::{Input, Key, RowId};

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut plot = Plot::new();
plot.circles(vec![(1.0, 2.0), (2.0, 5.0), (3.0, 3.0)])
    .x(|r| r.0)
    .y(|r| r.1);
// A guide line that is decoration, not data.
plot.line(vec![(1.0, 3.0), (3.0, 3.0)])
    .x(|r| r.0)
    .y(|r| r.1)
    .focusable(false);
plot.resolve(&cx, 640.0, 400.0)?;

assert_eq!(plot.focused(), None);
plot.handle(Input::key(Key::Right));
assert_eq!(plot.focused(), Some(RowId { layer: 0, row: 0 }));
plot.handle(Input::key(Key::End));
assert_eq!(plot.focused(), Some(RowId { layer: 0, row: 2 }));

// Up would move to the next layer, but that layer is not focusable.
plot.handle(Input::key(Key::Up));
assert_eq!(plot.focused(), Some(RowId { layer: 0, row: 2 }));

// Zoom in about the centre of the plot, then put the view back.
let zoomed = plot.handle(Input::key(Key::ZoomIn));
assert!(zoomed.redraw);
plot.handle(Input::key(Key::ResetView));

plot.handle(Input::key(Key::Escape));
assert_eq!(plot.focused(), None);
# Ok(())
# }
```

`focusable(false)` on a layer takes it out of the order and out of the data
table. The box plot's whiskers, caps and medians and the area charts' outline
layers are already not focusable, so the arrows walk the data and not its
decoration.

**Limit.** Above `Retain::AUTO_MAX_ROWS` (10 million) visible rows at full
extent, a layer that has dropped its CPU data cannot be walked: keyboard focus
skips it, and `pick_row` and the marquee brush find nothing there, until you
zoom in below the limit. Pointer hover and click still work through the GPU
pick. This is tracked as GUP-667.

## Screen readers

Gup computes the words; a screen reader needs a platform to speak them.

**Row announcements.** `focus_text()` is what to say while a row has keyboard
focus: the row's tooltip with line breaks replaced by `"; "` if its layer has
one, else its fields as `name: value` joined by `"; "`, then its place among the
layer's rows in view, `" (2 of 3)"`. Numbers are said as the axes label them
(three significant digits, the thousands grouping and minus sign of the tick
labels; times in ISO 8601 UTC). It is `None` without keyboard focus.

```rust
use gup::prelude::*;
use gup::{Input, Key};

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut plot = Plot::new();
plot.circles(vec![(0.8, 0.9), (0.9, 0.95), (1.0, 0.99)])
    .x(|r| r.0)
    .y(|r| r.1);
plot.x_title("Mass").y_title("Span");
plot.resolve(&cx, 320.0, 200.0)?;

assert_eq!(plot.focus_text(), None);
plot.handle(Input::key(Key::Right));
plot.handle(Input::key(Key::Right));
assert_eq!(plot.focus_text(), Some("Mass: 0.9; Span: 0.95 (2 of 3)"));
# Ok(())
# }
```

**Browser live region.** `web::CanvasApp` makes the canvas focusable
(`tabindex="0"` unless the page set one), listens for `keydown` on it (calling
`preventDefault` only for keys the chart uses, so Tab still leaves the canvas)
and inserts an `aria-live="polite"` `aria-atomic="true"` element after it
(attribute `data-gup-live`). After each frame it writes `focus_text()` there,
only when the text changed, so a screen reader speaks each row as the arrows
move. Dropping the app removes it.

**Browser data table.** The app also inserts a visually hidden
`<table data-gup-table>` after the live region: the caption is the description's
title, the columns are the fields, and there is one row per data row of every
focusable layer, however much of the data is in view. A screen reader's table
navigation can read the data without the keyboard. It is rebuilt only when the
description changes, and holds at most 1,000 rows: the caption then says "(first
1,000 of N rows)". Rows of a layer that has dropped both its rows and its
columns have no fields and are skipped.

The same table is available to any host as `data_table(max_rows)`:

```rust
use gup::prelude::*;

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut chart = gup::scatter((0..2_500).map(|i| (i as f64, (i % 7) as f64)))
    .x(|r| r.0)
    .y(|r| r.1)
    .title("Readings")
    .x_title("Minute")
    .y_title("Level");
chart.resolve(&cx, 640.0, 400.0)?;

let table = chart.data_table(1_000).expect("resolved");
assert_eq!(table.headers, ["Minute", "Level"]);
assert_eq!(table.rows.len(), 1_000);
assert_eq!(table.omitted, 1_500);
assert_eq!(table.caption, "Readings (first 1,000 of 2,500 rows)");
assert_eq!(table.rows[3], ["3", "3"]);
# Ok(())
# }
```

The browser parts need a page, and `gup::web` exists only on `wasm32` with the
`web` feature. This is the whole setup; the label, live region and table need
nothing more:

```rust,ignore
// wasm32 only: `gup::web` needs the `web` feature on that target.
async fn run(canvas: web_sys::HtmlCanvasElement) -> gup::Result<()> {
    let chart = gup::scatter(vec![(1.0, 2.0), (2.0, 5.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Readings");
    let _app = gup::web::show(canvas, chart).await?;
    Ok(())
}
```

**Outside the browser** nothing speaks `focus_text()`. A window or egui app
should read `chart.focus_text()` after forwarding keys and give it to its
platform's accessibility API.

## Colour and contrast

Gup's default palette is the Okabe-Ito set, which is designed to stay
distinguishable under colour-vision deficiency, in an order chosen so the first
four colours are four different hues (`OKABE_ITO_DISTINCT`; the published order
is `OKABE_ITO`). These are the measured smallest pairwise colour differences
(CIEDE2000, Machado 2009 severity 1; about 2.3 is just noticeable) among the
first n series, from the tests in `theme.rs`:

| Series shown             | normal | deuteranopia | protanopia | tritanopia |
| ------------------------ | -----: | -----------: | ---------: | ---------: |
| default, first 3         |  37.02 |        24.91 |      12.26 |      14.01 |
| default, first 4         |  37.02 |        16.11 |      12.26 |      12.07 |
| default, first 5         |  22.31 |        15.54 |      12.26 |      12.07 |
| `HIGH_CONTRAST`, first 4 |  38.40 |        20.53 |      18.20 |      12.07 |
| `HIGH_CONTRAST`, first 5 |  37.02 |        16.11 |      12.26 |      12.07 |

The default palette has eight colours and includes two blues, so a chart of five
or more series shows both; the first four never do. A colour difference is not a
second channel (see [What you must do](#what-you-must-do)).

`Theme::high_contrast()` is for low-vision readers: black text and axes on white
(21:1), a grid at 3:1, a subtitle at 12.6:1, larger text, a focus ring at 21:1,
and the `HIGH_CONTRAST` palette, whose eight colours are each between 3.06:1 and
21:1 against the background. Dimmed (unbrushed) rows are drawn in the grid grey.
Mark sizes, line widths and the sequential and diverging palettes are not
themed. `Theme::dark()` has its own palette, with the blues swapped so that the
first colour is legible on the dark background.

```rust
use gup::prelude::*;

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut chart = gup::scatter(vec![(1.0, 2.0), (2.0, 1.0)])
    .x(|r| r.0)
    .y(|r| r.1)
    .theme(Theme::high_contrast());
let image = chart.render(&cx, 320, 200)?;
// A white page.
assert_eq!(image.get_pixel(0, 0).0, Color::WHITE.to_rgba8());
# Ok(())
# }
```

Gup does not read the operating system's contrast or colour-scheme preference:
choose the theme in your application.

## What you must do

- **Give charts a title and axis titles.** They head the description, name the
  fields in announcements (`Mass: 0.9` and not `x: 0.9`) and caption the data
  table. A chart without them is described as "Chart" with axes named "x" and
  "y".
- **Set `.description(..)` when the generated text is not enough.** It says what
  a chart is made of, not what it shows: add the finding ("Life expectancy rises
  with income") or the reading the chart exists for.
- **Do not rely on colour alone.** The palettes are chosen to be distinct under
  common colour-vision deficiencies, but two series that differ only in hue are
  still one channel. Use position, size or labels (`Plot::text`, the bar value
  labels) for the distinction that matters.
- **Keep tooltips meaningful.** A layer's tooltip is its announcement: the live
  region speaks the tooltip text, line breaks becoming `"; "`. A tooltip that
  says only "click for more" is announced as that.
- **Route the text where it is heard** outside the browser (see
  [Screen readers](#screen-readers)), and let keyboard users reach the chart: in
  egui the widget takes keys only while it has focus (click it or Tab into it).

```rust
use gup::prelude::*;
use gup::{Input, Key};

# fn main() -> gup::Result<()> {
let cx = Context::shared()?;
let mut chart = gup::scatter(vec![("Ada", 36.0, 1815.0), ("Alan", 41.0, 1912.0)])
    .x(|r| r.1)
    .y(|r| r.2)
    .x_title("Age")
    .y_title("Born")
    .tooltip(|r| format!("{}\nAge {}", r.0, r.1));
chart.resolve(&cx, 640.0, 400.0)?;
chart.handle(Input::key(Key::Right));
assert_eq!(chart.focus_text(), Some("Ada; Age 36 (1 of 2)"));
# Ok(())
# }
```

## Not supported

- **Bevy input.** The Bevy integration does not forward keys.
- **Platform accessibility APIs for native windows** (AT-SPI, UI Automation,
  NSAccessibility). A `gup::show` window is a drawn surface with no
  accessibility tree. Gup computes the text (`describe`, `focus_text`,
  `data_table`); putting it in the platform tree is the application's job.
- **Patterns or textures as a second channel** for colour. Series differ by
  colour (and by what you add) only.
- **Sonification.** There is no audio rendering of data.
- **OS preferences.** No automatic contrast, dark-mode or reduced-motion
  following.

## Tested and untested

The behaviour above is covered by unit tests, doctests, golden images and a
headless-browser smoke test (`mask wasm-browser`) that checks the canvas's
`tabindex`, `aria-label`, live region, data table and key handling. **No screen
reader has been tested by hand** (no Orca, NVDA, JAWS or VoiceOver run): what
this guide says a screen reader receives is what the DOM contains, not a
recording of what it said.

The guides in [`archive/accessibility/`](./archive/accessibility/) describe a
system that was deleted; they are history, not this page.
