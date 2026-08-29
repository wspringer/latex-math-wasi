//! One JSON request in, SVG or PDF bytes out. Used by the `wasm32-wasip1` command
//! (`crates/wasi`, stdin → stdout) and exported over a plain C ABI for
//! `wasm32-unknown-unknown` (browser). No filesystem, no environment, no host imports
//! beyond memory.
//!
//! # Request
//!
//! ```json
//! {
//!   "tex": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
//!   "format": "svg",              // or "pdf", "png"
//!   "font_size": 16,              // user units per em (SVG px / PDF pt); default 16
//!   "style": "display",           // or "text"; default display
//!   "padding": 0,                 // user units around the bbox; default 0
//!   "scale": 1,                   // png only: device pixels per user unit; default 1
//!   "fonts": ["<base64>", 1234],  // per font: base64 string, or byte length into the font blob
//!   "levels": [0, 0, 1, 1],       // font index per level: display, text, script, scriptscript
//!   "scales": [1, 1, 0.7, 0.5]    // optional; default from the text font's MATH table
//! }
//! ```
//!
//! `fonts` entries that are numbers are byte lengths: the fonts are the consecutive
//! slices of that length in the font blob passed alongside the request (the browser
//! path). Base64 strings carry the font inline (the stdin path). `levels` defaults to
//! the CLI's rule: 1 font → all levels, 2 → `[0,0,1,1]`, 3 → `[0,0,1,2]`, 4 → `[0,1,2,3]`.

use base64::Engine;
use latex_math_core::{Font, FontSet, Options, Style};
use latex_math_pdf::{to_pdf, PdfOptions};
use latex_math_png::{to_png, PngOptions};
use latex_math_svg::{to_svg, SvgOptions};
use serde::Deserialize;

/// A parsed request.
#[derive(Debug, Deserialize)]
pub struct Request {
    /// LaTeX math fragment.
    pub tex: String,
    /// `"svg"`, `"pdf"` or `"png"`.
    #[serde(default = "default_format")]
    pub format: String,
    /// User units per em.
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    /// `"display"` or `"text"`.
    #[serde(default = "default_style")]
    pub style: String,
    /// Space around the bounding box.
    #[serde(default)]
    pub padding: f64,
    /// PNG only: device pixels per user unit.
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Fonts, inline or by length into the blob.
    pub fonts: Vec<FontSource>,
    /// Font index per level.
    #[serde(default)]
    pub levels: Option<[usize; 4]>,
    /// Glyph scale per level.
    #[serde(default)]
    pub scales: Option<[f64; 4]>,
}

fn default_format() -> String {
    "svg".into()
}
fn default_font_size() -> f64 {
    16.0
}
fn default_scale() -> f64 {
    1.0
}
fn default_style() -> String {
    "display".into()
}

/// Where a font's bytes come from.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FontSource {
    /// Byte length of the next slice of the font blob.
    Length(u64),
    /// Base64-encoded font file.
    Base64(String),
}

