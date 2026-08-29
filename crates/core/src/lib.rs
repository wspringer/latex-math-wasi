//! LaTeX math → positioned render tree, using OpenType MATH fonts.
//!
//! The parser and layout engine are derived from [KenyC/ReX](https://github.com/KenyC/ReX)
//! (MIT, see `LICENSE-ReX`), itself a fork of ReTeX/ReX. This crate strips ReX down to
//! parsing + layout, takes fonts as bytes, and flattens the layout into a [`RenderTree`]
//! of glyph instances and rules that the `svg` and `pdf` crates consume.
//!
//! No `std::fs`, no font discovery, no network.

#[macro_use]
mod macros;

pub mod dimensions;
pub mod error;
pub mod font;
mod geometry;
pub mod layout;
pub mod parser;
pub mod render;
pub mod tree;

pub use error::Error;
pub use font::backend::ttf_parser::TtfMathFont as Font;
pub use layout::Style;
pub use tree::{BBox, GlyphInstance, ImageBox, RenderTree, Rule};

use layout::engine::LayoutBuilder;
use render::Renderer;

/// Layout options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Em size in user units. Every coordinate in the render tree is in these units.
    pub font_size: f64,
    /// Starting math style (`Display` for `$$…$$`, `Text` for `$…$`).
    pub style: Style,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            font_size: 16.0,
            style: Style::Display,
        }
    }
}

/// The fonts for one layout: a list of fonts plus, for each math level
/// (display, text, script, scriptscript), the index of the font to use.
///
/// Each level draws glyphs from its own font and reads MATH constants from it, so an
/// optical-size cut (e.g. a "Caption" or "Tiny" master) can serve the small levels.
/// Optical sizes keep their em — they change the design, not the size — so script
/// levels are still scaled: by default by the *text* font's `ScriptPercentScaleDown` /
/// `ScriptScriptPercentScaleDown`, or by [`FontSet::with_scales`].
#[derive(Clone, Copy)]
pub struct FontSet<'s, 'a> {
    fonts: &'s [Font<'a>],
    levels: [usize; 4],
    scales: Option<[f64; 4]>,
}

impl<'s, 'a> FontSet<'s, 'a> {
    /// `levels` = font index for `[display, text, script, scriptscript]`.
    pub fn new(fonts: &'s [Font<'a>], levels: [usize; 4]) -> Result<Self, Error> {
        if fonts.is_empty() {
            return Err(Error::InvalidFontSet(0));
        }
        for &i in &levels {
            if i >= fonts.len() {
                return Err(Error::InvalidFontSet(i));
            }
        }
        Ok(FontSet {
            fonts,
            levels,
            scales: None,
        })
    }

    /// One font for every level.
    pub fn single(font: &'s Font<'a>) -> Self {
        FontSet {
            fonts: std::slice::from_ref(font),
            levels: [0; 4],
            scales: None,
        }
    }

    /// Explicit glyph scale per level (display, text, script, scriptscript), replacing
    /// the text font's `ScriptPercentScaleDown` values. TeX's classic sizes are
    /// `[1.0, 1.0, 0.7, 0.5]`.
    pub fn with_scales(mut self, scales: [f64; 4]) -> Self {
        self.scales = Some(scales);
        self
    }

    /// The per-level scales in effect, if overridden.
    pub fn scales(&self) -> Option<[f64; 4]> {
        self.scales
    }

    /// The font list; [`GlyphInstance::font`] indexes into it.
    pub fn fonts(&self) -> &'s [Font<'a>] {
        self.fonts
    }

    /// Font index per level: display, text, script, scriptscript.
    pub fn levels(&self) -> [usize; 4] {
        self.levels
    }

    /// Fonts per level, as the layout engine wants them.
    fn per_level(&self) -> [&'s Font<'a>; 4] {
        self.levels.map(|i| &self.fonts[i])
    }
}

