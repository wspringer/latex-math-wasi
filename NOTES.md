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

## M1 — SVG at parity with ReX (2026-08-28)

### What was done

- `crates/unicode-math`: vendored from KenyC (build.rs generates the symbol table from
  `unicode-math-table.tex` + `unimathsymbols.txt`; build-deps `regex`/`nom` run on the
  host only, so they do not affect the wasm dependency tree).
- `crates/core`: vendored KenyC's `parser`, `layout`, `font` (ttf-parser backend only),
  `render` (the box walker + bbox backend), `dimensions`, `geometry`. Deleted the five
  renderer backends, the `pdf-rs/font` backend, and `serde`/`log`. Public API is
  `Font::parse(&[u8])`, `Options { font_size, style }`, `render(tex, &font, &opts) ->
  RenderTree`. `tree::TreeBackend` implements ReX's `Backend` and flattens the walk into
  `Vec<GlyphInstance { font, gid, x, y, size }>` + `Vec<Rule>`. Attribution in
  `crates/core/LICENSE-ReX`.
- `crates/svg`: one `<path>` per distinct `(font, gid)` in `<defs>` (sorted, `BTreeSet`),
  `<use transform="translate(x y) scale(s)">` per instance, `<rect>` per rule. Fixed
  precision (3 decimals for positions, 6 for scale, 2 for outline points), trailing
  zeros and `-0` normalised. Space-like glyphs with empty outlines are skipped.
- `crates/cli`: `latex-math --font F [--font F…] [--format svg] [--size N]
  [--style display|text] [--padding N] [-o out] 'formula'`; hand-rolled arg parsing,
  no extra deps. `--format pdf` is a stub until M3.
- Tests: 18-formula corpus (ReX's 14 README samples + the 4 from its TeX-comparison
  suite) × STIX Two Math / XITS Math / Latin Modern Math → 54 golden SVGs under
  `tests/golden/`. Mismatches rasterise expected/actual/diff PNGs into
  `target/visual-diff/` with resvg (red = lost, blue = new). Also: determinism
  (render twice → identical), and rasterise-not-blank for every formula.
- CI: fmt, clippy `-D warnings`, tests, and a separate job that builds the library
  crates for `wasm32-wasip1` and `wasm32-unknown-unknown` and greps `cargo tree` for
  `cc`/`cmake`/`bindgen`/`pkg-config`/`*-sys`.

### Parity with KenyC/ReX, measured

Compared glyph positions from the M0 spike binary (unmodified KenyC, XITS, 12 pt) with
our CLI (`--size 16`, since KenyC converts pt→px at 96/72) on six corpus formulas.
Every x coordinate is identical. Every y coordinate is identical **except on
subscripts**, where ours sit higher by exactly `(SubscriptTopMax − SubscriptShiftDown)`
× scale — 0.15 em in XITS (400 vs 250), 0.158 em in STIX Two, 0.097 em in Latin
Modern. That is the inherited `subscript_shift_down` bug, fixed here; TeX rule 18a uses
`SubscriptShiftDown` (σ16). Nested scripts and sub+sup pairs shift by derived amounts
because the sub/sup gap rule redistributes the change. So: parity everywhere, minus one
known bug.

### Surprises

- ttf-parser returns the CFF `FontMatrix` as `f32`. `1/1000` is not representable, so
  every coordinate carried ~3e-7 relative noise and `1` laid out 6.7600003 px high at
  10 px. `TtfMathFont` now uses the exact `f64` `1/unitsPerEm` from `head` unless the
  CFF matrix genuinely disagrees (skew, or scale off by >1e-4).
- KenyC's `LayoutBuilder::font_size` silently took *points* and multiplied by 96/72.
  Removed: `font_size` is user units per em, full stop. Everything in the tree is in
  those units.
- ReX's `\text{…}` goes through the math cmap glyph by glyph — fine for `\mathrm{d}`
  and short words, no kerning, no shaping. Good enough; `rustybuzz` stays out.
- Colour (`\color`, `\gray`) parses and lays out, but the render tree drops it
  (brief: glyphs and rules, nothing else). The `quartic` sample still renders; the
  "Quartic" label is just black.
- The vendored code needed ~35 clippy fixes to pass `-D warnings`; all mechanical.
  The build script's `regex = "*"` was pinned to `1`.

### Corrections to the brief

- Font size: the brief never says what unit `--size` is; I chose user units per em
  (SVG px, PDF pt) rather than TeX points, because that is what the PDF `Tf` operator
  and the SVG viewBox want, and it keeps the SVG/PDF coordinate-equality property test
  (M3) trivial.

