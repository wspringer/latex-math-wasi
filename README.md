# latex-wasi

Pure-Rust LaTeX-math → SVG / PDF renderer driven by OpenType MATH fonts. No TeX, no C,
compiles to `wasm32-wasip1`.

```
nix develop
cargo run -p latex-wasi-cli -- --font tests/fonts/STIXTwoMath-Regular.otf \
    'x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}' > quadratic.svg
```

Crates: `core` (parser + OpenType MATH layout → render tree), `svg`, `pdf` (real text,
subsetted CID fonts), `cli`, `wasm` (JSON request → bytes, C ABI for the browser),
`wasi` (`wasm32-wasip1` command: request on stdin, bytes on stdout).

```
cargo build --release -p latex-wasi-wasi --target wasm32-wasip1
wasmtime run target/wasm32-wasip1/release/latex-wasi-wasi.wasm < request.json > out.svg
cargo build --release -p latex-wasi-wasm --target wasm32-unknown-unknown   # browser
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
