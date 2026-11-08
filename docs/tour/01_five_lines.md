# 1. From Rows to a Chart

A chart starts from your rows and one closure for each thing you want to see.

Rows are your own structs. This one is all the chart needs to know about:

<!-- tour: docs/tour/snippets/01_five_lines.rs#country -->

```rust
struct Country {
    region: &'static str,
    gdp: f64,
    life: f64,
    people: f64,
}
```

<!-- tour: docs/tour/snippets/01_five_lines.rs#five_lines -->

```rust
gup::scatter(countries())
    .x(|c| c.gdp)
    .y(|c| c.life)
    .color(|c| c.region)
    .x_title("GDP per person (US$)")
    .y_title("Life expectancy (years)")
```

![A scatter chart of twelve countries in four colours, with a legend](../../tests/golden/tour/01_five_lines.png)

`countries()` returns twelve of them. Any iterator of any type works, so a `Vec`
of your own structs does. `.x` and `.y` say where each row goes, `.color` gives
each region a colour and a legend entry, and the two titles label the axes.
Everything else (axis ranges, ticks, theme) has a default.

To keep the chart, ask it to save itself. `save_png` and `save_svg` take the
same arguments.

<!-- tour: docs/tour/snippets/01_five_lines.rs#save -->

```rust
pub fn save(dir: &Path) -> gup::Result<()> {
    let cx = Context::shared()?;
    let mut chart = five_lines();
    chart.save_png(&cx, dir.join("chart.png"), 640, 400)?;
    chart.save_svg(&cx, dir.join("chart.svg"), 640, 400)
}
```

`gup::show(chart)` opens the same chart in a window, where the wheel zooms and a
drag pans. `chart.to_svg(&cx, 640, 400)` returns the SVG as a `String`.

**Go deeper**: [Tutorial 1](../tutorials/01_getting_started.md), from
[Step 2](../tutorials/01_getting_started.md#step-2-draw-your-first-scatter-chart).
API: `gup::scatter`, `ChartExt::save_png`, `ChartExt::save_svg`, `gup::show`.

**Not in 0.1**: there is no HTML export, and no PDF export (see `CHANGELOG.md`,
"Known limitations").
