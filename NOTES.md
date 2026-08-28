# NOTES

Decisions, surprises, and things that turned out to be wrong in the brief.
Newest milestone at the bottom.

## M0 — spike and recommendation (2026-08-28)

### What was read

Cloned and read, not skimmed from READMEs:

| repo | last push | what it is |
|---|---|---|
| `ReTeX/ReX` (upstream) | 2020-07 | original; font = compile-time Rust tables generated from XITS |
| `grafeia/ReX` (s3bk) | 2022-02 | runtime font parsing via `pdf-rs/font`, output via `pathfinder`; all git deps |
| `laurmaedje/ReX` | 2022-12 | fork of grafeia, "trim down", ttf-parser 0.17 + nom; abandoned when Typst grew its own math |
| `KenyC/ReX` | **2026-02** | fork of grafeia; `MathFont` trait, ttf-parser backend, 5 pluggable renderers, 599 commits, regression + TeX-comparison test harness, MIT |
| `0x0sky/ReX` | 2026-08 | upstream + a WebGPU demo page, three commits, no engine changes |

### Upstream ReX: not a viable base

- There is no font loading. `fonts/stix2/src/{glyphs,variants,kernings,symbols}.rs`
  are ~12k lines of generated tables for XITS; `UNITS_PER_EM` is a constant.
  Everything is hardcoded to that one font.
- The `Renderer` trait hands the backend `symbol(pos, unicode: u32, scale)` —
  a **codepoint**, not a glyph id. Variants/assemblies are addressed by codepoint
  too. That cannot drive a PDF `Tj` or a subsetter.
- SVG output is `<text>` elements plus a `@font-face` pointing at
  `http://rex.breeden.cc/rex-xits.otf`. No outlines.
- No wasm story at all (2018-era crates, `static_map` proc macros).

### grafeia fork: right shape, wrong dependencies

- Replaced the tables with `pdf-rs/font` (git) and added a `Backend` trait:
  `symbol(pos, gid: u16, scale, &MathFont)`, `rule(pos, w, h)`,
  `begin_color/end_color`. This is the interface shape we want.
- Output goes through `pathfinder_*` (git, servo) — heavy, unmaintained, and
  it drags `font`'s whole PDF-oriented tree along. Not wasip1-friendly.
- Contains a copy-paste bug in `Constants::new`:
  `subscript_shift_down` is read from `SubscriptTopMax`. **KenyC inherited it**
  (`src/font/backend/ttf_parser.rs:624`). Subscripts sit slightly wrong in
  every fork descended from grafeia. Fix on day one.

### KenyC fork: the base

Facts, from source:

- **Layout/output separation.** `layout::engine::LayoutEngine<'f, F: MathFont>`
  produces a `Layout<'f, F>` tree of `LayoutNode { width, height, depth, node }`
  where `node` is `HorizontalBox | VerticalBox | Grid | Glyph | Rule | Kern | Color`.
  `render::Renderer::render(&layout, &mut impl Backend<F>)` walks it and calls:
  - `FontBackend<F>::symbol(pos: Cursor{x,y}, gid: GlyphId(u16), scale: f64, &F)`
    — `scale` is the effective font size in px (font size × ScriptPercentScaleDown),
    y grows downward, `pos` is the glyph origin on its baseline.
  - `GraphicsBackend::rule(pos, w, h)` (top-left corner), `begin_color/end_color`,
    optional `bbox` for debug.
  So per glyph you get exactly (font ref, glyph id, x, y, scale). That is the
  render tree the brief asks for; our `core` crate will implement a `Backend`
  that flattens the walk into `Vec<GlyphInstance>` + `Vec<Rule>`.
- **Font loading.** `font::MathFont` trait: `glyph_index`, `glyph_from_gid`
  (bbox/advance/lsb/italics/top-accent), `kern_for(gid, height, corner)`,
  `constants()`, `font_units_to_em()`, `horz_variant`/`vert_variant`,
  `glyph_script_alternate` (`ssty`). `TtfMathFont<'a>` wraps
  `ttf_parser::Face<'a>` — bytes in, no I/O. Glyph assembly (extender repeat
  count + overlap interpolation, the HarfBuzz approach) is implemented in the
  backend (`construct_glyphs`), not in ttf-parser.
- **Nothing XITS-specific** in the engine. Three TeX parameters that have no
  MATH-table equivalent are hardcoded as TeX's defaults: `delimiter_factor`
  0.901, `delimiter_short_fall` 0.1em, `null_delimiter_space` 0.1em; plus array
  strut/baselineskip constants in `layout/constants.rs`. Regression tests run
  against XITS, Garamond Math, Fira Math and Asana Math.
- **wasm.** With `--no-default-features --features ttfparser-fontparser` the
  lib's entire dependency tree is `ttf-parser 0.25`, `serde`, `log`, and a
  vendored `unicode-math` (build.rs generates the symbol table from
  `unicode-math-table.tex`). Verified: `cargo build --lib --target wasm32-wasip1`
  succeeds, and the spike binary below builds for wasip1 too.
