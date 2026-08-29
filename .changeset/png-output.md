---
default: minor
---

#### PNG output

`--format png` (CLI) and `"format": "png"` (wasm request) rasterize the SVG with resvg,
at `--scale` / `"scale"` device pixels per user unit. Transparent background by default;
`PngOptions::background` sets a colour. New crate `latex-math-png`.
