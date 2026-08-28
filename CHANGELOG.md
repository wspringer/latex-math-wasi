# Changelog

Maintained by Knope from change files in `.changeset/`. Versions are tagged
`v<version>`; every release carries the `wasm32-wasip1` command module, the
browser module, and a `provenance.json` naming the git revision and the pinned
versions of the libraries compiled into them.

## 0.1.0

Initial workspace: `core` (LaTeX math parser and OpenType MATH layout engine,
derived from KenyC/ReX, fonts as bytes, optical-size font sets), `svg`
(deterministic `<defs>`/`<use>` outlines), `pdf` (real text, embedded subsetted
CID fonts), `cli`, `wasm` (C-ABI browser module) and `wasi` (`wasm32-wasip1`
command). Golden-file, visual-diff and cross-backend tests over STIX Two Math,
XITS Math and Latin Modern Math.
