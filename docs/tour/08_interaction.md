# 8. Interaction

Tooltips, zoom, pan and brushing, all driven by `Input` values, so the same code
runs in a window or in a test.

A chart does not need a window to be interactive: you send it `Input` values and
it answers with what changed. The two snippets below send them by hand, so the
images can be drawn without one. In a window, `gup::show` sends them for you. To
point at a mark, the snippets ask the chart where a data point is drawn:
`chart.pixel(x, y)` gives the point in the chart's pixels, through the axes' own
scales (so it is right for a log or time axis too), as of the last resolve.
`chart.value(point)` is its inverse.

## A tooltip, after a zoom and a pan

<!-- tour: docs/tour/snippets/08_interaction.rs#tooltip -->

```rust
let mut chart = gup::scatter(birds())
    .x(|b| b.mass)
    .y(|b| b.wingspan)
    .radius(5.0)
    .title("Birds of Aotearoa")
    .x_title("Mass (kg)")
    .y_title("Wingspan (m)")
    .tooltip(|b| format!("{}\n{} kg, {} m", b.name, b.mass, b.wingspan));
let cx = Context::shared()?;
chart.resolve(&cx, 640.0, 400.0)?;

// Zoom with the wheel over the flock, then drag the view to centre it.
let flock = chart.pixel(0.7, 0.8).unwrap();
let _ = chart.handle(Input::wheel(flock, 5.0));
let to = Point::new(flock.x + 110.0, flock.y);
let _ = chart.handle(Input::pointer_down(flock, Button::Primary));
let _ = chart.handle(Input::pointer_moved(to));
let _ = chart.handle(Input::pointer_up(to, Button::Primary));
chart.resolve(&cx, 640.0, 400.0)?;

// Point at the Kereru where it is now: the next resolve draws the tooltip.
let kereru = chart.pixel(0.65, 0.80).unwrap();
let _ = chart.handle(Input::pointer_moved(kereru));
chart.resolve(&cx, 640.0, 400.0)?;
Ok(chart)
```

![A zoomed scatter chart with a tooltip beside the Kereru](../../tests/golden/tour/08_tooltip.png)

`.tooltip(..)` builds the text from the row. `handle` records what the pointer
is over and says whether to redraw; the tooltip itself, and a thin ring on the
hovered point (`hover_highlight(false)` turns it off), are drawn by the next
`resolve`. The wheel zooms about the pointer and a drag pans; here both bring
the middle of the flock into view, so five of the twelve birds are out of the
picture.

## A brush

<!-- tour: docs/tour/snippets/08_interaction.rs#brush -->

```rust
let mut chart = gup::scatter(birds())
    .x(|b| b.mass)
    .y(|b| b.wingspan)
    .radius(6.0)
    .title("Birds of Aotearoa")
    .x_title("Mass (kg)")
    .y_title("Wingspan (m)");
chart.plot_mut().drag(Drag::Brush);
let cx = Context::shared()?;
let plot = chart.resolve(&cx, 640.0, 400.0)?.layout.plot;

// Drag from the bottom-left corner to just right of the Kea.
let right = chart.pixel(0.95, 0.0).unwrap().x;
let from = Point::new(plot.left() + 1.0, plot.bottom() - 1.0);
let to = Point::new(right, plot.top() + 1.0);
let _ = chart.handle(Input::pointer_down(from, Button::Primary));
let _ = chart.handle(Input::pointer_moved(to));
let _ = chart.handle(Input::pointer_up(to, Button::Primary));
Ok(chart)
```

![Twelve birds with a translucent rectangle over the seven that are selected](../../tests/golden/tour/08_brush.png)

With `Drag::Brush`, a drag selects the rows whose marks lie inside the
rectangle, and the other rows fade. `on_brush` tells you which rows are
selected, so a table or a second chart can follow.

**Go deeper**: [Tutorial 4](../tutorials/04_interactions.md), the
[tooltip](../tutorials/04_interactions.md#step-1-tooltips),
[zoom and pan](../tutorials/04_interactions.md#step-3-zoom-pan-and-reset) and
[brush](../tutorials/04_interactions.md#step-5-brushing) sections. API: `Input`,
`Response`, `Drag`, `Plot::on_brush`.

**Not in 0.1**: there is no builder call that hovers a row from data, so the
snippets send the pointer to the pixel that `chart.pixel` reports.
