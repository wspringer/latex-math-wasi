# Changelog

Maintained by Knope from change files in `.changeset/`. Versions are tagged
`v<version>`; every release carries the `wasm32-wasip1` command module, the
browser module, and a `provenance.json` naming the git revision and the pinned
versions of the libraries compiled into them.

## 0.1.1 (2026-09-01)

### Features

#### `\color{name}{…}` with a caller-defined palette; `\phantom` fixed

Colour scopes in the formula now reach the output. CSS names work as before; other names
are defined by the caller — `--define name=SPEC` on the CLI, `"palette"` in the request —
so part of a formula can be a CMYK or spot colour in PDF. Undefined names are an error.
`\phantom{…}` now takes space without drawing (it used to draw).

#### Baseline metrics for inline placement

`--format metrics` / `"format": "metrics"` returns `{width, height, depth, ascent, em, ex}`
in user units for the given padding — `depth` is baseline to bottom edge, so an inline
image is placed with `vertical-align: -depth`. The SVG root now carries the same value as
`style="vertical-align:-<depth>px"` (MathJax's convention).

#### PDF fill colour: gray, RGB, CMYK or a spot colour

`--color` / `"color"` sets the fill. PDF takes `gray`, `rgb`, `cmyk`, or `spot` (a
`Separation` colour space with a CMYK alternate, shown as a swatch by InDesign); the
default is now an explicit 100 % K. SVG/PNG take `gray`/`rgb`/`#rrggbb` and refuse
cmyk/spot instead of converting.

#### PNG output

`--format png` (CLI) and `"format": "png"` (wasm request) rasterize the SVG with resvg,
at `--scale` / `"scale"` device pixels per user unit. Transparent background by default;
`PngOptions::background` sets a colour. New crate `latex-math-png`.

## 0.1.0

Initial workspace: `core` (LaTeX math parser and OpenType MATH layout engine,
derived from KenyC/ReX, fonts as bytes, optical-size font sets), `svg`
(deterministic `<defs>`/`<use>` outlines), `pdf` (real text, embedded subsetted
CID fonts), `cli`, `wasm` (C-ABI browser module) and `wasi` (`wasm32-wasip1`
command). Golden-file, visual-diff and cross-backend tests over STIX Two Math,
XITS Math and Latin Modern Math.
