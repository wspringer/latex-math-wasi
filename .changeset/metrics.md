---
default: minor
---

#### Baseline metrics for inline placement

`--format metrics` / `"format": "metrics"` returns `{width, height, depth, ascent, em, ex}`
in user units for the given padding — `depth` is baseline to bottom edge, so an inline
image is placed with `vertical-align: -depth`. The SVG root now carries the same value as
`style="vertical-align:-<depth>px"` (MathJax's convention).
