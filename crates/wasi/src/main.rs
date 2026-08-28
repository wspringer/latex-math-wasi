//! `wasm32-wasip1` command: JSON request on stdin (fonts inline as base64), SVG or PDF
//! bytes on stdout, error message on stderr with exit code 1. Needs no preopened
//! directories. See `latex_math_wasm` for the request schema.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut request = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut request) {
        eprintln!("error: reading stdin: {e}");
        return ExitCode::FAILURE;
    }
    match latex_math_wasm::handle(&request, &[]) {
        Ok(bytes) => {
            if let Err(e) = std::io::stdout().write_all(&bytes) {
                eprintln!("error: writing stdout: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}
