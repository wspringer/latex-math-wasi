// Drives the wasm32-unknown-unknown cdylib through its C ABI from node.
// usage: node scripts/wasm-smoke.mjs '<tex>' svg|pdf <out-file>
import { readFileSync, writeFileSync } from "node:fs";

const wasm = readFileSync("target/wasm32-unknown-unknown/release/latex_wasi_wasm.wasm");
const { instance } = await WebAssembly.instantiate(wasm, {});
const { memory, latex_wasi_alloc, latex_wasi_free, latex_wasi_render } = instance.exports;

const font = readFileSync("tests/fonts/STIXTwoMath-Regular.otf");
const request = new TextEncoder().encode(JSON.stringify({
  tex: process.argv[2],
  format: process.argv[3],
  font_size: 16,
  padding: 2,
  fonts: [font.length], // byte length: the font is the next slice of the blob
}));

const put = (bytes) => {
  const ptr = latex_wasi_alloc(bytes.length);
  new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
  return ptr;
};
const reqPtr = put(request);
const blobPtr = put(font);
const packed = latex_wasi_render(reqPtr, request.length, blobPtr, font.length);
const ptr = Number(packed >> 32n);
const len = Number(packed & 0xffffffffn);
const out = new Uint8Array(memory.buffer, ptr, len).slice();
latex_wasi_free(ptr, len);
latex_wasi_free(reqPtr, request.length);
latex_wasi_free(blobPtr, font.length);

if (out[0] !== 0) {
  console.error("error:", new TextDecoder().decode(out.subarray(1)));
  process.exit(1);
}
writeFileSync(process.argv[4], out.subarray(1));
console.log("ok", len - 1, "bytes");