## M2 — arbitrary MATH fonts (2026-08-28)

- All 18 corpus formulas render without error in eight fonts: the three committed
  ones plus Asana Math, Fira Math, Garamond Math, TeX Gyre Pagella Math (CFF) and
  Noto Sans Math (TrueType `glyf` outlines — the only non-CFF MATH font I could find).
  Inspected the CFF/TrueType/Asana renders by eye; nothing font-specific broke.
  Those five are not committed (brief: three free fonts for committed tests).
- Bug found and fixed: `glyph_from_gid` treated a glyph without a bounding box as a
  missing glyph. Space (gid 1 in STIX Two) has an advance but no contours, so
  `\operatorname{lim sup}` failed with `MissingGlyphGID(1)`. Contourless glyphs now get
  an empty box. `\text{a b}` had worked only because the text path never asked for the
  space's box.
- Noto Sans Math's `ssty` alternates are visibly heavier than its text glyphs; that is
  the font's design, not a scaling error — the engine substitutes `ssty` level 1/2 for
  script/scriptscript as ReX does.
- Not supported (parser): `\boldsymbol`, `\,` inside `\operatorname{}`. Left as is.

## M3 — PDF with embedded subsetted fonts (2026-08-28)

### Shape

`crates/pdf`: one page, media box = bbox (+ padding), y flipped so the PDF origin is
the bbox's bottom-left. Per used font index: `Type0` (Identity-H) → `CIDFont` →
`FontDescriptor` → font program, plus a ToUnicode CMap. Subsetting via `subsetter`
0.2 (`default-features = false` — the default pulls `skrifa` for variable fonts,
which we do not need). CFF fonts: the subsetter always emits a CID-keyed CFF with an
identity charset, so the bare `CFF ` table is embedded as `FontFile3 /CIDFontType0C`
and CID = subset gid. TrueType fonts: whole subset as `FontFile2`,
`CIDFontType2`, `/CIDToGIDMap /Identity`. Widths are `/W [0 [w0 w1 …]]` in 1/1000 em.

The content stream is written by hand rather than through `pdf_writer::Content`:
pdf-writer encodes `Str` as a literal with octal escapes (`(\000\002) Tj`), which is
legal but unreadable, and prints `f32` shortest-repr (`4.2720003`). Ours is
`/F0 16 Tf`, relative `Td`, `<0002> Tj`, `x y w h re f`, all at 3 decimals — the same
precision as the SVG, and byte-deterministic. `Td` deltas are computed from the
*rounded* previous position so the accumulated position never drifts (the first
version drifted 2e-3 after ~30 glyphs and failed the cross-backend test).

### Verified outside cargo

`qpdf --check` clean; `pdffonts` shows `CID Type 0C … emb yes sub yes uni yes`
(CFF) and `CID TrueType` (Noto Sans Math); `pdftotext` recovers `sin(𝑥) d𝑥 = 𝜋`
through the ToUnicode map (math-italic codepoints U+1D465 etc., as the font's cmap
says); poppler and Ghostscript rasterise correctly. A 5.9 kB PDF embeds a 838 kB
STIX Two Math.

### Tests

- `svg_and_pdf_agree_on_every_glyph_and_rule`: parses both outputs back (SVG `<use>`
  transforms and `<rect>`s; PDF `Td`/`Tj`/`re` tokens) and checks every glyph and rule
  lands at the same place, for all 54 corpus×font combinations. Space glyphs are in
  the PDF (real text) but not the SVG (no outline), so the SVG side is compared after
  filtering by "has an outline".
- Determinism, `CIDFontType0C` present, subset-tagged BaseFont, ToUnicode present,
  output < 1/10 of the font size.

### Details worth knowing

- Subset tag: FNV-1a over the sorted original glyph ids → six letters. Same glyph set,
  same tag, so identical formulas produce identical PDFs.
- ToUnicode picks the *smallest* Unicode codepoint that maps to a glyph. Glyphs reached
  only via `ssty`/size variants/assemblies have no cmap entry and get no mapping;
  readers will show them as nothing when copying. Mapping variants back to their base
  character would need the GSUB/MathVariants reverse lookup — possible later.
