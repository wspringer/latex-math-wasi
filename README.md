# latex-wasi

Pure-Rust LaTeX-math → SVG / PDF renderer driven by OpenType MATH fonts. No TeX, no C,
compiles to `wasm32-wasip1`.

```
nix develop
cargo run -p latex-wasi-cli -- --font tests/fonts/STIXTwoMath-Regular.otf \
    'x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}' > quadratic.svg
```

Crates: `core` (parser + OpenType MATH layout → render tree), `svg`, `cli`; `pdf` and
`wasm` follow. The layout engine derives from [KenyC/ReX](https://github.com/KenyC/ReX)
(MIT). Decisions and findings are recorded in [NOTES.md](NOTES.md).

Test fonts in `tests/fonts/` are STIX Two Math and XITS Math (SIL OFL 1.1) and Latin
Modern Math (GUST Font License). Commercial fonts must never be committed; `.gitignore`
blocks font files outside that directory.
