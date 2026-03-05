# Examples Index

All runnable examples. For a guided learning path, see [README.md](README.md).

**🖼️ [Visual Gallery](https://au-phiware.github.io/gup/)** — Browse rendered
screenshots of every example.

Run any example with: `cargo run --example <name>`

---

## Basic (`basic/`)

| Example             | Description                                |
| ------------------- | ------------------------------------------ |
| `01_hello_chart`    | Minimal scatter plot (console output only) |
| `02_scatter_window` | Visual scatter plot in a GPU window        |
| `03_line_chart`     | Line chart API demonstration               |
| `04_bar_chart`      | Bar chart API demonstration                |
| `hello_world`       | Hello world example                        |

---

## Showcase (`showcase/`)

| Example              | Description                           |
| -------------------- | ------------------------------------- |
| `business_dashboard` | Multi-chart business dashboard layout |

---

## Charts & Marks

| Example                           | Description                                       |
| --------------------------------- | ------------------------------------------------- |
| `scatter_plot_demo`               | Scatter plot with selection and interaction        |
| `boxplot`                         | Basic box plot rendering                           |
| `boxplot_rendering_demo`          | Box plot rendering pipeline demo                   |
| `boxplot_builder_demo`            | Box plot chart builder API                         |
| `multi_category_boxplot`          | Multi-category box plots                           |
| `observable_plot_showcase`        | Chart builder API showcase                         |
| `observable_plot_visual_showcase` | Visual showcase of chart builder API               |
| `integration_showcase`            | Full integration showcase of multiple chart types  |
| `custom_mark_demo`                | Implementing a custom mark type                    |
| `multi_pass_mark_demo`            | Multi-pass mark rendering                          |
| `bar_chart`                       | Bar chart variations                               |
| `line_chart_demo`                 | Line chart rendering demo                          |
| `area_chart_demo`                 | Area chart rendering                               |
| `heatmap_chart`                   | Heatmap chart rendering                            |
| `violin_plot_demo`                | Violin plot rendering                              |
| `density_scatter_overlay`         | Density scatter plot overlay                       |
| `treemap`                         | Treemap layout rendering                           |
| `treemap_window`                  | Windowed treemap rendering with keyboard controls  |
| `ordinal_scale`                   | Ordinal scale demonstration                        |
| `choropleth_world_population`     | Choropleth world population map                    |
| `choropleth_gpu_recolor`          | GPU-side choropleth recolouring demo               |
| `color_scale_heatmap`             | Colour scale heatmap                               |
| `composite_bar_trend`             | Composite bar and trend chart                      |
| `composite_scatter_regression`    | Scatter plot with regression line                  |
| `composite_mixed_data`            | Composite chart with per-layer data types          |
| `geo_world_map`                   | World map geographic rendering                     |
| `geographic_projection`           | Geographic projection demo                         |
| `force_directed_graph`            | Force-directed graph layout                        |
| `scatter_3d`                      | 3D scatter plot                                    |
| `scatter_3d_with_axes`            | 3D scatter plot with axes and grid                 |

---

## Axis & Grid

| Example                             | Description                              |
| ----------------------------------- | ---------------------------------------- |
| `axis_showcase`                     | Axis rendering and configuration         |
| `axis_tick_integration_visual_demo` | Axis tick rendering integration          |
| `tick_generation_visual_demo`       | Automatic tick generation algorithm demo |
| `scale_axis_integration_demo`       | Scale-to-axis integration                |
| `grid_visual_demo`                  | Grid line rendering                      |
| `grid_scatter_demo`                 | Grid with scatter plot overlay           |
| `label_formatting_demo`             | Axis label formatting and positioning    |

---

## Text Rendering

| Example                  | Description                               |
| ------------------------ | ----------------------------------------- |
| `text_rendering_demo`    | GPU SDF text rendering pipeline           |
| `text_clipping_demo`     | Text clipping and viewport bounds         |
| `debug_text_positioning` | Debug tool for text character positioning |
| `atlas_viewer`           | Font atlas inspection tool                |
| `msdf_debug`             | MSDF glyph generation debug view          |
| `multi_font_demo`        | Multi-font atlas rendering                |
| `multi_font_chart_demo`  | Charts using multiple font families       |

---

## Interaction & Selection

| Example                        | Description                               |
| ------------------------------ | ----------------------------------------- |
| `gpu_selection_demo`           | GPU-side selection and hit testing         |
| `interactive_selection_demo`   | Interactive mark selection with mouse      |
| `hover_reveal_demo`            | Hover-reveal tooltip integration           |
| `chart_hover_reveal_demo`      | Chart builder hover-reveal API             |
| `interaction_debug_visualizer` | Debug visualiser for interaction pipeline  |
| `attr_binding_demo`            | Attribute binding pipeline demo            |
| `brush_selection`              | Brush selection interaction                |
| `interactive_circles`          | Interactive circles demo                   |
| `interactive_graph`            | Interactive force-directed graph rendering |
| `linked_views`                 | Linked views coordination                  |
| `zoom_pan`                     | Zoom and pan interaction                   |

---

## Animation & Streaming

| Example                       | Description                          |
| ----------------------------- | ------------------------------------ |
| `advanced_temporal_animation` | Temporal animation system demo       |
| `animation_events`            | Animation event system               |
| `spline_animation_curves`     | Spline-interpolated animation curves |
| `keyframe_animation_storage`  | Storage buffer keyframe animation    |
| `async_streaming_demo`        | Async data streaming to GPU          |
| `data_transition_scatter`     | Data transition scatter animation    |
| `streaming_live_chart`        | Streaming live data chart            |
| `streaming_lod_scatter`       | Streaming LOD scatter plot           |

---

## Patterns

| Example                          | Description                              |
| -------------------------------- | ---------------------------------------- |
| `pattern_rendering_demo`         | Procedural pattern rendering             |
| `pattern_pipeline_demo`          | Pattern pipeline integration             |
| `multi_mark_pattern_showcase`    | Patterns across multiple mark types      |
| `texture_vs_procedural_patterns` | Texture vs procedural pattern comparison |

---

## Blend Modes

| Example                | Description                     |
| ---------------------- | ------------------------------- |
| `blend_modes_showcase` | GPU blend mode showcase         |
| `visual_blend_demo`    | Visual blend mode demonstration |

---

## Composition

| Example                               | Description                            |
| ------------------------------------- | -------------------------------------- |
| `parallel_composition_demo`           | Parallel composition of visualizations |
| `composition_error_recovery_showcase` | Error recovery in composition chains   |

---

## Performance & Profiling

| Example                        | Description                            |
| ------------------------------ | -------------------------------------- |
| `performance_trend_demo`       | Performance trend visualization        |
| `baseline_recommendation_demo` | Automated baseline recommendation demo |
| `surface_performance_demo`     | Surface configuration and performance  |
| `z_sort_demo`                  | Z-order radix sort performance demo    |

---

## Accessibility

| Example                  | Description                        |
| ------------------------ | ---------------------------------- |
| `web_accessibility_demo` | Web accessibility DOM overlay demo |

---

## Debug & Development

| Example                        | Description                          |
| ------------------------------ | ------------------------------------ |
| `gpu_debug_demo`               | GPU debug tools demo                 |
| `gpu_debug_visualization_demo` | GPU hit test debug visualiser        |
| `buffer_demo`                  | GPU buffer pool management demo      |
| `buffer_validation_demo`       | Buffer validation and debugging      |
| `resource_graph_demo`          | GPU resource dependency graph        |
| `context_demo`                 | Render context lifecycle demo        |
| `adaptive_lod_debug`           | Adaptive LOD debug visualisation     |
| `lod_pyramid_debug`            | LOD pyramid debug visualisation      |

---

## Window & Surface

| Example               | Description                              |
| --------------------- | ---------------------------------------- |
| `simple_window`       | Minimal wgpu window setup                |
| `multi_window_demo`   | Multiple windows with shared GPU context |
| `windowed_demo`       | Windowed application scaffold            |
| `surface_events_demo` | Window/surface event integration         |
| `position_sync_demo`  | Visualization position synchronization   |
| `web_dashboard_demo`  | Web-based dashboard with WASM            |

---

## Shader Development

| Example                | Description                    |
| ---------------------- | ------------------------------ |
| `shader_pipeline_demo` | ComposableShaderPipeline usage |

---

## Export

| Example      | Description                         |
| ------------ | ----------------------------------- |
| `export_png` | PNG export via GPU off-screen rendering |
| `svg_export` | SVG export demonstration            |
| `html_export` | HTML export demonstration           |
| `pdf_export` | PDF export demonstration            |

---

## Intermediate (`intermediate/`)

| Example             | Description                                    |
| ------------------- | ---------------------------------------------- |
| `styled_scatter`    | Data-driven styling with categorical colours   |
| `multi_series_line` | Multiple time series visualisation             |
| `categorical_bar`   | Categorical data with vertical/horizontal bars |

---

## Tutorials (`tutorials/`)

| Example                    | Description                             |
| -------------------------- | --------------------------------------- |
| `tutorial01_scatter`       | Tutorial 1: Getting started scatter chart |
| `tutorial04_interactions`  | Tutorial 4: Interactions demo           |
| `tutorial05_streaming`     | Tutorial 5: Streaming data demo         |
| `tutorial06_custom_marks`  | Tutorial 6: Custom marks demo           |