/// Parses `tex` and lays it out with `fonts`, returning a flat render tree.
///
/// The tree's origin is the baseline start of the formula; y grows downward.
/// [`GlyphInstance::font`] indexes [`FontSet::fonts`].
pub fn render(tex: &str, fonts: &FontSet<'_, '_>, options: &Options) -> Result<RenderTree, Error> {
    let nodes = parser::parse(tex)?;
    let mut builder = LayoutBuilder::new(fonts.per_level());
    if let Some(scales) = fonts.scales {
        builder = builder.scales(scales);
    }
    let layout = builder
        .font_size(options.font_size)
        .style(options.style)
        .layout(&nodes)?;
    let size = layout.size();
    let bbox = layout.full_bounding_box();
    let mut backend = tree::TreeBackend::new(fonts.fonts().iter().collect());
    Renderer::new().render(&layout, &mut backend);
    let mut tree = backend.finish();
    tree.width = size.width;
    tree.height = size.height;
    tree.depth = size.depth;
    tree.bbox = tree::BBox {
        x_min: bbox.x_min,
        y_min: bbox.y_min,
        x_max: bbox.x_max,
        y_max: bbox.y_max,
    };
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STIX: &[u8] = include_bytes!("../../../tests/fonts/STIXTwoMath-Regular.otf");

    /// If the font's coverage of mathematical alphanumeric characters is exhaustive in all styles,
    /// then the library should not fail parsing and laying out on any of these.
    /// Test for bugs like <https://github.com/KenyC/ReX/issues/6>.
    #[test]
    fn all_alphanumeric_style_combinations_must_work() {
        let font = Font::parse(STIX).unwrap();
        let alphanumeric: Vec<char> = (0..0x7F_u32)
            .filter_map(char::from_u32)
            .filter(|c| c.is_alphanumeric())
            .collect();
        for env in [
            None,
            Some("mathcal"),
            Some("mathrm"),
            Some("mathfrak"),
            Some("mathbb"),
        ] {
            for character in &alphanumeric {
                let formula = match env {
                    Some(env) => format!(r"\{env}{{{character}}}"),
                    None => character.to_string(),
                };
                render(&formula, &FontSet::single(&font), &Options::default())
                    .unwrap_or_else(|e| panic!("{formula}: {e:?}"));
            }
        }
    }

    /// Space glyphs have an advance but no contours; they must lay out as empty boxes,
    /// not fail as missing glyphs.
    #[test]
    fn contourless_glyphs_lay_out() {
        let font = Font::parse(STIX).unwrap();
        let a = render(
            r"\operatorname{lim sup}_{n} a_n",
            &FontSet::single(&font),
            &Options::default(),
        )
        .unwrap();
        let b = render(r"\text{a b}", &FontSet::single(&font), &Options::default()).unwrap();
        let c = render(r"\text{ab}", &FontSet::single(&font), &Options::default()).unwrap();
        assert!(a.glyphs.len() >= 8);
        assert!(b.width > c.width, "the space must advance");
    }

    #[test]
    fn simple_formula_produces_glyphs_and_a_rule() {
        let font = Font::parse(STIX).unwrap();
        let tree = render(r"\frac{a}{b}", &FontSet::single(&font), &Options::default()).unwrap();
        assert_eq!(tree.glyphs.len(), 2);
        assert_eq!(tree.rules.len(), 1);
        assert!(tree.glyphs.iter().all(|g| g.font == 0));
        // numerator above the baseline, denominator below
        assert!(tree.glyphs[0].y < 0.0 && tree.glyphs[1].y > 0.0);
    }
}

/// What a consumer needs to place an output document inline with text: the document's
/// box and where its baseline is, plus the em and ex of the text font at the rendered
/// size, so the numbers can be re-expressed in font-relative units. All in user units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Document width.
    pub width: f64,
    /// Document height (`ascent + depth`).
    pub height: f64,
    /// Baseline to bottom edge. Place the image with `vertical-align: -depth`.
    pub depth: f64,
    /// Top edge to baseline.
    pub ascent: f64,
    /// User units per em (`Options::font_size`).
    pub em: f64,
    /// x-height of the text-style font at that size, if the font records one.
    pub ex: Option<f64>,
}

impl Metrics {
    /// Fixed-precision JSON, three decimals, keys in a fixed order — byte-identical across
    /// the CLI and the wasm builds. `ex` is `null` when the font has no x-height.
    pub fn to_json(&self) -> String {
        let ex = match self.ex {
            Some(v) => format!("{v:.3}"),
            None => "null".to_string(),
        };
        format!(
            r#"{{"width":{:.3},"height":{:.3},"depth":{:.3},"ascent":{:.3},"em":{:.3},"ex":{ex}}}"#,
            self.width, self.height, self.depth, self.ascent, self.em
        )
    }
}

/// Metrics of `tree` as rendered with `fonts` and `options`, for a document with
/// `padding` user units around the bbox (the same `padding` given to the backends).
pub fn metrics(
    tree: &RenderTree,
    fonts: &FontSet<'_, '_>,
    options: &Options,
    padding: f64,
) -> Metrics {
    let b = tree.image_box(padding);
    let text_font = &fonts.fonts()[fonts.levels()[1]];
    Metrics {
        width: b.width,
        height: b.height,
        depth: b.depth,
        ascent: b.ascent(),
        em: options.font_size,
        ex: text_font.x_height_em().map(|x| x * options.font_size),
    }
}