- `FontDescriptor`: `Symbolic` (+`Italic` when the post table's italic angle ≠ 0),
  `StemV` estimated from the weight class as Typst does. No `CIDSet`, no
  `Length1` on CFF. Fine for readers and InDesign; PDF/A would want more.
- No stream compression (would need `miniz_oxide`; pure Rust, easy to add later).

## M4 — optical size sets (2026-08-28)

### What changed in the engine

`LayoutEngine` now holds `fonts: [&F; 4]`, `metrics: [FontMetricsCache; 4]` and
`scales: [f64; 4]`, indexed by level (display, text, script, scriptscript). Every
`self.font` / `self.metrics_cache.constants()` in the engine became
`self.font_at(context.style)` / `self.constants_at(context.style)`, so a construct is
laid out with the constants of the font *for the style it is being laid out in*, and
the parent's constants are what decide where a child goes — the sup/sub shift, limits
gaps, radical gaps all read `context` (parent) while the child is built with
`context.superscript_variant()` etc. Positions were already computed in user units in
the parent's box, so cross-size alignment needed no extra work; the tests below pin it.

Public API: `FontSet::new(&[Font], levels: [usize; 4])`, `FontSet::single(&font)`,
`FontSet::with_scales([f64; 4])`, `render(tex, &set, &opts)`. `GlyphInstance::font`
indexes the set's font list, so the SVG/PDF backends already handled multiple fonts —
`to_pdf` emits one Type0 font + subset per used index (`two_fonts_become_two_embedded_subsets`).

CLI: `--font` repeatable; 1 → all levels, 2 → `[0,0,1,1]`, 3 → `[0,0,1,2]`, 4 →
`[0,1,2,3]`; `--levels D,T,S,SS` for anything else.

### Correction to the brief: scripts still scale

The brief says: "When a distinct font is supplied for a style, do **not** additionally
apply `ScriptPercentScaleDown`. The design already accounts for the size." I
implemented that first, and it is wrong. An optical size does not change the em — a
Caption cut set at 24 px is 24 px tall; it has the larger x-height, looser spacing and
sturdier hairlines that make it legible *when set small*. TeX does the same thing with
`cmr5`: it is a distinct design, and it is still set at 5 pt for scriptscript. With the
brief's rule, `x^{2}` with STIX text and Latin Modern scripts rendered the superscript
at full text size (see the first mixed render, `target/optical/mixed.png` before the
fix). What *is* right in the brief: the script level must draw from the script font and
read that font's MATH constants (axis height, rule thickness, …) — and it does.

So the rule is: level scale is a property of the level, defaulting to the **text**
font's `ScriptPercentScaleDown` / `ScriptScriptPercentScaleDown` (LuaTeX takes them
from the current text font too), overridable with `FontSet::with_scales` (TeX's
`[1, 1, 0.7, 0.5]`, or whatever Minion Math's documentation recommends for
Caption/Tiny). The script font's own percentages are ignored: they describe how *it*
would like its scripts scaled, which is a question for its own text level.

If you know Minion Math's cuts to be pre-scaled (I doubt it — Minion Pro Opticals are
not, and the `size` GPOS feature exists precisely because optical cuts share an em),
`with_scales([1.0; 4])` gives the brief's behaviour with no code change.

### Also fixed while in there

`to_font` (user units → font units, used to pick delimiter/radical variants) ignored
the level scale, so at script level ReX asked for a variant `1/scale` too small and
then drew it at the script size. Now it divides by font size × scale. None of the 54
goldens changed (the corpus has no script-level `\left…\right` or radicals), but
`e^{\sqrt{x+1}}` now gets a correctly sized radical.

### Tests (`crates/core/tests/optical.rs`)

- single font → sizes `[16, 11.2, 8.8]` (STIX 70/55).
- distinct script font → glyph `font == 1`, sizes still `[16, 11.2, 8.8]` from the
  text font's percentages, not XITS's 75/60.