/// Runs a request. `blob` supplies the bytes for `FontSource::Length` entries, in order.
pub fn handle(request_json: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
    let request: Request =
        serde_json::from_slice(request_json).map_err(|e| format!("bad request: {e}"))?;

    let mut font_data: Vec<Vec<u8>> = Vec::with_capacity(request.fonts.len());
    let mut offset = 0usize;
    for (i, source) in request.fonts.iter().enumerate() {
        match source {
            FontSource::Length(len) => {
                let len =
                    usize::try_from(*len).map_err(|_| format!("font {i}: length too large"))?;
                let end = offset
                    .checked_add(len)
                    .filter(|&e| e <= blob.len())
                    .ok_or_else(|| {
                        format!(
                            "font {i}: length {len} exceeds the font blob ({} bytes)",
                            blob.len()
                        )
                    })?;
                font_data.push(blob[offset..end].to_vec());
                offset = end;
            }
            FontSource::Base64(text) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(text.trim())
                    .map_err(|e| format!("font {i}: bad base64: {e}"))?;
                font_data.push(bytes);
            }
        }
    }
    if font_data.is_empty() {
        return Err("at least one font is required".into());
    }
    let fonts: Vec<Font<'_>> = font_data
        .iter()
        .enumerate()
        .map(|(i, b)| Font::parse(b).map_err(|e| format!("font {i}: {e}")))
        .collect::<Result<_, _>>()?;

    let levels = request.levels.unwrap_or(match fonts.len() {
        1 => [0, 0, 0, 0],
        2 => [0, 0, 1, 1],
        3 => [0, 0, 1, 2],
        _ => [0, 1, 2, 3],
    });
    let mut set = FontSet::new(&fonts, levels).map_err(|e| format!("{e:?}"))?;
    if let Some(scales) = request.scales {
        set = set.with_scales(scales);
    }
    let style = match request.style.as_str() {
        "display" => Style::Display,
        "text" => Style::Text,
        other => return Err(format!("unknown style {other:?}")),
    };
    let options = Options {
        font_size: request.font_size,
        style,
    };
    let tree =
        latex_math_core::render(&request.tex, &set, &options).map_err(|e| format!("{e:?}"))?;
    let refs: Vec<&Font<'_>> = fonts.iter().collect();
    match request.format.as_str() {
        "svg" => to_svg(
            &tree,
            &refs,
            &SvgOptions {
                padding: request.padding,
                ..SvgOptions::default()
            },
        )
        .map(String::into_bytes)
        .map_err(|e| e.to_string()),
        "pdf" => to_pdf(
            &tree,
            &refs,
            &PdfOptions {
                padding: request.padding,
            },
        )
        .map_err(|e| e.to_string()),
        "png" => to_png(
            &tree,
            &refs,
            &SvgOptions {
                padding: request.padding,
                ..SvgOptions::default()
            },
            &PngOptions {
                scale: request.scale,
                ..PngOptions::default()
            },
        )
        .map_err(|e| e.to_string()),
        other => Err(format!("unknown format {other:?}")),
    }
}

// ---- C ABI for wasm32-unknown-unknown -------------------------------------------------
//
// Host protocol:
//   ptr = latex_math_alloc(len)            allocate an input buffer, write into it
//   r   = latex_math_render(req, req_len, blob, blob_len)
//         → u64: (result_ptr << 32) | result_len; result[0] is 0 for success (payload
//           follows) or 1 for error (UTF-8 message follows)
//   latex_math_free(ptr, len)              free any buffer obtained from this module
// Input buffers are not consumed; free them yourself.

/// Allocates `len` bytes and returns the pointer (null for `len == 0`).
///
/// # Safety
/// The returned buffer must be freed with [`latex_math_free`] using the same `len`.
#[no_mangle]
pub unsafe extern "C" fn latex_math_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Frees a buffer returned by [`latex_math_alloc`] or [`latex_math_render`].
///
/// # Safety
/// `ptr`/`len` must be exactly what this module handed out.
#[no_mangle]
pub unsafe extern "C" fn latex_math_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len != 0 {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

/// Renders a request; see the module docs for the protocol.
///
/// # Safety
/// `req`/`blob` must point to `req_len`/`blob_len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn latex_math_render(
    req: *const u8,
    req_len: usize,
    blob: *const u8,
    blob_len: usize,
) -> u64 {
    let req = if req.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(req, req_len)
    };
    let blob = if blob.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(blob, blob_len)
    };
    let mut out = match handle(req, blob) {
        Ok(bytes) => {
            let mut v = Vec::with_capacity(bytes.len() + 1);
            v.push(0u8);
            v.extend_from_slice(&bytes);
            v
        }
        Err(msg) => {
            let mut v = Vec::with_capacity(msg.len() + 1);
            v.push(1u8);
            v.extend_from_slice(msg.as_bytes());
            v
        }
    };
    out.shrink_to_fit();
    let len = out.len();
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ((ptr as u64) << 32) | (len as u64)
}
