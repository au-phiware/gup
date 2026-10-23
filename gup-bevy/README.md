# gup-bevy

Bevy integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library, built on `gup-core`.

```rust,ignore
let image = images.add(chart_image(size, 1.0));
commands.spawn((GupChart::new(chart, size, image.clone()), Sprite::from_image(image)));
```

`GupPlugin` makes a `GupContext` on Bevy's own wgpu device, draws every
`GupChart` into a texture each frame and copies it into the Bevy image (GPU to
GPU, no readback). Add rows to a live chart with an `Appender`.

| gup-bevy | Bevy | wgpu |
| -------- | ---- | ---- |
| 0.1      | 0.18 | 27.x |

See [the integration guide](../docs/BEVY_INTEGRATION.md) and
`cargo run -p gup-bevy --example bevy_scatter`. The crate is a workspace member
but is left out of `mask all-check` and the `tests` CI job (its dependencies are
heavy); the `integrations` job lints and tests it:

```bash
cargo clippy -p gup-bevy --all-targets -- -D warnings
xvfb-run -a cargo test -p gup-bevy -- --test-threads=1
```
