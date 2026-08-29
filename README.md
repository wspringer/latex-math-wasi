# latex-math-wasi

Pure-Rust LaTeX-math → SVG / PDF renderer driven by OpenType MATH fonts. No TeX, no C,
compiles to `wasm32-wasip1`.

```
nix develop
cargo run -p latex-math-cli -- --font tests/fonts/STIXTwoMath-Regular.otf \
    'x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}' > quadratic.svg
cargo run -p latex-math-cli -- --font tests/fonts/STIXTwoMath-Regular.otf \
    --format png --scale 2 -o quadratic.png 'x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}'
```

Crates: `core` (parser + OpenType MATH layout → render tree), `svg`, `pdf` (real text,
subsetted CID fonts), `png` (the SVG rasterized with resvg; `--scale` for density), `cli`,
`wasm` (JSON request → bytes, C ABI for the browser), `wasi` (`wasm32-wasip1` command: request on stdin, bytes on stdout).

Inline placement: `--format metrics` (request `"format": "metrics"`) returns
`{"width","height","depth","ascent","em","ex"}` in user units for the same padding —
`depth` is baseline to bottom edge, so an inline image sits right with
`vertical-align: -depth`. The SVG carries the same value in its root `style`.

Colour: `--color` (request `"color"`) sets the fill. PDF takes `gray:K`, `rgb:R,G,B`,
`cmyk:C,M,Y,K` or a spot colour `spot:NAME:TINT:C,M,Y,K` (a `Separation` colour space with
a CMYK alternate — InDesign shows it as a swatch); default is 100 % K. SVG/PNG are sRGB
only: `gray`, `rgb` or `#rrggbb`.

```
cargo build --release -p latex-math-wasi --target wasm32-wasip1
wasmtime run target/wasm32-wasip1/release/latex-math-wasi.wasm < request.json > out.svg
cargo build --release -p latex-math-wasm --target wasm32-unknown-unknown   # browser
scripts/check-wasm.sh   # proves both produce the native CLI's bytes
```

The request schema is documented in `crates/wasm/src/lib.rs`; `scripts/wasm-smoke.mjs`
shows the browser-side calling convention.

Optical sizes: pass one font per math level (`--font` ×4 = display, text, script,
scriptscript, or `--levels`). Each level draws from and reads MATH constants from its
own font; script levels are scaled by the text font's `ScriptPercentScaleDown` unless
overridden (`FontSet::with_scales`). The layout engine derives from [KenyC/ReX](https://github.com/KenyC/ReX)
(MIT). Decisions and findings are recorded in [NOTES.md](NOTES.md).

Test fonts in `tests/fonts/` are STIX Two Math and XITS Math (SIL OFL 1.1) and Latin
Modern Math (GUST Font License). Commercial fonts must never be committed; `.gitignore`
blocks font files outside that directory.

## Releasing

One version for the whole workspace (`Cargo.toml`, `[workspace.package]`), managed
by [Knope](https://knope.tech). A PR that changes what ships — layout, output, API,
a dependency bump that alters output — carries a change file (`knope document-change`,
or `.changeset/<slug>.md` with a `default: patch|minor|major` header and a
`#### summary` line). No change file, no release: right for docs, CI and
dependency bumps that leave the golden files untouched. Conventional commit subjects
(`feat:`, `fix:`, `ci:`, …) stay as history hygiene; they do not decide versions.

The Knope bot keeps a release PR open; merging it bumps the version, tags
`v<version>` and publishes the GitHub release, to which `release.yml` attaches
`latex-math-<version>-wasip1.wasm`, `latex-math-<version>-browser.wasm`,
`provenance.json` (git revision, vendored ReX revision, versions of the libraries
compiled in) and `SHA256SUMS`.

Dependencies: Dependabot proposes crate bumps weekly, CI verifies them (goldens,
no-C check, wasm/native byte equality). The vendored engine has no dependency to
bump — `scripts/rex-upstream.sh` lists what KenyC/ReX has done since
`crates/core/REX-UPSTREAM`.
