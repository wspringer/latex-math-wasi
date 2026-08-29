//! Render tree → PNG.
//!
//! Rasterizes the SVG produced by `latex-math-svg` with resvg (pure Rust, no system
//! fonts involved: the SVG contains only paths and rects), so the PNG shows exactly
//! what the SVG says. Output is deterministic: same tree, same options, same bytes.

use latex_math_core::{Font, RenderTree};
use latex_math_svg::{to_svg, SvgError, SvgOptions};
use resvg::tiny_skia::{Color, Pixmap, Transform};
use resvg::usvg;
use std::fmt;

/// PNG rendering options. Layout options (padding, precision) come from [`SvgOptions`].
#[derive(Debug, Clone, PartialEq)]
pub struct PngOptions {
    /// Device pixels per user unit. With the default 16 user units per em, `1.0` gives a
    /// 16 px em; `2.0` renders at "retina" density.
    pub scale: f64,
    /// Background colour as RGBA, or `None` for transparent.
    pub background: Option<[u8; 4]>,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background: None,
        }
    }
}

/// Errors from PNG rendering.
#[derive(Debug)]
pub enum PngError {
    /// Producing the intermediate SVG failed.
    Svg(SvgError),
    /// resvg could not parse the SVG we generated (a bug in the SVG backend).
    Parse(usvg::Error),
    /// `scale` is not a positive finite number, or the image would be too large.
    BadSize,
    /// PNG encoding failed.
    Encode(String),
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngError::Svg(e) => write!(f, "svg: {e}"),
            PngError::Parse(e) => write!(f, "svg parse: {e}"),
            PngError::BadSize => write!(f, "bad scale or image size"),
            PngError::Encode(e) => write!(f, "png encode: {e}"),
        }
    }
}

impl std::error::Error for PngError {}

impl From<SvgError> for PngError {
    fn from(e: SvgError) -> Self {
        PngError::Svg(e)
    }
}

/// Pixel size of the PNG that [`to_png`] produces for an SVG of `width` × `height` user
/// units at `scale`: each side is rounded up, and never below one pixel.
pub fn pixel_size(width: f64, height: f64, scale: f64) -> Option<(u32, u32)> {
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let side = |v: f64| -> Option<u32> {
        let px = (v * scale).ceil();
        if !px.is_finite() || px > u32::MAX as f64 {
            return None;
        }
        Some((px as u32).max(1))
    };
    Some((side(width)?, side(height)?))
}

/// Renders `tree` to PNG bytes. `fonts` is the same slice the tree was rendered with.
pub fn to_png(
    tree: &RenderTree,
    fonts: &[&Font<'_>],
    svg_options: &SvgOptions,
    options: &PngOptions,
) -> Result<Vec<u8>, PngError> {
    let svg = to_svg(tree, fonts, svg_options)?;
    let usvg_tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(PngError::Parse)?;
    let size = usvg_tree.size();
    let (w, h) = pixel_size(size.width() as f64, size.height() as f64, options.scale)
        .ok_or(PngError::BadSize)?;
    let mut pixmap = Pixmap::new(w, h).ok_or(PngError::BadSize)?;
    if let Some([r, g, b, a]) = options.background {
        pixmap.fill(Color::from_rgba8(r, g, b, a));
    }
    let s = options.scale as f32;
    resvg::render(
        &usvg_tree,
        Transform::from_scale(s, s),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| PngError::Encode(e.to_string()))
}