- explicit `with_scales([1, 1, .7, .5])` → `[16, 11.2, 8.0]`.
- script glyph's outline is XITS's own (gid resolves to U+1D465 in XITS; outline
  differs from STIX's), and the advance used is XITS's.
- superscript placement: STIX parent with XITS script equals STIX parent with a second
  STIX object as script, differs from an XITS parent, and equals
  `-SuperscriptShiftUp(STIX) × 16` — the child font does not move the child.
- a fraction inside a superscript takes `FractionRuleThickness` from the script font.

## M5 — wasm32-wasip1 and browser (2026-08-28)

- `crates/wasm` (`cdylib` + `rlib`): `handle(request_json, font_blob) -> Result<Vec<u8>, String>`
  plus a three-function C ABI (`latex_math_alloc`, `latex_math_render`,
  `latex_math_free`) for `wasm32-unknown-unknown`. No `wasm-bindgen`: the module has
  zero imports, so it instantiates with `{}` from any host, and the JS side is 20 lines
  (`scripts/wasm-smoke.mjs`). Result is `(ptr << 32) | len`; first byte is the status.
- `crates/wasi`: the `wasm32-wasip1` command. stdin → JSON (fonts inline as base64),
  stdout → SVG/PDF bytes, stderr + exit 1 on error. Verified with `wasmtime run` and
  **no** `--dir`: nothing touches the filesystem.
- Request schema is one struct shared by both paths; `fonts` entries are either base64
  strings or byte lengths into a separate blob (browser: pass `Uint8Array`s, no base64
  round trip). `levels`/`scales` as in `FontSet`.
- `scripts/check-wasm.sh` builds both modules and requires their SVG and PDF output to
  be byte-identical to the native CLI's. It is; CI runs it (wasmtime + node actions).
  This is the cross-platform determinism check the brief asked for: same f64 arithmetic
  everywhere (no transcendental functions in the engine), same formatting code.
- Dependency tree for either wasm target: 31 crates, none with a build script that
  compiles C. Release modules are ~750 kB (the generated Unicode-math symbol table is a
  good chunk); `opt-level = "s"` + LTO already on. `wasm-opt` would shave more but is a
  C++ tool, so it stays out of the build.
- `serde`/`serde_json`/`base64` are used only by the wasm crate; `core`/`svg`/`pdf`
  stay serde-free.

### CI toolchain note (post-M5)

CI was red from M2 to M5: `dtolnay/rust-toolchain@stable` is Rust 1.98, which adds the
`mismatched_lifetime_syntaxes` lint and a stricter `needless_late_init`, while my shell
outside `nix develop` still had rustup's 1.85. Fixed the nine sites; the rule is now:
run `cargo clippy` **inside** `nix develop`, which pins the same stable as CI.
While in there: `frac` read the fraction constants from the *enclosing* style instead
of the fraction's own (`\dfrac`/`\tfrac` overrides) — corrected to `frac_context`.

## Release tooling (2026-08-28)

Modelled on `../lilypond-wasi`, with one deliberate difference. lilypond-wasi has two
version axes — its *recipe* and the *upstream LilyPond* it tails — so its tags are
`<variant>/<lilypond>-p<recipe>` and Knope owns only the recipe number. Here there is
one axis: the engine is vendored (not tailed) and every library we rely on is pinned
in `Cargo.lock` and compiled into the artifact. So Knope owns the workspace version
outright, tags are plain `v<version>`, and the "what went into this" question is
answered by `provenance.json` on each release (git revision, vendored ReX revision,
ttf-parser / pdf-writer / subsetter / serde_json versions) rather than by the tag.

Same conventions as lilypond-wasi otherwise: change files in `.changeset/` are the
only release source (`ignore_conventional_commits = true`), conventional subjects
stay as hygiene, no change file = no release, Knope Bot maintains the release PR.
Knope 0.21 handles `[workspace.package] version` in a plain `Cargo.toml` entry;
`Cargo.lock` needs one `dependency = …` entry per workspace crate so `--locked`
builds (now used in CI) do not break after a bump. Note that Knope treats
`minor` as a patch bump while the version is 0.x.

Tracking dependencies: Dependabot (weekly, grouped) for crates; CI decides whether a
bump is safe (goldens, no-C scan, wasm ≡ native). The vendored engine gets
`scripts/rex-upstream.sh`, which lists KenyC/ReX commits since
`crates/core/REX-UPSTREAM` — porting is manual and recorded here.

## Rename: latex-wasi → latex-math-wasi (2026-08-28)

The name promised a LaTeX engine in wasm; the project is math mode only (a non-goal
in the brief). Renamed while it is cheap — no tagged release, nothing published:

- repo `wspringer/latex-wasi` → `wspringer/latex-math-wasi`, pairing with the planned
  `latex-math-mcp` the way `lilypond-wasi` pairs with `lilypond-mcp`;
- crates `latex-wasi-*` → `latex-math-*` (`latex_math_core` …). "wasi" stays in the
  repo name (the product is a wasi build) but not in the crate names — `core`, `svg`
  and `pdf` have nothing to do with wasi;
- binary `latex-wasi` → `latex-math`; release assets `latex-math-<v>-{wasip1,browser}.wasm`;
  C ABI `latex_math_{alloc,render,free}`; PDF producer string `latex-math`.

Proof it was a pure rename: the 54 golden SVGs are untouched, `scripts/check-wasm.sh`
still reports wasip1 and browser output identical to native, insta snapshots were
moved (`latex_wasi_core__…` → `latex_math_core__…`), and `--locked` builds pass with
the lock file only renamed. Vendored ReX namespaces and `REX-UPSTREAM` unchanged.

## PNG output (2026-08-29)

Added for the planned MCP server, which wants an inline preview the agent can look at
(as lilypond-mcp does) without a native rasterizer in the Node package. `latex-math-png`
takes the SVG backend's output and rasterizes it with resvg — the same crate the golden
visual diff already used as a dev-dependency — so the PNG shows exactly what the SVG
says, and there is no third geometry backend to keep in sync. `default-features = false`
on resvg: the `text` feature would pull in fontdb and system-font discovery (which we
must never have in the engine), and the raster-image decoders are dead weight.

`PngOptions { scale, background }`: `scale` is device pixels per user unit (2.0 = retina),
`background` RGBA or transparent. Pixel size is `ceil(size × scale)`, minimum 1 px.
CLI `--format png --scale N`; request field `"scale"`. Deterministic (checked in tests and
by `check-wasm.sh`, which now compares png as well: wasip1 and browser bytes identical to
native).

Cost: resvg + usvg + tiny-skia + png compile to wasm without C (`cargo tree` scan still
clean on both targets); the release modules grow from ~750 kB to ~1.7 MB (browser 1,742,490 bytes,
wasip1 1,675,834 bytes) — tiny-skia is most of it. Acceptable for an npm package; if it
ever matters, a direct RenderTree → tiny-skia backend would drop usvg (~300 kB). The "not blank" test compares the PNG's byte length with that of a
transparent PNG of the same size — cheap, and enough: an image with glyphs never
compresses to the empty image's size.

## Baseline metrics (2026-08-29)

For the MCP to be a drop-in for `math-svg-mcp`, the model needs what that server
returns: width, height and depth (distance from the baseline to the bottom edge), so it
can set `vertical-align` for inline expressions. MathJax gets there by measuring in `ex`
with a guessed `xHeightRatio`; we have the real font, so this is exact.

- `RenderTree::image_box(padding)` is now the single definition of the document
  rectangle (bbox + padding, baseline at tree `y = 0`); SVG and PDF both size from it,
  and PNG inherits it through the SVG. Before, each backend computed the same four numbers
  on its own.
- `latex_math_core::metrics(tree, fonts, options, padding)` → `Metrics { width, height,
  depth, ascent, em, ex }`, `ex` from the *text-style* font's `OS/2 sxHeight` (with an
  optical-size set that is the Regular cut, which is what surrounding text would use).
  `Metrics::to_json()` is hand-formatted at three decimals so the CLI and both wasm builds
  emit identical bytes (`check-wasm.sh` now compares `metrics` too).
