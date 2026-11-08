# A Tour of Gup

Thirteen short pages, one for each area of what Gup can do. Each has a snippet
of Rust and the image it draws, and links to the tutorial that teaches the
topic. The tour is for seeing the breadth in ten minutes; the
[tutorials](../tutorials/README.md) are for learning.

Every snippet is a real Rust file in [`snippets/`](snippets/), compiled and
drawn by `tests/tour.rs`. Every image is a golden image of that code, and a test
checks that each page shows exactly the code that was drawn.

1. [From rows to a chart](01_five_lines.md): a scatter chart from your own rows,
   saved as a PNG and an SVG.
2. [One builder for each chart](02_vocabulary.md): scatter, line, bar, barh,
   area, histogram, heatmap and box plot.
3. [Position, size and colour](03_encodings.md): a log axis, sizes and a colour
   ramp, and the scales behind them.
4. [Axes and labels](04_axes_and_labels.md): a time axis, a second y axis and
   tick labels you write.
5. [Colour and themes](05_colour_and_themes.md): the default palette, a dark
   theme and a high-contrast theme.
6. [Lines and curves](06_lines_and_curves.md): steps, smooth curves and
   translucent lines.
7. [Text and annotation](07_text_and_annotation.md): halos, overlap hiding and
   fallback positions for labels.
8. [Interaction](08_interaction.md): tooltips, zoom, pan and brushing.
9. [Accessibility](09_accessibility.md): keyboard navigation and text for screen
   readers.
10. [Live data](10_live_data.md): rows that arrive while the chart is on screen.
11. Big data: charts of millions of rows, with measured numbers (coming in
    GUP-670)
12. Under the builders: your own scale and mark in WGSL (coming in GUP-670)
13. Everywhere it runs: the window, egui, Bevy and the browser (coming in
    GUP-670)
