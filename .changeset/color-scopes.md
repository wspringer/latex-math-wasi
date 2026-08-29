---
default: minor
---

#### `\color{name}{…}` with a caller-defined palette; `\phantom` fixed

Colour scopes in the formula now reach the output. CSS names work as before; other names
are defined by the caller — `--define name=SPEC` on the CLI, `"palette"` in the request —
so part of a formula can be a CMYK or spot colour in PDF. Undefined names are an error.
`\phantom{…}` now takes space without drawing (it used to draw).