- New format `metrics` in the CLI and the request schema — a second call, deterministic
  and a few milliseconds; simpler than an envelope around the PDF/PNG bytes.
- The SVG root gets `style="vertical-align:-<depth>px"`, MathJax's convention, so the SVG
  alone is enough for HTML embedding. All 54 goldens changed by exactly that one header
  line; rasters are unaffected (same viewBox, same content).

## PDF colour (2026-08-29)

The user's reason to want PDF over SVG for InDesign: SVG is sRGB only, print wants CMYK
values or a spot colour. Until now the content stream set no colour at all, which means
PDF's default `DeviceGray 0` — black, but by accident and in a space that colour
management may treat differently from body text.

`PdfOptions.color: Color` — `Gray`, `Rgb`, `Cmyk`, or `Spot { name, tint, cmyk }`. The
fill operator is the first thing in the content stream and applies to glyphs and rules
alike (`g`/`rg`/`k`, or `/CS0 cs t scn`). A spot colour is a `[/Separation /Name
/DeviceCMYK fn]` colour space in the page resources, with a type-2 (exponential, N = 1)
tint transform from `[0 0 0 0]` to the given CMYK, so a device without the colorant
renders the alternate; pdf-writer escapes the name (`/PANTONE#20300#20C`). Default is
`Cmyk [0 0 0 1]`: 100 % K, what surrounding body text is on a press.

Verified: qpdf `--check` clean for gray/cmyk/spot; poppler renders the spot file in the
alternate colour; the colour line is the only difference between two PDFs of the same
formula (`colour_does_not_move_anything`). Components are validated to 0–1 and a spot
colour must have a name (`PdfError::BadColor`).

CLI `--color gray:K | rgb:R,G,B | cmyk:C,M,Y,K | spot:NAME:TINT:C,M,Y,K | #rrggbb`; request
`"color": {"gray": g} | {"rgb": [..]} | {"cmyk": [..]} | {"spot": {"name", "tint", "cmyk"}}`.
SVG/PNG accept gray/rgb (mapped to `#rrggbb`) and refuse cmyk/spot with a clear error
rather than converting silently — the whole point is that the numbers stay the numbers.
Not done: ICC-based spaces and PDF/X output intents; InDesign does not need them for a
placed graphic.
