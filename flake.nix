{
  description = "latex-math-wasi: pure-Rust LaTeX math -> SVG/PDF with OpenType MATH fonts";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
          targets = [ "wasm32-wasip1" "wasm32-unknown-unknown" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.wasmtime      # run the wasm32-wasip1 command target
            pkgs.wasm-tools    # inspect/validate wasm modules
            pkgs.cargo-deny    # license/dependency policy (no C deps)
            pkgs.git
            pkgs.gh
          ];
          # Make sure the nix toolchain wins over ~/.cargo/bin rustup shims.
          shellHook = ''
            export PATH="${rustToolchain}/bin:$PATH"
          '';
        };
      });
}
