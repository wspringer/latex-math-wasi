---
default: minor
---

#### PDF fill colour: gray, RGB, CMYK or a spot colour

`--color` / `"color"` sets the fill. PDF takes `gray`, `rgb`, `cmyk`, or `spot` (a
`Separation` colour space with a CMYK alternate, shown as a swatch by InDesign); the
default is now an explicit 100 % K. SVG/PNG take `gray`/`rgb`/`#rrggbb` and refuse
cmyk/spot instead of converting.