- **Parser** covers what the brief needs and more: `\text`, `\mbox`,
  `\operatorname`, `\mathrm`/`\mathbf`/…, user macros, `\sqrt[n]`, arrays and
  matrices, `\big…\Bigg`, `\left\middle\right`, `\underline`, colours.
  `\text{}` is laid out glyph-by-glyph from cmap with no shaping — so
  `rustybuzz` is not needed (brief's guess was right).
- **Determinism.** No `HashMap` in the engine; the grid uses `BTreeMap`;
  arithmetic is `+ - * / min max floor ceil` on f64. Byte-identical output
  across native/wasm is achievable.
- **Single-font assumption — the thing we must change.** `LayoutEngine` holds
  one `&'f F` and one `FontMetricsCache`; `LayoutContext` carries `style` and
  `font_size`; `convert::to_px` multiplies by
  `scale_factor(style)` = 1 / ScriptPercentScaleDown / ScriptScriptPercentScaleDown.
  Every glyph lookup goes through `self.font` and every constant through
  `self.metrics_cache.constants()`. All of these are greppable call sites in
  `layout/engine.rs` (1404 lines) and `layout/convert.rs`. The refactor for
  optical sizes is: engine holds a `FontSet` + four metrics caches, resolves
  `(font, constants, scale_factor)` from `context.style`, and `LayoutGlyph`
  carries a font *index* instead of `&F`. Positions are already computed in
  px in the parent's coordinate space, so cross-size baseline alignment falls
  out once constants are per-style.
- Warts worth knowing: `LayoutBuilder::font_size(x)` silently converts pt→px
  (×96/72); italic-correction and sub/superscript kern lookups use the parent's
  font size for the script glyph (marked `TODO` in source, matters more with
  distinct script fonts); `#[deny(missing_docs)]` is on for most modules.

### From-scratch over ttf-parser: evaluated, rejected

`ttf-parser 0.25`'s `math` module exposes everything the MATH table has:
all 56 constants, italics/top-accent/kern-info per glyph, extended-shape
coverage, vertical/horizontal constructions with variants and assemblies.
What it does *not* give you is any algorithm: no assembly solver, no layout.
A from-scratch engine is ~1.5k lines of layout (App. G + MATH), ~2.5k of
parser/macros/environments, the symbol table, plus the months of TeX-comparison
tuning KenyC has already done (their harness renders each sample with real
LaTeX and image-diffs). We would spend M1 re-deriving what already exists and
end up with the same architecture. Not competitive.

### Spike

`spike/` — KenyC layout (pinned rev) → collecting `Backend` → glyph ids →
`ttf-parser::OutlineBuilder` → SVG `<path>` per glyph + `<rect>` per rule.
Quadratic formula and Cauchy–Schwarz rendered correctly with XITS at 12pt
(inspected via a resvg rasterisation). Builds for `wasm32-wasip1`.

### Downstream crates checked

`pdf-writer 0.15`, `subsetter 0.2.6` (now built on `skrifa`/`write-fonts`),
`ttf-parser 0.25`, `resvg 0.48`: all build for `wasm32-wasip1` and
`wasm32-unknown-unknown`; `cargo tree` for both targets contains no `cc`,
`*-sys`, `cmake` or `bindgen`. `resvg` stays a dev-dependency only.

### Corrections to the brief

1. "grafeia/ReX in particular" — grafeia is dormant (2022) and pathfinder-bound.
   **KenyC/ReX** is its maintained descendant and already has the abstractions
   the brief asked me to check for. That is the base.
2. Upstream ReX does not "write SVG" in any reusable sense; it emits `<text>`
   against a webfont. "Parity with upstream ReX on the sample corpus" (M1)
   should mean parity with **KenyC's** regression renders, not upstream's
   `samples/*.svg`, which cannot be re-generated without their webfont.
3. `ScriptPercentScaleDown` is applied in ReX exactly as the brief describes,
   and it is applied inside `to_px`, so switching it off per style when a
   distinct font is supplied is a one-line predicate, not a restructure.

### Decision

Vendor KenyC/ReX's `parser`, `layout`, `font` (ttf-parser backend only) and
`unicode-math` into `crates/core` under its MIT licence with attribution,
delete the five renderer backends and the `pdf-rs/font` backend, fix the
`subscript_shift_down` bug, and refactor the engine around a `FontSet`.
Vendoring rather than a git dependency because the FontSet change touches the
engine's core types and cannot be layered on from outside.

### Tooling

`flake.nix` provides rustc/cargo 1.98 (oxalica rust-overlay, stable) with the
`wasm32-wasip1` and `wasm32-unknown-unknown` targets, clippy, rustfmt,
rust-analyzer, `wasmtime`, `wasm-tools`, `cargo-deny`. `.envrc` → `use flake`.
