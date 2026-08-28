#!/usr/bin/env sh
# Builds the wasm32-wasip1 command and the wasm32-unknown-unknown cdylib, runs both
# (wasmtime / node) on a corpus formula, and requires their SVG and PDF output to be
# byte-identical to the native CLI's. Run from the repository root.
set -eu

FONT=tests/fonts/STIXTwoMath-Regular.otf
TEX='\left\vert \sum_k a_kb_k \right\vert \leq \left(\sum_k a_k^2\right)^{\frac12}'
OUT=target/check-wasm
mkdir -p "$OUT"

cargo build -q -p latex-wasi-cli
cargo build -q --release -p latex-wasi-wasi --target wasm32-wasip1
cargo build -q --release -p latex-wasi-wasm --target wasm32-unknown-unknown

B64=$(base64 < "$FONT" | tr -d '\n')
TEX_JSON=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$TEX")

for fmt in svg pdf; do
  target/debug/latex-wasi --font "$FONT" --format "$fmt" --padding 2 "$TEX" > "$OUT/native.$fmt"

  printf '{"tex": %s, "format": "%s", "font_size": 16, "padding": 2, "fonts": ["%s"]}' \
    "$TEX_JSON" "$fmt" "$B64" > "$OUT/request.$fmt.json"
  # No --dir: the module must not need a preopened directory.
  wasmtime run target/wasm32-wasip1/release/latex-wasi-wasi.wasm < "$OUT/request.$fmt.json" > "$OUT/wasi.$fmt"
  cmp "$OUT/wasi.$fmt" "$OUT/native.$fmt"
  echo "wasip1  $fmt: identical to native ($(wc -c < "$OUT/wasi.$fmt" | tr -d ' ') bytes)"

  node scripts/wasm-smoke.mjs "$TEX" "$fmt" "$OUT/browser.$fmt" > /dev/null
  cmp "$OUT/browser.$fmt" "$OUT/native.$fmt"
  echo "browser $fmt: identical to native"
done
