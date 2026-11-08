# Archived documents

These documents describe the system Gup had before the RFC-001 core (deleted in
GUP-486 and GUP-488). They are kept for history and as input to future work, not
as a description of the code: nothing here is current.

- [`accessibility/`](./accessibility/) - the old accessibility layer (ARIA,
  AT-SPI, VoiceOver, UI Automation, keyboard focus, patterns, sonification), its
  WCAG 2.1 AA audit and its testing guides. They describe deleted code (AT-SPI,
  UI Automation, sonification and a WCAG conformance claim for it), not what Gup
  does. What Gup does today is in
  [`docs/ACCESSIBILITY.md`](../ACCESSIBILITY.md).
- [`research/`](./research/) - the Rust-to-WGSL transpilation research and the
  existing-solutions analysis (an approach that was not adopted), and the
  graphics-programming patterns of the old render path.

Current documentation: [docs/README.md](../README.md).
